"""离线核对正式评估前置证据；不打开封存答案，不计算语言准确率。"""
from __future__ import annotations

import argparse
from collections import Counter
import csv
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import sys

from benchmark import source_sha256

ROOT = Path(__file__).resolve().parents[1]
PROTOCOL = ROOT / "docs/cl008-real-resource-protocol.md"
SPLITS = ("train", "dev", "heldout")
INDEX_FIELDS = {"case_id", "family_id", "source_id", "source_unit_id", "near_duplicate_group", "split",
                "normalized_input_sha256", "task_sha256"}
SHA = re.compile(r"[0-9a-f]{64}\Z")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def protocol_digest() -> str:
    # Git中的协议身份固定为UTF-8/LF；实际数据/二进制仍核对原始字节。
    return hashlib.sha256(PROTOCOL.read_text(encoding="utf-8").replace("\r\n", "\n").encode("utf-8")).hexdigest()


def require(value: bool, message: str) -> None:
    if not value:
        raise ValueError(message)


def no_duplicate_keys(pairs: list[tuple]) -> dict:
    result = {}
    for key, value in pairs:
        require(key not in result, "JSON对象字段重复")
        result[key] = value
    return result


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=no_duplicate_keys)


def checked(item: dict, folder: Path) -> Path:
    require(isinstance(item, dict) and isinstance(item.get("path"), str)
            and isinstance(item.get("sha256"), str) and bool(SHA.fullmatch(item["sha256"])), "文件描述或SHA缺失")
    path = (folder / item["path"]).resolve()
    require(path.is_file() and path.stat().st_size <= 64 * 1024 * 1024 and digest(path) == item["sha256"], "证据文件缺失、超限或SHA不符")
    return path


def instant(value: str) -> datetime:
    result = datetime.fromisoformat(value)
    require(result.tzinfo is not None, "时间必须含时区")
    return result


def normalized_digest(value: str) -> str:
    return hashlib.sha256(value.lower().replace("ü", "v").encode("utf-8")).hexdigest()


def known_seen_inputs() -> set[str]:
    excluded = set()
    for name in ("tests/fixtures/input-v1.tsv", "tests/evaluation/algorithm-v1.tsv"):
        rows = csv.DictReader((ROOT / name).read_text(encoding="utf-8").splitlines()[1:], delimiter="\t")
        for row in rows:
            if "input" in row:
                excluded.add(normalized_digest(row["input"]))
            else:
                excluded.add(normalized_digest(row["events"]))
                if row["preedit"] != "-":
                    excluded.add(normalized_digest(row["preedit"]))
                for token in row["events"].split():
                    if token.startswith("text:"):
                        excluded.add(normalized_digest(token[5:]))
    return excluded


