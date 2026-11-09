#!/usr/bin/env python3
"""Differential test runner.

Contract: build/compiler INPUT.c -o OUTPUT.ll  -> textual LLVM IR, exit 0.
For each tests/*.c:
  1. project compiler -> .ll; clang (backend only) links .ll -> binary A
  2. clang (black-box oracle) compiles the .c -> binary B
  3. run both; compare exit code, stdout, stderr.
Env: CC_UNDER_TEST, CLANG (default "clang"), TIMEOUT (seconds, default 10).
Exit status 0 only if all tests pass.
"""
import os, shutil, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPILER = Path(os.environ.get("CC_UNDER_TEST", ROOT / "build" / "compiler"))
CLANG = os.environ.get("CLANG", "clang")
TIMEOUT = float(os.environ.get("TIMEOUT", "10"))

def run(cmd):
    try:
        p = subprocess.run(cmd, capture_output=True, timeout=TIMEOUT)
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return "timeout", b"", b""
    except OSError as e:
        return "oserror", b"", str(e).encode()

def test(src, tmp):
    name = src.stem
    ll, a, b = tmp / f"{name}.ll", tmp / f"{name}.mine", tmp / f"{name}.ref"
    rc, _, err = run([str(COMPILER), str(src), "-o", str(ll)])
    if rc != 0:
        return f"compiler failed ({rc}): {err.decode(errors='replace').strip()[:300]}"
    rc, _, err = run([CLANG, "-w", str(ll), "-o", str(a)])
    if rc != 0:
        return f"clang rejected IR ({rc}): {err.decode(errors='replace').strip()[:300]}"
    rc, _, err = run([CLANG, "-w", "-std=c11", str(src), "-o", str(b)])
    if rc != 0:
        return f"oracle compile failed ({rc}): {err.decode(errors='replace').strip()[:300]}"
    got, exp = run([str(a)]), run([str(b)])
    if got != exp:
        return f"mismatch: got exit={got[0]} out={got[1][:100]!r}, expected exit={exp[0]} out={exp[1][:100]!r}"
    return None

def main():
    tests = sorted((ROOT / "tests").glob("*.c"))
    problems = []
    if not (COMPILER.is_file() and os.access(COMPILER, os.X_OK)):
        problems.append(f"compiler not found/executable: {COMPILER}")
    if shutil.which(CLANG) is None:
        problems.append(f"clang not found: {CLANG}")
    if problems:
        for t in tests:
            print(f"FAIL {t.name}: " + "; ".join(problems))
        print(f"\n0/{len(tests)} passed")
        return 1
    passed = 0
    with tempfile.TemporaryDirectory() as d:
        for t in tests:
            r = test(t, Path(d))
            print(f"{'PASS' if r is None else 'FAIL'} {t.name}" + ("" if r is None else f": {r}"))
            passed += r is None
    print(f"\n{passed}/{len(tests)} passed")
    return 0 if passed == len(tests) else 1

if __name__ == "__main__":
    sys.exit(main())
