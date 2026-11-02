"""真实子进程中断故障注入；不是物理断电或系统安装器验收。"""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from zipfile import ZipFile

from tools.data_pipeline.pipeline import activate, digest, read_json, verify_release, write_json
from tools.data_pipeline.reading_audit import audit_readings
from tools.data_pipeline.recovery import process_identity, recover_store
from tools.data_pipeline.pipeline import build


CHILD = """
import os,sys
from pathlib import Path
from tools.data_pipeline import pipeline as p
release,store,phase=Path(sys.argv[1]),Path(sys.argv[2]),sys.argv[3]
if phase=='lock':
    with p.publish_lock(store):
        os._exit(73)
if phase=='copy-mid':
    def copy(source,destination):
        destination.mkdir()
        (destination/'runtime.tsv').write_bytes(b'partial-copy')
        os._exit(73)
    p.shutil.copytree=copy
if phase=='cache-renamed':
    original=Path.rename
    def rename(path,target):
        result=original(path,target)
        if path.name=='release':
            os._exit(73)
        return result
    Path.rename=rename
if phase in {'pointer-written','pointer-replaced','rollback-before','rollback-after'}:
    original=p.os.replace
    def replace(source,target):
        if phase in {'pointer-written','rollback-before'}:
            os._exit(73)
        original(source,target)
        os._exit(73)
    p.os.replace=replace
if phase.startswith('rollback'):
    p.rollback(store)
else:
    p.activate(release,store)
raise AssertionError('fault injection not reached')
"""


def child_command(code, *arguments):
    # Windows venv入口是启动器，Popen.pid不一定是执行锁代码的进程；直接持有实际解释器句柄。
    executable = getattr(sys, "_base_executable", sys.executable) if os.name == "nt" else sys.executable
    root = str(Path(__file__).resolve().parents[1])
    bootstrap = "import sys;sys.path.insert(0,sys.argv.pop(1));exec(sys.argv.pop(1))"
    return [executable, "-X", "utf8", "-I", "-c", bootstrap, root, code, *map(str, arguments)]


