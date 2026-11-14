#!/usr/bin/env python3
"""Repeatable compile-time benchmark: this compiler against Clang.

For every translation unit in the chosen input groups it measures

* this compiler's frontend (C to LLVM IR text), split into the phases
  that `--time-phases` reports: lex, preprocess, parse, sema, irgen and
  write (writing the IR file);
* the LLVM backend: `clang -O<n> -c` turning that IR into an object;
* Clang compiling the same C file straight to an object at `-O<n>`, with
  the same language mode and macros;

plus each process's peak resident memory. Every TU is measured `--runs`
times and the median of each number is kept. Group figures are sums of
the per-TU medians (peak memory is the largest per-TU median).

The frontend does not take `-O`, so it is measured once per run and
counted at every level, like the milestone scorecards.

Inputs (all optional, a group is skipped when its tree is missing):
  sqlite3     tests/external/sqlite/*/sqlite3.c
  shell       tests/external/sqlite/*/shell.c
  lua         the Lua 5.4.7 release build's TUs (lua and luac)
  doom        the doomgeneric TUs the DOOM harness builds
  torture     five of the largest passing gcc.c-torture/execute files

Usage:
  tools/bench_compile.py [--runs 3] [--opt O0 --opt O2] [--input NAME ...]
                         [--json OUT.json] [--md OUT.md] [--label TEXT]

Env: CC_UNDER_TEST (default build/compiler), CLANG (default clang).
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXT = ROOT / "tests" / "external"
COMPILER = os.environ.get("CC_UNDER_TEST", str(ROOT / "build" / "compiler"))
CLANG = os.environ.get("CLANG", "clang")
PHASES = ["lex", "preprocess", "parse", "sema", "irgen", "write"]
GROUPS = ["sqlite3", "shell", "lua", "doom", "torture"]
TORTURE = ["strlen-5.c", "pr92904.c", "pr58574.c", "memset-2.c", "builtin-bitops-1.c"]


def sqlite_dir() -> Path | None:
    hits = sorted((EXT / "sqlite").glob("sqlite-amalgamation-*"))
    return hits[-1] if hits else None


def makefile_list(path: Path, var: str) -> list[str]:
    match = re.search(rf"^{var}\s*=\s*(.*)$", path.read_text(), re.M)
    if not match:
        raise SystemExit(f"cannot find {var} in {path}")
    return match.group(1).split()


def group_units(name: str) -> list[tuple[Path, list[str]]]:
    """(source, flags) for each TU of a group; flags include -std."""
    if name in ("sqlite3", "shell"):
        src = sqlite_dir()
        if src is None:
            return []
        flags = [
            "-std=gnu11",
            "-D_DEFAULT_SOURCE",
            "-DSQLITE_THREADSAFE=0",
            "-DSQLITE_ENABLE_FTS5",
            "-DSQLITE_ENABLE_RTREE",
            "-DSQLITE_ENABLE_MATH_FUNCTIONS",
            "-I",
            str(src),
        ]
        return [(src / f"{name}.c", flags)]
    if name == "lua":
        src = EXT / "lua" / "lua-5.4.7" / "src"
        if not (src / "Makefile").exists():
            return []
        mk = src / "Makefile"
        objs = []
        for var in ("CORE_O", "LIB_O", "LUA_O", "LUAC_O"):
            objs += makefile_list(mk, var)
        flags = ["-std=gnu11", "-DLUA_COMPAT_5_3", "-DLUA_USE_LINUX", "-I", str(src)]
        return [(src / (o[:-2] + ".c"), flags) for o in objs]
    if name == "doom":
        src = EXT / "doomgeneric" / "doomgeneric"
        if not (src / "Makefile").exists():
            return []
        objs = [o for o in makefile_list(src / "Makefile", "SRC_DOOM") if o != "doomgeneric_xlib.o"]
        files = [src / (o[:-2] + ".c") for o in objs]
        files += [ROOT / "harness" / "doom" / "dg_headless.c", ROOT / "harness" / "doom" / "dg_alloc.c"]
        flags = ["-std=gnu11", "-DNORMALUNIX", "-DLINUX", "-D_DEFAULT_SOURCE", "-I", str(src)]
        return [(f, flags) for f in files]
    if name == "torture":
        src = EXT / "gcc-torture" / "gcc" / "testsuite" / "gcc.c-torture" / "execute"
        if not src.is_dir():
            return []
        return [(src / f, ["-std=gnu11"]) for f in TORTURE]
    raise SystemExit(f"unknown input group {name}")


def measure(cmd: list[str]) -> tuple[float, float, str]:
    """Wall seconds, peak RSS in MiB, and stderr of one process."""
    start = time.perf_counter()
    proc = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    err = proc.stderr.read().decode(errors="replace") if proc.stderr else ""
    _, status, usage = os.wait4(proc.pid, 0)
    wall = time.perf_counter() - start
    proc.returncode = os.waitstatus_to_exitcode(status)
    if proc.returncode != 0:
        raise RuntimeError(f"failed ({proc.returncode}): {' '.join(cmd)}\n{err[-2000:]}")
    return wall, usage.ru_maxrss / 1024.0, err


def phases_from(err: str) -> dict[str, float]:
    for line in err.splitlines():
        if line.startswith("time-phases: "):
            return json.loads(line[len("time-phases: ") :])
    raise RuntimeError("no time-phases line; is the compiler new enough?")


def bench_unit(src: Path, flags: list[str], opts: list[str], runs: int, work: Path) -> dict:
    ll = work / (src.stem + ".ll")
    obj = work / (src.stem + ".o")
    samples: dict[str, list[float]] = {}

    def add(key: str, value: float) -> None:
        samples.setdefault(key, []).append(value)

    for _ in range(runs):
        wall, rss, err = measure([COMPILER, "--time-phases", *flags, str(src), "-o", str(ll)])
        add("front_wall", wall)
        add("front_rss", rss)
        for phase, secs in phases_from(err).items():
            add(f"phase_{phase}", secs)
        for opt in opts:
            wall, rss, _ = measure([CLANG, f"-{opt}", "-w", "-c", str(ll), "-o", str(obj)])
            add(f"{opt}_back_wall", wall)
            add(f"{opt}_back_rss", rss)
            wall, rss, _ = measure([CLANG, f"-{opt}", "-w", *flags, "-c", str(src), "-o", str(obj)])
            add(f"{opt}_clang_wall", wall)
            add(f"{opt}_clang_rss", rss)
    result = {k: statistics.median(v) for k, v in samples.items()}
    result["ir_bytes"] = ll.stat().st_size
    result["ir_lines"] = sum(1 for _ in ll.open("rb"))
    return result


def summarize(units: dict[str, dict], opts: list[str]) -> dict:
    total: dict[str, float] = {}
    for r in units.values():
        for k, v in r.items():
            if k.endswith("_rss"):
                total[k] = max(total.get(k, 0.0), v)
            else:
                total[k] = total.get(k, 0.0) + v
    for opt in opts:
        ours = total["front_wall"] + total[f"{opt}_back_wall"]
        total[f"{opt}_ours_wall"] = ours
        total[f"{opt}_ratio"] = ours / total[f"{opt}_clang_wall"]
    return total


def clang_version() -> str:
    out = subprocess.run([CLANG, "--version"], capture_output=True, text=True, check=False).stdout
    return out.splitlines()[0] if out else "unknown"


def git_head() -> str:
    out = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, capture_output=True, text=True)
    return out.stdout.strip() or "unknown"


def markdown(data: dict) -> str:
    opts = data["opts"]
    lines = [
        f"# Compile-time benchmark: {data['label']}",
        "",
        f"- Date: {data['date']}",
        f"- Compiler commit: `{data['commit']}`",
        f"- Reference: {data['clang']}",
        f"- Machine: {data['machine']}",
        f"- Runs per TU: {data['runs']} (median kept)",
        "",
        "Ours = this compiler's frontend plus `clang -O<n> -c` on its IR. Clang = Clang compiling the C",
        "directly at the same level. Group rows are sums of per-TU medians; peak memory is the largest TU.",
        "",
    ]
    head = "| Input | TUs | Frontend | " + " | ".join(
        f"{o} backend | {o} ours | {o} Clang | {o} ratio" for o in opts
    ) + " | Front peak | Clang peak | IR size |"
    lines += [head, "|" + "---|" * (head.count("|") - 1)]
    for name, g in data["groups"].items():
        t = g["total"]
        cells = [name, str(len(g["units"])), f"{t['front_wall']:.2f} s"]
        for o in opts:
            cells += [
                f"{t[f'{o}_back_wall']:.2f} s",
                f"{t[f'{o}_ours_wall']:.2f} s",
                f"{t[f'{o}_clang_wall']:.2f} s",
                f"{t[f'{o}_ratio']:.2f}x",
            ]
        clang_peak = max(t[f"{o}_clang_rss"] for o in opts)
        cells += [f"{t['front_rss']:.1f} MiB", f"{clang_peak:.1f} MiB", f"{t['ir_bytes'] / 1e6:.1f} MB"]
        lines.append("| " + " | ".join(cells) + " |")
    lines += ["", "## Frontend phases (seconds)", ""]
    lines.append("| Input | " + " | ".join(PHASES) + " |")
    lines.append("|---|" + "---|" * len(PHASES))
    for name, g in data["groups"].items():
        t = g["total"]
        lines.append(f"| {name} | " + " | ".join(f"{t.get('phase_' + p, 0.0):.2f}" for p in PHASES) + " |")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--opt", action="append", choices=["O0", "O1", "O2", "O3"])
    ap.add_argument("--input", action="append", choices=GROUPS)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--md", type=Path)
    ap.add_argument("--label", default="run")
    args = ap.parse_args()
    opts = args.opt or ["O0", "O2"]
    data = {
        "label": args.label,
        "date": time.strftime("%Y-%m-%d"),
        "commit": git_head(),
        "compiler": COMPILER,
        "clang": clang_version(),
        "machine": f"{platform.machine()}, {os.cpu_count()} CPUs, {platform.platform()}",
        "runs": args.runs,
        "opts": opts,
        "groups": {},
    }
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        for name in args.input or GROUPS:
            units = group_units(name)
            if not units:
                print(f"{name}: skipped (sources missing)", file=sys.stderr)
                continue
            results = {}
            for src, flags in units:
                results[src.name] = bench_unit(src, flags, opts, args.runs, work)
                r = results[src.name]
                print(
                    f"{name}/{src.name}: front {r['front_wall']:.2f}s "
                    + " ".join(f"{o} back {r[f'{o}_back_wall']:.2f}s clang {r[f'{o}_clang_wall']:.2f}s" for o in opts),
                    file=sys.stderr,
                    flush=True,
                )
            data["groups"][name] = {"units": results, "total": summarize(results, opts)}
    text = markdown(data)
    print(text)
    if args.json:
        args.json.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")
    if args.md:
        args.md.write_text(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
