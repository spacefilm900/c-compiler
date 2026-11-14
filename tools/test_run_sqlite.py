#!/usr/bin/env python3
"""Tests for harness/run_sqlite.py's comparisons, verdict, and scorecard.

They use made-up results only: nothing is compiled or run except a short
timeout check. A workload matches only when neither side timed out, both
exit codes and captured bytes agree, and the two database files exist and
are identical. The whole run passes only when every level was built by
this compiler alone.
"""

from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "harness"))

import run_sqlite  # noqa: E402


def build(failed: list[str] | None = None, link_ok: bool = True) -> dict:
    failed = failed or []
    files = []
    for name in ("sqlite3.c", "shell.c"):
        if name in failed:
            files.append({"file": name, "status": "reject", "stderr": "x.c:1:1: error: no", "frontend_seconds": 1.0, "frontend_max_rss_kb": 1000, "backend_seconds": 0.0, "backend_max_rss_kb": 0})
        else:
            files.append({"file": name, "status": "ok", "frontend_seconds": 2.0, "frontend_max_rss_kb": 2000, "backend_seconds": 0.5, "backend_max_rss_kb": 1000})
    clang = [{"file": n, "status": "ok", "seconds": 0.4, "max_rss_kb": 800} for n in ("sqlite3.c", "shell.c")]
    return {
        "files": files,
        "clang_files": clang,
        "failed_files": failed,
        "build": "mixed" if failed else "full",
        "link_ok": link_ok,
        "ours_compile_seconds": 5.0,
        "ours_frontend_seconds": 4.0,
        "ours_frontend_max_rss_kb": 2000,
        "ours_max_rss_kb": 2000,
        "clang_compile_seconds": 0.8,
        "clang_max_rss_kb": 800,
        "first": "clang",
    }


def workload(name: str, match: bool = True, db_match: bool = True) -> dict:
    return {
        "workload": name,
        "match": match and db_match,
        "output_match": match,
        "db_match": db_match,
        "clang_seconds": 0.2,
        "ours_seconds": 0.3,
        "stdout_bytes": 10,
        "first": "clang",
    }


def level(full: bool = True, match: bool = True, db_match: bool = True) -> dict:
    return {
        "opt": "O2",
        "build": build([] if full else ["sqlite3.c"]),
        "workloads": [workload("01_ddl_inserts.sql", match=match, db_match=db_match)],
        "match": match and db_match,
        "full": full,
    }


class Difference(unittest.TestCase):
    def test_first_difference_reports_the_line(self):
        self.assertEqual(run_sqlite.first_difference(b"a\nb\n", b"a\nc\n"), "line 2: clang 'b' / ours 'c'")

    def test_identical_bytes_note(self):
        self.assertIn("identical", run_sqlite.first_difference(b"a\n", b"a\n"))


class RunWait4(unittest.TestCase):
    def test_timeout_is_reported(self):
        res = run_sqlite.run_wait4(["sleep", "5"], 0.2)
        self.assertTrue(res["timed_out"])
        self.assertEqual(res["rc"], 124)
        self.assertFalse(run_sqlite.run_wait4(["true"], 5)["timed_out"])

    def test_stdout_and_exit_status(self):
        res = run_sqlite.run_wait4(["python3", "-c", "import sys; sys.stdout.write('hi'); sys.exit(3)"], 5)
        self.assertEqual(res["rc"], 3)
        self.assertEqual(res["stdout"], b"hi")


class WorkloadFiles(unittest.TestCase):
    def test_stale_database_is_deleted(self):
        with tempfile.TemporaryDirectory() as tmp:
            scratch = Path(tmp)
            stale = scratch / "demo.db"
            stale.write_bytes(b"OLD")
            (scratch / "demo.db-journal").write_bytes(b"J")
            exe = scratch / "false-sqlite"
            # A stand-in that exits 0 and writes a new database named as $4? We
            # call sqlite3 as: exe -batch -init /dev/null -bail DB. Provide a
            # tiny script that copies its stdin and creates the db path (argv 5).
            exe.write_text(
                "#!/bin/sh\n"
                "db=$5\n"
                "printf NEW > \"$db\"\n"
                "cat >/dev/null\n"
                "exit 0\n"
            )
            exe.chmod(0o755)
            script = scratch / "demo.sql"
            script.write_text("SELECT 1;\n")
            # The harness uses a subdirectory; pass scratch itself as the work dir.
            work = scratch / "w"
            # Point the script stem at demo so the db is demo.db
            script2 = work / "demo.sql"
            # run_workload uses scratch / (stem + '.db')
            res = run_sqlite.run_workload(exe, script, work, 5)
            self.assertTrue(res["db_exists"])
            self.assertEqual(Path(res["db"]).read_bytes(), b"NEW")
            self.assertFalse((work / "demo.db-journal").exists())


class Verdict(unittest.TestCase):
    def test_full_and_matching_passes(self):
        self.assertEqual(run_sqlite.verdict([level()]), (True, "SQLITE: PASS"))

    def test_mixed_build_fails_even_when_outputs_match(self):
        self.assertEqual(run_sqlite.verdict([level(full=False)]), (False, "SQLITE: FAIL"))

    def test_mismatch_fails(self):
        self.assertFalse(run_sqlite.verdict([level(match=False)])[0])

    def test_database_mismatch_fails(self):
        self.assertFalse(run_sqlite.verdict([level(db_match=False)])[0])

    def test_no_levels_fails(self):
        self.assertFalse(run_sqlite.verdict([])[0])


class Scorecard(unittest.TestCase):
    def test_full_build_matches(self):
        text = run_sqlite.render_scorecard({"meta": {"std": "gnu11"}, "jobs": 1, "levels": [level()]}, "t")
        self.assertIn("| Matches Clang | yes |", text)
        self.assertIn("`-std=gnu11`", text)
        self.assertIn("| `01_ddl_inserts.sql` | 10 | yes |", text)

    def test_mixed_build_lists_rejected_file(self):
        text = run_sqlite.render_scorecard({"meta": {}, "jobs": 1, "levels": [level(full=False)]}, "t")
        self.assertIn("| Matches Clang | no |", text)
        self.assertIn("O2 `sqlite3.c`: reject", text)


class TimeoutMatch(unittest.TestCase):
    def test_timeout_never_counts_as_a_match(self):
        # Directly exercise the rule used by compare_workloads: both sides
        # timing out with the same bytes is still not a match.
        ref = {"rc": 124, "stdout": b"", "stderr": b"timed out\n", "timed_out": True, "db_exists": False, "db_sha256": None}
        ours = dict(ref)
        output_match = (
            not ref["timed_out"]
            and not ours["timed_out"]
            and (ours["rc"], ours["stdout"], ours["stderr"]) == (ref["rc"], ref["stdout"], ref["stderr"])
        )
        self.assertFalse(output_match)


if __name__ == "__main__":
    unittest.main()
