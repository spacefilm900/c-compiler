#!/usr/bin/env python3
"""Run gcc.c-torture/execute as input programs.

The files are fetched by scripts/fetch_gcc_torture.sh. They are test
programs only. This runner compiles each with this compiler, links with
clang -O0 -lm, and checks that the program's own self-test exits 0.
It does not compare with GCC or Clang, and it does not read the GCC
compiler sources.

The tests are GNU C, so this compiler runs with `-std=gnu11`, its opt-in
GNU mode (see README). 20041212-1.c compares a `void *` with a function
pointer, which strict C11 rejects. A test whose `dg-options` comment asks
for `-fgnu89-inline` gets that option too: those tests rely on GNU89
`inline` semantics, where a plain `inline` definition is an external one.

The whole `builtins/` directory is skipped. GCC's harness links each
of those tests with a `-lib.c` companion and `lib/main.c`; they are
not standalone programs, and they exist to exercise compiler builtins.
`ieee/` is included. A `.x` file is DejaGnu metadata for other targets;
on x86_64 Linux those files return 0, so they are not skips.

`RUN_TIMEOUT` is the CPU limit. The wall-clock limit is five times
that, so a short test queued behind other jobs is not reported as a
hang.

harness/gcc_torture_known_failures.txt lists failures with a reason
category. An unlisted failure fails the run. A listed test that passes
fails the run. A compiler panic fails the run even when listed.

The list is for x86_64 Linux. On another target (read from `clang
-dumpmachine`; a native AArch64 Linux host, or CLANG and CC_UNDER_TEST
wrappers that add the target), harness/gcc_torture_known_failures_<arch>.txt
adjusts it: `-TEST reason` drops an x86_64 entry that passes there, and
`+TEST category reason` adds a failure only that target has. Every line
gives its reason. On macOS (an `-apple-` triple),
harness/gcc_torture_known_failures_macos_<arch>.txt then adjusts the
result the same way, for what the macOS SDK and C library change.

Env: CC_UNDER_TEST, CLANG, TORTURE_ROOT, TORTURE_JOBS, TORTURE_RUN_TIMEOUT.
"""

from __future__ import annotations

import argparse
import os
import re
try:
    import resource
except ImportError:  # Windows: no rlimits; the wall-clock timeout remains
    resource = None
import subprocess
import sys
import tempfile
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "harness"))
from target_arch import arch_of, clang_triple  # noqa: E402

COMPILER = Path(os.environ.get("CC_UNDER_TEST", ROOT / "build" / "compiler"))
CLANG = os.environ.get("CLANG", "clang")
SUITE = Path(
    os.environ.get(
        "TORTURE_ROOT",
        ROOT / "tests" / "external" / "gcc-torture",
    )
)
EXECUTE = SUITE / "gcc" / "testsuite" / "gcc.c-torture" / "execute"
KNOWN_PATH = ROOT / "harness" / "gcc_torture_known_failures.txt"
RUN_TIMEOUT = float(os.environ.get("TORTURE_RUN_TIMEOUT", "2"))
WALL_TIMEOUT = RUN_TIMEOUT * 5
COMPILE_TIMEOUT = 30.0


def load_known(path: Path) -> dict[str, str]:
    known: dict[str, str] = {}
    if not path.is_file():
        return known
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        text = line.strip()
        if not text or text.startswith("#"):
            continue
        rel, _, reason = text.partition(" ")
        reason = reason.strip() or "known"
        if rel in known:
            sys.exit(f"{path}:{lineno}: duplicate entry for {rel}")
        known[rel] = reason
    return known


def apply_target_overlay(known: dict[str, str], arch: str) -> None:
    """Adjust the x86_64 list for `arch` from its overlay file, if any. On
    macOS, `arch` is `macos_<arch>` (gcc_torture_known_failures_macos_x86_64.txt);
    see target_overlays."""
    path = ROOT / "harness" / f"gcc_torture_known_failures_{arch}.txt"
    if arch == "x86_64" or not path.is_file():
        return
    baseline = set(known)
    seen: set[str] = set()
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        text = line.strip()
        if not text or text.startswith("#"):
            continue
        sign, body = text[0], text[1:]
        rel, _, reason = body.partition(" ")
        usage = "expected '+TEST category reason' or '-TEST reason'"
        if sign not in "+-" or not rel or not reason.strip():
            sys.exit(f"{path}:{lineno}: {usage}")
        if sign == "+" and len(reason.split(None, 1)) < 2:
            sys.exit(f"{path}:{lineno}: {usage} (a new entry needs a category and a reason)")
        if rel in seen:
            sys.exit(f"{path}:{lineno}: {rel} appears more than once")
        seen.add(rel)
        if sign == "-":
            if rel not in baseline:
                sys.exit(f"{path}:{lineno}: {rel} is not in the x86_64 list")
            del known[rel]
        else:
            if rel in baseline:
                sys.exit(f"{path}:{lineno}: {rel} is already in the x86_64 list")
            known[rel] = reason.strip().split()[0]


