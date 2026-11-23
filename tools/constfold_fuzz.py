#!/usr/bin/env python3
"""Constant-folding differential fuzzer (original; hardening, PLAN.md step 14).

Each generated program evaluates one random arithmetic expression twice:
once as a static initializer, which this compiler folds at compile time,
and once at run time with every constant read through a `volatile`
object, so the code generator and LLVM compute it. Clang (the black-box
oracle) builds the same program with -fsanitize=undefined; a program with
undefined behaviour, or one Clang rejects as not constant, is discarded.
The candidate must print what Clang prints, at -O0 and -O2.

Usage: python3 tools/constfold_fuzz.py --count 2000 --jobs 4 --out DIR
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

INT_TYPES = [
    "_Bool", "char", "signed char", "unsigned char", "short",
    "unsigned short", "int", "unsigned", "long", "unsigned long",
    "long long", "unsigned long long",
]
FLOAT_TYPES = ["float", "double"]
BIN_INT = ["+", "-", "*", "/", "%", "<<", ">>", "&", "|", "^",
           "<", ">", "<=", ">=", "==", "!=", "&&", "||"]
BIN_FLT = ["+", "-", "*", "/", "<", ">", "==", "!=", "&&", "||"]
INT_LITS = ["0", "1", "2", "3", "7", "31", "32", "63", "127", "128", "255",
            "256", "32767", "32768", "65535", "65536", "0x7fffffff",
            "0x80000000", "0xffffffff", "0x100000000", "-1", "-128",
            "0x7fffffffffffffff", "0x8000000000000000",
            "0xffffffffffffffff", "1u", "1ul", "1ll", "-1ll", "'a'",
            "'\\377'", "100000", "-2147483647"]
FLT_LITS = ["0.0", "0.5", "1.5", "-2.25", "1e10", "3.0f", "-0.0",
            "255.9", "65535.5", "4294967295.0", "1e-3", "2147483647.0",
            "9.2233720368547758e18", "0.1f"]


class Gen:
    def __init__(self, rng):
        self.rng = rng
        self.leaves = []  # (type, literal)

    def leaf(self, floating):
        lit = self.rng.choice(FLT_LITS if floating else INT_LITS)
        idx = len(self.leaves)
        ty = "double" if floating else "long long"
        if lit.endswith("ull") or lit.startswith("0x") and len(lit) > 18:
            ty = "unsigned long long"
        if lit in ("0xffffffffffffffff", "0x8000000000000000"):
            ty = "unsigned long long"
        self.leaves.append((ty, lit))
        return lit, idx

    def expr(self, depth):
        """Return (const_text, runtime_text)."""
        r = self.rng.random()
        if depth <= 0 or r < 0.2:
            floating = self.rng.random() < 0.2
            lit, idx = self.leaf(floating)
            # The runtime form casts the volatile back to the literal's own
            # type, so both forms have the same type.
            return f"({lit})", f"(({type_of(lit)})v{idx})"
        if r < 0.35:
            ty = self.rng.choice(INT_TYPES + FLOAT_TYPES)
            c, v = self.expr(depth - 1)
            return f"(({ty}){c})", f"(({ty}){v})"
        if r < 0.45:
            op = self.rng.choice(["-", "~", "!", "+"])
            c, v = self.expr(depth - 1)
            return f"({op}{c})", f"({op}{v})"
        if r < 0.55:
            a, b, cc = (self.expr(depth - 1) for _ in range(3))
            return (f"({a[0]} ? {b[0]} : {cc[0]})", f"({a[1]} ? {b[1]} : {cc[1]})")
        op = self.rng.choice(BIN_INT)
        a = self.expr(depth - 1)
        b = self.expr(depth - 1)
        return f"({a[0]} {op} {b[0]})", f"({a[1]} {op} {b[1]})"


def type_of(lit):
    """The C type of a literal from the lists above (LP64)."""
    if lit.startswith("'"):
        return "int"
    if "." in lit or "e" in lit and not lit.startswith("0x"):
        return "float" if lit.endswith("f") else "double"
    neg = lit.startswith("-")
    body = lit[1:] if neg else lit
    suffix = ""
    while body and body[-1] in "uUlL":
        suffix = body[-1].lower() + suffix
        body = body[:-1]
    value = int(body, 0)
    hexa = body.startswith("0x")
    unsigned = "u" in suffix
    longs = suffix.count("l")
    order = [("int", 31, False), ("unsigned", 32, True), ("long", 63, False),
             ("unsigned long", 64, True)]
    if longs == 2:
        order = [("long long", 63, False), ("unsigned long long", 64, True)]
    elif longs == 1:
        order = order[2:]
    for name, bits, is_unsigned in order:
        if unsigned and not is_unsigned:
            continue
        if is_unsigned and not (unsigned or hexa):
            continue
        if value < (1 << bits):
            return name
    return "unsigned long long"


def program(seed):
    rng = random.Random(seed)
    gen = Gen(rng)
    const, runtime = gen.expr(rng.randint(2, 5))
    result = rng.choice(INT_TYPES + ["double"])
    vols = "\n".join(
        f"    volatile {type_of(lit)} v{i} = {lit};" for i, (_, lit) in enumerate(gen.leaves)
    )
    fmt = '"%.17g %.17g\\n", (double)g, (double)r' if result == "double" else \
        '"%llx %llx\\n", (unsigned long long)g, (unsigned long long)r'
    return f"""#include <stdio.h>
static {result} g = ({result})({const});
int main(void) {{
{vols}
    {result} r = ({result})({runtime});
    printf({fmt});
    return 0;
}}
"""


def run(cmd, cwd, timeout=60):
    try:
        p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True,
                           timeout=timeout, errors="replace")
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return "timeout", "", ""


def one(seed, args):
    work = Path(tempfile.mkdtemp(prefix=f"cf{seed}-"))
    try:
        src = work / "t.c"
        src.write_text(program(seed))
        code, _, _ = run([args.clang, "-O0", "-w", "-fsanitize=undefined",
                          "-fno-sanitize-recover=all", str(src), "-o", "ref"], work)
        if code != 0:
            return seed, "invalid"
        code, ref, _ = run(["./ref"], work, 20)
        if code != 0:
            return seed, "invalid"
        code, _, err = run([args.compiler, str(src), "-o", "t.ll"], work)
        if code != 0:
            status = "rejects" if code == 1 else "crash"
            return seed, keep(seed, work, args, f"{status}: {err[-500:]}")
        for opt in ("-O0", "-O2"):
            code, _, err = run([args.clang, opt, "-w", "t.ll", "-o", "c"], work)
            if code != 0:
                return seed, keep(seed, work, args, f"bad-ir {opt}: {err[-500:]}")
            code, out, _ = run(["./c"], work, 20)
            if code != 0 or out != ref:
                return seed, keep(seed, work, args, f"wrong {opt}: want {ref!r} got {out!r}")
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
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    # Each seed runs in its own temporary directory, so a relative tool
    # path must be made absolute first; a bare name is left to PATH.
    args.compiler = resolve_tool(args.compiler)
    args.clang = resolve_tool(args.clang)
    Path(args.out).mkdir(parents=True, exist_ok=True)
    totals = {}
    seeds = range(args.seed_start, args.seed_start + args.count)
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        for seed, status in pool.map(lambda s: one(s, args), seeds):
            totals[status] = totals.get(status, 0) + 1
            if status not in ("agreement", "invalid"):
                print(f"SEED {seed} {status}", flush=True)
    print(totals)
    return 1 if set(totals) - {"agreement", "invalid"} else 0


if __name__ == "__main__":
    sys.exit(main())
