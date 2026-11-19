#!/usr/bin/env python3
"""Random calling-convention differential test (two translation units).

Each seed writes a pair of C files:

  abi_def.c  defines functions that print every argument they receive
             (fixed and variadic, read with va_arg) and return a value
             built from them;
  abi_use.c  calls each one with constants and prints the result.

The parameter and result types are random: integer and floating scalars
(long double included), structs with arrays, nested structs and
bit-fields, unions, and homogeneous floating aggregates of float, double
and long double (HFAs in AAPCS64). Functions take enough arguments to
spill out of the argument registers.

Four builds are compared: Clang for both files (the oracle), this
compiler for one file and Clang for the other (both ways), and this
compiler for both. All four must print the same text.

Usage: tools/abi_fuzz.py [--seed-start N] [--count N] [--out DIR]
                         [--int128] [--target x86_64-apple-macos|
                                     x86_64-w64-windows-gnu|x86_64-pc-windows-msvc]
(--out DIR keeps each failing pair in DIR/seedN.)
--int128 adds `__int128` and `unsigned __int128` to the scalar types (in a
variadic function only as the last argument, see only_last_int128; an
x86-64 or AArch64 target; the programs for a seed differ from those
without it). --target x86_64-apple-macos builds both compilers' code for
Intel macOS and runs it through the Linux stand-in in
harness/intel_mac_proxy.py. --target x86_64-w64-windows-gnu or
x86_64-pc-windows-msvc builds COFF objects for 64-bit Windows, links them
with MinGW-w64 and runs them under Wine (harness/windows_x64.py; the MSVC
target is a stand-in on MinGW-w64's headers and C runtime).
Env: CC_UNDER_TEST (default build/compiler), CLANG (default clang). Without
--target, the target is whatever both produce by default; for AArch64 under
qemu, use wrappers that add --target and -target.
Exit status: 0 when every compared seed agrees, 1 when a seed fails (an
oracle failure included), 2 when seeds were asked for but none could be
compared (every one skipped).
"""

from __future__ import annotations

import argparse
import os
import random
import shutil
import signal
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "harness"))
import intel_mac_proxy as intel  # noqa: E402
import windows_x64 as win  # noqa: E402
COMPILER = os.environ.get("CC_UNDER_TEST", str(ROOT / "build" / "compiler"))
CLANG = os.environ.get("CLANG", "clang")

# (C spelling, printf format, cast for printf, kind)
INTS = [
    ("char", "%d", "(int)", "int"),
    ("signed char", "%d", "(int)", "int"),
    ("unsigned char", "%u", "(unsigned)", "int"),
    ("short", "%d", "(int)", "int"),
    ("unsigned short", "%u", "(unsigned)", "int"),
    ("int", "%d", "", "int"),
    ("unsigned", "%u", "", "int"),
    ("long", "%ld", "", "int"),
    ("unsigned long long", "%llu", "", "int"),
]
FLOATS = [
    ("float", "%a", "(double)", "float"),
    ("double", "%a", "", "float"),
    ("long double", "%La", "", "float"),
]
# Types va_arg may name directly (no default argument promotion).
PROMOTED = {"int", "unsigned", "long", "unsigned long long", "double", "long double",
            "__int128", "unsigned __int128"}
# With --int128. I128 (defined in each file) prints the two halves.
INT128 = [
    ("__int128", "%llx:%llx", "I128", "int"),
    ("unsigned __int128", "%llx:%llx", "I128", "int"),
]
I128_MACRO = ("#define I128(x) (unsigned long long)((unsigned __int128)(x) >> 64), "
              "(unsigned long long)(x)")
# Set by --int128.
OPTIONS = {"int128": False}


def int_pool():
    return INTS + (INT128 if OPTIONS["int128"] else [])