def target_overlays(triple: str, arch: str) -> list[str]:
    """The overlay names for a target, applied in order: the processor's
    (`aarch64`), then, on macOS, `macos_<arch>` for what the macOS SDK
    and C library change."""
    names = [arch]
    if "-apple-" in triple:
        names.append(f"macos_{arch}")
    return names


def category(detail: str) -> str:
    low = detail.lower()
    rules = [
        ("panicked at", "panic"),
        ("long double", "long-double"),
        ("wide string", "wide-literal"),
        ("old-style function", "old-style-decl"),
        ("variable-length", "vla"),
        ("flexible array", "flexible-array"),
        ("anonymous", "anonymous-member"),
        ("empty struct", "empty-struct"),
        ("empty union", "empty-struct"),
        ("computed goto", "computed-goto"),
        ("nested function", "nested-function"),
        ("complex types", "complex"),
        ("alignment specifiers", "alignas"),
        ("_generic", "generic"),
        ("_static_assert", "static-assert"),
        ("thread-local", "tls"),
        ("atomic types", "atomic"),
        ("range designator", "case-range"),
        ("__builtin", "builtin"),
        ("asm", "asm"),
        ("typeof", "typeof"),
        ("statement expr", "stmt-expr"),
        ("timeout", "timeout"),
        ("exit ", "runtime"),
        ("signal", "runtime"),
        ("clang rejected ir", "bad-ir"),
        ("undefined reference", "link"),
        ("link failed", "link"),
    ]
    for needle, name in rules:
        if needle in low:
            return name
    return "compile-error"


def limit_program() -> None:
    cpu = max(1, int(RUN_TIMEOUT))
    resource.setrlimit(resource.RLIMIT_CPU, (cpu, cpu))
    try:
        resource.setrlimit(resource.RLIMIT_AS, (1 << 30, 1 << 30))
    except (ValueError, OSError):
        # macOS does not let a process lower its address-space limit; the
        # CPU limit and the wall-clock timeout still stop a runaway test.
        # Elsewhere the failure is an error.
        if sys.platform != "darwin":
            raise


def run(
    cmd: list[str], timeout: float, limit: bool = False, cwd: Path | None = None
) -> tuple[object, bytes, bytes]:
    try:
        proc = subprocess.run(
            cmd,
            cwd=cwd,
            capture_output=True,
            timeout=timeout,
            preexec_fn=limit_program if limit and resource is not None else None,
        )
        return proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired:
        return "timeout", b"", b""
    except OSError as err:
        return "oserror", b"", str(err).encode()


def clip(data: bytes, n: int = 240) -> str:
    return data.decode(errors="replace").strip().replace("\n", " ")[:n]


def tests() -> list[Path]:
    found: list[Path] = []
    for path in sorted(EXECUTE.rglob("*.c")):
        rel = path.relative_to(EXECUTE).as_posix()
        if rel.startswith("builtins/"):
            continue
        found.append(path)
    return found


DG_OPTIONS = re.compile(r"\{\s*dg-options\s+\"([^\"]*)\"")


def dg_flags(path: Path) -> list[str]:
    """The options from a test's dg-options comments that this compiler
    honours: only -fgnu89-inline."""
    text = path.read_text(errors="replace")
    words = [w for m in DG_OPTIONS.finditer(text) for w in m.group(1).split()]
    return ["-fgnu89-inline"] if "-fgnu89-inline" in words else []


