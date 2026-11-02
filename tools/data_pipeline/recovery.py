"""单写者发布的只读进程识别与显式中断恢复，不模拟物理断电。"""

from contextlib import contextmanager
import ctypes
import json
import os
from pathlib import Path
import re
import sys
import uuid

from .filesystem import assert_plain_path, assert_plain_store, assert_plain_tree


def process_identity(pid: int) -> dict:
    if type(pid) is not int or pid <= 0:
        raise ValueError("进程PID必须为正整数")
    if sys.platform == "win32":
        from ctypes import wintypes
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        kernel.OpenProcess.restype = wintypes.HANDLE
        kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
        kernel.WaitForSingleObject.restype = wintypes.DWORD
        kernel.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
        kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        handle = kernel.OpenProcess(0x1000 | 0x100000, False, pid)
        if not handle:
            error = ctypes.get_last_error()
            return {"status": "dead" if error == 87 else "unknown", "start": None, "os_error": error}
        try:
            status = kernel.WaitForSingleObject(handle, 0)
            if status == 0:
                return {"status": "dead", "start": None}
            times = [wintypes.FILETIME() for _ in range(4)]
            if status != 258 or not kernel.GetProcessTimes(handle, *[ctypes.byref(value) for value in times]):
                return {"status": "unknown", "start": None}
            return {"status": "alive", "start": str((times[0].dwHighDateTime << 32) | times[0].dwLowDateTime)}
        finally:
            kernel.CloseHandle(handle)
    # POSIX的无信号探测不能用于Windows；权限不明始终拒绝恢复。
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return {"status": "dead", "start": None}
    except PermissionError:
        return {"status": "unknown", "start": None}
    stat = Path(f"/proc/{pid}/stat")
    try:
        start = stat.read_text(encoding="utf-8").split(")", 1)[1].split()[19] if stat.exists() else None
    except OSError:
        start = None
    return {"status": "alive", "start": start}


@contextmanager
def store_guard(store: Path):
    """操作系统锁随进程退出自动释放，保护遗留owner锁的显式恢复。"""
    assert_plain_store(store)
    store.mkdir(parents=True, exist_ok=True)
    path = store / ".writer.guard"
    assert_plain_store(store)
    with path.open("a+b") as stream:
        stream.seek(0, 2)
        if stream.tell() == 0:
            stream.write(b"0")
            stream.flush()
        stream.seek(0)
        try:
            if sys.platform == "win32":
                import msvcrt
                msvcrt.locking(stream.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(stream.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            raise ValueError("已有发布写者或恢复进程持有guard") from error
        try:
            yield
        finally:
            stream.seek(0)
            if sys.platform == "win32":
                msvcrt.locking(stream.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(stream.fileno(), fcntl.LOCK_UN)


def recover_store(store: Path, owner_pid: int, *, rollback: bool = False, verifier=None) -> dict:
    from .pipeline import atomic_json, checked_pointer, read_json
    if type(owner_pid) is not int or owner_pid <= 0:
        raise ValueError("恢复必须指定已中断写者正PID")
    # guard自身可能尚不存在；不安全的孤儿也必须在第一次创建guard之前拒绝。
    def preflight() -> list[Path]:
        assert_plain_store(store)
        if not store.is_dir():
            raise ValueError("恢复存储不存在")
        paths = [store / ".writer.lock"]
        paths += [path for path in store.iterdir() if re.fullmatch(r"\.publish-[A-Za-z0-9_-]+", path.name)]
        temporary = store / "current.tmp"
        if temporary.exists():
            paths.append(temporary)
        for path in paths:
            assert_plain_tree(path)
        return paths

    preflight()
    with store_guard(store):
        orphan_paths = preflight()
        lock_path = store / ".writer.lock"
        if not lock_path.is_file() or lock_path.stat().st_size > 4096:
            raise ValueError("缺少可识别遗留owner锁")
        text = lock_path.read_text(encoding="utf-8")
        lock = json.loads(text) if text.startswith("{") else {"owner_pid": int(text)}
        if lock.get("owner_pid") != owner_pid:
            raise ValueError("恢复PID不匹配遗留owner锁")
        status = process_identity(owner_pid)
        if status["status"] != "dead":
            raise ValueError("原写者仍存活、PID已复用或状态不明；拒绝恢复")
        pointer_path = store / "current.json"
        assert_plain_path(pointer_path)
        current = read_json(pointer_path) if pointer_path.exists() else None
        if rollback and not current:
            raise ValueError("没有可恢复回退的指针")
        recovered_pointer = None
        if current:
            if current.get("schema_version") != 1:
                raise ValueError("恢复指针版本无效")
            if rollback:
                if not current.get("previous"):
                    raise ValueError("没有有效回退目标")
                checked_pointer(store, current["previous"], verifier=verifier)
                recovered_pointer = {"schema_version": 1, "current": current["previous"], "previous": current["current"]}
            else:
                checked_pointer(store, current["current"], verifier=verifier)
        # 只移动本工具固定命名的孤儿至隔离目录，不删除缓存或用户词典。
        quarantine = store / (".recovered-" + uuid.uuid4().hex)
        for path in orphan_paths:
            assert_plain_tree(path)
        quarantine.mkdir()
        for path in orphan_paths[1:]:
            path.rename(quarantine / path.name)
        if recovered_pointer is not None:
            atomic_json(pointer_path, recovered_pointer)
            current = recovered_pointer
        lock_path.rename(quarantine / lock_path.name)
        return {"recovered": True, "owner_pid": owner_pid, "owner_status": status,
                "current": current, "quarantined": [path.name for path in orphan_paths],
                "scope": "process interruption recovery, not physical power-loss durability"}
