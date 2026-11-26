"""将已校验的语义候选包投影为可离线加载的研究查询索引。"""

import argparse
from collections import defaultdict
import json
from pathlib import Path
import shutil
import tempfile

from . import pipeline as p
from . import semantic_bundle as bundle

NAMESPACE = "cccedict-zh"
CEDICT_LOCK = Path(__file__).resolve().parents[2] / "data/sources/locks/cccedict-20260604-mirror7019e1c.json"
MAX_HEADWORDS = 256
RESEARCH_SCOPE = "research_only"
USABLE_SOURCE_STATES = {"approved", RESEARCH_SCOPE}
SOURCE_FIELDS = ("id", "version", "artifact_sha256", "license_id", "status")


def build_cedict_candidate(release, expected, output, headwords):
    """只提取现有事实层内逐字相同的词条与原始英语释义。"""
    release, output = Path(release), Path(output)
    words = set(headwords)
    bundle.require(0 < len(words) <= MAX_HEADWORDS and all(bundle.text(word) for word in words),
                   "请明确提供不超过256个研究词头")
    verified = p.verify_release(release)
    manifest_sha = p.digest((release / "manifest.json").read_bytes())
    bundle.require(manifest_sha == expected, "词条目录清单摘要不符")
    lock = verified["source"]
    bundle.require(lock == p.read_json(CEDICT_LOCK) and lock["review_status"] == "approved",
                   "需要已核准的固定 CC-CEDICT 原件及授权")
    selected = []
    with (release / "records.jsonl").open(encoding="utf-8") as stream:
        for line in stream:
            row = json.loads(line)
            if words.intersection((row["text"], row["traditional"])):
                selected.append(row)
    bundle.require(selected, "指定词头未命中已校验的 CC-CEDICT 词条")
    records = []
    for row in selected:
        assertions = [
            {"source_id": lock["source_id"], "unit_id": row["source_record_id"],
             "pointer": f"{evidence['member']}:{evidence['line']}",
             "sha256": p.digest(evidence["raw"].encode("utf-8"))}
            for evidence in row["evidence"]
        ]
        for gloss in dict.fromkeys(row["raw_senses"]):
            identity = p.digest(p.canonical([row["id"], gloss]))[:32]
            records.append({
                "id": f"sense:{identity}", "kind": "sense", "status": RESEARCH_SCOPE,
                "source_assertions": assertions,
                "payload": {"namespace": NAMESPACE, "entry_id": row["id"], "language": "zh",
                            "definition": gloss, "pos_id": None},
            })
    bundle.require(records, "所选词头无可投影的来源释义")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".semantic-candidate-", dir=output.parent) as temporary:
        staging = Path(temporary) / "candidate"
        staging.mkdir()
        shutil.copytree(release, staging / "lexicon")
        shutil.copyfile(release / "LICENSE.txt", staging / "LICENSE.txt")
        (staging / "NOTICE.txt").write_bytes(
            (release / "NOTICE.txt").read_bytes()
            + "用途：仅研究。原始英语释义保留，不推断双向语义等价、短语关系、词性或真实词频。\n".encode()
        )
        p.write_json(staging / "rights.json", {
            "schema_version": 1, "kind": "cloudleaf-semantic-source-rights-v1",
            "source_id": lock["source_id"], "version": lock["version"], "artifact_sha256": lock["sha256"],
            "license_id": lock["license_id"], "license_sha256": lock["license_sha256"],
            "status": RESEARCH_SCOPE, "research_only": True, "scope": ["sense"],
        })
        protocols = {
            "semantics": "源释义保留原文。查英文释义为检索线索，不能据此推断中英译法等价。",
            "preprocessing": "词头精确选择，保留源条目和源形。单词多义不合并；同一词条重复原释义去重。",
            "lineage": "语义义项由稳定词条 ID 和原释义内容计算，绑定词条目录、原始行号和原文摘要。",
        }
        for key, statement in protocols.items():
            (staging / f"{key}.txt").write_text(statement + "\n", encoding="utf-8", newline="\n")
        content = {
            "format": "cloudleaf-semantic-content-v2", "schema_version": 2,
            "catalogs": [{"namespace": NAMESPACE, "language": "zh", "path": "lexicon",
                          "manifest_sha256": manifest_sha,
                          "facts_sha256": verified["resources"]["records.jsonl"]["sha256"]}],
            "sources": [{"id": lock["source_id"], "version": lock["version"],
                         "artifact_sha256": lock["sha256"], "license_id": lock["license_id"],
                         "license": bundle.descriptor(staging, "LICENSE.txt"),
                         "rights": bundle.descriptor(staging, "rights.json"),
                         "status": RESEARCH_SCOPE, "scope": ["sense"]}],
            "protocols": {key: bundle.descriptor(staging, f"{key}.txt") for key in protocols},
            "records": sorted(records, key=lambda row: row["id"]),
        }
        migration = {"schema_version": 1, "from_manifest_sha256": None, "mappings": []}
        p.write_json(staging / "content.json", content)
        p.write_json(staging / "migration.json", migration)
        p.write_json(staging / "reviews.json", {"schema_version": 1,
                                                "review_binding_sha256": bundle.review_binding(content, migration),
                                                "records": []})
        result = bundle.build_bundle(staging, output)
    return {
        "bundle_manifest_sha256": result["manifest_sha256"], "source_sha256": lock["sha256"],
        "source_catalog_manifest_sha256": manifest_sha, "source_catalog_entries": verified["entry_count"],
        "matched_entries": len(selected), "senses": len(records), "unmatched_headwords": sorted(
            words.difference(form for row in selected for form in (row["text"], row["traditional"]))
        ), "lookup_scope": RESEARCH_SCOPE, "production_blockers": len(result["production_blockers"]),
        "explicit_translations": 0, "explicit_phrases": 0, "pos_claims": 0,
    }


