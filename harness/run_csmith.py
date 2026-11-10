#!/usr/bin/env python3
"""Csmith differential runner.

Generates programs with Csmith 2.3.0 (scripts/fetch_csmith.sh), compiles
each with this compiler and with clang -std=c11 -O0, runs both, and
compares exit status and stdout. Csmith's own main prints a checksum
line (`checksum = ...`); that line is part of stdout. This is the usual
Csmith method (timeout around the compiler and the program, then compare
checksums). The program run also sets RLIMIT_CPU and RLIMIT_AS.

CI runs the seeds in harness/csmith_seeds.txt. A local batch uses
`--count N` (seeds 1..N) and is not part of CI. Options below keep
programs inside a few seconds. They leave packed structs off: `#pragma
pack` is ignored, so a packed layout would not match Clang.

harness/csmith_known_failures.txt lists seeds that still fail, with a
reason category. An unlisted failure fails the run. A listed seed that
passes fails the run. A compiler panic fails the run even when listed.

Env: CC_UNDER_TEST, CLANG, CSMITH, CSMITH_INCLUDE, CSMITH_RUN_TIMEOUT.
"""

from __future__ import annotations

import argparse
import os
import resource
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPILER = Path(os.environ.get("CC_UNDER_TEST", ROOT / "build" / "compiler"))
CLANG = os.environ.get("CLANG", "clang")
CSMITH = Path(
    os.environ.get(
        "CSMITH",
        ROOT / "tests" / "external" / "csmith" / "install" / "bin" / "csmith",
    )
)
INCLUDE = Path(
    os.environ.get(
        "CSMITH_INCLUDE",
        ROOT / "tests" / "external" / "csmith" / "src" / "runtime",
    )
)
SEEDS_PATH = ROOT / "harness" / "csmith_seeds.txt"
KNOWN_PATH = ROOT / "harness" / "csmith_known_failures.txt"
RUN_TIMEOUT = float(os.environ.get("CSMITH_RUN_TIMEOUT", "8"))
# Wall time is longer than the CPU limit so a CPU-bound program dies from
# SIGXCPU in both binaries, instead of one side winning the race between
# the parent's timeout and the kernel signal.
WALL_TIMEOUT = RUN_TIMEOUT * 2
COMPILE_TIMEOUT = 60.0

# Same options for every seed. Packed structs are off (see the docstring).
# The size caps keep the CI batch to a few minutes. `--count` uses them too.
CSMITH_OPTIONS = [
    "--no-packed-struct",
    "--no-argc",
    "--max-funcs",
    "4",
    "--max-block-depth",
    "3",
    "--max-block-size",
    "3",
    "--max-expr-complexity",
    "4",
    "--max-array-len-per-dim",
    "4",
    "--max-array-dim",
    "2",
    "--concise",
]


def load_seeds(path: Path) -> list[str]:
    seeds: list[str] = []
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        text = line.strip()
        if not text or text.startswith("#"):
            continue
        if not text.isdigit():
            sys.exit(f"{path}:{lineno}: seed must be an unsigned integer")
        if text in seeds:
            sys.exit(f"{path}:{lineno}: duplicate seed {text}")
        seeds.append(text)
    if not seeds:
        sys.exit(f"{path}: no seeds")
    return seeds


def load_known(path: Path) -> dict[str, str]:
    known: dict[str, str] = {}
    if not path.is_file():
        return known
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        text = line.strip()
        if not text or text.startswith("#"):
            continue
        seed, _, reason = text.partition(" ")
        reason = reason.strip() or "known"
        if seed in known:
            sys.exit(f"{path}:{lineno}: duplicate entry for {seed}")
        known[seed] = reason
    return known


def category(detail: str) -> str:
    low = detail.lower()
    rules = [
        ("panicked at", "panic"),
        ("long double", "long-double"),
        ("wide string", "wide-literal"),
        ("bit-field", "bit-field"),
        ("invalid operands", "strict-pointer"),
        ("incompatible types", "strict-pointer"),
        ("incompatible pointer", "strict-pointer"),
        ("inconclusive", "inconclusive"),
        ("timeout", "timeout"),
        ("mismatch", "checksum"),
        ("clang rejected ir", "bad-ir"),
        ("oracle compile failed", "csmith-invalid"),
    ]
    for needle, name in rules:
        if needle in low:
            return name
    return "compile-error"


def limit_program() -> None:
    # Csmith's compiler_test.pl uses an 8s program timeout. These limits
    # stop a generated program that forks or allocates without bound.
    cpu = max(1, int(RUN_TIMEOUT))
    resource.setrlimit(resource.RLIMIT_CPU, (cpu, cpu))
    resource.setrlimit(resource.RLIMIT_AS, (1 << 31, 1 << 31))


def run(
    cmd: list[str], timeout: float, limit: bool = False, cwd: Path | None = None
) -> tuple[object, bytes, bytes]:
    try:
        proc = subprocess.run(
            cmd,
            cwd=cwd,
            capture_output=True,
            timeout=timeout,
            preexec_fn=limit_program if limit else None,
        )
        return proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired:
        return "timeout", b"", b""
    except OSError as err:
        return "oserror", b"", str(err).encode()


def clip(data: bytes, n: int = 300) -> str:
    return data.decode(errors="replace").strip().replace("\n", " ")[:n]


