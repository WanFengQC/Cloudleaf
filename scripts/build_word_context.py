"""只对许可已登记的中文训练片段计数，产物仅用于本机研究。"""

from __future__ import annotations

import argparse
from collections import Counter
from functools import partial
import gzip
import hashlib
import json
from pathlib import Path
import re
import unicodedata

from prepare_context_corpus import canonical, is_han

ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / "config/language/round23-research-corpora.json"
RELEASE = "cloudleaf-word-context-v1"
BOS, EOS, UNKNOWN = "<s>", "</s>", "<unk>"
MAX_RECORDS = 200_000
MAX_BYTES = 16 * 1024 * 1024
MAX_SEQUENCES = 100_000
MAX_EDGES = 1_100_000
MAX_FRAGMENT = 128
LEXICON_HEADER = (
    "id\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id"
)
FRAGMENT_KEYS = {"sentence_id", "family", "fragment_index", "text", "sha256"}
FAMILY = re.compile(r"(?:cmn|base|eng):[1-9][0-9]*")
SPLIT_SALT = "cloudleaf-r16-tatoeba-family-v1:"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_locked(path, expected):
    raw = path.read_bytes()
    if len(raw) != expected["bytes"] or sha(raw) != expected["sha256"]:
        raise ValueError(f"固定原件的长度或 SHA-256 不符：{path.name}")
    return raw


def read_lexicon(raw):
    lines = raw.decode("utf-8").split("\n")
    if lines[:2] != ["# cloudleaf-lexicon-v2", LEXICON_HEADER] or lines[-1] != "":
        raise ValueError("词典版本、列或 LF 结尾不符")
    entries = Counter()
    identities = set()
    flags = Counter()
    for number, row in enumerate(lines[2:-1], 3):
        fields = row.split("\t")
        if len(fields) != 9 or not fields[0] or fields[0] in identities or not fields[2]:
            raise ValueError(f"词典行 {number} 格式或词条身份错误")
        identities.add(fields[0])
        flags[fields[4]] += 1
        if len(fields[2]) <= MAX_FRAGMENT and all(map(is_han, fields[2])):
            entries[fields[2]] += 1
    return entries, flags


def load_train(raw, excluded, external):
    """仅接收已经冻结且仍属于训练分区的行，摘要剔除跨分区完全重复。"""
    seen = set()
    families = set()
    stats = Counter()
    records = []
    for number, line in enumerate(raw.decode("utf-8").splitlines(), 1):
        row = json.loads(line)
        valid = (isinstance(row, dict) and set(row) == FRAGMENT_KEYS
                 and isinstance(row["family"], str) and FAMILY.fullmatch(row["family"])
                 and isinstance(row["sentence_id"], str) and row["sentence_id"].isascii()
                 and row["sentence_id"].isdigit() and int(row["sentence_id"]) > 0
                 and type(row["fragment_index"]) is int and row["fragment_index"] >= 0
                 and isinstance(row["text"], str) and 2 <= len(row["text"]) <= MAX_FRAGMENT
                 and all(map(is_han, row["text"]))
                 and unicodedata.normalize("NFC", row["text"]) == row["text"]
                 and sha(row["text"].encode("utf-8")) == row["sha256"])
        if not valid:
            raise ValueError(f"训练行 {number} 正文、来源身份或形状错误")
        family = row["family"]
        if int(sha((SPLIT_SALT + family).encode("utf-8"))[:16], 16) % 100 >= 80:
            raise ValueError(f"训练行 {number} 来源家族属于原冻结非训练分区")
        stats["source_fragments"] += 1
        stats["source_scalars"] += len(row["text"])
        families.add(family)
        if row["sha256"] in excluded:
            stats["blocked_exact_nontrain_hash"] += 1
        elif leaks(row["text"], external):
            stats["blocked_external_development_overlap"] += 1
        elif row["sha256"] in seen:
            stats["dropped_exact_train_duplicate"] += 1
        else:
            seen.add(row["sha256"])
            records.append(row)
    stats["unique_families_before_deduplication"] = len(families)
    stats["retained_families"] = len({row["family"] for row in records})
    stats["retained_sentence_ids"] = len({row["sentence_id"] for row in records})
    stats["retained_fragments"] = len(records)
    if not 0 < len(records) <= MAX_SEQUENCES:
        raise ValueError("训练片段数量为空或超出限制")
    return records, stats


