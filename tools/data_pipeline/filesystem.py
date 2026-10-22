"""发布路径的保守链接门禁；不替代并发路径替换的句柄级安全沙箱。"""

import os
from pathlib import Path
import stat


def assert_plain_path(path: Path) -> None:
    """先检查词法父路径，避免resolve先跟随Windows junction掩盖来源。"""
    absolute = Path(os.path.abspath(path))
    for component in [*reversed(absolute.parents), absolute]:
        try:
            information = component.lstat()
        except FileNotFoundError:
            continue
        if (stat.S_ISLNK(information.st_mode)
                or getattr(information, "st_file_attributes", 0) & 0x400
                or (stat.S_ISREG(information.st_mode) and information.st_nlink > 1)):
            raise ValueError("路径含链接或越界（包括重解析点、文件硬链接）")


def assert_plain_tree(path: Path) -> None:
    """逐层lstat后才枚举目录，不跟随嵌套junction或符号链接。"""
    pending = [path]
    while pending:
        current = pending.pop()
        assert_plain_path(current)
        if current.is_dir():
            pending.extend(current.iterdir())


def assert_plain_store(store: Path) -> None:
    assert_plain_path(store)
    for name in [".writer.guard", ".writer.lock", "current.json", "current.tmp", "releases"]:
        assert_plain_path(store / name)
