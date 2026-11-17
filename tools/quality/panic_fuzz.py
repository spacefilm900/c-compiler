#!/usr/bin/env python3
"""Find crash points that bad input can reach.

Feeds the compiler C files and simple token-level mutations of them (drop,
duplicate or swap a token, drop a line, truncate, insert a random token)
and records every Rust panic ("panicked at src/FILE.rs:LINE"), stack
overflow, or death by signal. A correct compiler gives a diagnostic and a
non-zero exit for bad input, never a crash.

Inputs are only fed to the compiler as test inputs; nothing here inspects
another compiler. Mutations are reproducible from --seed.

Usage: python3 tools/quality/panic_fuzz.py --compiler build/compiler
           --corpus tests --corpus tests/external/wacc/tests
           [--mutants-per-file 20] [--stack 1] [--arg -std=gnu11]
           [--seed 1] [--jobs 8] [--timeout 5]
           --out OUTDIR
Writes OUTDIR/sites.txt (src/file.rs:LINE, one per line, for
crash_points.py --confirmed), OUTDIR/crashes.csv (site, count, smallest
reproducer) and OUTDIR/repro/ (the smallest input per site).
"""
import argparse, csv, os, random, re, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

TOK = re.compile(r'"(?:\\.|[^"\\\n])*"|\'(?:\\.|[^\'\\\n])*\'|[A-Za-z_]\w*|\d[\w.]*|\.\.\.|<<=|>>=|->|\+\+|--|<<|>>|<=|>=|==|!=|&&|\|\||[-+*/%&|^]=|##|\S')
INSERTS = ["(", ")", "{", "}", "[", "]", ";", ",", "*", "&", "=", ":", "?", "...", "int", "struct",
           "union", "enum", "typedef", "sizeof", "_Alignof", "_Generic", "_Static_assert", "static",
           "const", "volatile", "restrict", "inline", "_Noreturn", "long", "double", "void", "char",
           "unsigned", "_Bool", "_Complex", "_Atomic", "_Thread_local", "register", "goto", "case",
           "default", "return", "#", "##", "#define", "#if", "#endif", "0", "1", "0x", "1e", "'a'",
           "\"s\"", "L'x'", "u8\"x\"", "\\", "@", "x", "->", ".", "__attribute__", "asm", "__builtin_va_arg",
           "\"", "'", "/*", "\\\n", "??=", "%:", "<:", "0x1p", "1.e+", "\\u00", "#include", "#line 0", "__VA_ARGS__",
           "\udcff"]  # \udcff is written as the invalid UTF-8 byte 0xFF

def mutate(text, rng):
    toks = [(m.start(), m.end()) for m in TOK.finditer(text)]
    op = rng.choice(["drop", "dup", "swap", "line", "trunc", "insert", "insert", "drop"])
    if not toks:
        return text
    i = rng.randrange(len(toks))
    s, e = toks[i]
    if op == "drop":
        return text[:s] + text[e:]
    if op == "dup":
        return text[:e] + " " + text[s:e] + text[e:]
    if op == "swap" and i + 1 < len(toks):
        s2, e2 = toks[i + 1]
        return text[:s] + text[s2:e2] + text[e:s2] + text[s:e] + text[e2:]
    if op == "line":
        lines = text.split("\n")
        del lines[rng.randrange(len(lines))]
        return "\n".join(lines)
    if op == "trunc":
        return text[:s]
    return text[:s] + rng.choice(INSERTS) + " " + text[s:]

PANIC = re.compile(r"panicked at (?:\S*/)?(src/\w+\.rs):(\d+):\d+")

def run(compiler, src, timeout, extra):
    with tempfile.TemporaryDirectory() as d:
        c = Path(d) / "t.c"
        c.write_text(src, errors="surrogateescape")
        try:
            p = subprocess.run([compiler, *extra, str(c), "-o", str(Path(d) / "t.ll")],
                               capture_output=True, timeout=timeout, cwd=d)
        except subprocess.TimeoutExpired:
            return "timeout", None
        err = p.stderr.decode(errors="replace")
        m = PANIC.search(err)
        if m:
            return "panic", f"{m.group(1)}:{m.group(2)}"
        if "panicked at" in err:
            m2 = re.search(r"panicked at ([^\n]+)", err)
            return "panic", "other:" + (m2.group(1).split(":")[0] if m2 else "?")
        if "overflowed its stack" in err:
            return "stackoverflow", "stack-overflow"
        if p.returncode < 0:
            return "signal", f"signal-{-p.returncode}"
        return "ok", None

def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--compiler", required=True)
    ap.add_argument("--corpus", action="append", required=True)
    ap.add_argument("--mutants-per-file", type=int, default=20)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    ap.add_argument("--timeout", type=float, default=5)
    ap.add_argument("--stack", type=int, default=1, help="apply 1..STACK mutations per variant")
    ap.add_argument("--arg", action="append", default=[], help="extra compiler argument, e.g. -std=gnu11")
    ap.add_argument("--max-bytes", type=int, default=20000, help="skip larger seed files")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    files = sorted({p for c in a.corpus for p in Path(c).rglob("*.c") if p.stat().st_size <= a.max_bytes})
    rng = random.Random(a.seed)
    jobs = []
    for f in files:
        text = f.read_text(errors="surrogateescape")
        inc = os.path.dirname(os.path.abspath(f))
        extra = ["-I", inc, *a.arg]
        jobs.append((str(f), "orig", text, extra))
        for k in range(a.mutants_per_file):
            v = text
            for _ in range(rng.randint(1, a.stack)):
                v = mutate(v, rng)
            jobs.append((str(f), f"m{k}", v, extra))
    print(f"{len(files)} seed files, {len(jobs)} runs", flush=True)
    comp = os.path.abspath(a.compiler)
    found = {}
    counts = {}
    with ThreadPoolExecutor(a.jobs) as ex:
        for (f, tag, text, extra), (status, site) in zip(jobs, ex.map(lambda j: run(comp, j[2], a.timeout, j[3]), jobs)):
            if status == "ok":
                continue
            if status == "timeout":
                site = "timeout"  # a hang (or a slow machine); kept with a reproducer
            counts[site] = counts.get(site, 0) + 1
            if site not in found or len(text) < len(found[site][2]):
                found[site] = (f, tag, text)
    out = Path(a.out); (out / "repro").mkdir(parents=True, exist_ok=True)
    with open(out / "crashes.csv", "w", newline="") as fh:
        w = csv.writer(fh); w.writerow(["site", "hits", "seed_file", "variant", "repro"])
        for site in sorted(found, key=lambda s: -counts[s]):
            f, tag, text = found[site]
            name = re.sub(r"[^\w.-]", "_", site) + ".c"
            (out / "repro" / name).write_text(text, errors="surrogateescape")
            w.writerow([site, counts[site], f, tag, "repro/" + name])
    (out / "sites.txt").write_text("".join(s + "\n" for s in sorted(found) if s.startswith("src/")))
    crashes = [s for s in found if s != "timeout"]
    print(f"runs {len(jobs)}, timeouts {counts.get('timeout', 0)}, distinct crash sites {len(crashes)}")
    for site in sorted(found, key=lambda s: -counts[s]):
        print(f"  {counts[site]:5}  {site}")

if __name__ == "__main__":
    main()