def shingles(text):
    return frozenset(text[i:i + 3] for i in range(len(text) - 2)) if len(text) >= 8 else frozenset()


def protected_development(rows):
    pairs = []
    for row in rows:
        if (row["split"] != "dev" or row["formal_gold"]
                or row["source_family"] != "wikimedia-wikinews-zh"):
            raise ValueError("外部开发来源或用途不符")
        text = row["source_text"]
        if text is None:
            continue
        if sha(text.encode("utf-8")) != row["source_text_sha256"]:
            raise ValueError("外部开发原文 SHA 不符")
        pairs.append((row["case"], text))
    return overlap_index(pairs)


def overlap_index(pairs):
    exact = set()
    features = {}
    postings = {}
    for identity, text in pairs:
        normalized = canonical(text)
        exact.add(normalized)
        shared = shingles(normalized)
        if shared:
            features[identity] = shared
            for shingle in shared:
                postings.setdefault(shingle, set()).add(identity)
    return exact, features, postings


def leaks(text, protected):
    exact, features, postings = protected
    normalized = canonical(text)
    if normalized in exact:
        return True
    train = shingles(normalized)
    candidates = set()
    for item in train:
        candidates.update(postings.get(item, ()))
    return any(5 * len(train & features[key]) >= 4 * len(train | features[key])
               for key in candidates)


def segment(text, vocabulary, word_widths):
    """完整覆盖路径最少未知字符，再最少词元，平局时从左向右选最长。"""
    size = len(text)
    best = [(0, 0)] * (size + 1)
    choices = [("", 0)] * size
    for start in range(size - 1, -1, -1):
        unknown, count = best[start + 1]
        preferred = ((unknown + 1, count + 1, -1), UNKNOWN, start + 1)
        for width in word_widths:
            end = start + width
            if end > size:
                continue
            word = text[start:end]
            if word in vocabulary:
                missed, tokens = best[end]
                option = ((missed, tokens + 1, -width), word, end)
                if option < preferred:
                    preferred = option
        scores, word, end = preferred
        best[start] = scores[:2]
        choices[start] = word, end

    start = 0
    while start < size:
        word, start = choices[start]
        yield word


def log2_fixed(value):
    # 与 Rust 模型统一 1024 单位，避免浮点舍入跨过分词平局。
    exponent = value.bit_length() - 1
    normalized = (value << 32) >> exponent
    fraction = 0
    for bit in range(9, -1, -1):
        normalized = (normalized * normalized) >> 32
        if normalized >= 1 << 33:
            normalized >>= 1
            fraction |= 1 << bit
    return exponent * 1024 + fraction


def observe_surfaces(records, vocabulary):
    widths = sorted({len(word) for word in vocabulary})
    observed = Counter()
    for row in records:
        text = row["text"]
        for start in range(len(text)):
            for width in widths:
                if start + width > len(text):
                    break
                word = text[start:start + width]
                if word in vocabulary:
                    observed[word] += 1
    return observed


def probable_costs(observed):
    total = log2_fixed(sum(observed.values()) + len(observed) + 1)
    return {word: total - log2_fixed(count + 1) for word, count in observed.items()} | {UNKNOWN: total}


def segment_probable(text, vocabulary, word_widths, costs):
    best = [(0, 0, 0)] * (len(text) + 1)
    choices = [(UNKNOWN, 0)] * len(text)
    for start in range(len(text) - 1, -1, -1):
        unknown, cost, tokens = best[start + 1]
        preferred = ((unknown + 1, cost + costs[UNKNOWN], tokens + 1, -1), UNKNOWN, start + 1)
        for width in word_widths:
            end = start + width
            if end > len(text):
                continue
            word = text[start:end]
            if word in vocabulary:
                unknown, cost, tokens = best[end]
                option = ((unknown, cost + costs[word], tokens + 1, -width), word, end)
                if option < preferred:
                    preferred = option
        scores, word, end = preferred
        best[start] = scores[:3]
        choices[start] = word, end

    start = 0
    while start < len(text):
        word, start = choices[start]
        yield word


