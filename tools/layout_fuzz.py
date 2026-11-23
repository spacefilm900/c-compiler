#!/usr/bin/env python3
"""Record layout differential fuzzer (original; hardening, PLAN.md step 14).

Each program declares random structs and unions: bit-fields of every
integer type (zero-width ones included), `_Bool`, `_Alignas`, arrays,
nested and anonymous members, and, with --gnu, `__attribute__((packed))`
and `aligned` on records without bit-fields. It prints sizeof, _Alignof
and offsetof for each record, then fills an object with a byte pattern
and prints every member, then stores into every member and reads each
one back (for a union, the last one stored). Clang is the black-box oracle; this compiler's output
must match at -O0 and -O2.

Usage: python3 tools/layout_fuzz.py --count 2000 --jobs 2 --out DIR [--gnu]
Failing programs are kept in DIR/fail/.
"""
import argparse
import concurrent.futures
import os
import random
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INTS = [("_Bool", 1), ("char", 8), ("signed char", 8), ("unsigned char", 8),
        ("short", 16), ("unsigned short", 16), ("int", 32), ("unsigned", 32),
        ("long", 64), ("unsigned long", 64), ("long long", 64),
        ("unsigned long long", 64)]
SCALARS = [t for t, _ in INTS] + ["float", "double", "void *"]


class Gen:
    def __init__(self, rng, gnu):
        self.rng = rng
        self.gnu = gnu
        self.decls = []
        self.count = 0

    def record(self, depth):
        rng = self.rng
        self.count += 1
        name = f"R{self.count}"
        kind = "union" if rng.random() < 0.25 else "struct"
        members = []  # (name, kind, type, bits)
        has_bits = False
        lines = []
        for i in range(rng.randint(1, 7)):
            m = f"m{i}"
            r = rng.random()
            if r < 0.45:
                ty, width = rng.choice(INTS)
                if rng.random() < 0.1:
                    lines.append(f"    {ty} : 0;")
                    has_bits = True
                    continue
                bits = rng.randint(1, width)
                lines.append(f"    {ty} {m} : {bits};")
                members.append((m, "bits", ty, bits))
                has_bits = True
            elif r < 0.75:
                ty = rng.choice(SCALARS)
                align = ""
                if rng.random() < 0.15:
                    align = f"_Alignas({rng.choice([1, 2, 4, 8, 16, 32])}) "
                    if align.startswith("_Alignas(1)") or align.startswith("_Alignas(2)"):
                        align = ""  # may be weaker than the type: a constraint violation
                lines.append(f"    {align}{ty} {m};")
                members.append((m, "scalar", ty, 0))
            elif r < 0.88:
                ty = rng.choice(SCALARS[:-1])
                n = rng.randint(1, 5)
                lines.append(f"    {ty} {m}[{n}];")
                members.append((m, "array", ty, n))
            elif depth > 0:
                inner = self.record(depth - 1)
                lines.append(f"    {inner} {m};")
                members.append((m, "record", inner, 0))
        if not members:
            lines.append("    int last;")
            members.append(("last", "scalar", "int", 0))
        attrs = ""
        if self.gnu and not has_bits and rng.random() < 0.4:
            attrs = rng.choice([" __attribute__((packed))",
                                f" __attribute__((aligned({rng.choice([1, 2, 4, 8, 16])})))",
                                " __attribute__((packed, aligned(2)))"])
        body = "\n".join(lines)
        self.decls.append((f"{kind}{attrs} {name} {{\n{body}\n}};", f"{kind} {name}", members))
        return f"{kind} {name}"


