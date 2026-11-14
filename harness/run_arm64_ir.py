#!/usr/bin/env python3
"""Check this compiler's arm64 IR without running it.

    python3 harness/run_arm64_ir.py [--target arm64-apple-macos|aarch64-linux-gnu]

Two checks, for each AArch64 target (both by default):

1. Signature lowering. The calling-convention battery
   (tests/arm64/xabi/*_def.c and tests/xabi/*_def.c), and every other test
   program both compilers accept, is compiled by this compiler and by Clang
   (the black-box oracle) for the same target.
   For each function the lowered return type, parameter types and the ABI
   attributes that change the calling convention (signext, zeroext, sret,
   byval, inreg, alignstack, and `...`) must match. Aggregate IR types are compared by
   their flattened leaves, so `[4 x float]` and Clang's `%struct.F4 =
   type { float, float, float, float }` are the same homogeneous aggregate,
   and Clang's `i1 zeroext` for `_Bool` matches this compiler's `i8 zeroext`.

2. Backend acceptance. Every test program (tests/*.c, tests/gnu/both/*.c,
   tests/arm64/**/*.c) that this compiler accepts for x86_64 is compiled
   for the target and handed to `llc -mtriple=<triple> -filetype=obj`; for
   arm64-apple-macos that writes a Mach-O object. A program this compiler
   rejects for the target passes only if Clang rejects it too. Programs that include
   system headers are skipped unless the AArch64 glibc headers are present
   (AARCH64_INCLUDE, default /usr/aarch64-linux-gnu/include); this needs no
   Mac, and the macOS SDK headers are not used.

Env: CC_UNDER_TEST, CLANG (default clang), LLC (default: llc, llc-18..21),
AARCH64_INCLUDE, JOBS.
"""
import argparse, concurrent.futures, os, re, shutil, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPILER = Path(os.environ.get("CC_UNDER_TEST", ROOT / "build" / "compiler"))
CLANG = os.environ.get("CLANG", "clang")
INCLUDE = Path(os.environ.get("AARCH64_INCLUDE", "/usr/aarch64-linux-gnu/include"))
TARGETS = {
    "arm64-apple-macos": ("arm64-apple-macos11", "arm64-apple-macosx11.0.0"),
    "aarch64-linux-gnu": ("aarch64-linux-gnu", "aarch64-unknown-linux-gnu"),
}
ABI_ATTRS = ("signext", "zeroext", "inreg", "sret", "byval", "alignstack")


def find_llc():
    if os.environ.get("LLC"):
        return os.environ["LLC"]
    for name in ["llc"] + [f"llc-{v}" for v in (21, 20, 19, 18)]:
        if shutil.which(name):
            return name
    return None


def run(cmd, timeout=60):
    p = subprocess.run(cmd, capture_output=True, timeout=timeout)
    return p.returncode, p.stdout.decode(errors="replace"), p.stderr.decode(errors="replace")


def split_top(text, sep=","):
    """Split at `sep` outside (), [], {} and <>."""
    out, depth, cur = [], 0, ""
    for ch in text:
        if ch in "([{<":
            depth += 1
        elif ch in ")]}>":
            depth -= 1
        if ch == sep and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def named_types(ir):
    types = {}
    for m in re.finditer(r"^(%[\w.$\"-]+) = type (.*)$", ir, re.M):
        types[m.group(1)] = m.group(2).strip()
    return types


def leaves(ty, types):
    """Flatten an IR type to its scalar leaves."""
    ty = ty.strip()
    if ty.startswith("<{") or ty.startswith("{"):
        inner = ty.strip("<>")[1:-1]
        return [leaf for part in split_top(inner) for leaf in leaves(part, types)]
    m = re.fullmatch(r"\[(\d+) x (.*)\]", ty)
    if m:
        return leaves(m.group(2), types) * int(m.group(1))
    if ty.startswith("%"):
        return leaves(types[ty], types)
    return [ty]


def canon_type(ty, types):
    ty = ty.strip()
    if ty.startswith(("%", "{", "<{", "[")):
        return "agg(" + ",".join(leaves(ty, types)) + ")"
    return ty


