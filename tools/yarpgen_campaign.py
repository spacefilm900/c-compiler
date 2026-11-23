#!/usr/bin/env python3
"""YARPGen differential campaign: this compiler against Clang.

YARPGen (https://github.com/intel/yarpgen) is used only as a black-box
program generator. `yarpgen --std=c` writes driver.c, func.c and init.h:
a program free of undefined behaviour that prints a checksum of its
results. For each seed:

1. Clang builds the program at -O0 and at -O2 (the reference). The seed
   counts only if both builds run to completion and print the same thing.
2. This compiler turns driver.c and func.c into LLVM IR (-std=gnu11, since
   the generator emits `__attribute__((aligned))`), Clang links that IR at
   -O0 and at -O2, and each run must print the reference output.

Results go to OUT/results.jsonl, one line per seed; failing seeds keep
their sources in OUT/fail/<seed>/. Re-running skips seeds already in
results.jsonl.

Usage (from the repository root, after `make`):

  python3 tools/yarpgen_campaign.py --yarpgen PATH/yarpgen \\
      --seed-start 1 --count 200 --jobs 4 --out /tmp/yarpgen-campaign
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import resource
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPILE_TIMEOUT = 300


def set_run_limits():
    """Generated programs may keep large arrays on the stack: allow 1 GiB of
    stack, and cap the address space at 4 GiB so one program cannot take
    the machine's memory. Set once in this process (before any worker
    thread starts) and inherited by every child: `preexec_fn` is not safe
    with threads."""
    resource.setrlimit(resource.RLIMIT_STACK, (1 << 30, 1 << 30))
    resource.setrlimit(resource.RLIMIT_AS, (4 << 30, 4 << 30))


def call(cmd, cwd, timeout):
    try:
        proc = subprocess.run(
            cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout,
            errors="replace",
        )
        return proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired:
        return "timeout", "", ""


def tail(text, n=1500):
    return text[-n:]


def run_seed(seed, args):
    work = Path(tempfile.mkdtemp(prefix=f"yg{seed}-"))
    rec = {"seed": seed}
    try:
        code, _, err = call(
            [args.yarpgen, "--std=c", f"--seed={seed}", "-o", str(work)], work, 120
        )
        if code != 0:
            rec.update(status="generation-failed", detail=tail(err))
            return rec
        sources = ["driver.c", "func.c"]
        ref = {}
        for opt in ("O0", "O2"):
            exe = work / f"clang-{opt}"
            code, _, err = call(
                [args.clang, f"-{opt}", "-w", "-mcmodel=medium", *sources, "-o", str(exe)],
                work, COMPILE_TIMEOUT,
            )
            if code != 0:
                rec.update(status="reference-build-failed", detail=tail(err))
                return rec
            code, out, err = call([str(exe)], work, args.timeout)
            if code != 0:
                rec.update(status="reference-run-failed", opt=opt, code=code)
                return rec
            ref[opt] = out
        if ref["O0"] != ref["O2"]:
            rec.update(status="reference-disagreement")
            return rec
        for src in sources:
            code, _, err = call(
                [args.compiler, "-std=gnu11", src, "-o", src + ".ll"],
                work, COMPILE_TIMEOUT,
            )
            if code != 0:
                status = "candidate-crash-or-ice" if code != 1 else "candidate-rejects"
                rec.update(status=status, file=src, code=code, detail=tail(err))
                return rec
        for opt in ("O0", "O2"):
            exe = work / f"hcc-{opt}"
            code, _, err = call(
                [args.clang, f"-{opt}", "-w", "-mcmodel=medium", "driver.c.ll", "func.c.ll",
                 "-o", str(exe)],
                work, COMPILE_TIMEOUT,
            )
            if code != 0:
                rec.update(status="candidate-bad-ir", opt=opt, detail=tail(err))
                return rec
            code, out, err = call([str(exe)], work, args.timeout)
            if code == -9:
                # Killed from outside (memory pressure on a loaded machine):
                # run it once more before calling it a failure.
                code, out, err = call([str(exe)], work, args.timeout)
            if code == "timeout":
                rec.update(status="candidate-timeout", opt=opt)
                return rec
            if code != 0 or out != ref["O0"]:
                rec.update(
                    status="candidate-wrong-output", opt=opt, code=code,
                    expected=tail(ref["O0"], 200), got=tail(out, 200),
                )
                return rec
        rec.update(status="agreement")
        return rec
    finally:
        if rec.get("status") not in (None, "agreement", "reference-disagreement",
                                     "reference-run-failed"):
            dest = Path(args.out) / "fail" / str(seed)
            dest.mkdir(parents=True, exist_ok=True)
            for name in ("driver.c", "func.c", "init.h"):
                if (work / name).is_file():
                    shutil.copy(work / name, dest / name)
        shutil.rmtree(work, ignore_errors=True)



def resolve_tool(path):
    """`path` made absolute when it names a file relative to here."""
    return os.path.abspath(path) if os.sep in path else path

def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--compiler", default=str(ROOT / "build" / "hcc"))
    parser.add_argument("--clang", default=os.environ.get("CLANG", "clang"))
    parser.add_argument("--yarpgen", required=True)
    parser.add_argument("--seed-start", type=int, default=1)
    parser.add_argument("--count", type=int, default=100)
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--timeout", type=int, default=60,
                        help="run timeout in seconds")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    # Each seed runs in its own temporary directory, so a relative tool
    # path must be made absolute first; a bare name is left to PATH.
    args.compiler = resolve_tool(args.compiler)
    args.clang = resolve_tool(args.clang)
    args.yarpgen = resolve_tool(args.yarpgen)

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    results = out / "results.jsonl"
    done = set()
    if results.is_file():
        for line in results.read_text().splitlines():
            if line.strip():
                done.add(json.loads(line)["seed"])
    seeds = [s for s in range(args.seed_start, args.seed_start + args.count)
             if s not in done]
    if seeds:
        set_run_limits()
    totals = {}
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool, \
            results.open("a") as log:
        for rec in pool.map(lambda s: run_seed(s, args), seeds):
            log.write(json.dumps(rec) + "\n")
            log.flush()
            totals[rec["status"]] = totals.get(rec["status"], 0) + 1
            print(f"SEED {rec['seed']} {rec['status']}", flush=True)
    print(json.dumps(totals, indent=2))
    bad = {k: v for k, v in totals.items() if k.startswith("candidate-")}
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
