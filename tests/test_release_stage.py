import hashlib
import json
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

    def artifact(self, subdir, version, binary, *, seal=True):
        folder = self.parent / subdir
        files = {
            "bin/cloudleaf-cli.exe": binary,
            "licenses/LICENSE": b"test licence",
            "licenses/wasi-0.11.1+wasi-snapshot-preview1/LICENSE": b"versioned licence",
            "provenance/source-lock.json": b"{}",
            "provenance/Cargo.lock": b"locked dependencies",
            "resources/catalog.json": b"{}",
        }
        for name, data in files.items():
            path = folder / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        if seal:
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

    def test_signature_request_is_detached_and_binds_verified_manifest_bytes(self):
        release = self.artifact("signing", "r28.1", b"payload")
        manifest = release / "manifest.json"
        before = manifest.read_bytes()
        request = release_stage.signature_request(release)
        self.assertEqual(request["sha256"], hashlib.sha256(before).hexdigest())
        self.assertEqual(request["signed_input"], "raw-manifest-bytes")
        self.assertEqual(request["signature_status"], "unsigned")
        self.assertEqual(before, manifest.read_bytes())
        (release / "bin/cloudleaf-cli.exe").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "身份"):
            release_stage.signature_request(release)

    def test_migration_copies_allowlisted_settings_and_generations_without_overwrite(self):
        target = self.parent / "new-host"
        release_stage.initialize(target)
        dictionary = self.user / "user-dictionary"
        dictionary.mkdir()
        saved = dictionary / "generation-00000000000000000001.clu"
        saved.write_bytes(b"user entries and learning")
        (dictionary / ".pending-00000000000000000002-00.clu").write_bytes(b"uncommitted")
        (self.user / "credentials.json").write_bytes(b"excluded")
        (self.user / "input-settings.json.bak").write_bytes(b"previous configuration")
        (self.user / "language-resources.json").write_bytes(b"private resource selections")
        receipt = release_stage.migrate_user(self.root, target)
        result = target / "user"
        self.assertEqual({"settings.json", "input-settings.json.bak", "language-resources.json", f"user-dictionary/{saved.name}"},
                         {row["path"] for row in receipt["files"]})
        for row in receipt["files"]:
            self.assertEqual((self.user / row["path"]).read_bytes(), (result / row["path"]).read_bytes())
        self.assertEqual(list((result / "user-dictionary").iterdir()), [result / "user-dictionary" / saved.name])
        self.assertFalse((result / "credentials.json").exists())
        with self.assertRaisesRegex(ValueError, "目标用户目录已存在"):
            release_stage.migrate_user(self.root, target)

    def test_migration_failure_keeps_destination_unpublished(self):
        target = self.parent / "new-host"
        release_stage.initialize(target)
        os.link(self.user / "settings.json", self.user / "input-settings.json")
        with self.assertRaisesRegex(ValueError, "链接"):
            release_stage.migrate_user(self.root, target)
        self.assertFalse((target / "user").exists())
        self.assertEqual((self.user / "settings.json").read_bytes(), b"existing user settings")


    def test_dictionary_registry_pins_source_archive_and_license_identity(self):
        registry = json.loads(release_stage.DICTIONARY_CATALOG.read_bytes())
        root = release_stage.DICTIONARY_CATALOG.parents[1]
        seen = set()
        for entry in registry["resources"]:
            identity = (entry["slot"], entry["runtime_sha256"])
            self.assertNotIn(identity, seen)
            seen.add(identity)
            lock_raw = (root / entry["source_lock"]).read_bytes()
            self.assertEqual(entry["source_lock_sha256"], hashlib.sha256(lock_raw).hexdigest())
            lock = json.loads(lock_raw)
            self.assertEqual(lock["review_status"], "approved")
            self.assertEqual(entry["source_id"], lock["source_id"])
            self.assertEqual(entry["source_sha256"], lock["sha256"])
            self.assertEqual(entry["license_id"], lock["license_id"])
            self.assertEqual(entry["license_sha256"], lock["license_sha256"])
            license_raw = (root / lock["license_file"]).read_bytes()
            self.assertEqual(entry["license_sha256"], hashlib.sha256(license_raw).hexdigest())
            self.assertIn(entry["admission"], {"approved_source", "research_only"})
            if entry["slot"] == "supplement":
                self.assertEqual(entry["admission"], "research_only")

    def test_resource_scope_binds_catalog_and_keeps_external_dictionary_default_off(self):
        release = self.artifact("round29", "r29.1", b"binary", seal=False)
        staged = release / "resources/config/language-resources.json"
        staged.parent.mkdir()
        staged.write_bytes(release_stage.DICTIONARY_CATALOG.read_bytes())
        descriptor = release_stage.stamp_dictionaries(release)
        self.assertIsNone(descriptor["base_payload"])
        self.assertEqual({"enabled_by_default": False, "scope": "external"}, descriptor["supplement"])
        self.assertEqual({"generation": None, "scope": "external"}, descriptor["user_dictionary"])
        with self.assertRaisesRegex(ValueError, "不可重写"):
            release_stage.stamp_dictionaries(release)
        release_stage.seal(release, "r29.1", "a" * 40)
        checked = release_stage.verify(release)
        self.assertIn(release_stage.DICTIONARY_SCOPE, {row["path"] for row in checked["files"]})
        staged.write_bytes(b"{}")
        with self.assertRaisesRegex(ValueError, "身份"):
            release_stage.verify(release)


    def test_approved_historical_registry_subset_can_be_verified_for_rollback(self):
        release = self.artifact("old-catalog", "r29.1", b"binary", seal=False)
        staged = release / "resources/config/language-resources.json"
        old = json.loads(release_stage.DICTIONARY_CATALOG.read_bytes())
        old["resources"] = [old["resources"][0]]
        raw = json.dumps(old, ensure_ascii=False).encode()
        staged.parent.mkdir()
        staged.write_bytes(raw)
        descriptor = {
            "schema_version": 1, "catalog_sha256": hashlib.sha256(raw).hexdigest(),
            "base_payload": None, "supplement": {"enabled_by_default": False, "scope": "external"},
            "user_dictionary": {"generation": None, "scope": "external"}
        }
        (release / release_stage.DICTIONARY_SCOPE).write_text(json.dumps(descriptor), encoding="utf-8")
        receipt = release_stage.seal(release, "r29.1", "a" * 40)
        host = release_stage.install(self.root, release)
        self.assertEqual(release_stage.status(self.root)["current"], host["current"])
        self.assertEqual(release_stage.verify(release), receipt)

    def test_release_refuses_unknown_lexicon_payload_even_when_requested_as_base(self):
        release = self.artifact("round29", "r29.1", b"binary", seal=False)
        staged = release / "resources/config/language-resources.json"
        staged.parent.mkdir()
        staged.write_bytes(release_stage.DICTIONARY_CATALOG.read_bytes())
        runtime = "resources/runtime.data"
        (release / runtime).write_bytes(b"# cloudleaf-lexicon-v2\n")
        with self.assertRaisesRegex(ValueError, "缺少资源身份"):
            release_stage.seal(release, "r29.1", "a" * 40)
        with self.assertRaisesRegex(ValueError, "研究或未知"):
            release_stage.stamp_dictionaries(release, runtime)
        with self.assertRaisesRegex(ValueError, "未声明"):
            release_stage.stamp_dictionaries(release)
        (release / runtime).rename(release / "provenance/supplement.data")
        with self.assertRaisesRegex(ValueError, "缺少资源身份"):
            release_stage.seal(release, "r29.1", "a" * 40)
        with self.assertRaisesRegex(ValueError, "仅允许"):
            release_stage.stamp_dictionaries(release, "provenance/supplement.data")
        self.assertFalse((release / release_stage.DICTIONARY_SCOPE).exists())
        self.assertFalse((release / release_stage.MANIFEST).exists())


if __name__ == "__main__":
    unittest.main()
