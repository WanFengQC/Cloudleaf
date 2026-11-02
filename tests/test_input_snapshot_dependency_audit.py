"""输入快照许可与旧Windows原件分别核验，新增包不改变旧清单格式。"""

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("dependency_audit", Path(__file__).resolve().parents[1] / "scripts/audit_windows_dependencies.py")
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)


class SnapshotLicenseAuditTests(unittest.TestCase):
    def test_nested_snapshot_notices_bind_metadata_paths_and_actual_original_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            original = root / "registry/pkg"
            (original / "third_party").mkdir(parents=True)
            (original / "Cargo.toml").write_text("", encoding="utf-8")
            source = original / "third_party/LICENSE"
            source.write_bytes(b"nested original license\n")
            windows = root / "licenses/windows"
            windows.mkdir(parents=True)
            (windows / "manifest.json").write_text("[]", encoding="utf-8")
            target = root / "licenses/input-snapshot/pkg-1.0.0"
            target.mkdir(parents=True)
            copied = target / "LICENSE"
            copied.write_bytes(source.read_bytes())
            package = {"id": "registry:pkg", "name": "pkg", "version": "1.0.0", "license": "MIT",
                       "source": "registry", "manifest_path": str(original / "Cargo.toml")}
            metadata = {"workspace_members": [], "packages": [package]}
            row = {"package": "pkg", "version": "1.0.0", "license": "MIT", "source": "registry",
                   "file": "licenses/input-snapshot/pkg-1.0.0/LICENSE", "source_relative": "third_party/LICENSE",
                   "bytes": copied.stat().st_size, "sha256": hashlib.sha256(copied.read_bytes()).hexdigest()}
            manifest = root / "licenses/input-snapshot/manifest.json"
            manifest.write_text(json.dumps([row]), encoding="utf-8")
            self.assertEqual(AUDIT.verify_licenses(root, metadata)[0]["name"], "pkg")
            source.write_bytes(b"changed original license\n")
            with self.assertRaisesRegex(AssertionError, "字节不同"):
                AUDIT.verify_licenses(root, metadata)
            source.write_bytes(copied.read_bytes())
            extra = original / "third_party/NOTICE"
            extra.write_bytes(b"another required original")
            with self.assertRaisesRegex(AssertionError, "完整保留"):
                AUDIT.verify_licenses(root, metadata)
            extra.unlink()
            for changes, message in [
                ({"license": "Apache-2.0"}, "metadata"),
                ({"source": "unreviewed"}, "metadata"),
                ({"source_relative": "../LICENSE"}, "越界"),
                ({"bytes": row["bytes"] + 1}, "长度"),
            ]:
                manifest.write_text(json.dumps([{**row, **changes}]), encoding="utf-8")
                with self.assertRaisesRegex(AssertionError, message):
                    AUDIT.verify_licenses(root, metadata)
            manifest.write_text(json.dumps([row, row]), encoding="utf-8")
            with self.assertRaisesRegex(AssertionError, "重复登记"):
                AUDIT.verify_licenses(root, metadata)


if __name__ == "__main__":
    unittest.main()
