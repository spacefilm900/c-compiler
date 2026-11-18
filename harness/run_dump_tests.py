#!/usr/bin/env python3
"""Golden tests for the stage dumps (`-E`, `--dump-tokens`, `--dump-ast`,
`--dump-sema`, `--dump-ir`).

For each harness/dumps/NAME.c, the compiler runs once per stage from inside
harness/dumps (so file names in the dumps are relative), always with
`-target x86_64-linux-gnu` so the output does not depend on the host, plus
any flags on a first line of the form `// dump-flags: FLAGS`. Each dump
must match harness/dumps/NAME.STAGE byte for byte, where STAGE is `i`,
`tokens`, `ast`, `sema` or `ll`.

It also checks that the dumps do not change normal compilation:
`--dump-ir` must print exactly the IR that `-o FILE` writes, every dump
written with `-o` must equal the one on standard output, and a second run
must give the same bytes (the dumps are deterministic).

Usage: run_dump_tests.py [--update]. `--update` rewrites the expected files
from the current compiler; review the diff before committing it.

Env: CC_UNDER_TEST (default build/compiler), TIMEOUT (seconds, default 10).
Exit status 0 only if every check passes.
"""
import argparse
import difflib
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DUMPS = ROOT / "harness" / "dumps"
COMPILER = Path(os.environ.get("CC_UNDER_TEST", ROOT / "build" / "compiler")).resolve()
TIMEOUT = float(os.environ.get("TIMEOUT", "10"))
TARGET = ["-target", "x86_64-linux-gnu"]

# (expected-file suffix, compiler flag)
STAGES = [
    ("i", "-E"),
    ("tokens", "--dump-tokens"),
    ("ast", "--dump-ast"),
    ("sema", "--dump-sema"),
    ("ll", "--dump-ir"),
]


def file_flags(source):
    """Extra flags from a `// dump-flags: ...` first line."""
    first = source.read_text().splitlines()[:1]
    prefix = "// dump-flags:"
    if first and first[0].startswith(prefix):
        return first[0][len(prefix):].split()
    return []


def run(args):
    """(exit status, stdout bytes, stderr text) of the compiler in DUMPS."""
    proc = subprocess.run([str(COMPILER), *args], cwd=DUMPS, capture_output=True,
                          timeout=TIMEOUT)
    return proc.returncode, proc.stdout, proc.stderr.decode(errors="replace")


def show_diff(expected, actual, label):
    lines = difflib.unified_diff(
        expected.decode(errors="replace").splitlines(keepends=True),
        actual.decode(errors="replace").splitlines(keepends=True),
        fromfile=f"{label} (expected)", tofile=f"{label} (actual)", n=2)
    sys.stdout.writelines(list(lines)[:60])
    print()


def check_file(source, update, tmp):
    """Returns the list of failure labels for one test program."""
    failures = []
    flags = TARGET + file_flags(source)
    name = source.name
    for suffix, flag in STAGES:
        label = f"{source.stem}.{suffix}"
        status, out, err = run([*flags, flag, name])
        if status != 0:
            print(f"FAIL {label}: exit {status}\n{err}")
            failures.append(label)
            continue
        expected_path = DUMPS / f"{source.stem}.{suffix}"
        if update:
            expected_path.write_bytes(out)
        elif not expected_path.exists():
            print(f"FAIL {label}: missing {expected_path.relative_to(ROOT)} (run with --update)")
            failures.append(label)
            continue
        elif expected_path.read_bytes() != out:
            print(f"FAIL {label}: dump differs from {expected_path.relative_to(ROOT)}")
            show_diff(expected_path.read_bytes(), out, label)
            failures.append(label)
            continue
        again = run([*flags, flag, name])
        if again[0] != 0 or again[1] != out:
            print(f"FAIL {label}: a second run gave exit {again[0]} or different output\n{again[2]}")
            failures.append(label)
            continue
        written = Path(tmp) / f"{label}.out"
        status, _, err = run([*flags, flag, name, "-o", str(written)])
        if status != 0 or written.read_bytes() != out:
            print(f"FAIL {label}: -o output differs from standard output\n{err}")
            failures.append(label)
            continue
        print(f"ok   {label}")
    # --dump-ir must not change the IR a normal compilation writes.
    normal = Path(tmp) / f"{source.stem}.normal.ll"
    status, _, err = run([*flags, name, "-o", str(normal)])
    dump_status, dumped, dump_err = run([*flags, "--dump-ir", name])
    label = f"{source.stem}: --dump-ir vs -o"
    if status != 0 or dump_status != 0 or normal.read_bytes() != dumped:
        print(f"FAIL {label}: exit {status} and {dump_status}\n{err}{dump_err}")
        failures.append(label)
    else:
        print(f"ok   {label}")
    return failures


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--update", action="store_true",
                        help="rewrite the expected dumps from the current compiler")
    args = parser.parse_args()
    if not COMPILER.exists():
        print(f"compiler not found: {COMPILER} (run make)")
        return 2
    sources = sorted(DUMPS.glob("*.c"))
    if not sources:
        print(f"no tests in {DUMPS}")
        return 2
    failures = []
    with tempfile.TemporaryDirectory() as tmp:
        for source in sources:
            failures += check_file(source, args.update, tmp)
    checks = len(sources) * (len(STAGES) + 1)
    print(f"{checks - len(failures)}/{checks} dump checks passed")
    if failures:
        print("failed: " + ", ".join(failures))
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