def one_test(src: str) -> tuple[str, str, str]:
    """Return (rel, status, detail). status is pass, fail, or panic."""
    path = Path(src)
    rel = path.relative_to(EXECUTE).as_posix()
    with tempfile.TemporaryDirectory(prefix="torture-") as tmp:
        work = Path(tmp)
        ll = work / "t.ll"
        exe = work / "t"
        rc, _, err = run(
            [str(COMPILER), "-std=gnu11", *dg_flags(path), str(path), "-o", str(ll)],
            COMPILE_TIMEOUT,
        )
        err_text = err.decode(errors="replace")
        if "panicked at" in err_text.lower():
            return rel, "panic", clip(err)
        if rc != 0:
            return rel, "fail", f"compiler failed ({rc}): {clip(err)}"
        rc, _, err = run(
            [CLANG, "-std=c11", "-O0", "-w", str(ll), "-o", str(exe), "-lm"],
            COMPILE_TIMEOUT,
        )
        if rc != 0:
            text = clip(err)
            if "undefined reference" in text or "undefined symbol" in text:
                return rel, "fail", f"link failed ({rc}): {text}"
            return rel, "fail", f"clang rejected IR ({rc}): {text}"
        rc, _, err = run([str(exe)], WALL_TIMEOUT, limit=True, cwd=work)
        if rc == "timeout":
            return rel, "fail", "timeout"
        if rc != 0:
            return rel, "fail", f"exit {rc}"
        return rel, "pass", ""


def main() -> int:
    parser = argparse.ArgumentParser(description="Run gcc.c-torture/execute.")
    parser.add_argument("--record", type=Path, help="write the known-failure list from this run")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("TORTURE_JOBS", "8")))
    args = parser.parse_args()
    if not COMPILER.is_file():
        sys.exit(f"compiler not found: {COMPILER}")
    if not EXECUTE.is_dir():
        sys.exit(f"torture tests not found: {EXECUTE}. Run scripts/fetch_gcc_torture.sh first.")
    # Refuse to run if the sparse checkout picked up compiler sources.
    stray = SUITE / "gcc" / "c"
    if stray.exists():
        sys.exit(f"refusing to run: {stray} looks like GCC compiler source")

    sources = tests()
    known = {} if args.record else load_known(KNOWN_PATH)
    triple = clang_triple(CLANG)
    if not triple:
        sys.exit(f"cannot read the target from `{CLANG} -dumpmachine`")
    arch = arch_of(triple)
    if not args.record:
        for overlay in target_overlays(triple, arch):
            apply_target_overlay(known, overlay)
    if arch != "x86_64":
        print(f"target {triple}")
    results: list[tuple[str, str, str]] = []
    jobs = max(1, args.jobs)
    with ProcessPoolExecutor(max_workers=jobs) as pool:
        futures = [pool.submit(one_test, str(src)) for src in sources]
        for fut in as_completed(futures):
            results.append(fut.result())
    results.sort()

    passed = 0
    known_fail = 0
    skipped_builtins = (
        sum(1 for _ in (EXECUTE / "builtins").rglob("*.c"))
        if (EXECUTE / "builtins").is_dir()
        else 0
    )
    problems: list[str] = []
    recorded: list[tuple[str, str]] = []
    for rel, status, detail in results:
        reason = category(detail)
        if status == "pass" and rel in known:
            problems.append(f"now passing {rel} (listed as {known[rel]})")
            print(f"NOW_PASS {rel}")
        elif status == "pass":
            passed += 1
        elif status == "panic":
            problems.append(f"panic {rel}: {detail}")
            print(f"PANIC {rel}: {detail}")
        elif rel in known:
            known_fail += 1
        elif args.record:
            recorded.append((rel, reason))
        else:
            problems.append(f"fail {rel} {reason}: {detail}")
            print(f"FAIL {rel} {reason}: {detail}")

    if args.record:
        lines = [
            "# Known failures for gcc.c-torture/execute.",
            "# One path relative to gcc/testsuite/gcc.c-torture/execute, then a category.",
            "# An unlisted failure fails harness/run_gcc_torture.py.",
            "# A listed test that passes fails the run.",
            "# A compiler panic fails the run even when the test is listed.",
            "# builtins/ is skipped: those tests are linked with a companion",
            "# and lib/main.c, and they target compiler builtins.",
            "",
        ]
        for rel, reason in recorded:
            lines.append(f"{rel} {reason}")
        args.record.write_text("\n".join(lines) + "\n", encoding="utf-8")
        print(f"\nrecorded {len(recorded)} failures in {args.record}")

    ran = {rel for rel, _, _ in results}
    if not args.record:
        for rel in sorted(set(known) - ran):
            problems.append(f"listed test {rel} was not run")
    print(
        f"\n{passed} passed, {known_fail if not args.record else len(recorded)} known failures, "
        f"{len(problems)} unexpected, {len(sources)} executed, "
        f"{skipped_builtins} builtins files skipped"
    )
    if problems and not args.record:
        return 1
    if any(status == "panic" for _, status, _ in results):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
