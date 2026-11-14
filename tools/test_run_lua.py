#!/usr/bin/env python3
"""Tests for harness/run_lua.py's outcome parsing, verdict, and scorecard.

They use made-up results and a made-up Makefile only: nothing is compiled
or run. A test-suite run passes only when it exits 0 and prints the
suite's "final OK !!!" line, and the whole run passes only when every
level was built by this compiler alone.
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "harness"))

import run_lua  # noqa: E402


def outcome(rc: int, text: str) -> dict:
    return run_lua.suite_outcome({"rc": rc, "stdout": text.encode(), "stderr": b"", "seconds": 1.0})


def build(config: str, failed: list[str] | None = None) -> dict:
    failed = failed or []
    files = [{"file": "a.c", "status": "ok"}] + [{"file": f, "status": "reject", "stderr": "x.c:1:1: error: no"} for f in failed]
    return {
        "config": config,
        "files": files,
        "failed_files": failed,
        "build": "mixed" if failed else "full",
        "link_ok": True,
        "ours_compile_seconds": 2.0,
        "ours_frontend_seconds": 1.0,
        "clang_compile_seconds": 1.0,
    }


def level(full: bool = True, match: bool = True) -> dict:
    ok = outcome(0, "***** FILE 'gc.lua'*****\nfinal OK !!!\n")
    return {
        "opt": "O2",
        "builds": {"release": build("release", [] if full else ["lvm.c"]), "ltests": build("ltests")},
        "suites": [{"suite": "user", "config": "release", "setting": "_U=true", "ref": ok, "ours": ok, "match": match}],
        "workloads": [{"workload": "w.lua", "match": match, "clang_seconds": 1.0, "ours_seconds": 1.0, "stdout_bytes": 3}],
        "luac": {"files": 1, "matching": 1, "differ": []},
        "match": match,
        "full": full,
    }


class Outcome(unittest.TestCase):
    def test_final_ok_and_files(self):
        res = outcome(0, "***** FILE 'main.lua'*****\n***** FILE 'gc.lua'*****\nfinal OK !!!\n")
        self.assertTrue(res["final_ok"])
        self.assertEqual(res["files"], ["main.lua", "gc.lua"])
        self.assertEqual(res["tail"], "")

    def test_exit_status_matters(self):
        self.assertFalse(outcome(1, "final OK !!!\n")["final_ok"])

    def test_missing_final_line_fails(self):
        res = outcome(0, "***** FILE 'gc.lua'*****\nassertion failed!\n")
        self.assertFalse(res["final_ok"])
        self.assertIn("assertion failed", res["tail"])


class Verdict(unittest.TestCase):
    def test_full_and_matching_passes(self):
        self.assertEqual(run_lua.verdict([level()]), (True, "LUA: PASS"))

    def test_mixed_build_fails_even_when_outputs_match(self):
        self.assertEqual(run_lua.verdict([level(full=False)]), (False, "LUA: FAIL"))

    def test_mismatch_fails(self):
        self.assertFalse(run_lua.verdict([level(match=False)])[0])

    def test_no_levels_fails(self):
        self.assertFalse(run_lua.verdict([])[0])


class Scorecard(unittest.TestCase):
    def test_full_build_matches(self):
        text = run_lua.render_scorecard({"meta": {"std": "gnu11"}, "jobs": 1, "levels": [level()]}, "t")
        self.assertIn("| Matches Clang | yes |", text)
        self.assertIn("`-std=gnu11`", text)
        self.assertIn("| `w.lua` | 3 | yes |", text)

    def test_mixed_build_lists_rejected_file(self):
        text = run_lua.render_scorecard({"meta": {}, "jobs": 1, "levels": [level(full=False)]}, "t")
        self.assertIn("| Matches Clang | no |", text)
        self.assertIn("O2 release `lvm.c`: reject", text)


class Sources(unittest.TestCase):
    def test_configurations_follow_the_makefile(self):
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp)
            (src / "Makefile").write_text(
                "CORE_O=\tlapi.o lvm.o\nLIB_O=\tlbaselib.o\nLUA_O=\tlua.o\nLUAC_O=\tluac.o\n"
            )
            old = run_lua.SRC
            run_lua.SRC = src
            try:
                release = [f.name for f in run_lua.config_files("release")]
                ltests = [f.name for f in run_lua.config_files("ltests")]
                progs = run_lua.programs("release")
            finally:
                run_lua.SRC = old
        self.assertEqual(release, ["lapi.c", "lvm.c", "lbaselib.c", "lua.c", "luac.c"])
        self.assertEqual(ltests, ["lapi.c", "lvm.c", "lbaselib.c", "lua.c", "ltests.c"])
        self.assertEqual(progs["lua"], ["lapi", "lvm", "lbaselib", "lua"])
        self.assertEqual(progs["luac"], ["lapi", "lvm", "lbaselib", "luac"])

    def test_first_difference(self):
        self.assertEqual(run_lua.first_difference(b"a\nb\n", b"a\nc\n"), "line 2: clang 'b' / ours 'c'")


if __name__ == "__main__":
    unittest.main()
