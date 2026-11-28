"""Unit tests for tools/bench_compile.py, tools/perf_guard.py and tools/perf_scorecard.py."""

from __future__ import annotations

import io
import json
import os
import shutil
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_compile  # noqa: E402
import perf_guard  # noqa: E402
import perf_scorecard  # noqa: E402

TOTAL_KEYS = ["front_wall", "front_rss", "ir_bytes"] + [
    f"{o}_{k}" for o in ("O0", "O2") for k in ("back_wall", "back_rss", "clang_wall", "clang_rss", "ours_wall", "ratio")
]


def run(commit: str, units: dict[str, list[str]], rev: str | None = None) -> dict:
    data = {"commit": commit, "clang": "clang", "machine": "m", "runs": 1, "opts": ["O0", "O2"], "groups": {}}
    if rev is not None:
        data["compiler_rev"] = rev
    for name, files in units.items():
        data["groups"][name] = {
            "units": {f: {} for f in files},
            "total": {k: 1.0 for k in TOTAL_KEYS},
        }
    return data


class CompilerRevTest(unittest.TestCase):
    def test_explicit_revision_wins(self) -> None:
        with mock.patch.dict(os.environ, {"CC_UNDER_TEST": "/frozen/compiler"}):
            self.assertEqual(bench_compile.compiler_rev("d57f313", "abc1234"), "d57f313")

    def test_external_compiler_without_revision_is_unknown(self) -> None:
        with mock.patch.dict(os.environ, {"CC_UNDER_TEST": "/frozen/compiler"}):
            self.assertEqual(bench_compile.compiler_rev(None, "abc1234"), "unknown")

    def test_tree_compiler_defaults_to_checkout(self) -> None:
        env = {k: v for k, v in os.environ.items() if k != "CC_UNDER_TEST"}
        with mock.patch.dict(os.environ, env, clear=True):
            self.assertEqual(bench_compile.compiler_rev(None, "abc1234"), "abc1234")

    def test_markdown_records_both_revisions(self) -> None:
        data = run("abc1234", {"torture": ["a.c"]}, rev="d57f313")
        data.update({"label": "x", "date": "2026-11-14"})
        text = bench_compile.markdown(data)
        self.assertIn("Measured compiler revision: `d57f313`", text)
        self.assertIn("Benchmark checkout commit: `abc1234`", text)


class ScorecardUnitsTest(unittest.TestCase):
    def write(self, before: dict, after: dict) -> tuple[int, str, Path]:
        tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, tmp)
        (tmp / "baseline.json").write_text(json.dumps(before))
        (tmp / "after.json").write_text(json.dumps(after))
        err = io.StringIO()
        with mock.patch.object(perf_scorecard, "PERF", tmp), redirect_stderr(err):
            code = perf_scorecard.main()
        return code, err.getvalue(), tmp

    def test_same_files_compare(self) -> None:
        before = run("aaa", {"lua": ["a.c", "b.c"]})
        after = run("bbb", {"lua": ["b.c", "a.c"]}, rev="ccc")
        code, _err, tmp = self.write(before, after)
        self.assertEqual(code, 0)
        card = json.loads((tmp / "scorecard.json").read_text())
        self.assertEqual((card["before_commit"], card["after_commit"]), ("aaa", "ccc"))

    def test_dropped_file_is_rejected(self) -> None:
        code, err, tmp = self.write(run("a", {"lua": ["a.c", "b.c"]}), run("b", {"lua": ["a.c"]}))
        self.assertEqual(code, 1)
        self.assertIn("only in baseline ['b.c']", err)
        self.assertFalse((tmp / "scorecard.json").exists())

    def test_missing_group_is_rejected(self) -> None:
        code, err, _tmp = self.write(run("a", {"lua": ["a.c"], "doom": ["d.c"]}), run("b", {"lua": ["a.c"]}))
        self.assertEqual(code, 1)
        self.assertIn("input doom is missing from the after run", err)


def phases(lex: float, pre: float, parse: float, sema: float, irgen: float) -> dict[str, float]:
    return {"lex": lex, "preprocess": pre, "parse": parse, "sema": sema, "irgen": irgen, "write": 0.02}


# sqlite3.c, measured on one machine (seconds), at three points of the history.
SQLITE_BEFORE_ATOMIC = phases(0.34, 0.40, 0.29, 0.15, 0.18)  # 44cc353, the performance phase
SQLITE_WITH_REGRESSION = phases(0.34, 0.38, 0.33, 0.55, 0.21)  # 44073c7
SQLITE_AFTER_FIX = phases(0.33, 0.38, 0.31, 0.28, 0.21)


class PerfGuardPhaseTest(unittest.TestCase):
    def failures(self, run: dict[str, float]) -> list[str]:
        return perf_guard.phase_failures(run, 0.45, 0.55)

    def test_the_semantic_analysis_regression_is_caught(self) -> None:
        failures = self.failures(SQLITE_WITH_REGRESSION)
        self.assertEqual(len(failures), 1)
        self.assertIn("sema", failures[0])

    def test_healthy_runs_pass(self) -> None:
        self.assertEqual(self.failures(SQLITE_BEFORE_ATOMIC), [])
        self.assertEqual(self.failures(SQLITE_AFTER_FIX), [])

    def test_a_slow_irgen_is_caught_too(self) -> None:
        failures = self.failures(phases(0.34, 0.40, 0.29, 0.15, 0.80))
        self.assertEqual(len(failures), 1)
        self.assertIn("irgen", failures[0])

    def test_speed_of_the_machine_cancels_out(self) -> None:
        slow = {name: seconds * 3 for name, seconds in SQLITE_AFTER_FIX.items()}
        self.assertEqual(self.failures(slow), [])
        slow = {name: seconds * 3 for name, seconds in SQLITE_WITH_REGRESSION.items()}
        self.assertEqual(len(self.failures(slow)), 1)

    def test_no_front_half_time_is_an_error(self) -> None:
        with self.assertRaises(ValueError):
            perf_guard.phase_shares(phases(0, 0, 0, 0.1, 0.1))

    def test_best_phases_takes_each_phase_from_its_fastest_run(self) -> None:
        noisy = phases(0.50, 0.40, 0.29, 0.15, 0.18)
        quiet = phases(0.34, 0.60, 0.29, 0.20, 0.18)
        best = perf_guard.best_phases([noisy, quiet])
        self.assertEqual((best["lex"], best["preprocess"], best["sema"]), (0.34, 0.40, 0.15))


if __name__ == "__main__":
    unittest.main()
