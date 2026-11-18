#!/usr/bin/env python3
"""Run the differential harness for a non-default target: an AArch64 one
under qemu-aarch64, or Intel macOS through its x86_64 Linux stand-in.

    python3 harness/run_cross.py --target aarch64-linux-gnu
    python3 harness/run_cross.py --target arm64-apple-macos
    python3 harness/run_cross.py --target x86_64-apple-macos

The test set is the one harness/run_tests.py uses on x86_64: tests/*.c,
tests/hosted/*.c, the tests/xabi/ pairs (this compiler's object linked with
Clang's in both directions), tests/gnu/*.c, tests/gnu/both/*.c, and the
AArch64 calling-convention tests in tests/arm64/ (pairs in tests/arm64/xabi/). Each
program is compiled by this compiler and by Clang (the black-box oracle)
for the same target, both binaries run under qemu-aarch64, and exit
status, stdout and stderr must match.

aarch64-linux-gnu is a real target: the binaries are static AArch64 Linux
executables linked against the AArch64 glibc (Debian/Ubuntu packages
libc6-dev-arm64-cross and libgcc-*-dev-arm64-cross, binutils for
aarch64-linux-gnu, and qemu-user).

arm64-apple-macos is an execution proxy, because there is no Mac here.
Both compilers emit IR for arm64-apple-macosx. LLVM then builds that IR for
the triple arm64-apple-darwin-elf: the AArch64 backend applies Apple's
calling convention (every variadic argument on the stack, packed stack
arguments, caller-extended narrow integers) but writes an ELF object, which
is linked with the AArch64 glibc and run under qemu. Headers come from the
same AArch64 glibc (-isysroot). Variadic glibc functions cannot be called
with Apple's convention, so calls to printf and its relatives are renamed
in both IR files to a small Apple-convention stdio shim
(harness/arm64/apple_stdio_shim.c) that formats each conversion through
fixed-argument helpers compiled for Linux (harness/arm64/linux_fmt_helpers.c).
long double is double on Apple arm64, so glibc's long double functions do
not match; harness/cross_known_failures.txt lists what that excludes. What
this proxy cannot show — Mach-O linking, the macOS SDK headers, and the
macOS C library — needs a real Mac.

x86_64-apple-macos is also a proxy, described in harness/intel_mac_proxy.py:
both compilers emit IR for x86_64-apple-macosx10.15.0 over the x86_64 glibc
headers, llc builds ELF objects for the triple x86_64-apple-darwin-elf, and
the programs run natively, linked with glibc. System V variadic calls are
the same on both systems, so no stdio shim is needed. The AArch64 tests in
tests/arm64/ are not run for it.

harness/cross_known_failures.txt lists expected failures as
"TARGET LABEL  # reason". A listed test that passes is reported so the
line can be removed. An unlisted failure fails the run.

Env: CC_UNDER_TEST, CLANG (default clang), QEMU_AARCH64 (default
qemu-aarch64), AARCH64_LINK (default "clang --target=aarch64-linux-gnu
-static"), AARCH64_INCLUDE (default /usr/aarch64-linux-gnu/include),
TIMEOUT (seconds, default 20), JOBS (default: CPU count), TORTURE_ROOT
(for --torture), LLC and OPT (x86_64-apple-macos; default llc and opt, or
a versioned llc-21 .. llc-18).
"""
import argparse, concurrent.futures, os, re, shlex, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "harness"))
import intel_mac_proxy as intel  # noqa: E402
from run_gcc_torture import dg_flags  # noqa: E402
import run_gcc_torture  # noqa: E402
COMPILER = Path(os.environ.get("CC_UNDER_TEST", ROOT / "build" / "compiler"))
CLANG = os.environ.get("CLANG", "clang")
QEMU = os.environ.get("QEMU_AARCH64", "qemu-aarch64")
LINK = shlex.split(os.environ.get("AARCH64_LINK", "clang --target=aarch64-linux-gnu -static"))
INCLUDE = Path(os.environ.get("AARCH64_INCLUDE", "/usr/aarch64-linux-gnu/include"))
TIMEOUT = float(os.environ.get("TIMEOUT", "20"))
KNOWN = ROOT / "harness" / "cross_known_failures.txt"
APPLE_TRIPLE = "arm64-apple-macos11"
PROXY_TRIPLE = "arm64-apple-darwin-elf"
# Clang predefines these as nullability keywords for Darwin; glibc uses
# __nonnull as an attribute macro.
CLANG_APPLE_FIXUPS = ["-U__nonnull", "-U__nullable", "-U__null_unspecified"]
SHIMMED = re.compile(r"@(printf|fprintf|sprintf|snprintf|vprintf|vfprintf|vsprintf|vsnprintf|nanl)\b")