def train(records, entries, costs=None, allow_compaction=False):
    vocab = set(entries)
    widths = sorted({len(word) for word in vocab}, reverse=True)
    split = segment if costs is None else partial(segment_probable, costs=costs)
    unigrams = Counter()
    bigrams = Counter()
    training = Counter()
    for row in records:
        words = list(split(row["text"], vocab, widths))
        training["segments"] += 1
        training["scalars"] += len(row["text"])
        training["tokens"] += len(words)
        if UNKNOWN in words:
            training["segments_with_unknown"] += 1
        else:
            training["fully_covered_segments"] += 1
        sequence = [BOS, *words, EOS]
        unigrams.update(sequence)
        bigrams.update(zip(sequence, sequence[1:]))
    training["unknown_tokens"] = unigrams[UNKNOWN]
    training["bigrams_observed"] = sum(bigrams.values())
    if ((not allow_compaction and len(unigrams) + len(bigrams) > MAX_RECORDS)
            or training["bigrams_observed"] > MAX_EDGES):
        raise ValueError("词元或转移资源超出上限，禁止静默截断")
    return unigrams, bigrams, training


def compact_counts(unigrams, bigrams, max_rows=MAX_RECORDS):
    tokens, pairs = unigrams, bigrams
    threshold = 0
    rare = set()
    limits = sorted({total for word, total in unigrams.items() if word not in (BOS, EOS, UNKNOWN)})
    for limit in limits:
        if len(tokens) + len(pairs) <= max_rows:
            break
        threshold = limit
        rare = {word for word, total in unigrams.items()
                if word not in (BOS, EOS, UNKNOWN) and total <= threshold}
        tokens, pairs = Counter(), Counter()
        for word, count in unigrams.items():
            tokens[UNKNOWN if word in rare else word] += count
        for (previous, following), count in bigrams.items():
            key = (UNKNOWN if previous in rare else previous, UNKNOWN if following in rare else following)
            pairs[key] += count
    if len(tokens) + len(pairs) > max_rows:
        raise ValueError("保留边界及未知词后，统计仍超出模型行数上限")
    affected = [(pair, count) for pair, count in bigrams.items() if rare.intersection(pair)]
    return tokens, pairs, {
        "rare_count_threshold_inclusive": threshold,
        "remapped_types": len(rare),
        "remapped_token_occurrences": sum(unigrams[word] for word in rare),
        "pair_rows_with_remapped_endpoint": len(affected),
        "prior_contexts_with_remapped_pair": len({pair[0] for pair, _ in affected}),
        "pair_occurrences_reassigned": sum(count for _, count in affected),
        "total_pair_occurrences": sum(bigrams.values()),
        "coalesced_pair_rows": len(bigrams) - len(pairs),
        "discarded_pair_occurrences": 0,
        "unigram_exact_surface_tokens_retained": sum(count for word, count in tokens.items()
                                                     if word not in (BOS, EOS, UNKNOWN)),
        "raw_rows": len(unigrams) + len(bigrams),
        "model_rows": len(tokens) + len(pairs),
    }


def count_inventory(counts):
    inventory = {label: {"rows": 0, "occurrences": 0}
                 for label in ("one", "two_to_four", "five_plus")}
    for count in counts.values():
        label = "one" if count == 1 else "two_to_four" if count < 5 else "five_plus"
        inventory[label]["rows"] += 1
        inventory[label]["occurrences"] += count
    return inventory


def encode(lexicon_sha, corpus_sha, unigrams, bigrams):
    rows = [f"# {RELEASE}", f"lexicon_sha256\t{lexicon_sha}", f"corpus_sha256\t{corpus_sha}"]
    rows.extend(f"W\t{word}\t{unigrams[word]}" for word in sorted(unigrams))
    rows.extend(f"B\t{first}\t{second}\t{bigrams[first, second]}"
                for first, second in sorted(bigrams))
    data = ("\n".join(rows) + "\n").encode("utf-8")
    if len(data) > MAX_BYTES:
        raise ValueError("词语上下文文件超出 16 MiB")
    return data


def corpus_receipt(lock, corpus_dir, source_dir, excluded_file):
    train_raw = read_locked(corpus_dir / "train-fragments.jsonl", lock["train_fragments"])
    nontrain = read_locked(corpus_dir / "nontrain-fulltext-sha256.txt", lock["nontrain_hashes"])
    for filename, exact in lock["original_files"].items():
        read_locked(source_dir / filename, exact)
    excludes = set(nontrain.decode("ascii").splitlines())
    extra = None
    if excluded_file is not None:
        raw = excluded_file.read_bytes()
        rows = raw.decode("ascii").splitlines()
        if not rows or any(not re.fullmatch(r"[0-9a-f]{64}", value) for value in rows):
            raise ValueError("外部评估排除清单必须逐行使用小写 SHA-256")
        excludes.update(rows)
        extra = {"sha256": sha(raw), "hashes": len(set(rows))}
    return train_raw, excludes, extra


