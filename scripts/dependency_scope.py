"""按已经解析的 Cargo 包图选择许可清单所覆盖的发布闭包。"""
INPUT_ROOTS = {
    "cloudleaf-types", "cloudleaf-pinyin", "cloudleaf-dictionary", "cloudleaf-candidate",
    "cloudleaf-input", "cloudleaf-search", "cloudleaf-user", "cloudleaf-cli", "cloudleaf-windows-ime",
    "cloudleaf-settings",
}
IPC_ROOTS = {"cloudleaf-translation-ipc"}
INTEGRITY_ROOTS = {"cloudleaf-translation-integrity"}
TRANSLATION_ROOTS = IPC_ROOTS | INTEGRITY_ROOTS | {
    "cloudleaf-translation", "cloudleaf-offline", "cloudleaf-translation-panel", "cloudleaf-bridge",
    "cloudleaf-translation-worker",
}
IPC_REGISTRY = {
    "itoa", "memchr", "proc-macro2", "quote", "serde", "serde_core", "serde_derive",
    "serde_json", "syn", "unicode-ident", "zmij",
}


def validate_input_dependency_boundary(metadata):
    """使用实际传递闭包验证输入程序只接入轻量协议和完整性模块。"""
    inputs = select_workspace_closure(metadata, INPUT_ROOTS)
    input_locals = {p["name"] for p in inputs["packages"] if p["id"] in inputs["workspace_members"]}
    assert input_locals <= INPUT_ROOTS | IPC_ROOTS | INTEGRITY_ROOTS, "输入依赖包含翻译提供者或未审工作区模块"
    ipc = select_workspace_closure(metadata, IPC_ROOTS)
    assert len(ipc["workspace_members"]) == 1, "轻量 IPC 不能链接其它工作区模块"
    assert {p["name"] for p in ipc["packages"] if p["id"] not in ipc["workspace_members"]} == IPC_REGISTRY, "轻量 IPC 注册表闭包与审计约定不符"
    assert not any(p["name"].startswith(("candle-", "gemm", "tokio", "reqwest", "rustls")) or p["name"] == "sentencepiece-rs"
                   for p in inputs["packages"]), "输入构建包含模型或联网后端"
    return inputs, ipc


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