def run(cmd, cwd=None):
    try:
        p = subprocess.run(cmd, capture_output=True, timeout=TIMEOUT, cwd=cwd)
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return "timeout", b"", b""
    except OSError as e:
        return "oserror", b"", str(e).encode()


def short(err):
    return err.decode(errors="replace").strip()[:300]


class Toolchain:
    def __init__(self, target, tmp):
        self.target = target
        self.apple = target == "arm64-apple-macos"
        self.intel = target == intel.TARGET
        self.tmp = tmp
        self.shims = []
        self.llc = self.opt = None
        if self.apple:
            sysroot = tmp / "sysroot"
            (sysroot / "usr").mkdir(parents=True)
            (sysroot / "usr" / "include").symlink_to(INCLUDE)
            self.sysroot = sysroot

    def setup(self):
        """Build the Apple stdio shim, or the Intel macOS stand-in's
        sysroot. Returns an error string or None."""
        if self.intel:
            self.sysroot = intel.make_sysroot(self.tmp)
            if self.sysroot is None:
                return "the x86_64 glibc headers are missing"
            self.llc, self.opt = intel.find_tools()
            if self.llc is None or self.opt is None:
                return "llc and opt of the same LLVM version, 18 or later, not found (set LLC and OPT)"
            return None
        if not self.apple:
            return None
        shim_a = self.tmp / "apple_stdio_shim.o"
        shim_l = self.tmp / "linux_fmt_helpers.o"
        rc, _, err = run([CLANG, f"--target={PROXY_TRIPLE}", "-O1", "-w", "-c",
                          str(ROOT / "harness" / "arm64" / "apple_stdio_shim.c"), "-o", str(shim_a)])
        if rc != 0:
            return f"shim build failed: {short(err)}"
        rc, _, err = run(LINK + ["-O1", "-w", "-c",
                                 str(ROOT / "harness" / "arm64" / "linux_fmt_helpers.c"), "-o", str(shim_l)])
        if rc != 0:
            return f"helper build failed: {short(err)}"
        self.shims = [str(shim_a), str(shim_l)]
        return None

    def ours(self, src, ll, std, extra=()):
        cmd = [str(COMPILER), "-target", self.target, *extra]
        if self.apple or self.intel:
            cmd += ["-isysroot", str(self.sysroot)]
        if self.intel:
            cmd += intel.OURS_FLAGS
        if self.apple:
            # The glibc stand-in sysroot: see intel_mac_proxy.OURS_FLAGS.
            cmd.append("-D_GCC_LIMITS_H_")
        if std != "c11":
            cmd.append(f"-std={std}")
        return run(cmd + [str(src), "-o", str(ll)])

    def oracle_obj(self, src, obj, std):
        """Clang's object for one translation unit."""
        if self.intel:
            ll = obj.with_suffix(".clang.ll")
            rc, out, err = run([CLANG, "-target", intel.CLANG_TRIPLE, "-isysroot", str(self.sysroot),
                                *intel.CLANG_FLAGS, "-w", f"-std={std}", "-S", "-emit-llvm",
                                str(src), "-o", str(ll)])
            if rc != 0:
                return rc, out, err
            return self.ll_obj(ll, obj)
        if self.apple:
            ll = obj.with_suffix(".clang.ll")
            rc, out, err = run([CLANG, "-target", APPLE_TRIPLE, "-isysroot", str(self.sysroot),
                                *CLANG_APPLE_FIXUPS, "-w", f"-std={std}", "-S", "-emit-llvm",
                                str(src), "-o", str(ll)])
            if rc != 0:
                return rc, out, err
            return self.ll_obj(ll, obj)
        return run(LINK + ["-w", f"-std={std}", "-c", str(src), "-o", str(obj)])

    def ll_obj(self, ll, obj):
        """Object file for an IR file from either compiler."""
        if self.intel:
            proxy = ll.with_suffix(".proxy.ll")
            proxy.write_text(intel.proxy_ir(ll.read_text(errors="replace")))
            for cmd in intel.object_commands(self.llc, self.opt, proxy, obj):
                result = run(cmd)
                if result[0] != 0:
                    return result
            return result
        if self.apple:
            text = ll.read_text(errors="replace")
            renamed = ll.with_suffix(".proxy.ll")
            renamed.write_text(SHIMMED.sub(lambda m: "@__apple_" + m.group(1), text))
            return run([CLANG, f"--target={PROXY_TRIPLE}", "-w", "-c", str(renamed), "-o", str(obj)])
        return run(LINK + ["-w", "-c", str(ll), "-o", str(obj)])

    def link(self, objs, out):
        if self.intel:
            return run([CLANG, *[str(o) for o in objs], "-lm", "-o", str(out)])
        return run(LINK + [str(o) for o in objs] + self.shims + ["-lm", "-o", str(out)])

    def execute(self, binary):
        if self.intel:
            return run([str(binary)])
        return run([QEMU, str(binary)])