def family_quotas(total: int) -> dict:
    weights = (70, 15, 15)
    counts = [total * weight // 100 for weight in weights]
    order = sorted(range(3), key=lambda index: (-(total * weights[index] % 100), index))
    for index in order[:total - sum(counts)]:
        counts[index] += 1
    return dict(zip(SPLITS, counts))


def sources(plan: dict, folder: Path) -> dict:
    result = {}
    for source in plan.get("sources", []):
        key = source["id"]
        require(key not in result and source.get("role") in ("lexicon", "evaluation-corpus"), "来源重复或角色缺失")
        require(source.get("version") and source.get("official_url", "").startswith("https://")
                and source.get("review_status") == "approved" and source.get("approved_scope"), "来源缺少版本/官方URL/批准范围")
        permissions = source.get("permissions", {})
        require(permissions.get("evaluation") is True and all(type(permissions.get(key)) is bool for key in
                ("commercial", "modification", "redistribution")), "未明确允许评估，或商业/修改/再分发权限未知")
        checked(source["license"], folder)
        checked(source["audit"], folder)
        checked(source["artifact"], folder)
        result[key] = source
    require({source["role"] for source in result.values()} == {"lexicon", "evaluation-corpus"}, "缺少独立评估语料或词库来源批准")
    return result


def gold(plan: dict, folder: Path, approved: dict) -> dict:
    value = plan.get("gold", {})
    require(value.get("status") == "independent-human-reviewed" and value.get("source_id") in approved
            and approved[value["source_id"]]["role"] == "evaluation-corpus", "缺少已核验的独立人工gold来源")
    require(value.get("dictionary_derived") is False and value.get("synthetic") is False
            and value.get("old_seed_used") is False and value.get("independent_annotators") is True, "gold不是独立人工标注或使用已见种子")
    checked(value["annotation_protocol"], folder)
    checked(value["human_review"], folder)
    return {"status": value["status"], "source_id": value["source_id"], "automatic_independence_proven": False}


def split_index(plan: dict, folder: Path, approved: dict) -> tuple[list[dict], dict]:
    require(plan.get("normalization") == "ascii-lower-umlaut-v1", "规范摘要版本未固定")
    path = checked(plan.get("family_index"), folder)
    rows, ids, owners, family_split = [], set(), {}, {}
    seen = known_seen_inputs()
    with path.open(encoding="utf-8") as stream:
        for line in stream:
            row = json.loads(line, object_pairs_hook=no_duplicate_keys)
            require(set(row) == INDEX_FIELDS and all(isinstance(value, str) and value for value in row.values()), "家族清单字段缺失或含明文答案/额外字段")
            require(row["case_id"] not in ids and row["split"] in SPLITS, "案例重复或split无效")
            require(row["source_id"] in approved and approved[row["source_id"]]["role"] == "evaluation-corpus", "样本来源未获评估批准")
            require(SHA.fullmatch(row["normalized_input_sha256"]) and SHA.fullmatch(row["task_sha256"]), "样本摘要无效")
            require(row["normalized_input_sha256"] not in seen, "样本与41事件或83已见seed重叠")
            ids.add(row["case_id"])
            split = row["split"]
            for key in [("family", row["family_id"]), ("unit", row["source_id"], row["source_unit_id"]),
                        ("near", row["near_duplicate_group"]),
                        ("input", row["normalized_input_sha256"]), ("task", row["task_sha256"])]:
                require(owners.setdefault(key, split) == split, "家族/来源段落/近重复/规范输入/任务跨split泄漏")
            family_split[row["family_id"]] = split
            rows.append(row)
            require(len(rows) <= 1_000_000, "评估案例数量超限")
    require(rows, "家族清单为空")
    families = {split: sum(value == split for value in family_split.values()) for split in SPLITS}
    require(families == family_quotas(len(family_split)), "家族70/15/15预登记配额不符")
    require(families["heldout"] >= 2000, "heldout不足2000个独立家族")
    return rows, {"cases": dict(Counter(row["split"] for row in rows)), "families": families,
                  "total_cases": len(rows), "total_families": len(family_split), "index_sha256": digest(path)}


def seal(plan: dict, folder: Path) -> dict:
    value = plan.get("seal", {})
    require(value.get("state") == "sealed" and value.get("gold_opened") is False
            and value.get("custodian_id") and value.get("custodian_id") != plan.get("evaluator_id"), "heldout未封存/已打开/保管者与评估者未分离")
    require(value.get("family_index_sha256") == plan["family_index"]["sha256"], "封存未绑定当前家族清单")
    artifact = checked(value["artifact"], folder)
    require(artifact.stat().st_size > 0 and value.get("protection") == "custodian-encrypted", "封存保护声明或文件为空")
    checked(value["custodian_attestation"], folder)
    require(instant(plan["registered_at"]) <= instant(value["sealed_at"]) <= datetime.now(timezone.utc), "注册/封存时间无效")
    return {"state": "sealed", "artifact_sha256": digest(artifact), "gold_opened_by_tool": False,
            "encryption_and_custodian_identity_independently_verified": False}


def freeze(plan: dict, folder: Path, approved: dict) -> dict:
    value = plan.get("freeze", {})
    require(instant(plan["seal"]["sealed_at"]) <= instant(value["frozen_at"]) <= datetime.now(timezone.utc), "参数冻结早于封存或位于未来")
    parameters = checked(value["parameters"], folder)
    require(value.get("source_sha256") == source_sha256(), "实际源码与冻结指纹不符")
    require(value.get("protocol_sha256") == protocol_digest(), "冻结协议不符")
    binary, lexicon = checked(value["binary"], folder), checked(value["lexicon"], folder)
    source = approved.get(value.get("lexicon_source_id"), {})
    require(source.get("role") == "lexicon" and source.get("artifact", {}).get("sha256") == digest(lexicon), "冻结词库未绑定获批来源产物")
    return {"parameters_sha256": digest(parameters), "source_sha256": value["source_sha256"],
            "binary_sha256": digest(binary), "lexicon_sha256": digest(lexicon), "frozen_at": value["frozen_at"]}


def predictions(path: Path, rows: list[dict], frozen: dict) -> dict:
    value = read_json(path)
    require(value.get("schema_version") == 1 and value.get("bindings") == {key: frozen[key] for key in
            ("parameters_sha256", "source_sha256", "binary_sha256", "lexicon_sha256")}, "预测未绑定冻结版本")
    require(instant(value["run_started_at"]) >= instant(frozen["frozen_at"]), "预测运行早于参数冻结")
    expected = {row["case_id"] for row in rows if row["split"] == "heldout"}
    actual = set()
    statuses = Counter()
    for row in value.get("cases", []):
        require(row.get("case_id") not in actual and row.get("case_id") in expected, "预测重复或包含冻结分母之外的案例")
        require(row.get("status") in ("ok", "rejected", "budget", "unsupported") and isinstance(row.get("candidates"), list)
                and type(row.get("fallback_preserved")) is bool, "预测状态/原文回退/候选缺失")
        actual.add(row["case_id"])
        statuses[row["status"]] += 1
    require(actual == expected, "预测丢失冻结heldout案例，禁止删除失败分母")
    return {"expected_cases": len(expected), "actual_cases": len(actual), "statuses": dict(statuses), "language_metrics_computed": False}


def audit(plan: dict, folder: Path, result_path: Path | None = None) -> dict:
    issues, sections = [], {}
    def stage(name, action):
        try:
            value = action()
            sections[name] = {"passed": True, "details": value}
            return value
        except (ValueError, KeyError, TypeError, OSError) as error:
            issues.append({"section": name, "reason": str(error)})
            sections[name] = {"passed": False}
            return None
    def protocol():
        require(plan.get("schema_version") == 1 and plan.get("experiment") == "cloudleaf-formal-language-prerequisites-v1", "计划版本无效")
        require(plan.get("protocol_sha256") == protocol_digest() and plan.get("family_ratios") == [70, 15, 15]
                and plan.get("minimum_heldout_families") == 2000, "既有协议、比例或heldout门限被修改")
        require(plan.get("evaluator_id") and instant(plan["registered_at"]) <= datetime.now(timezone.utc), "评估者或预登记时间缺失")
        return {"protocol_sha256": protocol_digest(), "family_ratios": [70, 15, 15], "minimum_heldout_families": 2000}
    stage("protocol", protocol)
    approved = stage("sources", lambda: sources(plan, folder)) or {}
    stage("gold", lambda: gold(plan, folder, approved))
    indexed = stage("split_isolation", lambda: split_index(plan, folder, approved))
    if indexed is not None:
        rows, summary = indexed
        sections["split_isolation"]["details"] = summary
    else:
        rows = []
    stage("heldout_seal", lambda: seal(plan, folder))
    frozen = stage("parameter_freeze", lambda: freeze(plan, folder, approved))
    if result_path is not None:
        stage("prediction_denominator", lambda: predictions(result_path, rows, frozen or {}))
    return {"schema_version": 1, "experiment": "cloudleaf-language-readiness-report-v1", "prerequisites_ready": not issues,
            "language_accuracy_valid": False, "top1": None, "top5": None, "mrr": None, "sealed_gold_opened_by_tool": False,
            "sections": sections, "issues": issues,
            "limitations": ["只核验证据及摘要一致性，不自动证明人工标注、近重复语义、加密或保管者身份真实。",
                            "未读取封存答案、未执行语言打分；前置材料通过不等于正式质量或产品验收通过。"]}


def run() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--predictions", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("输出已存在，禁止覆盖历史证据")
    report = audit(read_json(args.plan), args.plan.parent, args.predictions)
    report.update(plan_sha256=digest(args.plan), tool_sha256=digest(Path(__file__)), source_sha256=source_sha256(),
                  checked_at_utc=datetime.now(timezone.utc).isoformat())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"output": str(args.output), "prerequisites_ready": report["prerequisites_ready"], "issues": report["issues"]}, ensure_ascii=False))
    if not report["prerequisites_ready"]:
        raise SystemExit(2)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    run()
