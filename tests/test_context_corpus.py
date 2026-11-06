import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/prepare_context_corpus.py"
SPEC = importlib.util.spec_from_file_location("context_corpus", SCRIPT)
CORPUS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CORPUS)


class ContextCorpusTests(unittest.TestCase):
    def test_han_fragments_keep_original_scalars_and_cut_nonhan_boundaries(self):
        self.assertEqual(CORPUS.fragments("甲乙，丙𠮷！繁體ABC12简体"), ["甲乙", "丙𠮷", "繁體", "简体"])
        self.assertFalse(CORPUS.is_han("😀"))
        self.assertFalse(CORPUS.is_han("a"))

    def test_leakage_normalization_does_not_rewrite_training_language(self):
        self.assertEqual(CORPUS.canonical("ＡＢＣ：甲　乙！"), "abc甲乙")
        self.assertNotEqual(CORPUS.canonical("繁體"), CORPUS.canonical("繁体"))

    def test_changed_frozen_source_is_rejected_before_output_creation(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            (source / "cmn_sentences_detailed.tsv.bz2").write_bytes(b"changed-original")
            output = source / "output"
            result = subprocess.run([sys.executable, str(SCRIPT), "--source-dir", str(source), "--output-dir", str(output)], capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"source identity mismatch", result.stderr)
            self.assertFalse(output.exists())
            self.assertEqual((source / "cmn_sentences_detailed.tsv.bz2").read_bytes(), b"changed-original")

    def test_source_and_destination_are_explicit_cli_inputs(self):
        result = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"--source-dir", result.stderr)
        self.assertIn(b"--output-dir", result.stderr)
