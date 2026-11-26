"""核验固定中文来源、统计覆盖，并记录守恒的离线软分词计数。"""
import argparse
from collections import Counter, defaultdict
import gzip
import json
from pathlib import Path
import unicodedata

from build_word_context import (
    BOS, EOS, MAX_BYTES, MAX_EDGES, MAX_RECORDS, POLICY, ROOT, UNKNOWN,
    corpus_receipt, load_train, observe_surfaces, protected_development, read_lexicon,
    read_locked, segment, sha,
)

SCALE = 1024
FORMAT = "cloudleaf-word-context-estimate-v1"


def slices(text, vocab, widths):
    options = [[] for _ in text]
    minimum = [0] * (len(text) + 1)
    for pos in range(len(text) - 1, -1, -1):
        possible = [(pos + 1, UNKNOWN, minimum[pos + 1] + 1)]
        for width in widths:
            end = pos + width
            if end > len(text):
                break
            word = text[pos:end]
            if word in vocab:
                possible.append((end, word, minimum[end]))
        minimum[pos] = min(option[2] for option in possible)
        options[pos] = [(end, word) for end, word, missed in possible if missed == minimum[pos]]
    return options, minimum[0]


def allocate(total, weights):
    denominator = sum(weights)
    quotients = [divmod(total * weight, denominator) for weight in weights]
    shares = [quotient for quotient, _ in quotients]
    remainder = total - sum(shares)
    order = sorted(range(len(shares)), key=lambda index: (-quotients[index][1], index))
    for index in order[:remainder]:
        shares[index] += 1
    return shares


def add_estimates(text, vocab, widths, observed, unigrams, pairs):
    options, missed = slices(text, vocab, widths)
    positions = [Counter() for _ in range(len(text) + 1)]
    positions[0][BOS] = SCALE
    unigrams[BOS] += SCALE
    unigrams[EOS] += SCALE
    multiple_edges = sum(len(edges) > 1 for edges in options)
    for pos, edges in enumerate(options):
        weights = [1 if word == UNKNOWN else observed[word] + 1 for _, word in edges]
        for previous, inbound in positions[pos].items():
            shares = [inbound] if len(edges) == 1 else allocate(inbound, weights)
            for (end, word), mass in zip(edges, shares, strict=True):
                if mass:
                    positions[end][word] += mass
                    unigrams[word] += mass
                    pairs[previous, word] += mass
    for previous, mass in positions[-1].items():
        pairs[previous, EOS] += mass
    return missed, multiple_edges


def validate_estimates(unigrams, pairs, n, missed):
    incoming, outgoing = Counter(), Counter()
    for (previous, following), count in pairs.items():
        if count <= 0:
            raise ValueError("软转移必须为正整数")
        outgoing[previous] += count
        incoming[following] += count
    if (incoming[BOS] or outgoing[EOS] or unigrams[BOS] != n * SCALE or unigrams[EOS] != n * SCALE
            or missed * SCALE != unigrams[UNKNOWN]
            or incoming != Counter({key: val for key, val in unigrams.items() if key != BOS})
            or outgoing != Counter({key: val for key, val in unigrams.items() if key != EOS})):
        raise ValueError("软分词计数出入不守恒")


def dictionary(raw, fixed):
    if fixed is None or fixed["bytes"] != len(raw):
        raise ValueError("词典未绑定批准的固定 SHA")
    vocabulary, flags = read_lexicon(raw)
    lines = [line.split("\t") for line in raw.decode("utf-8").splitlines()[2:]]
    reads, sounds, pairs = defaultdict(set), defaultdict(set), Counter()
    equivalents = defaultdict(set)
    for _, pinyin, surface, _, _, _, _, _, _ in lines:
        reads[surface].add(pinyin)
        sounds[pinyin].add(surface)
        pairs[surface, pinyin] += 1
        equivalents[unicodedata.normalize("NFKC", surface)].add(surface)
    widest = sorted(((key, len(values)) for key, values in sounds.items() if " " not in key),
                    key=lambda value: (-value[1], value[0]))[:8]
    stats = {
        "entries": len(lines), "surfaces": len(reads), "frequency_semantics": dict(flags),
        "duplicate_surface_reading_rows": sum(count - 1 for count in pairs.values()),
        "surfaces_with_multiple_readings": sum(len(values) > 1 for values in reads.values()),
        "nfkc_equivalent_distinct_surface_groups": sum(len(values) > 1 for values in equivalents.values()),
        "most_ambiguous_single_syllables": widest,
        "syllables_with_over_100_distinct_surfaces": sum(" " not in key and len(values) > 100
                                                       for key, values in sounds.items()),
    }
    return vocabulary, reads, pairs, stats