def canon_param(param, types):
    if param == "...":
        return "..."
    # The type is the first token, or a bracketed group.
    m = re.match(r"(\[[^\]]*\]|\{[^}]*\}|<\{[^}]*\}>|\S+)(.*)", param)
    ty, rest = m.group(1), m.group(2)
    attrs = []
    for attr in ABI_ATTRS:
        am = re.search(rf"\b{attr}(\(([^)]*)\))?", rest)
        if am:
            if attr == "alignstack":
                attrs.append(am.group(0))
            elif attr in ("sret", "byval") and am.group(2):
                attrs.append(f"{attr}({canon_type(am.group(2), types)})")
            else:
                attrs.append(attr)
    ty = canon_type(ty, types)
    if ty == "i1":
        ty = "i8"  # Clang passes _Bool as i1; this compiler as i8
    return " ".join([ty] + attrs)


def signatures(ir):
    types = named_types(ir)
    sigs = {}
    for m in re.finditer(r"^define\s+(.*?)@([\w.$]+)\((.*)$", ir, re.M):
        head, name, tail = m.group(1), m.group(2), m.group(3)
        depth, end = 1, 0
        for end, ch in enumerate(tail):
            depth += {"(": 1, ")": -1}.get(ch, 0)
            if depth == 0:
                break
        params = tail[:end]
        words = [w for w in split_top(head, " ") if w]
        skip = {"internal", "dso_local", "private", "linkonce_odr", "weak", "noundef",
                "inreg", "signext", "zeroext", "noalias", "nonnull", "hidden"}
        rettoks = [w for w in words if w not in skip and not w.startswith(("dereferenceable", "align"))]
        ret = canon_type(rettoks[-1], types) if rettoks else "?"
        attrs = [a for a in ("signext", "zeroext") if a in words]
        if ret == "i1":
            ret = "i8"
        ps = [canon_param(p, types) for p in split_top(params)]
        sigs[name] = " ".join([ret] + attrs) + " (" + ", ".join(ps) + ")"
    return sigs


def battery():
    return sorted((ROOT / "tests" / "arm64" / "xabi").glob("*_def.c")) + sorted(
        (ROOT / "tests" / "xabi").glob("*_def.c"))


def check_signatures(target, tmp, sysroot):
    """Compare lowered signatures for the battery (which must compile) and,
    as a wider sample, for every test program both compilers accept."""
    clang_triple, _ = TARGETS[target]
    must = set(battery())
    sources = sorted(must) + [p for p in programs() if p not in must]

    def one(src):
        if uses_system_headers(src) and sysroot is None:
            return 0, []
        std = "gnu11" if "gnu" in src.parts else "c11"
        tag = f"{target}-{src.parent.name}-{src.stem}"
        ours, ref = tmp / f"{tag}.sig.ll", tmp / f"{tag}.sigref.ll"
        cmd = [str(COMPILER), f"-std={std}", "-target", target, "-I", str(src.parent), str(src),
               "-o", str(ours)]
        oracle = [CLANG, "-target", clang_triple, f"-std={std}", "-O0", "-S", "-emit-llvm", "-w",
                  "-I", str(src.parent), str(src), "-o", str(ref)]
        if sysroot is not None:
            cmd[1:1] = ["-isysroot", str(sysroot)]
            oracle[1:1] = ["-isysroot", str(sysroot), "-U__nonnull", "-U__nullable",
                           "-U__null_unspecified"]
        rc, _, err = run(cmd)
        rc2, _, err2 = run(oracle)
        if src not in must and (rc or rc2):
            return 0, []
        if rc:
            return 0, [f"{src.name}: this compiler failed: {err.strip()[:300]}"]
        if rc2:
            return 0, [f"{src.name}: clang failed: {err2.strip()[:300]}"]
        mine, theirs = signatures(ours.read_text()), signatures(ref.read_text())
        compared, failures = 0, []
        for name in sorted(theirs):
            if name not in mine:
                # Clang keeps unused static functions at -O0; this compiler may not.
                continue
            compared += 1
            if mine[name] != theirs[name]:
                failures.append(f"{src.relative_to(ROOT)}: {name}\n    ours:  {mine[name]}\n"
                                f"    clang: {theirs[name]}")
        return compared, failures

    workers = int(os.environ.get("JOBS", os.cpu_count() or 2))
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        results = list(pool.map(one, sources))
    return sum(r[0] for r in results), [f for r in results for f in r[1]]


