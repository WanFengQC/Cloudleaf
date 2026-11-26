"""自有临时子进程发布中断，不是物理断电、真实审批或系统输入法。"""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from tools.data_pipeline import pipeline as p
from tools.data_pipeline import semantic_bundle as s
from test_data_recovery import child_command
from test_semantic_bundle import fixture, trust

CHILD = """
import json,os,sys
from pathlib import Path
from tools.data_pipeline import pipeline as p
from tools.data_pipeline import semantic_bundle as s
release,store,policy,phase=Path(sys.argv[1]),Path(sys.argv[2]),json.loads(sys.argv[3]),sys.argv[4]
if phase=='lock':
    with p.publish_lock(store):
        os._exit(73)
if phase=='copy-mid':
    def copy(source,destination):
        destination.mkdir()
        (destination/'content.json').write_bytes(b'partial-copy')
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
    s.rollback(store,trust_policy=policy)
else:
    s.activate(release,store,trust_policy=policy)
raise AssertionError('fault injection not reached')
"""


class SemanticRecoveryTests(unittest.TestCase):
    def test_initial_lock_crash_requires_external_policy_before_recovery_mutation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = fixture(root)
            bundle, store = root / 'bundle', root / 'store'
            s.build_bundle(source, bundle)
            policy = trust(root, [bundle])
            child = subprocess.Popen(child_command(CHILD, bundle, store, json.dumps(policy), 'lock'),
                                     cwd=Path(__file__).resolve().parents[1], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            stdout, stderr = child.communicate(timeout=30)
            self.assertEqual(child.returncode, 73, (stdout, stderr))
            self.assertFalse((store / 'current.json').exists())
            before = {path.name: path.read_bytes() for path in store.iterdir() if path.is_file()}
            with self.assertRaisesRegex(ValueError, '策略'):
                s.recover(store, child.pid, trust_policy=None)
            self.assertEqual({path.name: path.read_bytes() for path in store.iterdir() if path.is_file()}, before)
            self.assertFalse(list(store.glob('.recovered-*')))
            result = s.recover(store, child.pid, trust_policy=policy)
            self.assertTrue(result['recovered'])
            self.assertIsNone(result['current'])
            self.assertFalse((store / 'current.json').exists())

    def test_bundle_child_exit_at_existing_publication_and_rollback_boundaries(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first, second = root / 'first', root / 'second'
            s.build_bundle(fixture(root), first)
            s.build_bundle(fixture(root, 'second-input', p.digest((first / 'manifest.json').read_bytes()), True), second)
            policy = trust(root, [first, second])
            for phase in ['lock', 'copy-mid', 'cache-renamed', 'pointer-written', 'pointer-replaced', 'rollback-before', 'rollback-after']:
                with self.subTest(phase=phase):
                    store = root / phase
                    s.activate(first, store, trust_policy=policy)
                    if phase.startswith('rollback'):
                        s.activate(second, store, trust_policy=policy)
                    user = store / 'user-dictionary.json'
                    user.write_bytes(b'private fixture')
                    before = (store / 'current.json').read_bytes()
                    child = subprocess.Popen(child_command(CHILD, second, store, json.dumps(policy), phase),
                                             cwd=Path(__file__).resolve().parents[1], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                    stdout, stderr = child.communicate(timeout=30)
                    self.assertEqual(child.returncode, 73, (stdout, stderr))
                    self.assertEqual(p.read_json(store / '.writer.lock')['owner_pid'], child.pid)
                    if phase not in {'pointer-replaced', 'rollback-after'}:
                        self.assertEqual((store / 'current.json').read_bytes(), before)
                    state = p.read_json(store / 'current.json')
                    s.verify_bundle(store / 'releases' / state['current']['release'], production=True, trust_policy=policy)
                    result = s.recover(store, child.pid, trust_policy=policy)
                    self.assertTrue(result['recovered'])
                    self.assertFalse((store / '.writer.lock').exists())
                    self.assertEqual(user.read_bytes(), b'private fixture')
                    s.activate(second, store, trust_policy=policy)


if __name__ == '__main__':
    unittest.main()