def one_seed(seed: str, work: Path) -> tuple[str, str]:
    """Return (status, detail). status is pass, fail, or panic."""
    src = work / f"{seed}.c"
    ll = work / f"{seed}.ll"
    mine = work / f"{seed}.mine"
    ref = work / f"{seed}.ref"
    rc, _, err = run(
        [str(CSMITH), "--seed", seed, *CSMITH_OPTIONS, "-o", str(src)],
        COMPILE_TIMEOUT,
        cwd=work,
    )
    if rc != 0 or not src.is_file():
        return "fail", f"csmith failed ({rc}): {clip(err)}"
    rc, _, err = run(
        [str(COMPILER), str(src), "-I", str(INCLUDE), "-o", str(ll)],
        COMPILE_TIMEOUT,
        cwd=work,
    )
    err_text = err.decode(errors="replace")
    if "panicked at" in err_text.lower():
        return "panic", clip(err)
    if rc != 0:
        return "fail", f"compiler failed ({rc}): {clip(err)}"
    rc, _, err = run(
        [CLANG, "-std=c11", "-O0", "-w", str(ll), "-o", str(mine), "-lm"],
        COMPILE_TIMEOUT,
        cwd=work,
    )
    if rc != 0:
        return "fail", f"clang rejected IR ({rc}): {clip(err)}"
    rc, _, err = run(
        [CLANG, "-std=c11", "-O0", "-w", "-I", str(INCLUDE), str(src), "-o", str(ref), "-lm"],
        COMPILE_TIMEOUT,
        cwd=work,
    )
    if rc != 0:
        return "fail", f"oracle compile failed ({rc}): {clip(err)}"
    got = run([str(mine)], WALL_TIMEOUT, limit=True, cwd=work)
    exp = run([str(ref)], WALL_TIMEOUT, limit=True, cwd=work)
    if got[0] == "timeout" or exp[0] == "timeout":
        return "fail", f"inconclusive timeout: ours={got[0]} clang={exp[0]}"
    # A checksum comparison needs both programs to finish with exit 0.
    # The same signal or the same nonzero status is not a passing result.
    if got[0] != 0 or exp[0] != 0:
        return (
            "fail",
            f"inconclusive exit ours={got[0]} clang={exp[0]} out={got[1][:60]!r}",
        )
    if got[1] != exp[1]:
        return (
            "fail",
            f"mismatch ours exit={got[0]} out={got[1][:80]!r} "
            f"clang exit={exp[0]} out={exp[1][:80]!r}",
        )
    if b"checksum = " not in got[1]:
        return "fail", "inconclusive: both exited 0 without a checksum"
    return "pass", ""


def main() -> int:
    parser = argparse.ArgumentParser(description="Run a fixed Csmith differential batch.")
    parser.add_argument(
        "--count",
        type=int,
        default=0,
        help="local batch: seeds 1..N instead of harness/csmith_seeds.txt",
    )
    parser.add_argument(
        "--record",
        type=Path,
        help="write failing seeds as a known-failure list and exit 0",
    )
    args = parser.parse_args()
    if not COMPILER.is_file():
        sys.exit(f"compiler not found: {COMPILER}")
    if not CSMITH.is_file():
        sys.exit(f"csmith not found: {CSMITH}. Run scripts/fetch_csmith.sh first.")
    if not (INCLUDE / "csmith.h").is_file():
        sys.exit(f"csmith runtime header missing: {INCLUDE / 'csmith.h'}")

    seeds = [str(n) for n in range(1, args.count + 1)] if args.count else load_seeds(SEEDS_PATH)
    known = {} if args.record else load_known(KNOWN_PATH)
    passed = 0
    known_fail = 0
    problems: list[str] = []
    recorded: list[tuple[str, str]] = []
    with tempfile.TemporaryDirectory(prefix="csmith-") as tmp:
        work = Path(tmp)
        for seed in seeds:
            status, detail = one_seed(seed, work)
            reason = category(detail)
            if status == "pass" and seed in known:
                problems.append(f"now passing {seed} (listed as {known[seed]})")
                print(f"NOW_PASS {seed}")
            elif status == "pass":
                passed += 1
                print(f"PASS {seed}")
            elif status == "panic":
                problems.append(f"panic {seed}: {detail}")
                print(f"PANIC {seed}: {detail}")
            elif seed in known:
                known_fail += 1
                print(f"KNOWN {seed} {known[seed]}")
            elif args.record:
                recorded.append((seed, reason))
                print(f"RECORD {seed} {reason}: {detail}")
            else:
                problems.append(f"fail {seed} {reason}: {detail}")
                print(f"FAIL {seed} {reason}: {detail}")

    if args.record:
        lines = [
            "# Known Csmith failures. One seed, then a reason category.",
            "# An unlisted failure fails harness/run_csmith.py.",
            "# A listed seed that passes fails the run.",
            "# A compiler panic fails the run even when the seed is listed.",
            "# Options are CSMITH_OPTIONS in harness/run_csmith.py.",
            "",
        ]
        for seed, reason in recorded:
            lines.append(f"{seed} {reason}")
        text = "\n".join(lines) + "\n"
        args.record.write_text(text, encoding="utf-8")
        print(f"\nrecorded {len(recorded)} failures in {args.record}")
        print(f"{passed} passed, {len(recorded)} recorded, {len(seeds)} seeds")
        return 1 if any(item.startswith("panic") for item in problems) else 0

    stale = sorted(set(known) - set(seeds))
    for seed in stale:
        problems.append(f"listed seed {seed} was not run")
    print(
        f"\n{passed} passed, {known_fail} known failures, "
        f"{len(problems)} unexpected, {len(seeds)} seeds"
    )
    if problems:
        print("\nUnexpected:")
        for item in problems:
            print(f"  {item}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