def research_sources(root, content):
    sources = {}
    for source in content["sources"]:
        if source["status"] not in USABLE_SOURCE_STATES:
            continue
        rights = bundle.read_json(bundle.checked_resource(root, source["rights"]))
        bundle.shape(rights, "schema_version kind source_id version artifact_sha256 license_id license_sha256 status research_only scope")
        bundle.require(type(rights["schema_version"]) is int and rights["schema_version"] == 1
                       and rights["kind"] == "cloudleaf-semantic-source-rights-v1"
                       and rights["source_id"] == source["id"]
                       and rights["license_sha256"] == source["license"]["sha256"]
                       and rights["research_only"] is (source["status"] == RESEARCH_SCOPE)
                       and all(rights[key] == source[key] for key in
                               ("version", "artifact_sha256", "license_id", "status", "scope")),
                       "研究查询来源权利声明不匹配")
        sources[source["id"]] = source
    return sources


def export_index(root, expected, output):
    root, output = Path(root), Path(output)
    snapshot = bundle.read_bundle(root)
    bundle.require(snapshot["manifest_sha256"] == expected, "语义包清单摘要不符")
    content = snapshot["content"]
    sources = research_sources(root, content)
    included = {identity: record for identity, record in snapshot["records"].items()
                if record["status"] in USABLE_SOURCE_STATES and record["payload"] is not None
                and all(a["source_id"] in sources and record["kind"] in sources[a["source_id"]]["scope"]
                        for a in record["source_assertions"])}
    entries, senses, relations, phrases = {}, [], [], []
    needed = defaultdict(set)
    for row in included.values():
        value = row["payload"]
        if row["kind"] in {"sense", "phrase"}:
            needed[value["namespace"]].add(value["entry_id"])
        if row["kind"] == "phrase":
            for component in value["components"]:
                needed[component["namespace"]].add(component["entry_id"])
    catalogs = []
    for catalog in content["catalogs"]:
        namespace = catalog["namespace"]
        folder = bundle.resource_path(root, catalog["path"])
        release = p.read_json(folder / "manifest.json")
        catalogs.append({"namespace": namespace, "language": catalog["language"],
                         "manifest_sha256": catalog["manifest_sha256"], "facts_sha256": catalog["facts_sha256"],
                         "runtime_sha256": release["resources"]["runtime.tsv"]["sha256"]})
        if not needed[namespace]:
            continue
        with (folder / "records.jsonl").open(encoding="utf-8") as stream:
            for line in stream:
                row = json.loads(line)
                if row["id"] in needed[namespace]:
                    entries[(namespace, row["id"])] = row
    for identity, record in sorted(included.items()):
        value = record["payload"]
        payload = {"id": identity, "assertions": record["source_assertions"], **value}
        if record["kind"] == "sense":
            fact = entries[value["namespace"], value["entry_id"]]
            pos = included.get(value["pos_id"])
            payload["part_of_speech"] = pos["payload"] if pos else None
            del payload["pos_id"]
            # 仅经过核准的 CC-CEDICT 原始英语字段支持英语反查，其他义项不从字面猜语言。
            payload["english_source_gloss"] = (
                value["language"] == "zh" and value["definition"] in fact["raw_senses"]
                and fact["source"] == "cccedict:mirror-20260604"
            )
            senses.append(payload)
        elif record["kind"] == "translation":
            if all(value[key] in included for key in ("source_sense_id", "target_sense_id")):
                relations.append(payload)
        elif record["kind"] == "phrase":
            phrases.append(payload)
    projection = {
        "format": "cloudleaf-semantic-lookup-v1", "schema_version": 1, "scope": RESEARCH_SCOPE,
        "bundle_manifest_sha256": expected, "content_sha256": snapshot["manifest"]["content_sha256"],
        "catalogs": sorted(catalogs, key=lambda row: row["namespace"]),
        "sources": [{key: row[key] for key in SOURCE_FIELDS} for _, row in sorted(sources.items())],
        "entries": [{"namespace": namespace, "entry_id": entry_id, "headword": row["text"]}
                    for (namespace, entry_id), row in sorted(entries.items())],
        "senses": senses, "relations": relations, "phrases": phrases,
    }
    raw = p.canonical(projection)
    bundle.require(len(raw) <= bundle.LIMIT and senses, "索引过大或缺少可读取的真实义项")
    with output.open("xb") as stream:
        stream.write(raw)
    return {"index_sha256": p.digest(raw), "bytes": len(raw), "bundle_manifest_sha256": expected,
            "scope": RESEARCH_SCOPE, "entries": len(entries), "senses": len(senses),
            "explicit_translations": len(relations), "explicit_phrases": len(phrases),
            "english_source_glosses": sum(row["english_source_gloss"] for row in senses),
            "catalogs": projection["catalogs"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("build-cedict-research")
    build.add_argument("--release", type=Path, required=True)
    build.add_argument("--release-manifest-sha256", required=True)
    build.add_argument("--output", type=Path, required=True)
    build.add_argument("--headword", action="append", required=True)
    export = commands.add_parser("export")
    export.add_argument("--bundle", type=Path, required=True)
    export.add_argument("--bundle-manifest-sha256", required=True)
    export.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "build-cedict-research":
            report = build_cedict_candidate(args.release, args.release_manifest_sha256, args.output, args.headword)
        else:
            report = export_index(args.bundle, args.bundle_manifest_sha256, args.output)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
