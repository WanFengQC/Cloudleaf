"""用固定 Tatoeba 开发分区冻结词典派生拼音研究验证，不读任何保留集。"""

from __future__ import annotations

import argparse
import gzip
import json
from pathlib import Path

from build_word_context import (
    POLICY, ROOT, SPLIT_SALT, leaks, load_train, overlap_index, protected_development, read_lexicon, read_locked, sha,
)

FREEZE_SALT = "cloudleaf-round23-tatoeba-frozen-validation-v1:"
CASE_COUNT = 40
MIN_CHAR, MAX_CHAR = 6, 24
MAX_KEYS = 64


def jsonl(raw):
    return [json.loads(line) for line in raw.decode("utf-8").splitlines()]


def select(rows, historical):
    used_ids = {row["source_sentence_id"] for row in historical}
    used_texts = {row["desired_text_sha256"] for row in historical}
    used_families = {row["family"] for row in rows if row["sentence_id"] in used_ids}
    candidates = sorted(rows, key=lambda row: sha((FREEZE_SALT + row["family"] + ":"
                         + row["sha256"]).encode("utf-8")))
    families, texts, selected = set(), set(), []
    for row in candidates:
        if (row["family"] in used_families or row["sha256"] in used_texts
                or row["family"] in families or row["sha256"] in texts
                or not MIN_CHAR <= len(row["text"]) <= MAX_CHAR):
            continue
        families.add(row["family"])
        texts.add(row["sha256"])
        selected.append(row)
        if len(selected) == CASE_COUNT:
            return selected
    raise ValueError("固定开发任务不足，不能以其它分区补齐")


def pronunciations(raw):
    read_lexicon(raw)
    lookup = {}
    for line in raw.decode("utf-8").splitlines()[2:]:
        identity, pinyin, word, *_ = line.split("\t")
        lookup.setdefault(word, []).append((identity, pinyin.replace(" ", "")))
    widths = sorted({len(word) for word in lookup if len(word) <= MAX_CHAR})
    return lookup, widths


def reading(text, lookup, widths):
    end = len(text)
    paths = [None] * end + [(0, (), "")]
    for p in range(end - 1, -1, -1):
        for width in widths:
            if p + width > end:
                break
            tail = paths[p + width]
            if tail is None:
                continue
            for identity, pinyin in lookup.get(text[p:p + width], ()):
                value = (tail[0] + 1, (identity, *tail[1]), pinyin + tail[2])
                if paths[p] is None or value < paths[p]:
                    paths[p] = value
    return None if paths[0] is None else paths[0][2]


def matching_sources(text, selected):
    return (row["sha256"] for row in selected
            if leaks(text, overlap_index([(row["sha256"], row["text"])])))


