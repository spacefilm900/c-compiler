#!/usr/bin/env python3
"""Summarise an lcov file from `cargo llvm-cov --lcov`, counting non-test
code only (lines inside #[cfg(test)] items and test-only modules are
dropped, using rust_scan), so a file's own unit-test module does not
inflate its coverage.

Prints per-file line and function coverage and writes, with --regions,
the largest runs of uncovered lines per file (a run may skip lines that
carry no counter, such as blank lines and comments), tagged with the
enclosing function.

Usage: python3 tools/quality/lcov_summary.py LCOV [LCOV...] [--root .] [--csv OUT]
           [--regions OUT.csv] [--top 15] [--files sema.rs,parser.rs,irgen.rs]
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
            cur = recs.setdefault(line[3:], {"lines": defaultdict(int), "fns": {}, "fnda": defaultdict(int)})
        elif line.startswith("DA:") and cur is not None:
            ln, hits = line[3:].split(",")[:2]
            cur["lines"][int(ln)] += int(hits)
        elif line.startswith("FN:") and cur is not None:
            ln, name = line[3:].split(",", 1)
            cur["fns"][name] = int(ln)
        elif line.startswith("FNDA:") and cur is not None:
            hits, name = line[5:].split(",", 1)
            cur["fnda"][name] += int(hits)
    return recs

def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("lcov", nargs="+"); ap.add_argument("--root", default=".")
    ap.add_argument("--csv"); ap.add_argument("--regions")
    ap.add_argument("--top", type=int, default=15)
    ap.add_argument("--files", default="sema.rs,parser.rs,irgen.rs")
    a = ap.parse_args()
    src = os.path.join(a.root, "src")
    test_mods = rust_scan.test_only_modules(rust_scan.strip(open(os.path.join(src, "lib.rs")).read()))
    rows, regions = [], []
    tl = th = tf = tfh = 0
    for path, r in sorted(parse(a.lcov).items()):
        rel = os.path.relpath(path, os.path.abspath(a.root))
        if not rel.startswith("src/") or os.path.basename(rel)[:-3] in test_mods:
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
        tl += n; th += h; tf += fn; tfh += fh
        rows.append({"file": rel, "lines": n, "lines_hit": h,
                     "line_pct": round(100 * h / n, 1) if n else 100.0,
                     "functions": fn, "functions_hit": fh,
                     "function_pct": round(100 * fh / fn, 1) if fn else 100.0})
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
    print(f"{'file':22} {'lines':>6} {'hit':>6} {'line%':>6} {'fns':>5} {'hit':>5} {'fn%':>6}")
    for r in rows:
        print(f"{r['file']:22} {r['lines']:6} {r['lines_hit']:6} {r['line_pct']:6} {r['functions']:5} {r['functions_hit']:5} {r['function_pct']:6}")
    print(f"{'TOTAL':22} {tl:6} {th:6} {round(100*th/tl,1):6} {tf:5} {tfh:5} {round(100*tfh/tf,1):6}")
    if a.csv:
        with open(a.csv, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=list(rows[0].keys())); w.writeheader(); w.writerows(rows)
            w.writerow({"file": "TOTAL", "lines": tl, "lines_hit": th, "line_pct": round(100*th/tl,1),
                        "functions": tf, "functions_hit": tfh, "function_pct": round(100*tfh/tf,1)})
    want = {"src/" + f for f in a.files.split(",")}
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
