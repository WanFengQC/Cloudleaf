"""验证新增方向只能显式登记，不能漏掉包、来源、许可证或损坏资源。"""

import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("workspace_audit", Path(__file__).resolve().parents[1] / "scripts/check_workspace.py")
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)


class AuditTests(unittest.TestCase):
    def test_external_packages_require_exact_version_license_and_source(self):
        member = {"id": "local", "name": "core", "version": "0.1.0", "license": "MIT", "source": None, "rust_version": "1.90.0", "edition": "2024"}
        external = {"id": "registry", "name": "binding", "version": "1.2.3", "license": "MIT OR Apache-2.0", "source": AUDIT.REGISTRY}
        metadata = {"workspace_members": ["local"], "packages": [member, external]}
        reviewed = {key: external[key] for key in ("name", "version", "license", "source")}
        policy = {"workspace_packages": {"core"}, "external_packages": {("binding", "1.2.3"): reviewed}}
        self.assertEqual(AUDIT.check_packages(metadata, policy, "1.90.0"), (1, 1))
        for key, value in [("version", "1.2.4"), ("license", "GPL-3.0"), ("source", "git+https://example.com/binding")]:
            changed = copy.deepcopy(metadata)
            changed["packages"][1][key] = value
            with self.assertRaises(AssertionError):
                AUDIT.check_packages(changed, policy, "1.90.0")
        for packages in [[member], [external]]:
            with self.assertRaises(AssertionError):
                AUDIT.check_packages({**metadata, "packages": packages}, policy, "1.90.0")

    def test_policies_reject_duplicates_unreviewed_sources_and_path_escape(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = root / "config/audits"
            directory.mkdir(parents=True)
            policy = {"schema_version": 1, "workspace_packages": ["core"], "external_packages": [], "resources": []}
            (directory / "core.json").write_text(json.dumps(policy), encoding="utf-8")
            self.assertEqual(AUDIT.load_policy(root)["workspace_packages"], {"core"})
            (directory / "other.json").write_text(json.dumps(policy), encoding="utf-8")
            with self.assertRaises(AssertionError):
                AUDIT.load_policy(root)
            for name in ["../outside", "/etc/passwd", "C:/Windows/file", "data\\escape"]:
                with self.assertRaises(AssertionError):
                    AUDIT.repository_file(root, name)

    def test_resource_hash_license_count_format_and_missing_registration(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "data/manifests").mkdir(parents=True)
            (root / "LICENSE").write_text("test license\n", encoding="utf-8")
            content = b'{"id":"a"}\n{"id":"b"}\n'
            resource = root / "data/words.jsonl"
            resource.write_bytes(content)
            reviewed = {"path": "data/words.jsonl", "source": "upstream:fixed", "license": "Unicode-3.0", "license_file": "LICENSE"}
            manifest = {**reviewed, "schema_version": 1, "sha256": hashlib.sha256(content).hexdigest(), "entry_count": 2, "format": "jsonl"}
            path = root / "data/manifests/words.json"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            policy = {"resources": {reviewed["path"]: reviewed}}
            AUDIT.check_resources(root, policy)
            for key, value in [("license", "MIT"), ("source", "cloudleaf:fake"), ("sha256", "0" * 64), ("entry_count", 3), ("format", "unknown")]:
                path.write_text(json.dumps({**manifest, key: value}), encoding="utf-8")
                with self.assertRaises(AssertionError):
                    AUDIT.check_resources(root, policy)
            path.write_text(json.dumps(manifest), encoding="utf-8")
            resource.write_bytes(b'[]\n')
            altered = {**manifest, "entry_count": 1, "sha256": hashlib.sha256(resource.read_bytes()).hexdigest()}
            path.write_text(json.dumps(altered), encoding="utf-8")
            with self.assertRaises(AssertionError):
                AUDIT.check_resources(root, policy)
            path.unlink()
            with self.assertRaises(AssertionError):
                AUDIT.check_resources(root, policy)


if __name__ == "__main__":
    unittest.main()
