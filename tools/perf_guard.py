#!/usr/bin/env python3
"""Compile-time guard for CI: fail when the frontend gets noticeably slower.

For sqlite3.c and shell.c it makes two checks, each on the best of `--runs`.

1. Against Clang. It times this compiler's frontend (C to LLVM IR) and
   `clang -O0 -c` on the same file, and fails when the frontend takes more
   than `--max-ratio` times as long as Clang, or more than `--max-seconds`
   outright. At the time of writing the frontend is about 1.0 to 1.3 times
   `clang -O0 -c` on CI, so 3.0 leaves more than two times of room for a
   noisy runner and still catches anything quadratic (sqlite3.c's frontend
   was 90 times Clang -O0 before the performance phase).

2. Against itself. A slowdown of one phase of 0.1 to 0.4 s on a 2 s
   compile hides inside runner noise on a ratio to Clang (a `_Atomic` check
   that ran for every declaration took sqlite3.c's semantic analysis from
   0.15 s to 0.55 s and was not noticed for two weeks: the ratio to Clang moved
   from about 1.0 to 1.5). The phase times from `--time-phases` are
   compared with each other instead, so the speed of the machine cancels
   out: semantic analysis may take at most `--max-sema-share` of the time
   spent lexing, preprocessing and parsing (0.15 before that regression,
   0.52 with it, 0.32 after the fix), and IR emission at most
   `--max-irgen-share` of it (0.27). Each phase uses its fastest run.

Usage: tools/perf_guard.py [--runs 2] [--max-ratio 3] [--max-seconds 10]
                           [--max-sema-share 0.45] [--max-irgen-share 0.55]
Env: CC_UNDER_TEST (default build/hcc), CLANG (default clang).
"""

from __future__ import annotations

import argparse
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_compile as bench  # noqa: E402


FRONT_PHASES = ("lex", "preprocess", "parse")


def phase_shares(phases: dict[str, float]) -> dict[str, float]:
    """Sema and irgen time as a fraction of lex + preprocess + parse time."""
    front = sum(phases[name] for name in FRONT_PHASES)
    if front <= 0:
        raise ValueError("no front-half time recorded")
    return {name: phases[name] / front for name in ("sema", "irgen")}


def phase_failures(
    phases: dict[str, float], max_sema: float, max_irgen: float
) -> list[str]:
    """Which phase-share limits `phases` breaks, as readable messages."""
    shares = phase_shares(phases)
    out = []
    if shares["sema"] > max_sema:
        out.append(f"sema is {shares['sema']:.2f} of lex+preprocess+parse (limit {max_sema})")
    if shares["irgen"] > max_irgen:
        out.append(f"irgen is {shares['irgen']:.2f} of lex+preprocess+parse (limit {max_irgen})")
    return out


def best_phases(runs: list[dict[str, float]]) -> dict[str, float]:
    """The fastest value of each phase over several runs."""
    return {name: min(run[name] for run in runs) for name in runs[0]}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", type=int, default=2)
    ap.add_argument("--max-ratio", type=float, default=3.0)
    ap.add_argument("--max-seconds", type=float, default=10.0)
    ap.add_argument("--max-sema-share", type=float, default=0.45)
    ap.add_argument("--max-irgen-share", type=float, default=0.55)
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
                phases = best_phases(
                    [
                        bench.phases_from(
                            bench.measure(
                                [bench.COMPILER, "--time-phases", *flags, str(src), "-o", str(work / "out.ll")]
                            )[2]
                        )
                        for _ in range(args.runs)
                    ]
                )
                problems = phase_failures(phases, args.max_sema_share, args.max_irgen_share)
                failed |= bool(problems)
                shares = phase_shares(phases)
                print(
                    f"{src.name}: sema {phases['sema']:.2f} s ({shares['sema']:.2f} of the "
                    f"front half, limit {args.max_sema_share}), irgen {phases['irgen']:.2f} s "
                    f"({shares['irgen']:.2f}, limit {args.max_irgen_share})"
                    + "".join(f" FAIL: {problem}" for problem in problems)
                    + ("" if problems else " ok")
                )
    if checked == 0:
        print("no SQLite sources; run scripts/fetch_sqlite.sh", file=sys.stderr)
        return 1
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
