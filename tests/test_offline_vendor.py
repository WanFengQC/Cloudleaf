"""核对真实运行时源补丁门禁，避免任意path依赖或运算源码变化混入。"""
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from check_offline_vendor import verify_offline_vendor, reviewed_local_source


class OfflineVendorTests(unittest.TestCase):
    def test_actual_source_and_exact_local_path_are_verified(self):
        reviewed = verify_offline_vendor(ROOT)
        self.assertEqual(len(reviewed), 9)
        package = {'name': 'candle-core', 'version': '0.9.2', 'source': None,
                   'manifest_path': str(ROOT / 'vendor/offline/candle-core/Cargo.toml')}
        self.assertEqual(reviewed_local_source(ROOT, package, reviewed), 'path+vendor/offline/candle-core')
        package['manifest_path'] = str(ROOT / 'not-approved/Cargo.toml')
        with self.assertRaises(AssertionError):
            reviewed_local_source(ROOT, package, reviewed)

    def fixture(self, root):
        (root / 'config').mkdir()
        shutil.copyfile(ROOT / 'config/offline-runtime-source-patches.json', root / 'config/offline-runtime-source-patches.json')
        shutil.copytree(ROOT / 'vendor/offline', root / 'vendor/offline')

    def test_operation_source_change_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            path = root / 'vendor/offline/gemm/src/lib.rs'
            path.write_bytes(path.read_bytes() + b'\n// changed operation source\n')
            with self.assertRaises(AssertionError):
                verify_offline_vendor(root)

    def test_modified_lock_or_extra_source_cannot_be_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            extra = root / 'vendor/offline/gemm/src/unreviewed.rs'
            extra.write_text('// mechanism fixture\n', encoding='utf-8')
            with self.assertRaises(AssertionError):
                verify_offline_vendor(root)
            extra.unlink()
            lock = root / 'config/offline-runtime-source-patches.json'
            lock.write_bytes(lock.read_bytes() + b'\n')
            with self.assertRaises(AssertionError):
                verify_offline_vendor(root)


if __name__ == '__main__':
    unittest.main()
