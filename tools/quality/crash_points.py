#!/usr/bin/env python3
"""List crash points in non-test Rust code under src/.

A crash point is a call to .unwrap(), .expect(...), .unwrap_unchecked(),
or a panic!, unreachable!, todo!, unimplemented!, assert!/assert_eq!/
assert_ne! (debug_assert* is excluded: release builds drop it). Comments,
string literals, #[cfg(test)] items and test-only modules are skipped.

Each site gets a rough category:
  a  true invariant: cannot fail for any input (writing to a String,
     a value the same function just checked or built, a phase that
     already guaranteed it)
  b  reachable from bad user input (confirmed by tools/quality/panic_fuzz.py,
     or clearly reached from file, environment or command-line input)
  c  unclear: depends on an earlier phase rejecting bad input, and no
     test shows whether it does

The heuristic rules are in classify(); --confirmed FILE (lines of
"src/file.rs:LINE") marks sites the fuzzer actually reached as b, and
--overrides FILE (CSV: file,line,category,reason) applies hand review.

Usage: python3 tools/quality/crash_points.py [--src src] [--confirmed F]
           [--overrides F] --csv OUT.csv [--summary OUT.md]
"""
import argparse, csv, os, re, sys
from collections import Counter, defaultdict
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rust_scan

PAT = re.compile(r"\.(unwrap|expect|unwrap_unchecked)\s*\(|\b(panic|unreachable|todo|unimplemented|assert|assert_eq|assert_ne)!\s*[\(\[\{]")

FRONT = {"lexer.rs", "pp.rs", "parser.rs"}
MID = {"sema.rs", "ast.rs", "builtins.rs"}
BACK = {"irgen.rs", "abi.rs", "aapcs64.rs", "float.rs", "fp80.rs", "target.rs"}
DRIVER = {"driver.rs", "main.rs", "lib.rs", "timing.rs", "dialect.rs", "diag.rs"}

def fn_names(clean):
    """Map line -> name of the innermost fn that starts at or before it."""
    starts = [(clean.count("\n", 0, m.start()) + 1, m.group(1))
              for m in re.finditer(r"\bfn\s+(\w+)", clean)]
    return starts

def enclosing(starts, line):
    name = ""
    for l, n in starts:
        if l <= line: name = n
        else: break
    return name

def classify(fname, kind, recv_line, raw_line, ctx):
    """Return (category, reason) from simple textual rules."""
    s = recv_line
    if re.search(r"\b(write|writeln)!\s*\(", ctx) and kind in ("unwrap", "expect"):
        return "a", "fmt::Write into a String cannot fail"
    if kind in ("unwrap", "expect") and re.search(r"(from_utf8|to_str\(\))", s) and fname in BACK | MID:
        return "a", "text the compiler built itself"
    if kind in ("unwrap", "expect") and re.search(r"(read_to_string|File::|fs::|env::)", s):
        return "b", "file, environment or argument input"
    if kind in ("unwrap", "expect") and re.search(r"\.(pop|last|last_mut|first|first_mut)\(\)", s) and re.search(r"(scope|stack|frame|blocks|loops|switch|break|cont|cond|parts)", s, re.I):
        return "a", "internal stack the same pass pushes and pops"
    if kind in ("unwrap", "expect") and re.search(r"\b(lock|try_into|try_from)\b", s):
        return "a", "conversion or lock that cannot fail here"
    if kind in ("assert", "assert_eq", "assert_ne"):
        return "a", "assertion on an internal invariant"
    if fname in DRIVER:
        return "c", "driver code: depends on caller"
    if kind == "unreachable":
        if fname in FRONT:
            return "c", "front-end match arm excluded by earlier logic; input-driven"
        return "a", "match arm excluded by the earlier match or phase"
    if kind in ("todo", "unimplemented"):
        return "b", "unfinished feature: any program using it reaches this"
    if fname in BACK:
        if re.search(r"\bty\b|\.ty\b|ty\.|ty\)|\btype", s) or re.search(r"(sema|checked|annotat|typed|resolved)", raw_line, re.I):
            return "a", "sema has typed and checked this node before irgen"
        return "c", "backend relies on sema having rejected the case"
    if fname in MID:
        if re.search(r"(sema|checked|annotat|typed|resolved|complete|always|must)", raw_line, re.I):
            return "a", "message states an invariant established earlier"
        return "c", "sema relies on parser or its own earlier checks"
    if fname in FRONT:
        if re.search(r"(peek|next|chars|bytes)\(\)", s):
            return "c", "token or character lookahead; input-driven"
        return "c", "front end, input-driven"
    return "c", "no rule matched"

