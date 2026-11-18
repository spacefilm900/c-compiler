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

tests/gnu/both/*.c use GNU spellings that strict C11 also accepts
(`__attribute__`, `__builtin_` functions, `__alignof__`): reserved
identifiers, not constraint violations. Each is compared with Clang in
both modes, -std=c11 and -std=gnu11.

On an AArch64 target (read from `clang -dumpmachine`: a native AArch64
Linux host, or wrappers that add the target), the AArch64
calling-convention tests in tests/arm64/ (pairs in tests/arm64/xabi/) are
run too, the same way.

Env: CC_UNDER_TEST, CLANG (default "clang"), TIMEOUT (seconds, default 10),
KNOWN_FAILURES (optional file of test labels, one per line, '#' comments:
those tests may fail without failing the run; the macOS job uses
harness/macos_known_failures.txt).
Exit status 0 only if every test not listed as a known failure passes.
"""
import os, re, shutil, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "harness"))
from target_arch import arch_of, clang_triple  # noqa: E402
COMPILER = Path(os.environ.get("CC_UNDER_TEST", ROOT / "build" / "compiler"))
CLANG = os.environ.get("CLANG", "clang")
TIMEOUT = float(os.environ.get("TIMEOUT", "10"))
MIN_CLANG_MAJOR = 18
# Apple numbers its Clang separately from LLVM. Apple Clang 16 (Xcode 16)
# reads the IR this compiler writes; the LLVM 15-17 problem behind
# MIN_CLANG_MAJOR is in the x86_64 backend, which a Mac does not use here.
MIN_APPLE_CLANG_MAJOR = 16

def clang_major():
    """(major version, is Apple Clang) from `clang --version`; major is None
    if it cannot be read."""
    try:
        out = subprocess.run([CLANG, "--version"], capture_output=True, text=True,
                             timeout=TIMEOUT).stdout
    except (OSError, subprocess.TimeoutExpired):
        return None, False
    m = re.search(r"version (\d+)", out)
    return (int(m.group(1)) if m else None), "Apple clang" in out

def known_failures():
    path = os.environ.get("KNOWN_FAILURES")
    if not path:
        return set()
    names = set()
    for line in Path(path).read_text().splitlines():
        line = line.split("#", 1)[0].strip()
        if line:
            names.add(line)
    return names

def run(cmd):
    try:
        p = subprocess.run(cmd, capture_output=True, timeout=TIMEOUT)
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return "timeout", b"", b""
    except OSError as e:
        return "oserror", b"", str(e).encode()

def incomplete(result):
    """A reason when a program did not run to completion, else None."""
    if not isinstance(result[0], int):
        return f"{result[0]}: {result[2].decode(errors='replace')[:200]}".rstrip(": ")
    if result[0] < 0:
        return f"killed by signal {-result[0]}"
    return None


# Prefix of a result where the Clang oracle's own program did not finish.
# A known-failure entry does not cover it: there is no reference result.
ORACLE_INCOMPLETE = "oracle program did not complete"


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
    reason = incomplete(exp)
    if reason:
        return f"{ORACLE_INCOMPLETE} ({reason})"
    reason = incomplete(got)
    if reason:
        return f"this compiler's program did not complete ({reason})"
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

def test_both(src, tmp):
    """Accepted in both modes: differential at -std=c11 and -std=gnu11."""
    return test(src, tmp) or test(src, tmp, "gnu11")


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
    reason = incomplete(expected)
    if reason:
        return f"{ORACLE_INCOMPLETE} ({reason})"

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
        reason = incomplete(got)
        if reason:
            return f"{label}: program did not complete ({reason})"
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
    both = sorted((ROOT / "tests" / "gnu" / "both").glob("*.c"))
    problems = []
    if shutil.which(CLANG):
        # The target decides whether tests/arm64 runs; a run that cannot
        # read it must not pass with that coverage silently missing.
        triple = clang_triple(CLANG)
        if not triple:
            problems.append(f"cannot read the target from `{CLANG} -dumpmachine`")
        elif arch_of(triple) == "aarch64":
            hosted += sorted((ROOT / "tests" / "arm64").glob("*.c"))
            crosses += sorted((ROOT / "tests" / "arm64" / "xabi").glob("*_def.c"))
    if not (COMPILER.is_file() and os.access(COMPILER, os.X_OK)):
        problems.append(f"compiler not found/executable: {COMPILER}")
    if shutil.which(CLANG) is None:
        problems.append(f"clang not found: {CLANG}")
    else:
        major, apple = clang_major()
        if apple:
            if major is None or major < MIN_APPLE_CLANG_MAJOR:
                problems.append(f"Apple Clang >= {MIN_APPLE_CLANG_MAJOR} is required; got: {major}")
        elif major is None or major < MIN_CLANG_MAJOR:
            problems.append(
                f"Clang/LLVM >= {MIN_CLANG_MAJOR} is required (LLVM 15-17 split an "
                f"i128 argument between a register and the stack); got: {major}"
            )
    total = len(tests) + len(hosted) + len(crosses) + len(gnu) + len(both)
    if problems:
        for t in tests:
            print(f"FAIL {t.name}: " + "; ".join(problems))
        for t in hosted:
            print(f"FAIL {t.parent.name}/{t.name}: " + "; ".join(problems))
        for t in crosses + gnu:
            print(f"FAIL {t.parent.name}/{t.name}: " + "; ".join(problems))
        for t in both:
            print(f"FAIL gnu/both/{t.name}: " + "; ".join(problems))
        print(f"\n0/{total} passed")
        return 1
    known = known_failures()
    passed = 0
    expected_failures = []
    now_passing = []

    def report(label, r):
        nonlocal passed
        if r is None:
            passed += 1
            if label in known:
                now_passing.append(label)
            print(f"PASS {label}")
        elif label in known and ORACLE_INCOMPLETE not in r:
            expected_failures.append(label)
            print(f"KNOWN {label}: {r}")
        else:
            print(f"FAIL {label}: {r}")

    with tempfile.TemporaryDirectory() as d:
        tmp = Path(d)
        for t in tests:
            report(t.name, test(t, tmp))
        for t in hosted:
            report(f"{t.parent.relative_to(ROOT / 'tests').as_posix()}/{t.name}", test(t, tmp))
        for t in crosses:
            use = t.with_name(t.name[: -len("_def.c")] + "_use.c")
            label = f"{t.parent.relative_to(ROOT / 'tests').as_posix()}/{t.name[: -len('_def.c')]}"
            report(label, f"missing {use.name}" if not use.is_file() else test_cross(t, use, tmp))
        for t in gnu:
            report(f"{t.parent.name}/{t.name}", test_gnu(t, tmp))
        for t in both:
            report(f"gnu/both/{t.name}", test_both(t, tmp))
    unexpected = total - passed - len(expected_failures)
    if known:
        print(f"\n{passed}/{total} passed, {len(expected_failures)} known failures, "
              f"{unexpected} unexpected failures")
        for label in now_passing:
            print(f"now passing (remove from {os.environ['KNOWN_FAILURES']}): {label}")
    else:
        print(f"\n{passed}/{total} passed")
    return 0 if unexpected == 0 else 1

if __name__ == "__main__":
    sys.exit(main())