class Gen:
    def __init__(self, rng: random.Random):
        self.rng = rng
        self.decls: list[str] = []
        self.n = 0

    def scalar(self, allow_float=True):
        pool = int_pool() + (FLOATS if allow_float else [])
        return ("scalar",) + self.rng.choice(pool)

    def hfa(self):
        name = f"H{self.n}"
        self.n += 1
        ft = self.rng.choice(FLOATS)
        count = self.rng.randint(1, 4)
        style = self.rng.choice(["fields", "array", "nested"])
        if style == "array":
            body = f"{ft[0]} a[{count}];"
            fields = [(f"a[{i}]", ("scalar",) + ft) for i in range(count)]
        elif style == "nested" and count >= 2:
            inner = f"H{self.n}i"
            self.decls.append(f"typedef struct {{ {ft[0]} x, y; }} {inner};")
            body = f"{inner} p; " + " ".join(f"{ft[0]} f{i};" for i in range(count - 2))
            fields = [("p.x", ("scalar",) + ft), ("p.y", ("scalar",) + ft)]
            fields += [(f"f{i}", ("scalar",) + ft) for i in range(count - 2)]
        else:
            body = " ".join(f"{ft[0]} f{i};" for i in range(count))
            fields = [(f"f{i}", ("scalar",) + ft) for i in range(count)]
        self.decls.append(f"typedef struct {{ {body} }} {name};")
        return ("struct", name, fields)

    def mixed(self, depth=0):
        name = f"S{self.n}"
        self.n += 1
        parts, fields = [], []
        for i in range(self.rng.randint(1, 5)):
            r = self.rng.random()
            if r < 0.15:
                base = self.rng.choice(["int", "unsigned"])
                width = self.rng.randint(1, 31)
                parts.append(f"{base} b{i} : {width};")
                # A bit-field narrower than int promotes to int, so the
                # unsigned one is cast back for %u.
                fmt, cast = ("%d", "") if base == "int" else ("%u", "(unsigned)")
                fields.append((f"b{i}", ("bits", base, fmt, cast, "int", width)))
            elif r < 0.3:
                t = self.scalar()
                k = self.rng.randint(1, 3)
                parts.append(f"{t[1]} a{i}[{k}];")
                fields += [(f"a{i}[{j}]", t) for j in range(k)]
            elif r < 0.4 and depth == 0:
                inner = self.rng.choice([self.hfa, lambda: self.mixed(1)])()
                parts.append(f"{inner[1]} s{i};")
                fields += [(f"s{i}.{f}", t) for f, t in inner[2]]
            else:
                t = self.scalar()
                parts.append(f"{t[1]} m{i};")
                fields.append((f"m{i}", t))
        if self.rng.random() < 0.1:
            # A union: only the first member is printed.
            t = self.scalar()
            u = self.scalar()
            name_u = f"U{self.n}"
            self.n += 1
            self.decls.append(f"typedef union {{ {t[1]} x; {u[1]} y; }} {name_u};")
            parts.append(f"{name_u} u;")
            fields.append(("u.x", t))
        self.decls.append(f"typedef struct {{ {' '.join(parts)} }} {name};")
        return ("struct", name, fields)

    def any_type(self):
        r = self.rng.random()
        if r < 0.35:
            return self.scalar()
        if r < 0.65:
            return self.hfa()
        return self.mixed()


def value(t, rng):
    """A constant initializer for t."""
    if t[0] == "struct":
        return "{" + ", ".join(value_leaf(ft, rng) for _, ft in t[2]) + "}"
    return value_leaf(t, rng)


def value_leaf(t, rng):
    if t[0] == "bits":
        width = t[5]
        hi = (1 << (width - 1)) - 1 if t[1] == "int" else (1 << width) - 1
        lo = -(1 << (width - 1)) if t[1] == "int" else 0
        return str(rng.randint(lo, hi))
    kind = t[4]
    if kind == "float":
        v = rng.choice([0.5, -1.25, 3.0, 1e10, -7.5e-3, 123.0625])
        suffix = {"float": "f", "double": "", "long double": "L"}[t[1]]
        return f"{v!r}{suffix}"
    spelling = t[1]
    if spelling in ("char", "signed char"):
        return str(rng.randint(-100, 100))
    if spelling == "unsigned char":
        return str(rng.randint(0, 255))
    if spelling == "short":
        return str(rng.randint(-30000, 30000))
    if spelling == "unsigned short":
        return str(rng.randint(0, 65535))
    if spelling == "int":
        return str(rng.randint(-2**31, 2**31 - 1))
    if spelling == "unsigned":
        return f"{rng.randint(0, 2**32 - 1)}u"
    if spelling == "long":
        return f"{rng.randint(-2**62, 2**62)}L"
    if spelling in ("__int128", "unsigned __int128"):
        high, low = rng.randint(0, 2**64 - 1), rng.randint(0, 2**64 - 1)
        return f"({spelling})(((unsigned __int128){high}ull << 64) | {low}ull)"
    return f"{rng.randint(0, 2**64 - 1)}ull"


def flatten(t, expr):
    """(expression, printf format, cast) for every printed leaf of t."""
    if t[0] == "struct":
        return [(f"{expr}.{f}", ft[2], ft[3]) for f, ft in t[2]]
    return [(expr, t[2], t[3])]