def freeze(corpus_dir, lexicon, output):
    policy = json.loads(POLICY.read_bytes())
    source = policy["sources"]["tatoeba-cmn-r16"]
    old = source["used_dev_tasks"]
    historical = jsonl(read_locked(ROOT / old["path"], old))
    if len(historical) != old["slots"]:
        raise ValueError("历史开发分母变更")
    raw = read_locked(corpus_dir / "dev-fragments.jsonl", source["dev_fragments"])
    candidates = jsonl(raw)
    selected = select(candidates, historical)

    external = policy["sources"]["wikinews-zh-r17"]["development_tasks"]
    external_index = protected_development(jsonl(gzip.decompress(read_locked(ROOT / external["path"], external))))
    train = read_locked(corpus_dir / "train-fragments.jsonl", source["train_fragments"])
    nontrain = set(read_locked(corpus_dir / "nontrain-fulltext-sha256.txt", source["nontrain_hashes"])
                   .decode("ascii").splitlines())
    trained, _ = load_train(train, nontrain, external_index)
    selected_index = overlap_index((row["sha256"], row["text"]) for row in selected)
    near = {digest for row in trained if leaks(row["text"], selected_index)
            for digest in matching_sources(row["text"], selected)}
    training_families = {row["family"] for row in trained}
    training_texts = {row["sha256"] for row in trained}
    for row in selected:
        bucket = int(sha((SPLIT_SALT + row["family"]).encode("utf-8"))[:16], 16) % 100
        if (row["family"] in training_families or row["sha256"] in training_texts
                or not 80 <= bucket < 90 or sha(row["text"].encode("utf-8")) != row["sha256"]):
            raise ValueError("冻结验证样本未通过训练家族及原文隔离")

    runtime = lexicon.read_bytes()
    runtime_sha = sha(runtime)
    if (locked := policy["lexicons"].get(runtime_sha)) is None or locked["bytes"] != len(runtime):
        raise ValueError("词典未绑定固定研究身份")
    lookup, widths = pronunciations(runtime)
    tasks = []
    for i, row in enumerate(selected, 1):
        pinyin = None if row["sha256"] in near else reading(row["text"], lookup, widths)
        status = ("blocked_near_training" if row["sha256"] in near else
                  "unmappable" if pinyin is None else "over_length" if len(pinyin) > MAX_KEYS else "ready")
        tasks.append({"case": f"r23frozen{i:03}", "source_text": row["text"],
                      "source_text_sha256": row["sha256"], "source_id": row["sentence_id"],
                      "component_family": row["family"], "source_family": source["source_family"],
                      "source_revision": source["revision"],
                      "source_url": f'https://tatoeba.org/en/sentences/show/{row["sentence_id"]}',
                      "license_id": source["license"], "usage": "engineering_only", "formal_gold": False,
                      "human_intent": None, "acceptable_outputs": None, "derived_pinyin": pinyin,
                      "mapping_status": status})
    encoded = ''.join(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n" for row in tasks)
    targets = ''.join(f'{row["case"]}\t{row["derived_pinyin"]}\t{row["source_text"]}\n'
                      for row in tasks if row["mapping_status"] == "ready")
    targets = "case\tpinyin\tsource_text\n" + targets
    ranked = "id\tscheme\traw\texpected\n" + "".join(
        f'{row["case"]}\tfull\t{row["derived_pinyin"]}\t{row["source_text"]}\n'
        for row in tasks if row["mapping_status"] == "ready"
    )
    receipt = {
        "schema": "cloudleaf-round23-frozen-research-v1",
        "source_family": source["source_family"],
        "usage": "engineering_only",
        "formal_gold": False,
        "frozen_slots": CASE_COUNT,
        "component_families": len({row["family"] for row in selected}),
        "excluded_previous_source_rows": len(historical),
        "selection": "fixed SHA ordering before pinyin; distinct component families and original hashes; length 6–24",
        "training_component_overlaps": 0,
        "training_exact_text_overlaps": 0,
        "blocked_near_training": sum(row["mapping_status"] == "blocked_near_training" for row in tasks),
        "near_duplicate_rule": "canonical equality or 3-scalar shingle Jaccard >= 4/5 on lengths >= 8; no replacement",
        "source_fragment_sha256": sha(raw),
        "runtime_sha256": runtime_sha,
        "tasks_sha256": sha(encoded.encode("utf-8")),
        "ready_targets_sha256": sha(targets.encode("utf-8")),
        "rank_replay_cases_sha256": sha(ranked.encode("utf-8")),
        "ready": sum(row["mapping_status"] == "ready" for row in tasks),
        "unmappable": sum(row["mapping_status"] == "unmappable" for row in tasks),
        "over_length": sum(row["mapping_status"] == "over_length" for row in tasks),
        "snapshot_restriction": "Same global Tatoeba source family as training, only component-disjoint internal validation. Historical Round16 component rule applies. No formal heldout read.",
    }
    output.mkdir(parents=True, exist_ok=False)
    (output / "tasks.jsonl").write_text(encoded, encoding="utf-8", newline="\n")
    (output / "ready-targets.tsv").write_text(targets, encoding="utf-8", newline="\n")
    (output / "rank-replay-cases.tsv").write_text(ranked, encoding="utf-8", newline="\n")
    (output / "receipt.json").write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n",
                                         encoding="utf-8", newline="\n")
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus-dir", type=Path, required=True)
    parser.add_argument("--lexicon", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(freeze(args.corpus_dir, args.lexicon, args.output_dir), ensure_ascii=False))


if __name__ == "__main__":
    main()