def build_ours(tc, src, base, std, extra=()):
    ll, obj = base.with_suffix(".ll"), base.with_suffix(".o")
    rc, _, err = tc.ours(src, ll, std, extra)
    if rc != 0:
        return None, f"compiler failed ({rc}): {short(err)}"
    rc, _, err = tc.ll_obj(ll, obj)
    if rc != 0:
        return None, f"LLVM rejected IR ({rc}): {short(err)}"
    return obj, None


def build_ref(tc, src, base, std):
    obj = base.with_suffix(".ref.o")
    rc, _, err = tc.oracle_obj(src, obj, std)
    if rc != 0:
        return None, f"oracle compile failed ({rc}): {short(err)}"
    return obj, None


def test(tc, src, work, std="c11"):
    base = work / f"{src.stem}.{std}"
    mine, err = build_ours(tc, src, base, std)
    if err:
        return err
    ref, err = build_ref(tc, src, base, std)
    if err:
        return err
    a, b = base.with_suffix(".mine"), base.with_suffix(".refbin")
    for objs, out, who in (([mine], a, "ours"), ([ref], b, "oracle")):
        rc, _, e = tc.link(objs, out)
        if rc != 0:
            return f"{who} link failed ({rc}): {short(e)}"
    got, exp = tc.execute(a), tc.execute(b)
    err = not_run(got, "ours") or not_run(exp, "oracle")
    if err:
        return err
    if got != exp:
        return mismatch(got, exp)
    return None


def not_run(result, who):
    """An error string when the program did not run to an exit status
    (qemu missing, or a time-out); such results are never compared."""
    if result[0] in ("timeout", "oserror"):
        return f"{who} did not run: {result[0]} {short(result[2])}".strip()
    return None


def mismatch(got, exp):
    """Describe the first difference between two (exit, stdout) results."""
    if got[0] != exp[0]:
        head = f"exit {got[0]}, expected {exp[0]}"
    else:
        head = f"exit {got[0]}"
    a, b = got[1].splitlines(), exp[1].splitlines()
    for i in range(max(len(a), len(b))):
        x = a[i] if i < len(a) else b"<missing>"
        y = b[i] if i < len(b) else b"<missing>"
        if x != y:
            return f"mismatch: {head}; line {i + 1}: got {x[:120]!r}, expected {y[:120]!r}"
    return f"mismatch: {head}"


def test_gnu(tc, src, work):
    rc, _, err = tc.ours(src, work / f"{src.stem}.strict.ll", "c11")
    if rc == 0:
        return "strict C11 accepted a GNU-only program"
    if b"error:" not in err:
        return "strict C11 rejected the program without an error message"
    return test(tc, src, work, "gnu11")


def test_both(tc, src, work):
    return test(tc, src, work) or test(tc, src, work, "gnu11")


