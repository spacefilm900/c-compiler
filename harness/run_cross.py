#!/usr/bin/env python3
"""Run the differential harness for an AArch64 target under qemu-aarch64.

    python3 harness/run_cross.py --target aarch64-linux-gnu
    python3 harness/run_cross.py --target arm64-apple-macos

The test set is the one harness/run_tests.py uses on x86_64: tests/*.c,
tests/hosted/*.c, the tests/xabi/ pairs (this compiler's object linked with
Clang's in both directions), tests/gnu/*.c and tests/gnu/both/*.c. Each
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

harness/cross_known_failures.txt lists expected failures as
"TARGET LABEL  # reason". A listed test that passes is reported so the
line can be removed. An unlisted failure fails the run.

Env: CC_UNDER_TEST, CLANG (default clang), QEMU_AARCH64 (default
qemu-aarch64), AARCH64_LINK (default "clang --target=aarch64-linux-gnu
-static"), AARCH64_INCLUDE (default /usr/aarch64-linux-gnu/include),
TIMEOUT (seconds, default 20), JOBS (default: CPU count).
"""
import argparse, concurrent.futures, os, re, shlex, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
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
        self.tmp = tmp
        self.shims = []
        if self.apple:
            sysroot = tmp / "sysroot"
            (sysroot / "usr").mkdir(parents=True)
            (sysroot / "usr" / "include").symlink_to(INCLUDE)
            self.sysroot = sysroot

    def setup(self):
        """Build the Apple stdio shim. Returns an error string or None."""
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

    def ours(self, src, ll, std):
        cmd = [str(COMPILER), "-target", self.target]
        if self.apple:
            cmd += ["-isysroot", str(self.sysroot)]
        if std != "c11":
            cmd.append(f"-std={std}")
        return run(cmd + [str(src), "-o", str(ll)])

    def oracle_obj(self, src, obj, std):
        """Clang's object for one translation unit."""
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
        if self.apple:
            text = ll.read_text(errors="replace")
            renamed = ll.with_suffix(".proxy.ll")
            renamed.write_text(SHIMMED.sub(lambda m: "@__apple_" + m.group(1), text))
            return run([CLANG, f"--target={PROXY_TRIPLE}", "-w", "-c", str(renamed), "-o", str(obj)])
        return run(LINK + ["-w", "-c", str(ll), "-o", str(obj)])

    def link(self, objs, out):
        return run(LINK + [str(o) for o in objs] + self.shims + ["-lm", "-o", str(out)])

    def execute(self, binary):
        return run([QEMU, str(binary)])


def build_ours(tc, src, base, std):
    ll, obj = base.with_suffix(".ll"), base.with_suffix(".o")
    rc, _, err = tc.ours(src, ll, std)
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
    if got != exp:
        return mismatch(got, exp)
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
    for label, pair in (("our-def", (objs["def"][0], objs["use"][1])),
                        ("our-use", (objs["def"][1], objs["use"][0])),
                        ("both-ours", (objs["def"][0], objs["use"][0]))):
        binary = base.with_suffix(f".{label}")
        rc, _, e = tc.link(list(pair), binary)
        if rc != 0:
            return f"{label}: link failed: {short(e)}"
        got = tc.execute(binary)
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True, choices=["aarch64-linux-gnu", "arm64-apple-macos"])
    parser.add_argument("--filter", default="", help="only labels containing this text")
    args = parser.parse_args()

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
    for t in sorted((ROOT / "tests" / "arm64").glob("*.c")):
        jobs.append((f"arm64/{t.name}", test, (t,)))
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