def print_stmt(label, t, expr):
    leaves = flatten(t, expr)
    fmt = " ".join(f for _, f, _ in leaves)
    args = ", ".join(f"{c}({e})" if c else e for e, _, c in leaves)
    return f'  printf("{label} {fmt}\\n", {args});'


def only_last_int128(params, variadic):
    """In a variadic function, a scalar __int128 only as the last argument.

    When a scalar __int128 goes on the stack because only one general
    register is left, Clang's caller (LLVM) leaves that register unused
    while Clang's va_arg for a later variadic argument reads it, so Clang's
    own two-unit build reads memory nobody wrote and is no reference. GCC
    uses the register on both sides (System V AMD64 3.2.3). This compiler
    emits what Clang does, with the same disagreement (STATUS.md). Earlier
    ones become unsigned long long; no random numbers are drawn, so the
    rest of the seed is unchanged."""
    u64 = ("scalar",) + next(t for t in INTS if t[0] == "unsigned long long")
    every = params + variadic
    fixed = [u64 if t[0] == "scalar" and t[1] in ("__int128", "unsigned __int128")
             and i != len(every) - 1 else t for i, t in enumerate(every)]
    return fixed[:len(params)], fixed[len(params):]


def make_pair(seed: int) -> tuple[str, str]:
    rng = random.Random(seed)
    g = Gen(rng)
    funcs = []
    for fi in range(rng.randint(3, 6)):
        ret = g.any_type()
        params = [g.any_type() for _ in range(rng.randint(1, 10))]
        variadic = []
        if rng.random() < 0.4:
            for _ in range(rng.randint(1, 5)):
                t = g.any_type()
                if t[0] == "scalar" and t[1] not in PROMOTED:
                    t = ("scalar",) + rng.choice(
                        [x for x in int_pool() + FLOATS if x[0] in PROMOTED])
                variadic.append(t)
        if variadic and params[-1][0] == "scalar" and params[-1][1] not in PROMOTED:
            # va_start's parameter must not change under the default
            # argument promotions (C11 7.16.1.4p4).
            params.append(("scalar",) + INTS[5])
        if variadic:
            params, variadic = only_last_int128(params, variadic)
        funcs.append((fi, ret, params, variadic))

    head = ["#include <stdio.h>", "#include <stdarg.h>"]
    if OPTIONS["int128"]:
        head.append(I128_MACRO)
    head += g.decls
    protos = []
    for fi, ret, params, variadic in funcs:
        plist = ", ".join(f"{t[1]} p{i}" for i, t in enumerate(params))
        if variadic:
            plist += ", ..."
        protos.append(f"{ret[1]} f{fi}({plist});")
    common = "\n".join(head + protos) + "\n"

    d = [common]
    for fi, ret, params, variadic in funcs:
        plist = ", ".join(f"{t[1]} p{i}" for i, t in enumerate(params))
        if variadic:
            plist += ", ..."
        d.append(f"{ret[1]} f{fi}({plist}) {{")
        for i, t in enumerate(params):
            d.append(print_stmt(f"f{fi}.p{i}", t, f"p{i}"))
        if variadic:
            d.append("  va_list ap;")
            d.append(f"  va_start(ap, p{len(params) - 1});")
            for i, t in enumerate(variadic):
                d.append(f"  {{ {t[1]} v = va_arg(ap, {t[1]});")
                d.append("  " + print_stmt(f"f{fi}.v{i}", t, "v") + " }")
            d.append("  va_end(ap);")
        d.append(f"  {ret[1]} r = {value(ret, rng)};")
        d.append("  return r;")
        d.append("}")

    u = [common, "int main(void) {"]
    for fi, ret, params, variadic in funcs:
        args = []
        for i, t in enumerate(params + variadic):
            u.append(f"  {t[1]} a{fi}_{i} = {value(t, rng)};")
            args.append(f"a{fi}_{i}")
        u.append(f"  {ret[1]} r{fi} = f{fi}({', '.join(args)});")
        u.append(print_stmt(f"f{fi}.ret", ret, f"r{fi}"))
    u.append("  return 0;")
    u.append("}")
    return "\n".join(d) + "\n", "\n".join(u) + "\n"


