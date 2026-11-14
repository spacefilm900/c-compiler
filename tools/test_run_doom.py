#!/usr/bin/env python3
"""Tests for harness/run_doom.py's verdict and scorecard.

They use made-up results only: nothing is compiled or run. A demo that
stopped at the --frames cap compared a prefix of the demo, so it must not
count as a full-demo pass or be described as played to its natural end.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "harness"))

import run_doom  # noqa: E402


def demo(end: str, frames: int, match: bool = True) -> dict:
    return {
        "wad": "freedoom1.wad",
        "demo": "demo1",
        "end": end,
        "gametics": 100 if end == "demo end" else None,
        "match": match,
        "compare": {"reference_frames": frames, "frames": frames, "matching": frames if match else 0,
                    "first_mismatch": None if match else 1},
    }


def level(*demos: dict) -> dict:
    return {
        "opt": "O2",
        "files": [],
        "failed_files": [],
        "build": "full",
        "link_ok": True,
        "demos": list(demos),
        "match": all(d["match"] for d in demos),
        "compare": {"reference_frames": sum(d["compare"]["reference_frames"] for d in demos)},
        "ours_compile_seconds": 1.0,
        "ours_frontend_seconds": 1.0,
        "clang_compile_seconds": 1.0,
        "ours_run_seconds": 1.0,
        "clang_run_seconds": 1.0,
    }


class Verdict(unittest.TestCase):
    def test_natural_end_passes(self):
        ok, text = run_doom.verdict([level(demo("demo end", 50))], allow_prefix=False)
        self.assertTrue(ok)
        self.assertEqual(text, "DOOM: PASS")

    def test_frame_cap_is_not_a_pass(self):
        ok, text = run_doom.verdict([level(demo("frame cap", 1))], allow_prefix=False)
        self.assertFalse(ok)
        self.assertIn("INCOMPLETE", text)

    def test_frame_cap_allowed_is_labelled_prefix(self):
        ok, text = run_doom.verdict([level(demo("frame cap", 1))], allow_prefix=True)
        self.assertTrue(ok)
        self.assertIn("prefix only", text)

    def test_mismatch_fails(self):
        ok, text = run_doom.verdict([level(demo("demo end", 5, match=False))], allow_prefix=True)
        self.assertFalse(ok)
        self.assertEqual(text, "DOOM: FAIL")


class Scorecard(unittest.TestCase):
    def payload(self, *demos: dict) -> dict:
        return {"meta": {}, "frames": 1, "jobs": 1, "files": 0, "levels": [level(*demos)]}

    def test_capped_demo_is_not_called_complete(self):
        text = run_doom.render_scorecard(self.payload(demo("frame cap", 1)), "t")
        self.assertNotIn("to their natural end", text)
        self.assertIn("prefix only", text)
        self.assertNotIn("| Matches Clang | yes |", text)

    def test_natural_end_is_called_complete(self):
        text = run_doom.render_scorecard(self.payload(demo("demo end", 50)), "t")
        self.assertIn("to their natural end", text)
        self.assertIn("| Matches Clang | yes |", text)


if __name__ == "__main__":
    unittest.main()
