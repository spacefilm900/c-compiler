#!/usr/bin/env python3
"""Differential test runner.

Contract: build/compiler INPUT.c -o OUTPUT.ll  -> textual LLVM IR, exit 0.
For each tests/*.c:
  1. project compiler -> .ll; clang (backend only) links .ll -> binary A
  2. clang (black-box oracle) compiles the .c -> binary B
  3. run both; compare exit code, stdout, stderr.

A test may #include "..." relative to its own directory (headers under tests/
are not themselves tests). #include <...> searches this compiler's include/
directory and then the host C library, the same search the compiler uses.
Clang still uses the host headers. The declarations agree, so printf,
strlen, malloc, and the macros in limits.h can be compared.

Harness tests from 119 onward in tests/*.c do not use preprocessor
directives or headers. tests/hosted/*.c may include system headers such
as <math.h> and are run the same way as tests/*.c.

tests/xabi/*_def.c is linked with the matching *_use.c in both directions:
this compiler's IR with a Clang object, and the reverse. Both must match
Clang compiling the two files together.

tests/gnu/*.c are GNU C: this compiler runs with -std=gnu11 and Clang with
-std=gnu11, and the two binaries must match. Each one is also compiled with
this compiler's default (strict C11), where it must be rejected with a
diagnostic and no crash, because every file there uses an extension that
is a C11 constraint violation.

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

def test(src, tmp, std="c11"):
    name = src.stem
    ll, a, b = tmp / f"{name}.ll", tmp / f"{name}.mine", tmp / f"{name}.ref"
    ours = [str(COMPILER)] + ([f"-std={std}"] if std != "c11" else [])
    rc, _, err = run(ours + [str(src), "-o", str(ll)])
    if rc != 0:
        return f"compiler failed ({rc}): {err.decode(errors='replace').strip()[:300]}"
    rc, _, err = run([CLANG, "-w", str(ll), "-o", str(a), "-lm"])
    if rc != 0:
        return f"clang rejected IR ({rc}): {err.decode(errors='replace').strip()[:300]}"
    rc, _, err = run([CLANG, "-w", f"-std={std}", str(src), "-o", str(b), "-lm"])
    if rc != 0:
        return f"oracle compile failed ({rc}): {err.decode(errors='replace').strip()[:300]}"
    got, exp = run([str(a)]), run([str(b)])
    if got != exp:
        return f"mismatch: got exit={got[0]} out={got[1][:100]!r}, expected exit={exp[0]} out={exp[1][:100]!r}"
    return None

def test_gnu(src, tmp):
    """GNU C: differential at -std=gnu11, and rejected in strict C11."""
    ll = tmp / f"{src.stem}.strict.ll"
    rc, _, err = run([str(COMPILER), str(src), "-o", str(ll)])
    if rc == 0:
        return "strict C11 accepted a GNU-only program"
    if not isinstance(rc, int) or rc < 0 or rc >= 100:
        return f"strict C11 did not exit with a diagnostic ({rc})"
    if b"error:" not in err:
        return "strict C11 rejected the program without an error message"
    return test(src, tmp, "gnu11")


def test_cross(def_src, use_src, tmp):
    """Link one translation unit from this compiler with one from Clang.

    `*_def.c` is the callee and `*_use.c` is main. Both directions must match
    a binary Clang built from both files.
    """
    name = def_src.name[: -len("_def.c")]
    ref = tmp / f"{name}.ref"

    def compile_ours(src, ll):
        return run([str(COMPILER), str(src), "-o", str(ll)])

    def clang_obj(src, obj):
        return run([CLANG, "-w", "-std=c11", "-c", str(src), "-o", str(obj)])

    rc, _, err = run([CLANG, "-w", "-std=c11", str(def_src), str(use_src), "-o", str(ref), "-lm"])
    if rc != 0:
        return f"oracle compile failed ({rc}): {err.decode(errors='replace').strip()[:300]}"
    expected = run([str(ref)])

    def mixed(our_src, clang_src, label):
        ll = tmp / f"{name}.{label}.ll"
        obj = tmp / f"{name}.{label}.o"
        binary = tmp / f"{name}.{label}"
        rc, _, err = compile_ours(our_src, ll)
        if rc != 0:
            return f"{label}: compiler failed ({rc}): {err.decode(errors='replace').strip()[:300]}"
        rc, _, err = clang_obj(clang_src, obj)
        if rc != 0:
            return f"{label}: clang failed ({rc}): {err.decode(errors='replace').strip()[:300]}"
        rc, _, err = run([CLANG, "-w", str(ll), str(obj), "-o", str(binary), "-lm"])
        if rc != 0:
            return f"{label}: link failed ({rc}): {err.decode(errors='replace').strip()[:300]}"
        got = run([str(binary)])
        if got != expected:
            return (
                f"{label}: mismatch got exit={got[0]} out={got[1][:80]!r}, "
                f"expected exit={expected[0]} out={expected[1][:80]!r}"
            )
        return None

    return mixed(def_src, use_src, "our-def") or mixed(use_src, def_src, "our-use")


def main():
    tests = sorted((ROOT / "tests").glob("*.c"))
    hosted = sorted((ROOT / "tests" / "hosted").glob("*.c"))
    crosses = sorted((ROOT / "tests" / "xabi").glob("*_def.c"))
    gnu = sorted((ROOT / "tests" / "gnu").glob("*.c"))
    problems = []
    if not (COMPILER.is_file() and os.access(COMPILER, os.X_OK)):
        problems.append(f"compiler not found/executable: {COMPILER}")
    if shutil.which(CLANG) is None:
        problems.append(f"clang not found: {CLANG}")
    total = len(tests) + len(hosted) + len(crosses) + len(gnu)
    if problems:
        for t in tests:
            print(f"FAIL {t.name}: " + "; ".join(problems))
        for t in hosted:
            print(f"FAIL {t.parent.name}/{t.name}: " + "; ".join(problems))
        for t in crosses + gnu:
            print(f"FAIL {t.parent.name}/{t.name}: " + "; ".join(problems))
        print(f"\n0/{total} passed")
        return 1
    passed = 0
    with tempfile.TemporaryDirectory() as d:
        tmp = Path(d)
        for t in tests:
            r = test(t, tmp)
            print(f"{'PASS' if r is None else 'FAIL'} {t.name}" + ("" if r is None else f": {r}"))
            passed += r is None
        for t in hosted:
            r = test(t, tmp)
            label = f"{t.parent.name}/{t.name}"
            print(f"{'PASS' if r is None else 'FAIL'} {label}" + ("" if r is None else f": {r}"))
            passed += r is None
        for t in crosses:
            use = t.with_name(t.name[: -len("_def.c")] + "_use.c")
            label = f"{t.parent.name}/{t.name[: -len('_def.c')]}"
            if not use.is_file():
                r = f"missing {use.name}"
            else:
                r = test_cross(t, use, tmp)
            print(f"{'PASS' if r is None else 'FAIL'} {label}" + ("" if r is None else f": {r}"))
            passed += r is None
        for t in gnu:
            r = test_gnu(t, tmp)
            label = f"{t.parent.name}/{t.name}"
            print(f"{'PASS' if r is None else 'FAIL'} {label}" + ("" if r is None else f": {r}"))
            passed += r is None
    print(f"\n{passed}/{total} passed")
    return 0 if passed == total else 1

if __name__ == "__main__":
    sys.exit(main())