def build(lexicon, corpus_dir, source_dir, output, excluded_file=None, segmentation_lexicon=None,
          strategy="minimum-tokens", compact=False):
    policy_raw = POLICY.read_bytes()
    policy = json.loads(policy_raw)
    locks = policy["sources"]
    source = locks["tatoeba-cmn-r16"]
    if source["permitted_scope"] != "local_research_training" or source["production_approved"]:
        raise ValueError("来源的研究训练许可范围与本工具不符")
    runtime = lexicon.read_bytes()
    lexicon_sha = sha(runtime)
    selected_lexicon = policy["lexicons"].get(lexicon_sha)
    if selected_lexicon is None or len(runtime) != selected_lexicon["bytes"]:
        raise ValueError("词典未绑定固定研究身份")
    entries, flags = read_lexicon(runtime)
    training_entries = entries
    segmentation_sha = lexicon_sha
    if segmentation_lexicon is not None:
        basis = segmentation_lexicon.read_bytes()
        segmentation_sha = sha(basis)
        basis_lock = policy["lexicons"].get(segmentation_sha)
        if (basis_lock is None or basis_lock["usage"] != "fixed_research_baseline"
                or len(basis) != basis_lock["bytes"]
                or selected_lexicon["usage"] != "fixed_research_supplement"
                or not runtime.startswith(basis)):
            raise ValueError("共用训练词表必须是追加型研究词典的原固定基线")
        training_entries, _ = read_lexicon(basis)
    train_raw, excludes, extra = corpus_receipt(source, corpus_dir, source_dir, excluded_file)
    development = locks["wikinews-zh-r17"]["development_tasks"]
    raw = read_locked(ROOT / development["path"], development)
    development_rows = [json.loads(line) for line in gzip.decompress(raw).decode("utf-8").splitlines()]
    if len(development_rows) != development["frozen_slots"]:
        raise ValueError("外部开发总分母变更")
    external = protected_development(development_rows)
    records, stats = load_train(train_raw, excludes, external)
    if stats["source_fragments"] != source["frozen_train_fragments"]:
        raise ValueError("冻结训练总分母变更")
    surface_observations = observe_surfaces(records, training_entries) if strategy == "surface-viterbi" else None
    costs = probable_costs(surface_observations) if surface_observations is not None else None
    allow_compaction = compact or costs is not None
    unigrams, bigrams, observed = train(records, training_entries, costs, allow_compaction)
    # 绑定依序实际参与统计的原文 SHA，源锁与去重/隔离规则另写入回执。
    corpus_sha = sha("".join(row["sha256"] + "\n" for row in records).encode("ascii"))
    model_unigrams, model_bigrams = unigrams, bigrams
    alternative = None
    raw_data = None
    if allow_compaction:
        model_unigrams, model_bigrams, compaction = compact_counts(unigrams, bigrams)
        raw_data = gzip.compress(json.dumps({
            "corpus_sha256": corpus_sha,
            "lexicon_sha256": lexicon_sha,
            "observed_surface_matches": None if surface_observations is None else sorted(surface_observations.items()),
            "conditioned_token_observations": sorted(unigrams.items()),
            "conditioned_pair_observations": sorted([*pair, count] for pair, count in bigrams.items()),
        }, ensure_ascii=False, separators=(",", ":")).encode("utf-8"), mtime=0)
        alternative = {
            "strategy": strategy,
            "observed_surface_types": None if surface_observations is None else len(surface_observations),
            "observed_count": None if surface_observations is None else sum(surface_observations.values()),
            "estimated_count": None,
            "uncovered_scalar_count": unigrams[UNKNOWN],
            "model_unknown_token_count": model_unigrams[UNKNOWN],
            "model_observed_lexicon_surface_types": sum(word in entries for word in model_unigrams),
            "model_covered_lexicon_entries": sum(entries[word] for word in model_unigrams if word in entries),
            "raw_word_count_distribution": count_inventory(unigrams),
            "raw_pair_count_distribution": count_inventory(bigrams),
            "model_pair_count_distribution": count_inventory(model_bigrams),
            "compaction": compaction,
            "raw_observations": {"file": "raw-counts.json.gz", "bytes": len(raw_data), "sha256": sha(raw_data)},
        }
    data = encode(lexicon_sha, corpus_sha, model_unigrams, model_bigrams)
    count_rows = sum(entries[word] for word in unigrams if word in entries)
    runtime_count = sum(flags.values())
    trace = {
        "format": RELEASE,
        "usage": "research_only",
        "production_approved": False,
        "formal_gold": False,
        "source_id": source["source_id"],
        "source_family": source["source_family"],
        "global_source_families": 1,
        "source_revision": source["revision"],
        "source_weight": source["source_weight"],
        "training_data_sha256": sha(train_raw),
        "corpus_sha256": corpus_sha,
        "additional_exclusions": extra,
        "external_development_guard": {
            "source_family": locks["wikinews-zh-r17"]["source_family"],
            "tasks_sha256": sha(raw),
            "frozen_slots": len(development_rows),
            "protected_distinct_texts": len(external[0]),
            "rule": "NFKC canonical equality or 3-scalar shingle Jaccard >= 4/5 on lengths >= 8",
            "limitations": "No independent semantic paraphrase or traditional/simplified equivalence claim",
        },
        "rules": {"deduplication": "exact UTF-8 fragment, retain first frozen eligible row",
                  "normalization": "require NFC and preserve original Han script, no conversion",
                  "tokenization": "minimize unknown scalars then token count; longest path choice on left tie",
                  "unit": "observed research token occurrences and adjacent token pairs",
                  "model_terms": "BOS/EOS around each retained fragment; unknown maps one uncovered scalar",
                  "non_han": "legacy frozen partition contains only Han stretches; original source remains intact"},
        "lexicon_sha256": lexicon_sha,
        "lexicon_status": selected_lexicon["usage"],
        "segmentation_lexicon_sha256": segmentation_sha,
        "training_vocabulary_surface_types": len(training_entries),
        "lexicon_entries": runtime_count,
        "lexicon_entries_with_observed_surface": count_rows,
        "lexicon_entries_without_observed_surface": runtime_count - count_rows,
        "lexicon_annotated_frequencies": dict(sorted(flags.items())),
        "han_lexicon_surface_types": len(entries),
        "observed_lexicon_surface_types": sum(word in entries for word in unigrams),
        "distinct_bigram_rows": len(bigrams),
        "distinct_unigram_rows_including_boundaries": len(unigrams),
        "frozen_filter": dict(sorted(stats.items())),
        "observed": dict(sorted(observed.items())),
        "policy_sha256": sha(policy_raw),
        "tool_sha256": sha(Path(__file__).read_bytes()),
        "model": {"file": "word-context.tsv", "bytes": len(data), "sha256": sha(data)},
        "formal_frequency": {"token_frequency": None, "document_frequency": None, "zipf": None},
    }
    if alternative is not None:
        trace["strategy"] = alternative
    if costs is not None:
        trace["rules"]["tokenization"] = (
            "minimum unknown scalars then summed Q10 negative log2 smoothed overlapping surface probabilities, "
            "then fewest words and longest left first")
        trace["rules"]["unit"] = (
            "real overlapping surface matches for segmentation prior; counts conditioned on chosen hard segmentation")
    output.mkdir(parents=True, exist_ok=False)
    (output / "word-context.tsv").write_bytes(data)
    if raw_data is not None:
        (output / "raw-counts.json.gz").write_bytes(raw_data)
    (output / "receipt.json").write_text(json.dumps(trace, indent=2, ensure_ascii=False) + "\n",
                                         encoding="utf-8", newline="\n")
    return trace


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lexicon", type=Path, required=True)
    parser.add_argument("--corpus-dir", type=Path, required=True)
    parser.add_argument("--source-dir", type=Path, required=True)
    parser.add_argument("--exclude-sha256", type=Path)
    parser.add_argument("--segmentation-lexicon", type=Path,
                        help="追加型研究词典沿用固定基线词表，不增加新词计数")
    parser.add_argument("--strategy", choices=("minimum-tokens", "surface-viterbi"), default="minimum-tokens")
    parser.add_argument("--compact-counts", action="store_true", help="研究模式下将低频词面映至未知词，保留原始计数")
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(build(args.lexicon, args.corpus_dir, args.source_dir, args.output_dir,
                           args.exclude_sha256, args.segmentation_lexicon, args.strategy, args.compact_counts),
                     ensure_ascii=False))


if __name__ == "__main__":
    main()
