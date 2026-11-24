#!/usr/bin/env python3
"""Summarise an lcov file from `cargo llvm-cov --lcov`, counting non-test
code only (lines inside #[cfg(test)] items and test-only modules are
dropped, using rust_scan), so a file's own unit-test module does not
inflate its coverage.

Prints per-file line, function and (when the lcov has BRDA records, as from
`cargo +nightly llvm-cov --branch`) branch coverage, then the same rolled up
per module (src/sema.rs and src/sema/*.rs are module "sema"). Writes, with --regions,
the largest runs of uncovered lines per file (a run may skip lines that
carry no counter, such as blank lines and comments), tagged with the
enclosing function.

Usage: python3 tools/quality/lcov_summary.py LCOV [LCOV...] [--root .] [--csv OUT]
           [--regions OUT.csv] [--top 15] [--files sema.rs,parser.rs,irgen.rs]
           [--modules-csv OUT.csv]
"""
import argparse, csv, os, re, sys
from collections import defaultdict
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rust_scan

def parse(paths):
    """Parse and merge lcov files: a line or function counts as hit if any
    file hits it (hit counts are summed)."""
    recs, cur = {}, None
    for line in (l for p in paths for l in open(p)):
        line = line.rstrip("\n")
        if line.startswith("SF:"):
            cur = recs.setdefault(line[3:], {"lines": defaultdict(int), "fns": {}, "fnda": defaultdict(int),
                                             "br": defaultdict(int)})
        elif line.startswith("DA:") and cur is not None:
            ln, hits = line[3:].split(",")[:2]
            cur["lines"][int(ln)] += int(hits)
        elif line.startswith("FN:") and cur is not None:
            ln, name = line[3:].split(",", 1)
            cur["fns"][name] = int(ln)
        elif line.startswith("BRDA:") and cur is not None:
            # BRDA:line,block,branch,taken ("-" means the block never ran).
            ln, blk, br, taken = line[5:].split(",")[:4]
            cur["br"][(int(ln), blk, br)] += 0 if taken == "-" else int(taken)
        elif line.startswith("FNDA:") and cur is not None:
            hits, name = line[5:].split(",", 1)
            cur["fnda"][name] += int(hits)
    return recs

def pct(hit, n):
    """Percentage to one decimal place; an empty set counts as fully covered."""
    return round(100 * hit / n, 1) if n else 100.0

def module_of(rel):
    """Module name of a source path: src/sema.rs and src/sema/x.rs are "sema"."""
    parts = rel.split("/")
    return parts[1] if len(parts) > 2 else parts[-1][:-3]

def module_rows(rows):
    """Roll per-file rows up into per-module rows, largest module first."""
    acc = {}
    for r in rows:
        m = acc.setdefault(module_of(r["file"]), dict.fromkeys(
            ("lines", "lines_hit", "functions", "functions_hit", "branches", "branches_hit"), 0))
        for k in m:
            m[k] += r[k]
    out = []
    for name, m in acc.items():
        out.append({"file": name, "lines": m["lines"], "lines_hit": m["lines_hit"],
                    "line_pct": pct(m["lines_hit"], m["lines"]),
                    "functions": m["functions"], "functions_hit": m["functions_hit"],
                    "function_pct": pct(m["functions_hit"], m["functions"]),
                    "branches": m["branches"], "branches_hit": m["branches_hit"],
                    "branch_pct": pct(m["branches_hit"], m["branches"])})
    return sorted(out, key=lambda r: -r["lines"])

def print_table(label, rows):
    print(f"{label:26} {'lines':>6} {'hit':>6} {'line%':>6} {'fns':>5} {'hit':>5} {'fn%':>6} {'brs':>6} {'hit':>6} {'br%':>6}")
    for r in rows:
        print(f"{r['file']:26} {r['lines']:6} {r['lines_hit']:6} {r['line_pct']:6} {r['functions']:5} "
              f"{r['functions_hit']:5} {r['function_pct']:6} {r['branches']:6} {r['branches_hit']:6} {r['branch_pct']:6}")

