#!/usr/bin/env python3
"""AWK Builder suites against the two One True AWK binaries.

The suites are private and are fetched by scripts/fetch_awk_posix_challenge.sh
and scripts/fetch_awk_volume.sh. They are not vendored, and cases/ is not
modified. Results are written under tests/external/awk-results/ (gitignored).

1. Independent POSIX AWK Challenge, annotated tag v0.6. The pin is the
   commit that tag points at (HEAD after checkout), not the tag object.
   Portable, observational, and nonportable are reported separately. The
   runner's default gate is the portable class. One True AWK is known to
   fail portable cases AWK-391, AWK-392, and AWK-399, so that gate may
   exit nonzero. This harness still compares the two JSON files. A
   compiler bug is a case whose status (or failure output) differs
   between the two binaries.

2. Shakespeare volume test. One portable program, two inputs. Stdout must
   match the pinned expected files. Wall time is printed.

Set AWK_EXTRA_ALLOW_MISSING=1 to exit 0 when the private clones are
absent. CI does not set that when AWK_SUITE_READ_TOKEN is present; a
missing clone is then an error. CI does not run this harness at all
when that secret is unset.

AWK_EXTRA_HOOK, if set, is a command run after the two suites with
AWK_CLANG and AWK_CCOMP in the environment. It is the extension point
for a later suite. This runner does not invent one.

Env: AWK_CLANG, AWK_CCOMP, AWK_POSIX_ROOT, AWK_VOLUME_ROOT,
AWK_EXTRA_ALLOW_MISSING, AWK_EXTRA_HOOK.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
POSIX = Path(
    os.environ.get(
        "AWK_POSIX_ROOT",
        ROOT / "tests" / "external" / "awk-posix-challenge",
    )
)
VOLUME = Path(
    os.environ.get(
        "AWK_VOLUME_ROOT",
        ROOT / "tests" / "external" / "awk-volume-tests",
    )
)
RESULTS = ROOT / "tests" / "external" / "awk-results"
# Commit of annotated tag v0.6. The tag object is 69b1f589… and is not HEAD.
PIN_POSIX = "b46ea063c6d25372b6a2010ac537ca19a8fd1b1e"
PIN_VOLUME = "46701e477fe48a7e0592b8022c77ac29c4114582"
RECORDED = "results/grok-v0.6-final-2026-11-08/onetrue.json"


def allow_missing() -> bool:
    return os.environ.get("AWK_EXTRA_ALLOW_MISSING", "") == "1"


def missing(what: str) -> int:
    if allow_missing():
        print(f"SKIP: {what} is not available.")
        return 0
    print(f"{what} is not available.")
    return 1


def head(path: Path) -> str:
    proc = subprocess.run(
        ["git", "-C", str(path), "rev-parse", "HEAD"],
        text=True,
        capture_output=True,
        check=False,
    )
    return proc.stdout.strip()


def run_posix(clang: Path, ours: Path) -> int:
    if not (POSIX / "tools" / "runner.py").is_file():
        return missing(f"awk-posix-challenge ({PIN_POSIX})")
    got = head(POSIX)
    if got != PIN_POSIX:
        print(f"awk-posix-challenge HEAD {got} is not pin {PIN_POSIX}")
        return 1
    RESULTS.mkdir(parents=True, exist_ok=True)
    clang_json = RESULTS / "clang-onetrue.json"
    our_json = RESULTS / "ccomp-onetrue.json"
    for binary, label, dest in (
        (clang, "clang-onetrue", clang_json),
        (ours, "ccomp-onetrue", our_json),
    ):
        dest.unlink(missing_ok=True)
        try:
            proc = subprocess.run(
                [
                    "python3",
                    "tools/runner.py",
                    "--cmd",
                    str(binary),
                    "--label",
                    label,
                    "--output",
                    str(dest),
                ],
                cwd=POSIX,
                stdin=subprocess.DEVNULL,
                timeout=1200,
                check=False,
            )
        except subprocess.TimeoutExpired:
            print(f"posix runner timed out for {label}")
            return 1
        if proc.returncode != 0:
            print(f"posix runner exited {proc.returncode} for {label}")
            # The portable gate is nonzero for the known onetrue FILENAME
            # cases. Keep going and compare the two JSON files below.
    summarize = subprocess.run(
        ["python3", "tools/summarize.py", "--markdown", str(clang_json), str(our_json)],
        cwd=POSIX,
        text=True,
        capture_output=True,
        stdin=subprocess.DEVNULL,
        timeout=60,
        check=False,
    )
    sys.stdout.write(summarize.stdout)
    sys.stderr.write(summarize.stderr)
    if not clang_json.is_file() or not our_json.is_file():
        print("posix runner did not write both result files")
        return 1
    return compare_posix(clang_json, our_json)


def case_key(row: dict) -> str:
    for key in ("id", "name", "case"):
        if key in row:
            return str(row[key])
    return json.dumps(row, sort_keys=True)[:80]


def rows_of(payload: object) -> list[dict]:
    if isinstance(payload, list):
        return [row for row in payload if isinstance(row, dict)]
    if isinstance(payload, dict):
        for key in ("cases", "results", "tests"):
            value = payload.get(key)
            if isinstance(value, list):
                return [row for row in value if isinstance(row, dict)]
        # A map of id -> row.
        if payload and all(isinstance(value, dict) for value in payload.values()):
            rows = []
            for key, value in payload.items():
                row = dict(value)
                row.setdefault("id", key)
                rows.append(row)
            return rows
    return []


def status_of(row: dict) -> str:
    for key in ("status", "result", "outcome"):
        if key in row:
            return str(row[key])
    return ""


def compare_posix(clang_json: Path, our_json: Path) -> int:
    clang = json.loads(clang_json.read_text(encoding="utf-8"))
    ours = json.loads(our_json.read_text(encoding="utf-8"))
    clang_rows = {case_key(row): row for row in rows_of(clang)}
    our_rows = {case_key(row): row for row in rows_of(ours)}
    if not clang_rows or not our_rows:
        print("could not read per-case rows from the runner JSON")
        return 1
    disagree = []
    for key in sorted(set(clang_rows) | set(our_rows)):
        left = clang_rows.get(key)
        right = our_rows.get(key)
        if left is None or right is None or status_of(left) != status_of(right):
            disagree.append(key)
            continue
        if status_of(left).lower() not in {"pass", "passed", "ok"}:
            if left.get("stdout") != right.get("stdout") or left.get("stderr") != right.get("stderr"):
                disagree.append(key)
    recorded = POSIX / RECORDED
    if recorded.is_file():
        print(f"recorded onetrue report is present: {RECORDED}")
    else:
        print(f"recorded onetrue report not in this checkout: {RECORDED}")
    if disagree:
        print(f"posix challenge: {len(disagree)} cases disagree between the two binaries")
        for key in disagree[:30]:
            print(f"  {key}")
        return 1
    print(f"posix challenge: {len(clang_rows)} cases agree between the two binaries")
    return 0


def run_volume(clang: Path, ours: Path) -> int:
    if not (VOLUME / "run.sh").is_file():
        return missing(f"awk-volume-tests ({PIN_VOLUME})")
    got = head(VOLUME)
    if got != PIN_VOLUME:
        print(f"awk-volume-tests HEAD {got} is not pin {PIN_VOLUME}")
        return 1
    if not (VOLUME / ".fetched").is_file():
        try:
            proc = subprocess.run(
                ["./fetch.sh"],
                cwd=VOLUME,
                stdin=subprocess.DEVNULL,
                timeout=180,
                check=False,
            )
        except subprocess.TimeoutExpired:
            print("volume fetch.sh timed out")
            return 1
        if proc.returncode != 0:
            print("volume fetch.sh failed")
            return 1
    failed = 0
    for label, binary in (("clang", clang), ("ccomp", ours)):
        print(f"volume {label}")
        try:
            proc = subprocess.run(
                ["./run.sh"],
                cwd=VOLUME,
                env={**os.environ, "AWKS": "onetrue", "ONETRUE_CMD": str(binary)},
                stdin=subprocess.DEVNULL,
                timeout=180,
                check=False,
            )
        except subprocess.TimeoutExpired:
            print(f"volume {label} timed out")
            failed = 1
            continue
        if proc.returncode != 0:
            print(f"volume {label} exited {proc.returncode}")
            failed = 1
    return failed


def main() -> int:
    clang = Path(os.environ.get("AWK_CLANG", ""))
    ours = Path(os.environ.get("AWK_CCOMP", ""))
    if not clang.is_file() or not ours.is_file():
        print("AWK_CLANG and AWK_CCOMP must be the two One True AWK binaries")
        return 1
    status = run_posix(clang, ours)
    status |= run_volume(clang, ours)
    hook = os.environ.get("AWK_EXTRA_HOOK", "").strip()
    if hook:
        # Trusted operator input. The person who starts the harness sets
        # AWK_EXTRA_HOOK; it is not read from a fetched suite.
        print(f"extra hook: {hook}")
        try:
            proc = subprocess.run(
                hook,
                shell=True,
                env={**os.environ, "AWK_CLANG": str(clang), "AWK_CCOMP": str(ours)},
                stdin=subprocess.DEVNULL,
                timeout=600,
                check=False,
            )
        except subprocess.TimeoutExpired:
            print("AWK_EXTRA_HOOK timed out")
            return 1
        if proc.returncode != 0:
            print(f"AWK_EXTRA_HOOK exited {proc.returncode}")
            status = 1
    else:
        print("AWK_EXTRA_HOOK is unset. No further suite is configured.")
    return status


if __name__ == "__main__":
    sys.exit(main())
