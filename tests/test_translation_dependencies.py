"""直接检查新许可门禁的损坏与遗漏，不复制审计业务。"""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
SPEC=importlib.util.spec_from_file_location('translation_dependency_audit',ROOT/'scripts/audit_translation_dependencies.py')
MODULE=importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

class TranslationLicenseTests(unittest.TestCase):
    def fixture(self,root):
        source=root/'cargo-fixture'
        source.mkdir()
        (source/'LICENSE').write_text('MIT mechanism fixture\n',encoding='utf-8')
        folder=root/'licenses/translation'
        folder.mkdir(parents=True)
        copied=folder/'fixture-1.0.0-LICENSE'
        copied.write_bytes((source/'LICENSE').read_bytes())
        rows=[{'package':'fixture','version':'1.0.0','license':'MIT','file':copied.relative_to(root).as_posix(),
               'bytes':copied.stat().st_size,'sha256':MODULE.sha(copied),'origin':'crate','source_name':'LICENSE'}]
        manifest=folder/'manifest.json'
        manifest.write_text(json.dumps(rows),encoding='utf-8')
        metadata={'workspace_members':[], 'packages':[{'id':'fixture','name':'fixture','version':'1.0.0','license':'MIT',
            'repository':'https://example.invalid/not-a-real-dependency','rust_version':None,'manifest_path':str(source/'Cargo.toml')} ]}
        return metadata,manifest,copied

    def test_actual_verifier_accepts_matching_fixture_and_rejects_modified_license(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            meta,_,copied=self.fixture(root)
            self.assertEqual(len(MODULE.verify_licenses(root,meta)),1)
            copied.write_text('changed\n',encoding='utf-8')
            with self.assertRaises(AssertionError):
                MODULE.verify_licenses(root,meta)

    def test_actual_verifier_rejects_missing_package_and_missing_original(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            meta,manifest,_=self.fixture(root)
            original_manifest=manifest.read_bytes()
            manifest.write_text('[]',encoding='utf-8')
            with self.assertRaises(AssertionError):
                MODULE.verify_licenses(root,meta)
            manifest.write_bytes(original_manifest)
            (root/'cargo-fixture/LICENSE').unlink()
            with self.assertRaises(AssertionError):
                MODULE.verify_licenses(root,meta)

    def test_input_core_production_dependency_closure_stays_local(self):
        import tomllib
        for path in (ROOT/'crates').glob('cloudleaf-*/Cargo.toml'):
            data=tomllib.loads(path.read_text(encoding='utf-8'))
            if data['package']['name']=='cloudleaf-translation':
                continue
            for name,dependency in data.get('dependencies',{}).items():
                self.assertTrue(name.startswith('cloudleaf-'),path)
                self.assertNotEqual(name,'cloudleaf-translation',path)
                self.assertTrue(dependency.get('workspace'),path)
