#!/usr/bin/env python3
"""Tests for harness/run_zlib.py's corpus, verdict and summary.

Nothing is compiled or run. The corpus must be the same bytes on every run
(the comparison is meaningless otherwise), and a run passes only when every
level was built by this compiler alone and every comparison matched.
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "harness"))

import run_zlib  # noqa: E402


def level(full: bool = True, match: bool = True) -> dict:
    files = [{"file": "deflate.c", "status": "ok"}]
    if not full:
        files.append({"file": "trees.c", "status": "ice", "stderr": "thread 'main' panicked"})
    return {
        "opt": "O2", "full": full, "match": match,
        "build": {"files": files, "failed_files": [] if full else ["trees.c"], "build": "full" if full else "mixed",
                  "link_ok": True, "ours_compile_seconds": 1.0, "clang_compile_seconds": 1.0},
        "tests": [{"test": "example", "match": match}],
        "round_trip": {"cases": 2, "matching": 2 if match else 1, "differ": [] if match else ["runs -9 -f"]},
    }


class Corpus(unittest.TestCase):
    def setUp(self) -> None:
        self.saved = run_zlib.SRC
        # zlib.h is the one corpus file read from the release; stand in for it.
        self.tmp = tempfile.TemporaryDirectory()
        run_zlib.SRC = Path(self.tmp.name)
        (run_zlib.SRC / "zlib.h").write_bytes(b"/* stand-in */\n")

    def tearDown(self) -> None:
        run_zlib.SRC = self.saved
        self.tmp.cleanup()

    def test_corpus_is_deterministic_and_varied(self) -> None:
        first, second = run_zlib.corpus(), run_zlib.corpus()
        self.assertEqual(first, second)
        names = [name for name, _ in first]
        self.assertEqual(len(names), len(set(names)))
        sizes = {name: len(data) for name, data in first}
        self.assertEqual(sizes["empty"], 0)
        self.assertEqual(sizes["one-byte"], 1)
        self.assertGreater(sizes["zeros-1M"], 1_000_000 - 1)
        self.assertGreater(sizes["random-300k"], 0)


class VerdictAndSummary(unittest.TestCase):
    def test_pass_needs_full_builds_and_matches(self) -> None:
        self.assertEqual(run_zlib.verdict([level()]), (True, "ZLIB: PASS"))
        self.assertFalse(run_zlib.verdict([level(full=False)])[0])
        self.assertFalse(run_zlib.verdict([level(match=False)])[0])
        self.assertFalse(run_zlib.verdict([])[0])

    def test_summary_names_failures(self) -> None:
        text = run_zlib.summary(level(full=False, match=False))
        self.assertIn("ice: trees.c: thread 'main' panicked", text)
        self.assertIn("round trip: 1/2", text)
        self.assertIn("runs -9 -f", text)
        self.assertIn("test example: match=no", text)

    def test_modes_cover_levels_and_strategies(self) -> None:
        flat = [" ".join(m) for m in run_zlib.MODES]
        for mode in ("-1", "-9", "-9 -f", "-1 -h", "-r"):
            self.assertIn(mode, flat)


if __name__ == "__main__":
    unittest.main()
