"""验证基准不会误测默认目录旧产物或另一个可执行文件。"""

import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("cloudleaf_benchmark", Path(__file__).resolve().parents[1] / "scripts/benchmark.py")
BENCHMARK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BENCHMARK)


class ArtifactTests(unittest.TestCase):
    def test_adapter_changes_are_part_of_source_provenance(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]:
                (root / name).write_text("fixture\n", encoding="utf-8")
            adapter = root / "adapters/windows/session.rs"
            adapter.parent.mkdir(parents=True)
            adapter.write_text("// 旧事务\n", encoding="utf-8")
            with patch.object(BENCHMARK, "ROOT", root):
                before = BENCHMARK.source_sha256()
                adapter.write_text("// 新事务\n", encoding="utf-8")
                self.assertNotEqual(before, BENCHMARK.source_sha256())

    def test_uses_only_actual_host_executable_in_custom_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            host = Path(temporary) / "custom-target" / "host.exe"
            host.parent.mkdir()
            host.write_bytes(b"test fixture")
            artifacts = [
                {"reason": "compiler-artifact", "target": {"name": "cloudleaf-evaluate", "kind": ["bin"]}, "executable": "stale-evaluator.exe"},
                {"reason": "compiler-artifact", "target": {"name": "cloudleaf-cli", "kind": ["bin"]}, "executable": str(host)},
                {"reason": "build-finished", "success": True},
            ]
            result = subprocess.CompletedProcess([], 0, "\n".join(json.dumps(row) for row in artifacts), "")
            with patch.object(BENCHMARK.subprocess, "run", return_value=result):
                self.assertEqual(BENCHMARK.build_host(), host)

    def test_missing_artifact_fails_instead_of_falling_back_to_stale_file(self):
        result = subprocess.CompletedProcess([], 0, '{"reason":"build-finished","success":true}', "")
        with patch.object(BENCHMARK.subprocess, "run", return_value=result):
            with self.assertRaises(RuntimeError):
                BENCHMARK.build_host()


if __name__ == "__main__":
    unittest.main()