def scan(src_dir):
    lib = os.path.join(src_dir, "lib.rs")
    test_mods = rust_scan.test_only_modules(rust_scan.strip(open(lib).read())) if os.path.exists(lib) else set()
    rows = []
    for fname in sorted(os.listdir(src_dir)):
        if not fname.endswith(".rs") or fname[:-3] in test_mods:
            continue
        raw = open(os.path.join(src_dir, fname)).read()
        clean = rust_scan.strip(raw)
        spans = rust_scan.test_spans(clean)
        starts = fn_names(clean)
        raw_lines = raw.split("\n")
        clean_lines = clean.split("\n")
        for m in PAT.finditer(clean):
            line = clean.count("\n", 0, m.start()) + 1
            if rust_scan.in_spans(line, spans):
                continue
            kind = m.group(1) or m.group(2)
            if kind == "expect" and re.search(r"\bself\s*$", clean[max(0, m.start() - 20):m.start()]):
                continue  # the parser's own Result-returning `expect` method
            ctx = "\n".join(clean_lines[max(0, line - 3):line])
            raw_ctx = " ".join(l.strip() for l in raw_lines[line - 1:line + 2])
            cat, why = classify(fname, kind, clean_lines[line - 1], raw_ctx, ctx)
            rows.append({"file": "src/" + fname, "line": line, "kind": kind,
                         "function": enclosing(starts, line), "category": cat,
                         "reason": why, "code": raw_lines[line - 1].strip()[:160]})
    return rows

def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--src", default="src")
    ap.add_argument("--confirmed")
    ap.add_argument("--overrides")
    ap.add_argument("--csv", required=True)
    ap.add_argument("--summary")
    a = ap.parse_args()
    rows = scan(a.src)
    by_key = {(r["file"], r["line"]): r for r in rows}
    if a.overrides and os.path.exists(a.overrides):
        for o in csv.DictReader(open(a.overrides)):
            r = by_key.get((o["file"], int(o["line"])))
            if r:
                r["category"], r["reason"] = o["category"], o["reason"]
    if a.confirmed and os.path.exists(a.confirmed):
        for l in open(a.confirmed):
            l = l.strip()
            if not l or l.startswith("#"): continue
            f, ln = l.rsplit(":", 1)
            r = by_key.get((f, int(ln)))
            if r:
                r["category"], r["reason"] = "b", "reached by panic_fuzz.py (confirmed crash on bad input)"
    with open(a.csv, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=["file", "line", "kind", "function", "category", "reason", "code"])
        w.writeheader(); w.writerows(rows)
    cats = Counter(r["category"] for r in rows)
    kinds = Counter(r["kind"] for r in rows)
    per = defaultdict(Counter)
    for r in rows: per[r["file"]][r["category"]] += 1
    out = ["# Crash points in non-test code", "",
           f"Total: {len(rows)}. By category: a (invariant) {cats['a']}, b (reachable from bad input) {cats['b']}, c (unclear) {cats['c']}.", "",
           "By kind: " + ", ".join(f"{k} {v}" for k, v in kinds.most_common()), "",
           "| File | Total | a | b | c |", "|---|---:|---:|---:|---:|"]
    for f in sorted(per, key=lambda f: -sum(per[f].values())):
        c = per[f]; out.append(f"| {f} | {sum(c.values())} | {c['a']} | {c['b']} | {c['c']} |")
    text = "\n".join(out) + "\n"
    if a.summary:
        open(a.summary, "w").write(text)
    print(text)

if __name__ == "__main__":
    main()