def uses_system_headers(src):
    text = src.read_text(errors="replace")
    return re.search(r"^\s*#\s*include\s*<(?!(stdarg|stddef|stdbool|stdalign|stdnoreturn|float|limits|iso646|stdint)\.h>)", text, re.M) is not None


def programs():
    progs = sorted((ROOT / "tests").glob("*.c"))
    progs += sorted((ROOT / "tests" / "gnu" / "both").glob("*.c"))
    progs += sorted((ROOT / "tests" / "arm64").rglob("*.c"))
    return progs


def check_llc(target, tmp, llc, sysroot):
    _, triple = TARGETS[target]
    def one(src):
        if uses_system_headers(src) and sysroot is None:
            return "skip", None
        std = "gnu11" if "gnu" in src.parts else "c11"
        x86 = run([str(COMPILER), f"-std={std}", "-I", str(src.parent), str(src), "-o", os.devnull])
        if x86[0]:
            return "skip", None  # an expected-error test
        ll = tmp / f"{target}-{src.parent.name}-{src.stem}.ll"
        cmd = [str(COMPILER), f"-std={std}", "-target", target, "-I", str(src.parent), str(src), "-o", str(ll)]
        if sysroot is not None:
            cmd[1:1] = ["-isysroot", str(sysroot)]
        rc, _, err = run(cmd)
        if rc:
            # A program the target itself rejects (an ELF section name on
            # Mach-O, say) passes when Clang rejects it too.
            oracle = [CLANG, "-target", TARGETS[target][0], f"-std={std}", "-fsyntax-only", "-w",
                      "-I", str(src.parent), str(src)]
            if sysroot is not None:
                oracle[1:1] = ["-isysroot", str(sysroot), "-U__nonnull", "-U__nullable",
                               "-U__null_unspecified"]
            if run(oracle)[0] != 0:
                return "rejected", None
            return "fail", f"{src.relative_to(ROOT)}: compile failed: {err.strip()[:300]}"
        rc, _, err = run([llc, f"-mtriple={triple}", "-filetype=obj", str(ll), "-o", str(ll.with_suffix(".o"))])
        if rc:
            return "fail", f"{src.relative_to(ROOT)}: llc rejected the IR: {err.strip()[:300]}"
        return "pass", None
    workers = int(os.environ.get("JOBS", os.cpu_count() or 2))
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        results = list(pool.map(one, programs()))
    counts = {k: sum(1 for r in results if r[0] == k) for k in ("pass", "fail", "skip", "rejected")}
    return counts, [msg for kind, msg in results if kind == "fail"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", choices=sorted(TARGETS), action="append")
    args = parser.parse_args()
    llc = find_llc()
    if llc is None:
        print("FAIL: llc not found (set LLC)")
        return 1
    bad = 0
    with tempfile.TemporaryDirectory() as d:
        tmp = Path(d)
        sysroot = None
        if INCLUDE.is_dir():
            sysroot = tmp / "sysroot"
            (sysroot / "usr").mkdir(parents=True)
            (sysroot / "usr" / "include").symlink_to(INCLUDE)
        for target in args.target or sorted(TARGETS):
            compared, failures = check_signatures(target, tmp, sysroot)
            for f in failures:
                print(f"FAIL signature {target} {f}")
            print(f"{target}: {compared} function signatures compared with Clang, {len(failures)} differ")
            counts, llc_failures = check_llc(target, tmp, llc, sysroot)
            for f in llc_failures:
                print(f"FAIL llc {target} {f}")
            print(f"{target}: llc -filetype=obj accepted {counts['pass']} programs, "
                  f"{counts['fail']} failed, {counts['rejected']} rejected by both compilers "
                  f"for this target, {counts['skip']} skipped")
            bad += len(failures) + len(llc_failures)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