def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("lcov", nargs="+"); ap.add_argument("--root", default=".")
    ap.add_argument("--csv"); ap.add_argument("--regions")
    ap.add_argument("--top", type=int, default=15)
    ap.add_argument("--files", default="sema.rs,parser.rs,irgen.rs")
    ap.add_argument("--modules-csv")
    a = ap.parse_args()
    src = os.path.join(a.root, "src")
    sources = {"src/" + f for f in rust_scan.source_files(src)}
    rows, regions = [], []
    tl = th = tf = tfh = tb = tbh = 0
    for path, r in sorted(parse(a.lcov).items()):
        rel = os.path.relpath(path, os.path.abspath(a.root))
        if rel not in sources:
            continue
        raw = open(os.path.join(a.root, rel)).read()
        clean = rust_scan.strip(raw)
        spans = rust_scan.test_spans(clean)
        fnstarts = sorted((clean.count("\n", 0, m.start()) + 1, m.group(1))
                          for m in re.finditer(r"\bfn\s+(\w+)", clean))
        lines = {l: h for l, h in r["lines"].items() if not rust_scan.in_spans(l, spans)}
        # A generic or inlined fn may appear under several mangled names: group by line.
        fnl = defaultdict(int)
        for name, ln in r["fns"].items():
            if not rust_scan.in_spans(ln, spans):
                fnl[ln] += r["fnda"].get(name, 0)
        n, h = len(lines), sum(1 for v in lines.values() if v > 0)
        fn, fh = len(fnl), sum(1 for v in fnl.values() if v > 0)
        brs = {k: v for k, v in r["br"].items() if not rust_scan.in_spans(k[0], spans)}
        b, bh = len(brs), sum(1 for v in brs.values() if v > 0)
        tl += n; th += h; tf += fn; tfh += fh; tb += b; tbh += bh
        rows.append({"file": rel, "lines": n, "lines_hit": h,
                     "line_pct": pct(h, n),
                     "functions": fn, "functions_hit": fh,
                     "function_pct": pct(fh, fn),
                     "branches": b, "branches_hit": bh, "branch_pct": pct(bh, b)})
        # uncovered runs
        run = []
        for l in sorted(lines):
            if lines[l] == 0:
                if run and any(x in lines and lines[x] > 0 for x in range(run[-1] + 1, l)):
                    regions.append((rel, run)); run = []
                run.append(l)
            elif run:
                regions.append((rel, run)); run = []
        if run: regions.append((rel, run))
        for i, (f, rr) in enumerate(regions):
            if f == rel and isinstance(rr, list):
                name = ""
                for s, nm in fnstarts:
                    if s <= rr[0]: name = nm
                    else: break
                regions[i] = (f, (rr[0], rr[-1], len(rr), name))
    rows.sort(key=lambda r: -r["lines"])
    total = {"file": "TOTAL", "lines": tl, "lines_hit": th, "line_pct": pct(th, tl),
             "functions": tf, "functions_hit": tfh, "function_pct": pct(tfh, tf),
             "branches": tb, "branches_hit": tbh, "branch_pct": pct(tbh, tb)}
    print_table("file", rows + [total])
    if a.csv:
        with open(a.csv, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=list(total.keys())); w.writeheader(); w.writerows(rows)
            w.writerow(total)
    mods = module_rows(rows)
    print()
    print_table("module", mods + [dict(total, file="TOTAL")])
    if a.modules_csv:
        with open(a.modules_csv, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=list(total.keys())); w.writeheader(); w.writerows(mods)
            w.writerow(total)
    # A name in --files also selects the files of its module directory:
    # "sema.rs" covers src/sema.rs and src/sema/*.rs.
    picked = a.files.split(",")
    want = {r["file"] for r in rows
            if any(r["file"] == "src/" + f or r["file"].startswith("src/" + f[:-3] + "/") for f in picked)}
    sel = sorted((x for x in regions if x[0] in want), key=lambda x: -x[1][2])
    if a.regions:
        with open(a.regions, "w", newline="") as fh:
            w = csv.writer(fh); w.writerow(["file", "first_line", "last_line", "uncovered_lines", "function"])
            for f, (s, e, k, nm) in sorted(regions, key=lambda x: -x[1][2]):
                if k >= 3: w.writerow([f, s, e, k, nm])
    for f in sorted(want):
        print(f"\nLargest uncovered runs in {f}:")
        for _, (s, e, k, nm) in [x for x in sel if x[0] == f][:a.top]:
            print(f"  {f}:{s}-{e}  {k} lines  in {nm}")

if __name__ == "__main__":
    main()
