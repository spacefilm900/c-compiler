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
(--out DIR keeps each failing pair in DIR/seedN.)
Env: CC_UNDER_TEST (default build/compiler), CLANG (default clang). The
target is whatever both produce by default; for AArch64 under qemu, use
wrappers that add --target and -target.
"""

from __future__ import annotations

import argparse
import os
import random
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
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
PROMOTED = {"int", "unsigned", "long", "unsigned long long", "double", "long double"}


class Gen:
    def __init__(self, rng: random.Random):
        self.rng = rng
        self.decls: list[str] = []
        self.n = 0

    def scalar(self, allow_float=True):
        pool = INTS + (FLOATS if allow_float else [])
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
                        [x for x in INTS + FLOATS if x[0] in PROMOTED])
                variadic.append(t)
        if variadic and params[-1][0] == "scalar" and params[-1][1] not in PROMOTED:
            # va_start's parameter must not change under the default
            # argument promotions (C11 7.16.1.4p4).
            params.append(("scalar",) + INTS[5])
        funcs.append((fi, ret, params, variadic))

    head = ["#include <stdio.h>", "#include <stdarg.h>"] + g.decls
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
    """(exit status, stdout, stderr); the status is "timeout" on a timeout."""
    try:
        p = subprocess.run(cmd, capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return "timeout", b"", b""
    return p.returncode, p.stdout, p.stderr


def build_and_run(work: Path, d: Path, u: Path, ours: set[str], tag: str):
    objs = []
    for src in (d, u):
        if src.name in ours:
            ll = work / f"{src.stem}.{tag}.ll"
            rc, _, err = run([COMPILER, str(src), "-o", str(ll)])
            if rc != 0:
                return f"compiler failed on {src.name}: {err.decode(errors='replace')[:300]}"
            objs.append(str(ll))
        else:
            obj = work / f"{src.stem}.{tag}.o"
            rc, _, err = run([CLANG, "-w", "-std=c11", "-c", str(src), "-o", str(obj)])
            if rc != 0:
                return f"clang failed on {src.name}: {err.decode(errors='replace')[:300]}"
            objs.append(str(obj))
    exe = work / f"abi.{tag}"
    rc, _, err = run([CLANG, "-w", *objs, "-o", str(exe)])
    if rc != 0:
        return f"link failed: {err.decode(errors='replace')[:300]}"
    return run([str(exe)])


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--seed-start", type=int, default=1)
    ap.add_argument("--count", type=int, default=100)
    ap.add_argument("--out", type=Path, help="keep failing pairs here")
    args = ap.parse_args()
    if shutil.which(CLANG) is None:
        sys.exit(f"clang not found: {CLANG}")
    failures = 0
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
    print(f"{args.count - failures}/{args.count} seeds agree")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
