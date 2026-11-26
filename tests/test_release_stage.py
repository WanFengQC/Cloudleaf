import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from tools import release_stage
from tools.data_pipeline.pipeline import rollback
from tools.data_pipeline.recovery import recover_store


class ReleaseStageTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="cloudleaf-release-")
        self.addCleanup(directory.cleanup)
        self.parent = Path(directory.name)
        self.root = self.parent / "host"
        release_stage.initialize(self.root)
        self.user = self.root / "user"
        self.user.mkdir()
        (self.user / "settings.json").write_bytes(b"existing user settings")
        (self.user / "user-dictionary.tsv").write_bytes(b"existing user dictionary")
        self.store = release_stage.managed(self.root)

    def artifact(self, subdir, version, binary):
        folder = self.parent / subdir
        files = {
            "bin/cloudleaf-cli.exe": binary,
            "licenses/LICENSE": b"test licence",
            "provenance/source-lock.json": b"{}",
            "provenance/Cargo.lock": b"locked dependencies",
            "resources/catalog.json": b"{}",
        }
        for name, data in files.items():
            path = folder / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        release_stage.seal(folder, version, "a" * 40)
        return folder

    def test_idempotent_install_upgrade_corrupt_current_rollback_and_uninstall(self):
        first = self.artifact("one", "r27.1", b"first program")
        second = self.artifact("two", "r27.2", b"second program")
        old = release_stage.install(self.root, first)
        self.assertEqual(release_stage.install(self.root, first), old)
        current = release_stage.install(self.root, second)
        self.assertEqual(current["previous"], old["current"])
        self.assertEqual(release_stage.status(self.root)["current"], current["current"])
        active = self.store / "releases" / current["current"]["release"]
        (active / "bin/cloudleaf-cli.exe").write_bytes(b"corrupt")
        with self.assertRaisesRegex(ValueError, "身份"):
            release_stage.status(self.root)
        restored = rollback(self.store, verifier=release_stage.verify)
        self.assertEqual(restored["current"], old["current"])
        for _ in range(2):
            self.assertEqual(release_stage.uninstall(self.root), {"active": False})
        self.assertEqual(release_stage.status(self.root), {"active": False})
        self.assertEqual((self.user / "settings.json").read_bytes(), b"existing user settings")
        self.assertEqual((self.user / "user-dictionary.tsv").read_bytes(), b"existing user dictionary")
        self.assertEqual(release_stage.install(self.root, first)["current"], old["current"])

    def test_refuse_different_artifact_for_same_version_and_modified_source(self):
        good = self.artifact("original", "r27.1", b"v1")
        conflicting = self.artifact("conflict", "r27.1", b"conflict")
        corrupted = self.artifact("bad", "r27.2", b"v2")
        old = release_stage.install(self.root, good)
        with self.assertRaisesRegex(ValueError, "相同版本"):
            release_stage.install(self.root, conflicting)
        (corrupted / "resources/catalog.json").write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "身份"):
            release_stage.install(self.root, corrupted)
        self.assertEqual(release_stage.status(self.root)["current"], old["current"])

    def test_real_process_exit_after_staging_retains_active_and_allows_recovery(self):
        one = self.artifact("one", "r27.1", b"one")
        two = self.artifact("two", "r27.2", b"two")
        old = release_stage.install(self.root, one)
        program = (
            "import os, sys; from pathlib import Path; from tools import release_stage; "
            "from tools.data_pipeline import pipeline; "
            "pipeline.atomic_json=lambda *_: os._exit(75); "
            "release_stage.install(Path(sys.argv[1]),Path(sys.argv[2]))"
        )
        with subprocess.Popen([sys.executable, "-c", program, str(self.root), str(two)]) as writer:
            self.assertEqual(writer.wait(timeout=20), 75)
            pid = writer.pid
        self.assertEqual(release_stage.status(self.root)["current"], old["current"])
        with self.assertRaisesRegex(ValueError, "已有发布写者"):
            release_stage.install(self.root, two)
        recovered = recover_store(self.store, pid, verifier=release_stage.verify)
        self.assertEqual(recovered["current"]["current"], old["current"])
        self.assertEqual(release_stage.install(self.root, two)["previous"], old["current"])
        self.assertEqual((self.user / "settings.json").read_bytes(), b"existing user settings")

    def test_refuses_unmanaged_nonempty_and_linked_payload(self):
        foreign = self.parent / "foreign"
        foreign.mkdir()
        (foreign / "unrelated.txt").write_text("keep", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "空的隔离目录"):
            release_stage.initialize(foreign)
        with self.assertRaisesRegex(ValueError, "根身份"):
            release_stage.uninstall(foreign)
        good = self.artifact("one", "r27.1", b"one")
        os.link(good / "bin/cloudleaf-cli.exe", good / "bin/duplicate.exe")
        with self.assertRaisesRegex(ValueError, "链接"):
            release_stage.install(self.root, good)
        self.assertEqual(release_stage.status(self.root), {"active": False})


if __name__ == "__main__":
    unittest.main()