def program(seed, gnu):
    rng = random.Random(seed)
    g = Gen(rng, gnu)
    for _ in range(rng.randint(1, 3)):
        g.record(2)
    out = ["#include <stdio.h>", "#include <stddef.h>", "#include <string.h>", ""]
    out += [d for d, _, _ in g.decls]
    out += ["", "int main(void) {"]
    for idx, (_, ty, members) in enumerate(g.decls):
        v = f"x{idx}"
        out.append(f"    printf(\"{ty} %zu %zu\\n\", sizeof({ty}), _Alignof({ty}));")
        for m, kind, mty, _ in members:
            if kind != "bits":
                out.append(f"    printf(\" {m} %zu\\n\", offsetof({ty}, {m}));")
        out.append(f"    {ty} {v};")
        out.append(f"    for (size_t i = 0; i < sizeof {v}; i++) ((unsigned char *)&{v})[i] = (unsigned char)(i * 37 + {seed % 251});")
        for m, kind, mty, _ in members:
            # A `_Bool` object holding a byte other than 0 or 1 has no
            # defined value, so plain `_Bool` members are not read here.
            if kind == "bits" or (kind == "scalar" and mty not in ("float", "double", "void *", "_Bool")):
                out.append(f"    printf(\" {m}=%llx\\n\", (unsigned long long)({v}.{m}));")
        # Store into the members, then read back what was stored. Padding
        # bytes, and the bytes of a union outside its last-stored member,
        # have unspecified values after a store (C11 6.2.6.1p6-7), so the
        # object's bytes are not compared. For a union only the last
        # stored member is read back.
        stored = []
        for m, kind, mty, bits in members:
            val = rng.randint(0, (1 << 64) - 1)
            if kind == "bits" or (kind == "scalar" and mty not in ("float", "double", "void *")):
                out.append(f"    {v}.{m} = ({mty}){val}ull;")
                stored.append((m, "%llx", "unsigned long long"))
            elif kind == "scalar" and mty != "void *":
                out.append(f"    {v}.{m} = ({mty}){rng.randint(-1000, 1000)}.5;")
                stored.append((m, "%.17g", "double"))
        if ty.startswith("union"):
            stored = stored[-1:]
        for m, fmt, cast in stored:
            out.append(f"    printf(\" {m}<-{fmt}\\n\", ({cast})({v}.{m}));")
    out += ["    return 0;", "}", ""]
    return "\n".join(out)


def run(cmd, cwd, timeout=60):
    try:
        p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True,
                           timeout=timeout, errors="replace")
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return "timeout", "", ""


def one(seed, args):
    work = Path(tempfile.mkdtemp(prefix=f"lay{seed}-"))
    std = "-std=gnu11" if args.gnu else "-std=c11"
    try:
        (work / "t.c").write_text(program(seed, args.gnu))
        code, _, _ = run([args.clang, std, "-O0", "-w", "t.c", "-o", "ref"], work)
        if code != 0:
            return seed, "invalid"
        code, ref, _ = run(["./ref"], work, 20)
        if code != 0:
            return seed, "invalid"
        code, _, err = run([args.compiler, std, "t.c", "-o", "t.ll"], work)
        if code != 0:
            return seed, keep(seed, work, args, f"{'rejects' if code == 1 else 'crash'}: {err[-600:]}")
        for opt in ("-O0", "-O2"):
            code, _, err = run([args.clang, opt, "-w", "t.ll", "-o", "c"], work)
            if code != 0:
                return seed, keep(seed, work, args, f"bad-ir {opt}: {err[-600:]}")
            code, out, _ = run(["./c"], work, 20)
            if code != 0 or out != ref:
                return seed, keep(seed, work, args, f"wrong {opt}:\n--- clang\n{ref}\n--- hcc\n{out}")
        return seed, "agreement"
    finally:
        shutil.rmtree(work, ignore_errors=True)


def keep(seed, work, args, why):
    dest = Path(args.out) / "fail"
    dest.mkdir(parents=True, exist_ok=True)
    shutil.copy(work / "t.c", dest / f"{seed}.c")
    (dest / f"{seed}.txt").write_text(why + "\n")
    return why.split(":")[0].split()[0]



def resolve_tool(path):
    """`path` made absolute when it names a file relative to here."""
    return os.path.abspath(path) if os.sep in path else path

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--compiler", default=str(ROOT / "build" / "hcc"))
    ap.add_argument("--clang", default=os.environ.get("CLANG", "clang"))
    ap.add_argument("--seed-start", type=int, default=1)
    ap.add_argument("--count", type=int, default=1000)
    ap.add_argument("--jobs", type=int, default=2)
    ap.add_argument("--gnu", action="store_true")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    # Each seed runs in its own temporary directory, so a relative tool
    # path must be made absolute first; a bare name is left to PATH.
    args.compiler = resolve_tool(args.compiler)
    args.clang = resolve_tool(args.clang)
    Path(args.out).mkdir(parents=True, exist_ok=True)
    totals = {}
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        for seed, status in pool.map(lambda s: one(s, args),
                                     range(args.seed_start, args.seed_start + args.count)):
            totals[status] = totals.get(status, 0) + 1
            if status not in ("agreement", "invalid"):
                print(f"SEED {seed} {status}", flush=True)
    print(totals, flush=True)
    return 1 if set(totals) - {"agreement", "invalid"} else 0


if __name__ == "__main__":
    sys.exit(main())