def test_cross(tc, def_src, use_src, work):
    name = def_src.name[: -len("_def.c")]
    base = work / name
    objs = {}
    for role, src in (("def", def_src), ("use", use_src)):
        o, err = build_ours(tc, src, work / f"{name}_{role}", "c11")
        if err:
            return f"{role}: {err}"
        r, err = build_ref(tc, src, work / f"{name}_{role}", "c11")
        if err:
            return f"{role}: {err}"
        objs[role] = (o, r)
    expected_bin = base.with_suffix(".refbin")
    rc, _, e = tc.link([objs["def"][1], objs["use"][1]], expected_bin)
    if rc != 0:
        return f"oracle link failed: {short(e)}"
    expected = tc.execute(expected_bin)
    err = not_run(expected, "oracle")
    if err:
        return err
    for label, pair in (("our-def", (objs["def"][0], objs["use"][1])),
                        ("our-use", (objs["def"][1], objs["use"][0])),
                        ("both-ours", (objs["def"][0], objs["use"][0]))):
        binary = base.with_suffix(f".{label}")
        rc, _, e = tc.link(list(pair), binary)
        if rc != 0:
            return f"{label}: link failed: {short(e)}"
        got = tc.execute(binary)
        err = not_run(got, label)
        if err:
            return err
        if got != expected:
            return f"{label}: {mismatch(got, expected)}"
    return None


def known_failures(target):
    known = {}
    if not KNOWN.is_file():
        return known
    for line in KNOWN.read_text().splitlines():
        body, _, reason = line.partition("#")
        parts = body.split()
        if len(parts) == 2 and parts[0] == target:
            known[parts[1]] = reason.strip()
    return known


