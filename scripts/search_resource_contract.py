"""搜索观测的入口合同；计数上界不冒充字节内存或取消接口。"""


def validate_case(row, expected, configuration=None):
    for key in ("id", "input", "family", "kind"):
        if key in expected and row.get(key) != expected[key]:
            raise ValueError(f"搜索观测{key}不匹配")
    statistics = row.get("statistics", {})
    budget = statistics.get("budget")
    pruned = statistics.get("beam_pruned", 0)
    if budget not in (None, "GraphEdges", "Expansions", "ResidentPaths"):
        raise ValueError("未知预算终止原因")
    if (budget is not None or pruned) and row.get("guaranteed"):
        raise ValueError("预算耗尽/剪枝不能作为完整oracle")
    if row.get("status") not in ("ok", "rejected"):
        raise ValueError("搜索状态无效")
    if configuration is None:
        return
    candidates = row["candidates"]
    if len(candidates) > configuration["output_k"] or len({item["text"] for item in candidates}) != len(candidates):
        raise ValueError("候选超限或正文重复")
    if row.get("raw_fallback") != expected["input"] or row.get("fallback_preserved") is not True:
        raise ValueError("原输入回退丢失")
    if (expected["kind"] == "invalid") != (row["status"] == "rejected"):
        raise ValueError("冻结探针的合法输入/拒绝状态不匹配")
    if row["status"] == "rejected":
        if candidates or row.get("guaranteed") is not False or row.get("error_kind") != "InvalidInput":
            raise ValueError("拒绝结果不能含候选或完整性声明")
        return
    if type(row.get("guaranteed")) is not bool or row["guaranteed"] != (budget is None and pruned == 0):
        raise ValueError("成功结果完整性与预算/剪枝观测矛盾")
    for observed, limit in (("graph_edges", "max_edges"), ("expansions", "max_expansions"), ("resident_high_water", "max_resident_paths")):
        value = statistics[observed]
        if type(value) is not int or not 0 <= value <= configuration[limit]:
            raise ValueError(f"{observed}超出确定性计数预算")
    if budget in ("Expansions", "ResidentPaths") and configuration["method"] != "reference" and candidates:
        raise ValueError("非Reference搜索预算耗尽必须丢弃部分路径")


def validate_run(run, cases, configuration):
    if run.get("format") != "cloudleaf-search-resource-observation-v2" or run.get("configuration") != configuration:
        raise ValueError("观测版本或完整配置不匹配")
    rows = run["cases"]
    if len(rows) != len(cases) or len({row["id"] for row in rows}) != len(cases):
        raise ValueError("观测分母缺失或重复")
    indexed = {row["id"]: row for row in rows}
    if set(indexed) != {case["id"] for case in cases}:
        raise ValueError("观测案例集合改变")
    for case in cases:
        validate_case(indexed[case["id"]], case, configuration)