def run(cmd, timeout=60):
    """(exit status, stdout, stderr); the status is "timeout" on a timeout.

    Each command gets its own process group, and a timeout kills the whole
    group, so a wrapper's children do not outlive the seed."""
    with subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          start_new_session=True) as proc:
        try:
            out, err = proc.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            # A descendant that left the group may still hold the pipes, so
            # do not wait for EOF: close them and reap the direct child.
            for pipe in (proc.stdout, proc.stderr):
                if pipe:
                    pipe.close()
            proc.wait()
            return "timeout", b"", f"timed out after {timeout} s".encode()
        return proc.returncode, out, err


class IntelMac:
    """Builds for x86_64-apple-macos through the Linux stand-in."""

    def __init__(self, tmp: Path):
        self.sysroot = intel.make_sysroot(tmp)
        self.llc, self.opt = intel.find_tools()
        if self.sysroot is None or self.llc is None or self.opt is None:
            sys.exit("the Intel macOS stand-in needs the x86_64 glibc headers, and llc and opt of the same LLVM version, 18 or later")

    def obj(self, src: Path, obj: Path, ours: bool):
        """(status, stderr) of building src into an ELF object."""
        ll = obj.with_suffix(".ll")
        if ours:
            cmd = [COMPILER, "-target", intel.TARGET, "-isysroot", str(self.sysroot),
                   *intel.OURS_FLAGS, str(src), "-o", str(ll)]
        else:
            cmd = [CLANG, "-target", intel.CLANG_TRIPLE, "-isysroot", str(self.sysroot),
                   *intel.CLANG_FLAGS, "-w", "-std=c11", "-S", "-emit-llvm", str(src), "-o", str(ll)]
        rc, _, err = run(cmd)
        if rc != 0:
            return rc, err
        try:
            text = ll.read_text(errors="replace")
        except OSError as exc:
            return 1, f"no IR written: {exc}".encode()
        proxy = obj.with_suffix(".proxy.ll")
        proxy.write_text(intel.proxy_ir(text))
        for step in intel.object_commands(self.llc, self.opt, proxy, obj):
            rc, _, err = run(step)
            if rc != 0:
                return rc, err
        return 0, b""


class Windows:
    """Builds for 64-bit Windows: COFF objects, linked by MinGW-w64 and run
    under Wine."""

    def __init__(self, target: str):
        problem = win.available()
        if problem:
            sys.exit(problem)
        self.target = target
        os.environ.update(win.wine_env())
        win.start_server()

    def obj(self, src: Path, obj: Path, ours: bool):
        """(status, stderr) of building src into a COFF object."""
        ll = obj.with_suffix(".ll")
        if ours:
            cmd = [COMPILER, "-target", self.target, *win.ours_flags(self.target), str(src),
                   "-o", str(ll)]
        else:
            cmd = [CLANG, "-target", self.target, *win.clang_flags(self.target), "-w",
                   "-std=c11", "-S", "-emit-llvm", str(src), "-o", str(ll)]
        rc, _, err = run(cmd)
        if rc != 0:
            return rc, err
        rc, _, err = run(win.object_command(CLANG, self.target, ll, obj))
        return rc, err

    def link(self, objs: list, exe: Path):
        return run(win.link_command(objs, exe, self.target))

    def exe(self, work: Path, tag: str) -> Path:
        return work / f"abi.{tag}.exe"

    def command(self, exe: Path):
        return win.run_command(exe)


# Set by --target.
CROSS: list = [None]


def exe_path(work: Path, tag: str) -> Path:
    if CROSS[0] is not None and hasattr(CROSS[0], "exe"):
        return CROSS[0].exe(work, tag)
    return work / f"abi.{tag}"


def run_exe(exe: Path):
    if CROSS[0] is not None and hasattr(CROSS[0], "command"):
        return run(CROSS[0].command(exe))
    return run([str(exe)])


def build_and_run(work: Path, d: Path, u: Path, ours: set[str], tag: str):
    objs = []
    for src in (d, u):
        if CROSS[0] is not None:
            obj = work / f"{src.stem}.{tag}.o"
            rc, err = CROSS[0].obj(src, obj, src.name in ours)
            who = "compiler" if src.name in ours else "clang"
            if rc != 0:
                return f"{who} failed on {src.name} ({rc}): {err.decode(errors='replace')[:300]}"
            objs.append(str(obj))
        elif src.name in ours:
            ll = work / f"{src.stem}.{tag}.ll"
            rc, _, err = run([COMPILER, str(src), "-o", str(ll)])
            if rc != 0:
                return f"compiler failed on {src.name} ({rc}): {err.decode(errors='replace')[:300]}"
            objs.append(str(ll))
        else:
            obj = work / f"{src.stem}.{tag}.o"
            rc, _, err = run([CLANG, "-w", "-std=c11", "-c", str(src), "-o", str(obj)])
            if rc != 0:
                return f"clang failed on {src.name} ({rc}): {err.decode(errors='replace')[:300]}"
            objs.append(str(obj))
    exe = exe_path(work, tag)
    if CROSS[0] is not None and hasattr(CROSS[0], "link"):
        rc, _, err = CROSS[0].link(objs, exe)
    else:
        rc, _, err = run([CLANG, "-w", *objs, "-o", str(exe)])
    if rc != 0:
        return f"link failed ({rc}): {err.decode(errors='replace')[:300]}"
    return run_exe(exe)


