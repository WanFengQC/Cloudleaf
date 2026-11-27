"""核对固定 171 键并发配对并汇总顺序与资源证据。"""

import argparse
import hashlib
import json
from pathlib import Path

from round22_worker_replay import parse, percentiles
from round26_worker_isolation import attach_windows, latency


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def summarize(directory, baseline, conditions_path):
    raw_path = directory / "summary.json"
    report = read(raw_path)
    conditions = read(conditions_path)
    runs = report["runs"]
    if report["errors"] or conditions["returncode"] != 0 or conditions["root_head"] != report["head"]:
        raise ValueError("运行失败或记录无法绑定当前源码")
    if len(runs) < 16 or len(runs) % 2 or any(row["external"] for row in conditions["observations"]):
        raise ValueError("配对轮数不足或观测窗口存在已知竞争进程")
    resources = report["resources"]
    identity = {
        "trace_sha256": resources["traces"]["171"]["sha256"],
        "runtime_sha256": resources["runtime"]["sha256"],
        "lexical_sha256": resources["lexical"]["sha256"],
        "catalog_sha256": resources["catalog"]["sha256"],
    }
    if any(baseline[key] != value for key, value in identity.items()):
        raise ValueError("固定输入、词典或模型版本发生变化")
    result_groups = {"idle": [], "infer": []}
    pairs = []
    first = {"idle": [], "infer": []}
    second = {"idle": [], "infer": []}
    for index in range(0, len(runs), 2):
        number = index // 2 + 1
        pair = runs[index:index + 2]
        order = ["idle", "infer"] if number % 2 else ["infer", "idle"]
        if [run["stage"] for run in pair] != order:
            raise ValueError(f"第 {number} 轮顺序不符合冻结交替条件")
        row = {"pair": number, "order": order}
        for results, run in zip((first, second), pair, strict=True):
            stage = run["stage"]
            if (run["repetition"], run["version"], run["trajectory"], run["mode"],
                run["policy"], run["events"], run["actual_compute"]) != (
                    number, "current", "171", "lexical", "normal", 171, 1):
                raise ValueError(f"第 {number} 轮运行条件不符")
            if run["candidate_sha256"] != baseline["candidate_sha256"]:
                raise ValueError(f"第 {number} 轮候选、提交或统计投影发生变化")
            path = directory / run["label"] / "result.json"
            if sha256(path) != run["record"]["sha256"]:
                raise ValueError(f"第 {number} 轮进程记录发生变化")
            raw = read(path)
            text = path.with_name("stdout.txt")
            if raw["returncode"] != 0 or sha256(text) != raw["stdout"]["sha256"]:
                raise ValueError(f"第 {number} 轮标准输出或返回码不符")
            lines = text.read_text(encoding="utf-8").splitlines()
            events, _, models, *_ = parse(lines, 171)
            cycles = {int(fields[1]): int(fields[2]) for fields in map(lambda ln: ln.split("\t"), lines)
                      if fields[0] == "CYCLE"}
            for event in events:
                event["cycles"] = cycles[event["index"]]
            model = models.get("main", {})
            if stage == "infer":
                if not model or "worker" not in run["actual"]:
                    raise ValueError(f"第 {number} 轮未观察到真实模型工作进程")
                events = attach_windows(events, model)["infer"]
            if (len(events) != 171
                    or any(field["before"] != field["actual"] or field["priority"] != 32
                           for field in run["actual"].values())):
                raise ValueError(f"第 {number} 轮事件窗口或实际调度不符")
            stats = latency(events)
            result_groups[stage].extend(events)
            results[stage].extend(events)
            row[stage] = {
                "wall_ns": stats["wall_ns"], "cpu_ns": stats["cpu_ns"],
                "cycles": stats["cycles"], "system_cpu_fraction": run["system_cpu_fraction"],
                "load_ns": model.get("load_ns"), "queue_wait_ns": model.get("queue_wait_ns"),
                "inference_ns": model.get("inference_ns"), "record_sha256": sha256(path),
            }
        row["ratio"] = row["infer"]["wall_ns"]["p95"] / row["idle"]["wall_ns"]["p95"]
        pairs.append(row)
    idle, infer = (latency(result_groups[key])["wall_ns"] for key in ("idle", "infer"))
    ratio = infer["p95"] / idle["p95"]
    aggregate = report["aggregate"]["current/171/lexical/infer/normal"]
    return {
        "head": report["head"], **identity, "samples": len(pairs),
        "candidate_sha256": baseline["candidate_sha256"],
        "binary_sha256": {key: report["executables"]["current"][key]["sha256"]
                          for key in ("probe", "worker")},
        "summary_sha256": sha256(raw_path), "conditions_sha256": sha256(conditions_path),
        "quiet_samples": len(conditions["observations"]),
        "idle": idle, "infer": infer, "ratio": ratio, "pairs": pairs,
        "gates": {"idle_p95": idle["p95"] <= 5_000_000, "idle_p99": idle["p99"] <= 10_000_000,
                  "infer_p95": infer["p95"] <= 8_000_000, "ratio": ratio <= 1.5},
        "within_ratio_gate": sum(pair["ratio"] <= 1.5 for pair in pairs),
        "first_wall_ns": {key: percentiles([event["wall_ns"] for event in value]) for key, value in first.items()},
        "second_wall_ns": {key: percentiles([event["wall_ns"] for event in value]) for key, value in second.items()},
        "translation": aggregate["translation"], "steps_per_second": aggregate["steps_per_second"],
        "system_cpu_fraction": aggregate["system_cpu_fraction"],
        "memory": {phase: report["aggregate"][f"current/171/lexical/{phase}/normal"]["peak_observed_memory"]
                   for phase in ("idle", "infer")},
    }


def main():
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("directory", type=Path)
    args.add_argument("baseline", type=Path)
    args.add_argument("conditions", type=Path)
    args.add_argument("out", type=Path)
    options = args.parse_args()
    data = summarize(options.directory, read(options.baseline), options.conditions)
    with options.out.open("x", encoding="utf-8") as stream:
        json.dump(data, stream, indent=2, ensure_ascii=False)
        stream.write("\n")
    print(json.dumps({key: data[key] for key in ("head", "samples", "ratio", "gates")}, ensure_ascii=False))


if __name__ == "__main__":
    main()
