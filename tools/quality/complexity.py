#!/usr/bin/env python3
"""Function length, nesting and cyclomatic complexity for Rust code in src/.

Written because rust-code-analysis-cli 0.0.25 would not build on the box (its
tree-sitter C++ grammar needs C++ standard headers that are not installed).
This is a textual approximation, standard library only:

  * comments and string literals are blanked first (rust_scan.strip);
  * a function is `fn name ... { ... }` with brace matching; closures and
    nested items count towards the enclosing function, and a nested `fn`
    is also reported on its own;
  * length = lines from the `fn` line to the closing brace, inclusive;
  * nesting = deepest brace level inside the body (the body itself is 1);
  * cyclomatic complexity (McCabe) = 1 + `if` + `while` + `for` +
    (match arms - 1 per `match`) + `&&` + `||`.  `if let` / `while let`
    count as `if` / `while`; `?` is not counted.
Test code (#[cfg(test)] items and test-only modules) is excluded unless
--include-tests is given.

Usage: python3 tools/quality/complexity.py [--src src] [--csv FUNCS.csv]
           [--files FILES.csv] [--top 25]
"""
import argparse, csv, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rust_scan

FN = re.compile(r"\bfn\s+(\w+)")

def functions(clean):
    for m in FN.finditer(clean):
        # body starts at the first '{' at paren/bracket/angle depth 0 before a ';'
        i, par = m.end(), 0
        while i < len(clean):
            ch = clean[i]
            if ch in "([": par += 1
            elif ch in ")]": par -= 1
            elif ch == ";" and par == 0: i = -1; break
            elif ch == "{" and par == 0: break
            i += 1
        if i < 0 or i >= len(clean):
            continue  # declaration without body (trait item, extern)
        d, j, deepest = 0, i, 0
        while j < len(clean):
            if clean[j] == "{":
                d += 1; deepest = max(deepest, d)
            elif clean[j] == "}":
                d -= 1
                if d == 0: break
            j += 1
        body = clean[i:j + 1]
        start = clean.count("\n", 0, m.start()) + 1
        end = clean.count("\n", 0, j) + 1
        arms = len(re.findall(r"=>", body))
        matches = len(re.findall(r"\bmatch\b", body))
        cc = (1 + len(re.findall(r"\bif\b", body)) + len(re.findall(r"\bwhile\b", body))
              + len(re.findall(r"\bfor\b(?!\s*<)", body)) + max(0, arms - matches)
              + body.count("&&") + body.count("||"))
        yield {"name": m.group(1), "start": start, "end": end,
               "lines": end - start + 1, "nesting": deepest, "cyclomatic": cc}

def scan(src, include_tests=False):
    lib = os.path.join(src, "lib.rs")
    test_mods = rust_scan.test_only_modules(rust_scan.strip(open(lib).read())) if os.path.exists(lib) else set()
    funcs, files = [], []
    for fname in sorted(os.listdir(src)):
        if not fname.endswith(".rs"):
            continue
        is_test_mod = fname[:-3] in test_mods
        if is_test_mod and not include_tests:
            continue
        raw = open(os.path.join(src, fname)).read()
        clean = rust_scan.strip(raw)
        spans = [] if include_tests else rust_scan.test_spans(clean)
        test_lines = sum(b - a + 1 for a, b, _ in spans)
        fs = [f for f in functions(clean) if not rust_scan.in_spans(f["start"], spans)]
        for f in fs:
            f["file"] = "src/" + fname
        funcs += fs
        total = raw.count("\n") + (0 if raw.endswith("\n") else 1)
        files.append({"file": "src/" + fname, "total_lines": total,
                      "non_test_lines": total - test_lines, "functions": len(fs),
                      "max_fn_lines": max((f["lines"] for f in fs), default=0),
                      "max_cyclomatic": max((f["cyclomatic"] for f in fs), default=0),
                      "fns_over_100": sum(f["lines"] > 100 for f in fs),
                      "fns_over_200": sum(f["lines"] > 200 for f in fs)})
    return funcs, files

def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--src", default="src")
    ap.add_argument("--csv"); ap.add_argument("--files")
    ap.add_argument("--top", type=int, default=25)
    ap.add_argument("--include-tests", action="store_true")
    a = ap.parse_args()
    funcs, files = scan(a.src, a.include_tests)
    if a.csv:
        with open(a.csv, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=["file", "name", "start", "end", "lines", "nesting", "cyclomatic"])
            w.writeheader()
            for f in sorted(funcs, key=lambda f: (f["file"], f["start"])): w.writerow({k: f[k] for k in w.fieldnames})
    if a.files:
        with open(a.files, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=list(files[0].keys()))
            w.writeheader(); w.writerows(sorted(files, key=lambda r: -r["non_test_lines"]))
    n = len(funcs)
    print(f"functions: {n}; over 50 lines: {sum(f['lines']>50 for f in funcs)}; over 100: {sum(f['lines']>100 for f in funcs)}; over 200: {sum(f['lines']>200 for f in funcs)}")
    print(f"cyclomatic over 10: {sum(f['cyclomatic']>10 for f in funcs)}; over 25: {sum(f['cyclomatic']>25 for f in funcs)}; over 50: {sum(f['cyclomatic']>50 for f in funcs)}")
    print(f"\nTop {a.top} by length:")
    for f in sorted(funcs, key=lambda f: -f["lines"])[:a.top]:
        print(f"  {f['lines']:5} lines  cc {f['cyclomatic']:4}  nest {f['nesting']:2}  {f['file']}:{f['start']} {f['name']}")
    print(f"\nTop {a.top} by cyclomatic complexity:")
    for f in sorted(funcs, key=lambda f: -f["cyclomatic"])[:a.top]:
        print(f"  cc {f['cyclomatic']:4}  {f['lines']:5} lines  nest {f['nesting']:2}  {f['file']}:{f['start']} {f['name']}")

if __name__ == "__main__":
    main()
