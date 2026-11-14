"""Unit tests for tools/bench_compile.py and tools/perf_scorecard.py."""

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


if __name__ == "__main__":
    unittest.main()
