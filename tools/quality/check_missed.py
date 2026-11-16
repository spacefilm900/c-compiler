#!/usr/bin/env python3
"""Re-test mutants that the unit tests missed against the main differential
suite (harness/run_tests.py), to see whether the external suites catch them.

cargo-mutants runs only `cargo test`. Most of this compiler's testing is
external, so a mutant the unit tests miss may still be caught by the suites.
For a sample of missed mutants this script copies the tree to a scratch
directory, applies the mutant's diff (from cargo-mutants' outcomes.json),
builds a release compiler, and runs the suite against it.

Usage: python3 tools/quality/check_missed.py --mutants-out DIR/mutants.out
           [--sample 20] --scratch /tmp/mutcheck --csv OUT.csv
"""
import argparse, csv, json, os, shutil, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent

def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--mutants-out", required=True)
    ap.add_argument("--sample", type=int, default=20)
    ap.add_argument("--scratch", required=True)
    ap.add_argument("--csv", required=True)
    a = ap.parse_args()
    mo = Path(a.mutants_out)
    outcomes = json.load(open(mo / "outcomes.json"))["outcomes"]
    missed = [o for o in outcomes if o["summary"] == "MissedMutant"]
    step = max(1, len(missed) // a.sample)
    sample = missed[::step][:a.sample]
    scratch = Path(a.scratch)
    if scratch.exists():
        shutil.rmtree(scratch)
    scratch.mkdir(parents=True)
    for item in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "src", "include", "harness", "tests"]:
        s = ROOT / item
        (shutil.copytree(s, scratch / item, symlinks=True) if s.is_dir() else shutil.copy2(s, scratch / item))
    pristine = scratch / "src.orig"
    shutil.copytree(scratch / "src", pristine)
    rows = []
    for o in sample:
        shutil.rmtree(scratch / "src"); shutil.copytree(pristine, scratch / "src")
        diff = (mo / o["diff_path"]).read_text()
        name = diff.split("\n", 2)[1][4:]
        p = subprocess.run(["patch", "-p0", "-s"], input=diff, text=True, cwd=scratch, capture_output=True)
        if p.returncode:
            rows.append({"mutant": name, "result": "patch failed", "detail": p.stderr[:200]}); continue
        b = subprocess.run(["cargo", "build", "--release", "-q"], cwd=scratch, capture_output=True, text=True)
        if b.returncode:
            rows.append({"mutant": name, "result": "build failed", "detail": b.stderr[-200:]}); continue
        env = dict(os.environ, CC_UNDER_TEST=str(scratch / "target/release/compiler"))
        t = subprocess.run([sys.executable, "harness/run_tests.py"], cwd=scratch, env=env, capture_output=True, text=True)
        last = (t.stdout.strip().split("\n") or [""])[-1]
        rows.append({"mutant": f"{o['scenario']['Mutant']['file']}:{o['scenario']['Mutant']['span']['start']['line']}: {name}",
                     "result": "caught by suite" if t.returncode else "missed by suite too", "detail": last})
        print(rows[-1]["result"], "|", rows[-1]["mutant"], "|", last, flush=True)
    with open(a.csv, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=["mutant", "result", "detail"]); w.writeheader(); w.writerows(rows)
    c = sum(r["result"] == "caught by suite" for r in rows)
    print(f"{c}/{len(rows)} sampled unit-test survivors are caught by harness/run_tests.py")

if __name__ == "__main__":
    main()
