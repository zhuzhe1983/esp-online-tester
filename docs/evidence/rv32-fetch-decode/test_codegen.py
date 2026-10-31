"""Regression tests for the disassembly normalizer. Run with python3 test_codegen.py."""
import contextlib
import io
import runpy
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

HERE = Path(__file__).resolve().parent

class NormalizeTest(unittest.TestCase):
    def test_zero_page_offset_matches_explicit_offset(self):
        class Result:
            def __init__(self, stdout): self.stdout = stdout
        def load(offset):
            text = f'1000: adrp x8, 0x2000\n1004: ldr q0, [x8{offset}]\n'
            with patch('subprocess.run', return_value=Result(text)):
                return self.module['dis'](Path('unused'), 'symbol')
        self.assertEqual(load(''), load(', #0x10'))

    @classmethod
    def setUpClass(cls):
        # Empty binary directories exercise no external commands at module load.
        import tempfile
        cls.tmp = tempfile.TemporaryDirectory()
        root = Path(cls.tmp.name)
        (root / 'base').mkdir(); (root / 'cand').mkdir()
        with patch.object(sys, 'argv', ['codegen.py', str(root / 'base'), str(root / 'cand'), str(root / 'out.json')]), contextlib.redirect_stdout(io.StringIO()):
            cls.module = runpy.run_path(str(HERE / 'codegen.py'))

    @classmethod
    def tearDownClass(cls): cls.tmp.cleanup()

if __name__ == '__main__': unittest.main()