class DataRecoveryTests(unittest.TestCase):
    @staticmethod
    def directory_link(link: Path, target: Path) -> None:
        if os.name == "nt":
            # junction无需开发者模式；路径来自本测试新建的临时目录。
            quote = lambda value: "'" + str(value).replace("'", "''") + "'"
            command = ("New-Item -ItemType Junction -Path " + quote(link)
                       + " -Target " + quote(target) + " -ErrorAction Stop | Out-Null")
            subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", command],
                           check=True, capture_output=True)
        else:
            link.symlink_to(target, target_is_directory=True)

    def test_store_and_ancestor_directory_links_reject_before_guard_creation(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for ancestor in [False, True]:
                with self.subTest(ancestor=ancestor):
                    physical = root / ("physical-parent" if ancestor else "physical-store")
                    physical.mkdir()
                    actual = physical / "store" if ancestor else physical
                    actual.mkdir(exist_ok=True)
                    write_json(actual / ".writer.lock", {"owner_pid": 12345})
                    (actual / "user-dictionary.json").write_bytes(b"private fixture")
                    before = {path.name: path.read_bytes() for path in actual.iterdir()}
                    link = root / ("linked-parent" if ancestor else "linked-store")
                    self.directory_link(link, physical)
                    requested = link / "store" if ancestor else link
                    with patch("tools.data_pipeline.recovery.process_identity") as identity:
                        with self.assertRaisesRegex(ValueError, "链接或越界"):
                            recover_store(requested, 12345)
                    identity.assert_not_called()
                    self.assertEqual({path.name: path.read_bytes() for path in actual.iterdir()}, before)

    def test_parent_traversal_is_not_normalized_past_a_directory_link(self):
        from tools.data_pipeline.recovery import store_guard
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            physical = root / "physical"
            physical.mkdir()
            link = root / "linked"
            self.directory_link(link, physical)
            target = root / "store"
            target.mkdir()
            with self.assertRaisesRegex(ValueError, "父级跳转"):
                with store_guard(link / ".." / "store"):
                    self.fail("链接路径不能经父级跳转消去检查")
            self.assertEqual(list(target.iterdir()), [])

    def test_nested_orphan_directory_link_is_rejected_before_any_recovery_mutation(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as temporary:
            store = Path(temporary)
            (store / "ordinary-inside-target").mkdir()
            orphan = store / ".publish-nested"
            orphan.mkdir()
            self.directory_link(orphan / "inside-link", store / "ordinary-inside-target")
            write_json(store / ".writer.lock", {"owner_pid": 12345})
            write_json(store / "current.json", {"schema_version": 1, "current": {}, "previous": {}})
            before = {name: (store / name).read_bytes() for name in [".writer.lock", "current.json"]}
            with patch("tools.data_pipeline.recovery.process_identity") as identity:
                with self.assertRaisesRegex(ValueError, "链接或越界"):
                    recover_store(store, 12345, rollback=True)
            identity.assert_not_called()
            self.assertFalse((store / ".writer.guard").exists())
            self.assertFalse(list(store.glob(".recovered-*")))
            self.assertTrue(orphan.is_dir())
            self.assertEqual({name: (store / name).read_bytes() for name in before}, before)

    def test_release_directory_links_and_linked_ancestors_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            physical = root / "physical"
            physical.mkdir()
            (physical / "release").mkdir()
            link = root / "linked"
            self.directory_link(link, physical)
            for candidate in [link, link / "release"]:
                with self.subTest(path=candidate):
                    with self.assertRaisesRegex(ValueError, "链接或越界"):
                        verify_release(candidate)

    def test_store_metadata_hardlinks_are_rejected_before_guard_writes(self):
        from tools.data_pipeline.recovery import store_guard
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in [".writer.guard", ".writer.lock", "current.json", "current.tmp"]:
                with self.subTest(name=name):
                    store = root / name.replace(".", "_")
                    store.mkdir()
                    outside = root / (store.name + "-outside")
                    outside.write_bytes(b"")
                    os.link(outside, store / name)
                    with self.assertRaisesRegex(ValueError, "链接或越界"):
                        with store_guard(store):
                            self.fail("硬链接不能取得发布guard")
                    self.assertEqual(outside.read_bytes(), b"")
                    self.assertFalse(list(store.glob(".recovered-*")))
                    self.assertEqual({path.name for path in store.iterdir()}, {name})

    def test_atomic_pointer_never_truncates_a_hardlink_or_existing_temporary(self):
        from tools.data_pipeline.pipeline import atomic_json
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            outside = root / "outside"
            outside.write_bytes(b"preserve-me")
            pointer = root / "current.json"
            temporary_path = root / "current.tmp"
            os.link(outside, temporary_path)
            with self.assertRaisesRegex(ValueError, "链接或越界"):
                atomic_json(pointer, {"schema_version": 1})
            self.assertEqual(outside.read_bytes(), b"preserve-me")
            self.assertFalse(pointer.exists())
            temporary_path.unlink()
            temporary_path.write_bytes(b"orphan-needs-explicit-recovery")
            with self.assertRaises(FileExistsError):
                atomic_json(pointer, {"schema_version": 1})
            self.assertEqual(temporary_path.read_bytes(), b"orphan-needs-explicit-recovery")

    def test_child_exit_recovery_at_copy_cache_and_pointer_boundaries(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            license_path = root / "LICENSE"
            license_path.write_bytes(b"test fixture license\n")
            releases = []
            for number, reading in enumerate(["yī", "èr"]):
                source, lock, release = root / f"source{number}.zip", root / f"lock{number}.json", root / f"release{number}"
                with ZipFile(source, "w") as package:
                    package.writestr("Unihan_Readings.txt", f"U+4E00\tkMandarin\t{reading}\n".encode("utf-8"))
                write_json(lock, {"schema_version": 1, "review_status": "approved", "format": "unihan-zip",
                                 "source_id": "cloudleaf:test-recovery", "version": f"fixture-{number}", "license_id": "MIT",
                                 "sha256": digest(source.read_bytes()), "license_sha256": digest(license_path.read_bytes()),
                                 "url": "https://example.invalid/fixture", "license_url": "https://example.invalid/license",
                                 "limitations": ["test fixture"], "attribution": "Cloudleaf fixture", "transformations": "test"})
                build(lock, source, license_path, release)
                releases.append(release)
            for phase in ["lock", "copy-mid", "cache-renamed", "pointer-written", "pointer-replaced", "rollback-before", "rollback-after"]:
                with self.subTest(phase=phase):
                    store = root / phase
                    initial = activate(releases[0], store)
                    if phase.startswith("rollback"):
                        initial = activate(releases[1], store)
                    user = store / "user-dictionary.json"
                    user.write_bytes(b"private-user-fixture")
                    user_sha = digest(user.read_bytes())
                    before = (store / "current.json").read_bytes()
                    # 仅启动自己的子进程；os._exit跳过finally模拟发布进程异常退出。
                    child = subprocess.Popen(child_command(CHILD, releases[1], store, phase),
                                             cwd=Path(__file__).resolve().parents[1], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                    stdout, stderr = child.communicate(timeout=30)
                    self.assertEqual(child.returncode, 73, (stdout, stderr))
                    self.assertTrue((store / ".writer.lock").exists())
                    self.assertEqual(read_json(store / ".writer.lock")["owner_pid"], child.pid)
                    if phase not in {"pointer-replaced", "rollback-after"}:
                        self.assertEqual((store / "current.json").read_bytes(), before)
                    state = read_json(store / "current.json")
                    verify_release(store / "releases" / state["current"]["release"])
                    recovered = recover_store(store, child.pid)
                    self.assertTrue(recovered["recovered"])
                    self.assertFalse((store / ".writer.lock").exists())
                    self.assertEqual(digest(user.read_bytes()), user_sha)
                    activate(releases[1], store)
                    self.assertEqual(digest(user.read_bytes()), user_sha)

    def test_alive_or_wrong_owner_and_unknown_status_never_clear_lock(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as temporary:
            store = Path(temporary)
            lock = store / ".writer.lock"
            write_json(lock, {"owner_pid": os.getpid(), "owner_start": process_identity(os.getpid())["start"]})
            original = lock.read_bytes()
            with self.assertRaisesRegex(ValueError, "仍存活"):
                recover_store(store, os.getpid())
            with self.assertRaisesRegex(ValueError, "不匹配"):
                recover_store(store, os.getpid() + 1)
            with patch("tools.data_pipeline.recovery.process_identity", return_value={"status": "unknown", "start": None}):
                with self.assertRaisesRegex(ValueError, "状态不明"):
                    recover_store(store, os.getpid())
            self.assertEqual(lock.read_bytes(), original)

    def test_parent_termination_releases_os_guard_but_requires_explicit_owner_recovery(self):
        with tempfile.TemporaryDirectory() as temporary:
            store = Path(temporary)
            code = ("import sys;from pathlib import Path;from tools.data_pipeline.pipeline import publish_lock;"
                    "\nwith publish_lock(Path(sys.argv[1])):\n print('ready',flush=True)\n sys.stdin.buffer.read(1)\n")
            child = subprocess.Popen(child_command(code, store), stdin=subprocess.PIPE,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                     cwd=Path(__file__).resolve().parents[1])
            try:
                self.assertEqual(child.stdout.readline().strip(), b"ready")
                self.assertEqual(read_json(store / ".writer.lock")["owner_pid"], child.pid)
                # 只终止本测试创建并仍持有Popen句柄的子进程，不按PID探测或操作其他进程。
                child.terminate()
                child.communicate(timeout=10)
                self.assertTrue((store / ".writer.lock").exists())
                self.assertTrue(recover_store(store, child.pid)["recovered"])
                self.assertFalse((store / ".writer.lock").exists())
            finally:
                if child.poll() is None:
                    child.kill()
                    child.communicate(timeout=10)

    def test_reading_audit_replays_source_without_changing_inventory(self):
        from tools.data_pipeline.pipeline import SYLLABLES
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, lock = root / "fixture.zip", root / "lock.json"
            with ZipFile(source, "w") as package:
                package.writestr("Unihan_Readings.txt", "U+3576\tkMandarin\tň\nU+55EF\tkMandarin\tǹg ēn\n".encode("utf-8"))
            write_json(lock, {"schema_version": 1, "review_status": "approved", "format": "unihan-zip",
                             "source_id": "cloudleaf:reading-test", "version": "test", "license_id": "MIT",
                             "sha256": digest(source.read_bytes()), "license_sha256": "a" * 64})
            before = set(SYLLABLES)
            report = audit_readings(lock, source)
            self.assertEqual(report["reading_count"], 2)
            self.assertTrue(all(row["uax38_rev39_syntax_matches"] for row in report["rows"]))
            self.assertEqual(report["impact_counts"]["character-has-another-runtime-reading"], 1)
            self.assertEqual(SYLLABLES, before)

    def test_unsafe_orphan_is_rejected_before_rollback_pointer_changes(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            store, outside = root / "store", root / "outside"
            store.mkdir()
            outside.mkdir()
            write_json(store / ".writer.lock", {"owner_pid": 12345})
            state = {"schema_version": 1, "current": {"release": "a", "manifest_sha256": "a"},
                     "previous": {"release": "b", "manifest_sha256": "b"}}
            write_json(store / "current.json", state)
            before = (store / "current.json").read_bytes()
            orphan = store / ".publish-unsafe"
            try:
                orphan.symlink_to(outside, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"本环境无法创建符号链接：{error}")
            with patch("tools.data_pipeline.recovery.process_identity", return_value={"status": "dead", "start": None}), \
                 patch("tools.data_pipeline.pipeline.checked_pointer"):
                with self.assertRaisesRegex(ValueError, "链接或越界"):
                    recover_store(store, 12345, rollback=True)
            self.assertEqual((store / "current.json").read_bytes(), before)
            self.assertTrue((store / ".writer.lock").exists())
