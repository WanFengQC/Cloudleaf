//! Windows 专属宿主边界；不实现候选算法，不在实时输入路径访问文件。

#[cfg(windows)]
#[path = "../../../adapters/windows/mod.rs"]
pub mod native;
