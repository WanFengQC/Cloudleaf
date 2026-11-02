"""许可副本必须完整对应实际包，不能只检查自填清单是否自洽。"""

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("windows_audit", Path(__file__).resolve().parents[1] / "scripts/audit_windows_dependencies.py")
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)


class LicenseAuditTests(unittest.TestCase):
    def test_changed_original_and_missing_extra_or_duplicate_notices_fail(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            original = root / "registry/pkg"
            original.mkdir(parents=True)
            (original / "Cargo.toml").write_text("", encoding="utf-8")
            source = original / "LICENSE-MIT"
            source.write_bytes(b"original license\n")
            target = root / "licenses/windows"
            target.mkdir(parents=True)
            copied = target / "pkg-1.0.0-LICENSE-MIT"
            copied.write_bytes(source.read_bytes())
            package = {"id": "registry:pkg", "name": "pkg", "version": "1.0.0", "license": "MIT",
                       "source": "registry", "manifest_path": str(original / "Cargo.toml")}
            metadata = {"workspace_members": [], "packages": [package]}
            row = {"package": "pkg", "version": "1.0.0", "file": "licenses/windows/" + copied.name,
                   "sha256": hashlib.sha256(copied.read_bytes()).hexdigest()}
            manifest = target / "manifest.json"
            manifest.write_text(json.dumps([row]), encoding="utf-8")
            self.assertEqual(AUDIT.verify_licenses(root, metadata)[0]["name"], "pkg")
            source.write_bytes(b"different original\n")
            with self.assertRaisesRegex(AssertionError, "字节不同"):
                AUDIT.verify_licenses(root, metadata)
            source.write_bytes(copied.read_bytes())
            (original / "LICENSE-APACHE").write_bytes(b"another required license")
            with self.assertRaisesRegex(AssertionError, "完整保留"):
                AUDIT.verify_licenses(root, metadata)
            (original / "LICENSE-APACHE").unlink()
            manifest.write_text(json.dumps([row, row]), encoding="utf-8")
            with self.assertRaisesRegex(AssertionError, "重复登记"):
                AUDIT.verify_licenses(root, metadata)
            manifest.write_text(json.dumps([{**row, "version": "2.0.0"}]), encoding="utf-8")
            with self.assertRaisesRegex(AssertionError, "包集合"):
                AUDIT.verify_licenses(root, metadata)


if __name__ == "__main__":
    unittest.main()