def torture(target, label_filter):
    """Run gcc.c-torture/execute for a cross target, built by this compiler
    alone (`-std=gnu11`, and `-fgnu89-inline` where the test's dg-options
    ask for it), each program's own self-test deciding. This is a
    report, not a gate: it lists failures that are not already in the
    x86_64 list harness/gcc_torture_known_failures.txt, and exits 0. Under
    the Apple proxy, programs that call glibc's `long double` functions or
    its asm-label redirects (`__isoc99_fscanf`, `__sysv_signal`) cannot
    work, because those are glibc's, not Apple's.

    For x86_64-apple-macos the x86_64 list applies (same processor, same
    `long double`) with the macOS overlay, so there it is a gate: a failure
    not in the list, a compiler panic even on a listed test, or a filter
    that selects nothing fails the run. Listed tests that pass are
    reported."""
    root = Path(os.environ.get("TORTURE_ROOT", ROOT / "tests" / "external" / "gcc-torture")) \
        / "gcc" / "testsuite" / "gcc.c-torture" / "execute"
    if not root.is_dir():
        print(f"FAIL: {root} is missing; run scripts/fetch_gcc_torture.sh")
        return 1
    files = sorted(p for p in root.rglob("*.c") if "builtins" not in p.parts)
    files = [p for p in files if label_filter in str(p.relative_to(root))]
    if target == intel.TARGET and not files:
        print(f"FAIL: no torture test matches the filter {label_filter!r}")
        return 1
    known = set()
    for line in (ROOT / "harness" / "gcc_torture_known_failures.txt").read_text().splitlines():
        line = line.split("#")[0].strip()
        if line:
            known.add(line.split()[0])
    if target == intel.TARGET:
        # The x86_64 list with the macOS overlay, as on the Intel Mac job.
        table = run_gcc_torture.load_known(run_gcc_torture.KNOWN_PATH)
        for overlay in run_gcc_torture.target_overlays(intel.IR_TRIPLE, "x86_64"):
            run_gcc_torture.apply_target_overlay(table, overlay)
        known = set(table)
    with tempfile.TemporaryDirectory() as d:
        tmp = Path(d)
        tc = Toolchain(target, tmp)
        setup_err = tc.setup()
        if setup_err:
            print(f"FAIL setup: {setup_err}")
            return 1

        def one(src):
            label = str(src.relative_to(root))
            work = tmp / label.replace("/", "_")
            work.mkdir()
            obj, err = build_ours(tc, src, work / "t", "gnu11", dg_flags(src))
            if err:
                return label, err
            rc, _, e = tc.link([obj], work / "t.bin")
            if rc != 0:
                return label, f"link failed: {short(e)}"
            rc, _, _ = tc.execute(work / "t.bin")
            return label, None if rc == 0 else f"exit {rc}"

        workers = int(os.environ.get("JOBS", os.cpu_count() or 2))
        with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
            results = list(pool.map(one, files))
    failed = [(label, err) for label, err in results if err]
    new = [(label, err) for label, err in failed if label not in known]
    for label, err in new:
        print(f"NOT_IN_X86_LIST {label}: {err.splitlines()[0][:200]}")
    # A panic is a compiler bug whatever the list says about the test.
    panics = [(label, err) for label, err in failed if label in known and "panicked" in err]
    for label, err in panics:
        print(f"PANIC_IN_LISTED {label}: {err.splitlines()[0][:200]}")
    if target == intel.TARGET:
        for label, err in results:
            if err is None and label in known:
                print(f"PASSES_BUT_IN_X86_LIST {label}")
    print(f"\n{target} torture: {len(results)} files, {len(results) - len(failed)} passed, "
          f"{len(failed)} failed, {len(new)} of them not in the x86_64 known-failure list, "
          f"{len(panics)} listed ones panicked")
    return 1 if target == intel.TARGET and (new or panics) else 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True,
                        choices=["aarch64-linux-gnu", "arm64-apple-macos", intel.TARGET])
    parser.add_argument("--filter", default="", help="only labels containing this text")
    parser.add_argument("--torture", action="store_true",
                        help="instead, run gcc.c-torture/execute (a gate for x86_64-apple-macos, "
                             "a report for the AArch64 targets; see torture())")
    args = parser.parse_args()
    if args.torture:
        return torture(args.target, args.filter)

    jobs = []
    for t in sorted((ROOT / "tests").glob("*.c")):
        jobs.append((t.name, test, (t,)))
    for t in sorted((ROOT / "tests" / "hosted").glob("*.c")):
        jobs.append((f"hosted/{t.name}", test, (t,)))
    for t in sorted((ROOT / "tests" / "xabi").glob("*_def.c")):
        use = t.with_name(t.name[: -len("_def.c")] + "_use.c")
        jobs.append((f"xabi/{t.name[: -len('_def.c')]}", test_cross, (t, use)))
    for t in sorted((ROOT / "tests" / "gnu").glob("*.c")):
        jobs.append((f"gnu/{t.name}", test_gnu, (t,)))
    for t in sorted((ROOT / "tests" / "gnu" / "both").glob("*.c")):
        jobs.append((f"gnu/both/{t.name}", test_both, (t,)))
    arm64 = [] if args.target == intel.TARGET else sorted((ROOT / "tests" / "arm64").glob("*.c"))
    for t in arm64:
        jobs.append((f"arm64/{t.name}", test, (t,)))
    arm64 = [] if args.target == intel.TARGET else sorted((ROOT / "tests" / "arm64" / "xabi").glob("*_def.c"))
    for t in arm64:
        use = t.with_name(t.name[: -len("_def.c")] + "_use.c")
        jobs.append((f"arm64/xabi/{t.name[: -len('_def.c')]}", test_cross, (t, use)))
    jobs = [j for j in jobs if args.filter in j[0]]

    known = known_failures(args.target)
    with tempfile.TemporaryDirectory() as d:
        tmp = Path(d)
        tc = Toolchain(args.target, tmp)
        setup_err = tc.setup()
        if setup_err:
            print(f"FAIL setup: {setup_err}")
            return 1

        def one(job):
            label, fn, fargs = job
            work = tmp / re.sub(r"[^A-Za-z0-9_.-]", "_", label)
            work.mkdir()
            return label, fn(tc, *fargs, work)

        workers = int(os.environ.get("JOBS", os.cpu_count() or 2))
        with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
            results = list(pool.map(one, jobs))

    passed = unexpected = known_failed = now_pass = 0
    for label, r in results:
        if r is None:
            passed += 1
            if label in known:
                now_pass += 1
                print(f"NOW_PASS {label} (remove it from {KNOWN.name})")
            else:
                print(f"PASS {label}")
        elif label in known:
            known_failed += 1
            print(f"KNOWN {label}: {known[label]}")
        else:
            unexpected += 1
            print(f"FAIL {label}: {r}")
    print(f"\n{args.target}: {len(results)} tests, {passed} passed, "
          f"{known_failed} known failures, {unexpected} unexpected failures, {now_pass} now passing")
    return 0 if unexpected == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
