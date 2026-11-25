"""按已经解析的 Cargo 包图选择许可清单所覆盖的发布闭包。"""
INPUT_ROOTS = {
    "cloudleaf-types", "cloudleaf-pinyin", "cloudleaf-dictionary", "cloudleaf-candidate",
    "cloudleaf-input", "cloudleaf-search", "cloudleaf-user", "cloudleaf-cli", "cloudleaf-windows-ime",
}
TRANSLATION_ROOTS = {"cloudleaf-translation", "cloudleaf-offline", "cloudleaf-translation-panel", "cloudleaf-bridge"}


def select_workspace_closure(metadata, names):
    members = set(metadata["workspace_members"])
    ids = {p["name"]: p["id"] for p in metadata["packages"] if p["id"] in members}
    assert names <= ids.keys(), "许可审计缺少预期的workspace入口"
    edges = {node["id"]: node["dependencies"] for node in metadata["resolve"]["nodes"]}
    seen = set()
    queue = [ids[name] for name in names]
    while queue:
        package = queue.pop()
        if package not in seen:
            seen.add(package)
            queue.extend(edges[package])
    return {
        "packages": [p for p in metadata["packages"] if p["id"] in seen],
        "workspace_members": sorted(members & seen),
    }