def rerun_reference(work: Path, ref) -> str:
    """Run Clang's own build three more times: "same" when every run matches
    `ref`, "differs" when a clean run prints something else, else the
    failure (a nonzero exit or a timeout), which is an oracle failure."""
    verdict = "same"
    for _ in range(3):
        again = run_exe(exe_path(work, "clang"))
        if again[0] == "timeout":
            return "timeout"
        if again[0] != 0:
            return f"exit {again[0]}: {again[2][:200]!r}"
        if again != ref:
            verdict = "differs"
    return verdict


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--seed-start", type=int, default=1)
    ap.add_argument("--count", type=int, default=100)
    ap.add_argument("--out", type=Path, help="keep failing pairs here")
    ap.add_argument("--int128", action="store_true", help="also use __int128 scalars")
    ap.add_argument("--target", choices=[intel.TARGET, *win.TARGETS],
                    help="build for Intel macOS through the Linux stand-in, or for "
                         "64-bit Windows under Wine")
    args = ap.parse_args()
    OPTIONS["int128"] = args.int128
    cross_tmp = None
    if args.target == intel.TARGET:
        cross_tmp = tempfile.TemporaryDirectory(prefix="abi-sysroot-")
        CROSS[0] = IntelMac(Path(cross_tmp.name))
    elif args.target in win.TARGETS:
        CROSS[0] = Windows(args.target)
    if shutil.which(CLANG) is None:
        sys.exit(f"clang not found: {CLANG}")
    failures = 0
    skipped = 0
    for seed in range(args.seed_start, args.seed_start + args.count):
        with tempfile.TemporaryDirectory(prefix="abi-") as tmp:
            work = Path(tmp)
            dsrc, usrc = make_pair(seed)
            d, u = work / "abi_def.c", work / "abi_use.c"
            d.write_text(dsrc)
            u.write_text(usrc)
            ref = build_and_run(work, d, u, set(), "clang")
            bad = []
            if isinstance(ref, str) or ref[0] != 0:
                detail = ref if isinstance(ref, str) else f"exit {ref[0]}: {ref[2][:200]!r}"
                bad.append(f"oracle failed: {detail}")
            elif (reruns := rerun_reference(work, ref)) != "same":
                if reruns != "differs":
                    bad.append(f"oracle failed on a rerun: {reruns}")
                else:
                    # Clang's own two-unit build prints something different
                    # on another run: it reads memory nobody wrote, so there
                    # is no reference to compare with. A backstop: the one
                    # known cause is avoided by only_last_int128.
                    skipped += 1
                    print(f"seed {seed}: SKIP (Clang's own build is not repeatable)")
                    continue
            else:
                for tag, ours in (("def", {d.name}), ("use", {u.name}), ("both", {d.name, u.name})):
                    got = build_and_run(work, d, u, ours, tag)
                    if isinstance(got, str):
                        bad.append(f"{tag}: {got}")
                    elif got[0] == "timeout":
                        bad.append(f"{tag}: timeout")
                    elif got != ref:
                        bad.append(tag)
            if bad:
                failures += 1
                print(f"seed {seed}: FAIL ({'; '.join(bad)})")
                if args.out:
                    dest = args.out / f"seed{seed}"
                    dest.mkdir(parents=True, exist_ok=True)
                    shutil.copy(d, dest)
                    shutil.copy(u, dest)
            else:
                print(f"seed {seed}: ok")
    agreed = args.count - failures - skipped
    note = f", {skipped} skipped (no repeatable Clang reference)" if skipped else ""
    print(f"{agreed}/{args.count - skipped} seeds agree{note}")
    if failures:
        return 1
    if args.count > 0 and agreed == 0:
        print("inconclusive: no seed was compared")
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
