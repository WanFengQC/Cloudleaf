"""验证仓库实际数据许可原件，而不是用新哈希掩盖导入时的换行转换。"""
from pathlib import Path
import hashlib
import json
import shutil
import tempfile
import unittest

from scripts.verify_input_lexicon import SOURCE_LICENSES, verify_source_licenses

ROOT = Path(__file__).resolve().parents[1]


class DataLicenseIntegrityTests(unittest.TestCase):
    def test_actual_license_bytes_match_frozen_source_lock(self):
        for lock_name, license_name in SOURCE_LICENSES:
            with self.subTest(source=lock_name):
                lock = json.loads((ROOT / "data/sources/locks" / lock_name).read_text(encoding="utf-8"))
                actual = hashlib.sha256((ROOT / "data/licenses" / license_name).read_bytes()).hexdigest()
                self.assertEqual(actual, lock["license_sha256"])

    def test_preflight_verifier_rejects_changed_original_without_rewriting_lock(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "data/sources/locks").mkdir(parents=True)
            (root / "data/licenses").mkdir(parents=True)
            for lock_name, license_name in SOURCE_LICENSES:
                for relative in (f"data/sources/locks/{lock_name}", f"data/licenses/{license_name}"):
                    shutil.copyfile(ROOT / relative, root / relative)
            self.assertEqual(len(verify_source_licenses(root)), len(SOURCE_LICENSES))
            for _, license_name in SOURCE_LICENSES:
                with self.subTest(license=license_name):
                    original = (root / "data/licenses" / license_name).read_bytes()
                    (root / "data/licenses" / license_name).write_bytes(original + b"\n")
                    with self.assertRaisesRegex(ValueError, license_name):
                        verify_source_licenses(root)
                    (root / "data/licenses" / license_name).write_bytes(original)


if __name__ == "__main__":
    unittest.main()
