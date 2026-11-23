"""Unit tests for the hardening generators (no compiler or Clang needed)."""

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import constfold_fuzz  # noqa: E402
import layout_fuzz  # noqa: E402
import yarpgen_campaign  # noqa: E402


class TypeOfLiteral(unittest.TestCase):
    """C11 6.4.4.1p5 on LP64: the first type in the list that holds the value."""

    def test_decimal(self):
        self.assertEqual(constfold_fuzz.type_of("7"), "int")
        self.assertEqual(constfold_fuzz.type_of("2147483648"), "long")
        self.assertEqual(constfold_fuzz.type_of("-2147483647"), "int")

    def test_hex_may_be_unsigned(self):
        self.assertEqual(constfold_fuzz.type_of("0x7fffffff"), "int")
        self.assertEqual(constfold_fuzz.type_of("0x80000000"), "unsigned")
        self.assertEqual(constfold_fuzz.type_of("0x100000000"), "long")
        self.assertEqual(constfold_fuzz.type_of("0x8000000000000000"), "unsigned long")

    def test_suffixes(self):
        self.assertEqual(constfold_fuzz.type_of("1u"), "unsigned")
        self.assertEqual(constfold_fuzz.type_of("1ul"), "unsigned long")
        self.assertEqual(constfold_fuzz.type_of("1ll"), "long long")
        self.assertEqual(constfold_fuzz.type_of("-1ll"), "long long")

    def test_other(self):
        self.assertEqual(constfold_fuzz.type_of("'a'"), "int")
        self.assertEqual(constfold_fuzz.type_of("1.5"), "double")
        self.assertEqual(constfold_fuzz.type_of("3.0f"), "float")
        self.assertEqual(constfold_fuzz.type_of("1e10"), "double")


class Programs(unittest.TestCase):
    def test_constfold_deterministic(self):
        self.assertEqual(constfold_fuzz.program(42), constfold_fuzz.program(42))
        self.assertNotEqual(constfold_fuzz.program(42), constfold_fuzz.program(43))
        text = constfold_fuzz.program(42)
        self.assertIn("static ", text)
        self.assertIn("volatile", text)

    def test_layout_deterministic(self):
        for gnu in (False, True):
            self.assertEqual(layout_fuzz.program(7, gnu), layout_fuzz.program(7, gnu))

    def test_layout_never_reads_plain_bool(self):
        # A `_Bool` holding a byte other than 0 or 1 has no defined value,
        # so a plain `_Bool` member is never read back from the pattern.
        import random

        checked = 0
        for seed in range(300):
            text = layout_fuzz.program(seed, False)
            rng = random.Random(seed)
            gen = layout_fuzz.Gen(rng, False)
            for _ in range(rng.randint(1, 3)):
                gen.record(2)
            for idx, (_, _, members) in enumerate(gen.decls):
                for name, kind, ty, _ in members:
                    if kind == "scalar" and ty == "_Bool":
                        checked += 1
                        read = f'" {name}=%llx\\n", (unsigned long long)(x{idx}.{name}));'
                        self.assertNotIn(read, text)
        self.assertGreater(checked, 0)

    def test_layout_no_packed_without_gnu(self):
        for seed in range(200):
            self.assertNotIn("__attribute__", layout_fuzz.program(seed, False))


class YarpgenResume(unittest.TestCase):
    def test_existing_results_are_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            results = Path(tmp) / "results.jsonl"
            results.write_text(json.dumps({"seed": 1, "status": "agreement"}) + "\n")
            sys.argv = ["yarpgen_campaign.py", "--yarpgen", "/nonexistent",
                        "--seed-start", "1", "--count", "1", "--out", tmp]
            self.assertEqual(yarpgen_campaign.main(), 0)
            self.assertEqual(len(results.read_text().splitlines()), 1)


if __name__ == "__main__":
    unittest.main()
