#!/usr/bin/env python3
"""Tests for harness/run_quickjs.py's comparisons, verdict and summary.

Nothing is compiled: the qjsc comparison runs two small shell scripts that
stand in for the two qjsc builds. A run passes only when every level was
built by this compiler alone and every comparison matched; an input both
builds reject counts as matching only when they reject it the same way.
"""

from __future__ import annotations

import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "harness"))

import run_quickjs  # noqa: E402


def fake_qjsc(path: Path, body: str) -> Path:
    path.write_text("#!/bin/sh\n" + body)
    path.chmod(path.stat().st_mode | stat.S_IXUSR)
    return path


class QjscComparison(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        src = self.dir / "src"
        (src / "tests").mkdir(parents=True)
        (src / "repl.js").write_text("1")
        (src / "tests" / "a.js").write_text("2")
        (src / "tests" / "bad.js").write_text("3")
        self.saved = run_quickjs.SRC
        run_quickjs.SRC = src

    def tearDown(self) -> None:
        run_quickjs.SRC = self.saved
        self.tmp.cleanup()

    # $3 is the output file; the last argument is the input.
    WRITE = 'for last; do :; done\ncase "$last" in bad.js) echo "SyntaxError" >&2; exit 1;; esac\n'

    def test_identical_outputs_and_identical_rejections_match(self) -> None:
        ref = fake_qjsc(self.dir / "ref", self.WRITE + 'echo "code $last" > "$3"\n')
        ours = fake_qjsc(self.dir / "ours", self.WRITE + 'echo "code $last" > "$3"\n')
        res = run_quickjs.compare_qjsc(ref, ours, self.dir / "out")
        self.assertEqual(res["files"], 3)
        self.assertEqual(res["matching"], 3)
        self.assertEqual(res["rejected_by_both"], ["bad.js"])
        self.assertEqual(res["differ"], [])

    def test_different_bytes_or_messages_differ(self) -> None:
        ref = fake_qjsc(self.dir / "ref", self.WRITE + 'echo "code $last" > "$3"\n')
        ours = fake_qjsc(
            self.dir / "ours",
            'for last; do :; done\ncase "$last" in bad.js) echo "Other" >&2; exit 1;; '
            'a.js) echo changed > "$3"; exit 0;; esac\necho "code $last" > "$3"\n',
        )
        res = run_quickjs.compare_qjsc(ref, ours, self.dir / "out")
        self.assertEqual(sorted(res["differ"]), ["a.js", "bad.js"])
        self.assertEqual(res["matching"], 1)


def level(full: bool = True, match: bool = True) -> dict:
    files = [{"file": "quickjs.c", "status": "ok"}]
    if not full:
        files.append({"file": "dtoa.c", "status": "reject", "stderr": "dtoa.c:1:1: error: no"})
    return {
        "opt": "O2",
        "full": full,
        "match": match,
        "build": {
            "files": files,
            "failed_files": [] if full else ["dtoa.c"],
            "build": "full" if full else "mixed",
            "link_ok": True,
            "ours_compile_seconds": 2.0,
            "ours_frontend_seconds": 1.0,
            "clang_compile_seconds": 1.0,
        },
        "qjsc": {"files": 2, "matching": 2, "rejected_by_both": [], "differ": []},
        "tests": [{"test": "tests/test_loop.js", "match": match, "ours_seconds": 0.1, "clang_seconds": 0.1,
                   **({} if match else {"first_difference": "line 1: clang 'a' / ours 'b'"})}],
        "test262": {
            "match": match,
            "clang": {"summary": "Result: 58/83558 errors", "seconds": 10.0},
            "ours": {"summary": "Result: 58/83558 errors", "seconds": 11.0},
            "upstream_known": 58,
            "only_ours_fails": [] if match else ["test262/test/x.js:1: Error"],
            "only_clang_fails": [],
        },
    }


class VerdictAndSummary(unittest.TestCase):
    def test_pass_needs_full_builds_and_matches(self) -> None:
        self.assertEqual(run_quickjs.verdict([level()]), (True, "QUICKJS: PASS"))
        self.assertFalse(run_quickjs.verdict([level(full=False)])[0])
        self.assertFalse(run_quickjs.verdict([level(match=False)])[0])
        self.assertFalse(run_quickjs.verdict([])[0])

    def test_summary_names_failures(self) -> None:
        text = run_quickjs.summary(level(full=False, match=False))
        self.assertIn("reject: dtoa.c: dtoa.c:1:1: error: no", text)
        self.assertIn("line 1: clang 'a' / ours 'b'", text)
        self.assertIn("only_ours_fails: test262/test/x.js:1: Error", text)
        self.assertIn("upstream known failures 58", text)

    def test_first_difference(self) -> None:
        self.assertEqual(run_quickjs.first_difference(b"a\nb\n", b"a\nc\n"), "line 2: clang 'b' / ours 'c'")
        self.assertEqual(run_quickjs.first_difference(b"a", b"a\n"), "same lines, different bytes")
        self.assertIn("<end>", run_quickjs.first_difference(b"a\nb", b"a"))

    def test_native_modules_are_built_as_shared_libraries(self) -> None:
        self.assertIn("-DJS_SHARED_LIBRARY", run_quickjs.unit_flags(True))
        self.assertNotIn("-DJS_SHARED_LIBRARY", run_quickjs.unit_flags(False))
        self.assertIn('-DCONFIG_VERSION="2026-06-04"', run_quickjs.unit_flags(False))


if __name__ == "__main__":
    os.environ.setdefault("PYTHONDONTWRITEBYTECODE", "1")
    unittest.main()
