#!/usr/bin/env python3
"""Compile-time guard for CI: fail when the frontend gets dramatically slower.

For sqlite3.c and shell.c it times this compiler's frontend (C to LLVM IR)
and `clang -O0 -c` on the same file, best of `--runs`, and fails when the
frontend takes more than `--max-ratio` times as long as Clang, or more than
`--max-seconds` outright. The limits are deliberately loose so that a noisy
runner never trips them; they exist to catch the kind of quadratic slowdown
the performance phase removed (sqlite3.c's frontend was 90x Clang -O0).

Usage: tools/perf_guard.py [--runs 2] [--max-ratio 6] [--max-seconds 30]
Env: CC_UNDER_TEST (default build/compiler), CLANG (default clang).
"""

from __future__ import annotations

import argparse
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_compile as bench  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", type=int, default=2)
    ap.add_argument("--max-ratio", type=float, default=6.0)
    ap.add_argument("--max-seconds", type=float, default=30.0)
    args = ap.parse_args()
    failed = False
    checked = 0
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        for group in ("sqlite3", "shell"):
            for src, flags in bench.group_units(group):
                ours = min(
                    bench.measure([bench.COMPILER, *flags, str(src), "-o", str(work / "out.ll")])[0]
                    for _ in range(args.runs)
                )
                clang = min(
                    bench.measure([bench.CLANG, "-O0", "-w", *flags, "-c", str(src), "-o", str(work / "out.o")])[0]
                    for _ in range(args.runs)
                )
                ratio = ours / clang
                bad = ratio > args.max_ratio or ours > args.max_seconds
                failed |= bad
                checked += 1
                print(
                    f"{src.name}: frontend {ours:.2f} s, clang -O0 {clang:.2f} s, "
                    f"ratio {ratio:.2f}x (limit {args.max_ratio}x, {args.max_seconds} s)"
                    + (" FAIL" if bad else " ok")
                )
    if checked == 0:
        print("no SQLite sources; run scripts/fetch_sqlite.sh", file=sys.stderr)
        return 1
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
