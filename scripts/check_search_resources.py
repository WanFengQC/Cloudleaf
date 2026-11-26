"""固定真实数据下的配对计数合同；保留预算、剪枝和不完整结果。"""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import subprocess

from benchmark import environment, source_sha256
from evaluate_real_lexicon import CASES, ROOT, build_host, compare, sha256, validate_cases, validate_evidence
from search_resource_contract import validate_run


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    runtime, evidence, counts = validate_evidence(args.evidence)
    cases = validate_cases()
    before = {"source": source_sha256(), "runtime": sha256(runtime), "evidence": sha256(args.evidence), "cases": sha256(CASES)}
    binary = build_host()
    report = {"format": "cloudleaf-search-count-gate-v1", "head": subprocess.check_output(["git", "rev-parse", "HEAD"], encoding="utf-8").strip(),
              "environment": environment(), "at_utc": datetime.now(timezone.utc).isoformat(), "identity": before,
              "binary_sha256": sha256(binary), "evidence": evidence, "lexicon_counts": counts, "groups": {},
              "count_contract_passed": False, "production_resource_ready": False, "top1": None, "top5": None, "mrr": None,
              "limitations": ["5次小样本延迟不证明正式产品性能", "resident_high_water是路径计数不是字节内存", "未实现用户取消API", "没有独立人工gold", "整体进程内存不能归属单算法"]}
    failures = []
    for group, limits in {"standard": (16384, 200000, 50000), "edges128": (128, 200000, 50000),
                          "expansions500": (16384, 500, 50000), "resident128": (16384, 200000, 128)}.items():
        runs = {}
        for method in ("reference", "exact", "position", "beam"):
            edges, expansions, resident = limits
            config = {"method": method, "beam_width": 8, "max_edges": edges, "max_expansions": expansions,
                      "max_resident_paths": resident, "max_input_bytes": 512, "max_letters": 256, "output_k": 5,
                      "context_pool": 20, "ranker": "Structural"}
            command = [str(binary), str(runtime), str(CASES), method, "8", "5", "1", str(edges), str(expansions), str(resident), "5"]
            result = subprocess.run(command, cwd=ROOT, capture_output=True)
            prefix = args.output / f"{group}-{method}"
            prefix.with_suffix(".stdout.json").write_bytes(result.stdout)
            prefix.with_suffix(".stderr.txt").write_bytes(result.stderr)
            metadata = {"command": command, "returncode": result.returncode, "stdout_sha256": sha256(prefix.with_suffix('.stdout.json')),
                        "stderr_sha256": sha256(prefix.with_suffix('.stderr.txt'))}
            try:
                if result.returncode:
                    raise ValueError("资源宿主返回失败")
                observed = json.loads(result.stdout)
                validate_run(observed, cases, config)
                runs[method] = observed
                metadata["contract_valid"] = True
            except (ValueError, KeyError, TypeError) as error:
                metadata.update(contract_valid=False, error=str(error))
                failures.append(f"{group}/{method}: {error}")
            prefix.with_suffix(".command.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        comparisons, differences = compare(runs, cases) if len(runs) == 4 else ([], ["invalid observation"])
        failures.extend(f"{group}:{value}" for value in differences)
        report["groups"][group] = {"configuration_limits": limits, "comparisons": comparisons, "failures": differences,
            "methods": {method: {"cases": len(run["cases"]), "rejected": sum(row["status"] == "rejected" for row in run["cases"]),
                "budget_cases": sum(row.get("statistics", {}).get("budget") is not None for row in run["cases"]),
                "pruned_cases": sum(row.get("statistics", {}).get("beam_pruned", 0) > 0 for row in run["cases"]),
                "max_p95_ns": max(row["p95_ns"] for row in run["cases"])} for method, run in runs.items()}}
    validate_evidence(args.evidence)
    after = {"source": source_sha256(), "runtime": sha256(runtime), "evidence": sha256(args.evidence), "cases": sha256(CASES)}
    if after != before:
        failures.append("源码或固定输入身份改变")
    report.update(count_contract_passed=not failures, failures=failures, total_observed_cases=sum(value["cases"] for group in report["groups"].values() for value in group["methods"].values()))
    (args.output / "summary.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"count_contract_passed": not failures, "failures": failures, "total_observed_cases": report["total_observed_cases"], "production_resource_ready": False}))
    return int(bool(failures))


if __name__ == "__main__":
    raise SystemExit(main())
