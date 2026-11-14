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


    def test_timeout_never_passes(self):
        res = run_lua.suite_outcome(
            {"rc": 0, "stdout": b"final OK !!!\n", "stderr": b"", "seconds": 1.0, "timed_out": True}
        )
        self.assertFalse(res["final_ok"])


class RunBytes(unittest.TestCase):
    def test_timeout_is_reported(self):
        res = run_lua.run_bytes(["sleep", "5"], 0.2)
        self.assertTrue(res["timed_out"])
        self.assertFalse(run_lua.run_bytes(["true"], 5)["timed_out"])


def fake_luac(directory: Path, name: str, body: str) -> Path:
    """A stand-in for luac: `luac -s -o DEST SRC` runs `body` with $3 = DEST."""
    exe = directory / name
    exe.write_text("#!/bin/sh\n" + body + "\n")
    exe.chmod(0o755)
    return exe


class Luac(unittest.TestCase):
    def compare(self, ref_body: str, ours_body: str, stale: bool = False) -> dict:
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            src = tmp_path / "a.lua"
            src.write_text("return 1\n")
            ref = fake_luac(tmp_path, "ref", ref_body)
            ours = fake_luac(tmp_path, "ours", ours_body)
            scratch = tmp_path / "scratch"
            if stale:
                scratch.mkdir()
                (scratch / "0-ref.luac").write_bytes(b"old")
                (scratch / "0-ours.luac").write_bytes(b"old")
            old = run_lua.luac_files
            run_lua.luac_files = lambda: [src]
            try:
                return run_lua.compare_luac(ref, ours, scratch)
            finally:
                run_lua.luac_files = old

    def test_identical_output_matches(self):
        res = self.compare('printf x > "$3"', 'printf x > "$3"')
        self.assertEqual((res["matching"], res["differ"]), (1, []))

    def test_both_failing_is_a_difference(self):
        res = self.compare("exit 1", "exit 1")
        self.assertEqual((res["matching"], res["differ"]), (0, ["a.lua"]))

    def test_one_failing_is_a_difference(self):
        res = self.compare('printf x > "$3"', 'printf x > "$3"; exit 2')
        self.assertEqual(res["differ"], ["a.lua"])

    def test_stale_outputs_are_not_reused(self):
        res = self.compare("exit 0", "exit 0", stale=True)
        self.assertEqual(res["differ"], ["a.lua"])


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
