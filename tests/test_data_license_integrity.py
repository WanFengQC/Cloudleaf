"""验证仓库实际数据许可原件，而不是用新哈希掩盖导入时的换行转换。"""
from pathlib import Path
import hashlib
import json
import unittest

ROOT = Path(__file__).resolve().parents[1]
PAIRS = [
    ("unihan-17.0.0.json", "Unicode-3.0.txt"),
    ("cccedict-20260604-mirror7019e1c.json", "CC-BY-SA-4.0.txt"),
]


class DataLicenseIntegrityTests(unittest.TestCase):
    def test_actual_license_bytes_match_frozen_source_lock(self):
        for lock_name, license_name in PAIRS:
            with self.subTest(source=lock_name):
                lock = json.loads((ROOT / "data/sources/locks" / lock_name).read_text(encoding="utf-8"))
                actual = hashlib.sha256((ROOT / "data/licenses" / license_name).read_bytes()).hexdigest()
                self.assertEqual(actual, lock["license_sha256"])


if __name__ == "__main__":
    unittest.main()