def coverage(rows, words, observed_words):
    widths = sorted({len(word) for word in words}, reverse=True)
    text = Counter(ch for row in rows for ch in row["text"])
    known_scalars = sum(count for ch, count in text.items() if ch in words)
    unknown = sum(list(segment(row["text"], words, widths)).count(UNKNOWN) for row in rows)
    keyword_missing = sum(1 for ch, count in text.items() if count >= 20 and ch not in words)
    return {"fragments": len(rows), "distinct_scalars": len(text), "scalars": sum(text.values()),
            "standalone_covered": known_scalars, "missing_standalone_repeated_scalars_ge20": keyword_missing,
            "unknown_after_word_cover": unknown, "observed_substrings": len(observed_words),
            "substring_observations": sum(observed_words.values())}


def write_json(path, value):
    with path.open("x", encoding="utf-8", newline="\n") as output:
        json.dump(value, output, ensure_ascii=False, sort_keys=True, indent=2)
        output.write("\n")


def soft_count(records, vocab, observed, lexicon_sha, corpus_sha, output):
    unigrams, pairs = Counter(), Counter()
    widths = sorted({len(value) for value in vocab})
    missed = alternatives = affected = 0
    for row in records:
        amount, branches = add_estimates(row["text"], vocab, widths, observed, unigrams, pairs)
        missed += amount
        alternatives += branches
        affected += bool(branches)
    validate_estimates(unigrams, pairs, len(records), missed)
    data = {
        "schema": FORMAT, "usage": "research_only", "eligible_for_reader": False,
        "observation_unit": None, "ticks_per_fragment": SCALE, "runtime_sha256": lexicon_sha,
        "corpus_sha256": corpus_sha, "estimated_words_ticks": sorted(unigrams.items()),
        "estimated_pairs_ticks": sorted((a, b, count) for (a, b), count in pairs.items()),
    }
    raw = json.dumps(data, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    packed = gzip.compress(raw, mtime=0)
    if len(raw) > MAX_BYTES or len(unigrams) + len(pairs) > 2 * MAX_RECORDS or sum(pairs.values()) > SCALE * MAX_EDGES:
        raise ValueError("软估计超过固定离线资源上限")
    with output.open("xb") as file:
        file.write(packed)
    return {"estimated_word_rows": len(unigrams), "estimated_pair_rows": len(pairs),
            "estimated_tokens_ticks": sum(unigrams.values()) - 2 * len(records) * SCALE,
            "estimated_pair_ticks": sum(pairs.values()), "estimated_unknown_ticks": unigrams[UNKNOWN],
            "observed_unknown_scalars": missed, "fragments_with_choices": affected,
            "positions_with_choices": alternatives, "conservation": True,
            "artifact_sha256": sha(packed), "artifact_bytes": len(packed),
            "raw_bytes": len(raw), "ticks_per_fragment": SCALE, "eligible_for_reader": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ("base", "supplement", "corpus-dir", "source-dir", "output-dir"):
        parser.add_argument("--" + flag, required=True, type=Path)
    args = parser.parse_args()
    policy = json.loads(POLICY.read_bytes())
    source = policy["sources"]["tatoeba-cmn-r16"]
    if source["permitted_scope"] != "local_research_training" or source["production_approved"]:
        raise ValueError("固定许可边界不符")
    train, excluded, _ = corpus_receipt(source, args.corpus_dir, args.source_dir, None)
    dev_raw = read_locked(args.corpus_dir / "dev-fragments.jsonl", source["dev_fragments"])
    development = [json.loads(line) for line in dev_raw.decode("utf-8").splitlines()]
    news = policy["sources"]["wikinews-zh-r17"]["development_tasks"]
    articles = [json.loads(row) for row in gzip.decompress(read_locked(ROOT / news["path"], news)).decode("utf-8").splitlines()]
    records, _ = load_train(train, excluded, protected_development(articles))
    corpus_sha = sha("".join(row["sha256"] + "\n" for row in records).encode("ascii"))
    manifest = json.loads((ROOT / "docs/reports/input-round17-evidence/supplement/manifest.json").read_bytes())
    base, supplement = args.base.read_bytes(), args.supplement.read_bytes()
    if not (sha(base) == manifest["baseline_sha256"] and sha(supplement) == manifest["combined_sha256"]
            and supplement.startswith(base)):
        raise ValueError("补充层固定身份或原始前缀不符")
    layer = read_locked(args.supplement.with_name("layer.tsv"), manifest["files"]["layer.tsv"])
    if layer.split(b"\n", 2)[2] != supplement[len(base):]:
        raise ValueError("补充层行身份不符")
    lineage = read_locked(args.supplement.with_name("lineage.jsonl"), manifest["files"]["lineage.jsonl"])
    proven = [json.loads(line) for line in lineage.decode("utf-8").splitlines()]
    additions = [line.split("\t") for line in supplement[len(base):].decode("utf-8").splitlines()]
    if len(proven) != len(additions) or any(
        row["id"] != fields[0] or row["text"] != fields[2] or row["pinyin"] != fields[1]
        or row["production_approved"] or row["frequency"] is not None or not row["source_evidence"]
        for row, fields in zip(proven, additions, strict=True)
    ):
        raise ValueError("补充层来源或批准范围不符")
    args.output_dir.mkdir(exist_ok=False, parents=True)
    result = {"schema": "cloudleaf-round25-coverage-v1", "corpus_sha256": corpus_sha,
              "train_sha256": sha(train), "dev_sha256": sha(dev_raw), "policy_sha256": sha(POLICY.read_bytes()),
              "layer_records": len(additions), "layer_source_zip_sha256": manifest["source_zip_sha256"],
              "layer_lineage_sha256": sha(lineage), "historical_external_license_development_only": True,
              "formal": {"top1": None, "top5": None, "mrr": None}, "variants": {}}
    profiles, observations = [], []
    for name, encoded in (("base", base), ("supplement", supplement)):
        digest = sha(encoded)
        vocab, reads, pairs, dictionary_stats = dictionary(encoded, policy["lexicons"].get(digest))
        observed = observe_surfaces(records, vocab)
        observed_dev = observe_surfaces(development, vocab)
        out = result["variants"][name] = {
            "lexicon_sha256": digest, "dictionary": dictionary_stats,
            "train": coverage(records, vocab, observed), "dev": coverage(development, vocab, observed_dev),
            "observed_multiword_surfaces": sum(len(word) >= 2 for word in observed),
        }
        out["soft"] = soft_count(records, vocab, observed, digest, corpus_sha,
                                 args.output_dir / f"{name}-estimates.json.gz")
        profiles.append((reads, pairs))
        observations.append(observed)
    before, after = profiles
    new = set(after[1]) - set(before[1])
    new_surfaces = {word for word, _ in new if word not in before[0]}
    observed = observations[-1]
    result["layer_analysis"] = {
        "all_added_are_single_scalar": all(len(fields[2]) == 1 for fields in additions),
        "unique_new_surface_readings": len(new), "completely_new_surfaces": len(new_surfaces),
        "added_readings_for_existing_surfaces": sum(word in before[0] for word, _ in new),
        "new_surfaces_never_observed_in_training": sum(observed[word] == 0 for word in new_surfaces),
        "new_surfaces_observed_once_in_training": sum(observed[word] == 1 for word in new_surfaces),
        "new_surfaces_observed_at_least_20_in_training": sum(observed[word] >= 20 for word in new_surfaces),
        "new_multiword_surfaces": sum(len(word) > 1 for word in new_surfaces),
        "semantic_assessment_label": "source-linked readings; independent reader judgement and register unavailable",
    }
    write_json(args.output_dir / "summary.json", result)
    print(json.dumps({"variants": len(result["variants"]), **result["layer_analysis"]}, ensure_ascii=False))


if __name__ == "__main__":
    main()
