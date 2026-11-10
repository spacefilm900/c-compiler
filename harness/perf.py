#!/usr/bin/env python3
"""Time this compiler against clang -O0 on a fixed set.

Compile time stops at LLVM IR on both sides: this compiler writes
textual IR, and Clang is timed with `-S -emit-llvm` at `-O0 -std=c11`.
Run time is the linked binaries, both produced with `clang -O0`.

Run time is the harness programs, One True AWK's own test suite, and
the Shakespeare volume test when that private tree is present.

The script writes reports/perf.json and prints a markdown table. It
does not fail because a number moved. It fails only when the harness
compiler or clang is missing, or when a harness program does not build.

Env: CC_UNDER_TEST, CLANG, PERF_JSON.
"""

from __future__ import annotations

import json
import os
import platform
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPILER = Path(os.environ.get("CC_UNDER_TEST", ROOT / "build" / "compiler"))
CLANG = os.environ.get("CLANG", "clang")
OUT = Path(os.environ.get("PERF_JSON", ROOT / "reports" / "perf.json"))
FLAGS = (
    "compile: this compiler's IR vs clang -std=c11 -O0 -S -emit-llvm; "
    "run: both binaries linked with clang -O0"
)


def clang_version() -> str:
    proc = subprocess.run([CLANG, "--version"], text=True, capture_output=True, check=False)
    return proc.stdout.splitlines()[0] if proc.stdout else "unknown"


def time_call(cmd: list[str], cwd: Path | None = None, timeout: float = 60) -> tuple[float, int]:
    start = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd,
            cwd=cwd,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            stdin=subprocess.DEVNULL,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return time.perf_counter() - start, 124
    return time.perf_counter() - start, proc.returncode


def compile_harness(work: Path) -> dict:
    sources = sorted((ROOT / "tests").glob("*.c"))
    ours = 0.0
    clang = 0.0
    ours_fail = 0
    clang_fail = 0
    for src in sources:
        dt, rc = time_call([str(COMPILER), str(src), "-o", str(work / f"{src.stem}.ll")])
        ours += dt
        ours_fail += rc != 0
        dt, rc = time_call(
            [
                CLANG,
                "-std=c11",
                "-O0",
                "-w",
                "-S",
                "-emit-llvm",
                str(src),
                "-o",
                str(work / f"{src.stem}.clang.ll"),
            ]
        )
        clang += dt
        clang_fail += rc != 0
    return {
        "files": len(sources),
        "ours_s": round(ours, 4),
        "clang_s": round(clang, 4),
        "ours_fail": ours_fail,
        "clang_fail": clang_fail,
    }


def run_harness(work: Path) -> dict:
    sources = sorted((ROOT / "tests").glob("*.c"))
    ours = 0.0
    clang = 0.0
    ran = 0
    for src in sources:
        ll = work / f"{src.stem}.run.ll"
        mine = work / f"{src.stem}.mine"
        ref = work / f"{src.stem}.ref"
        quiet = {
            "stdout": subprocess.DEVNULL,
            "stderr": subprocess.DEVNULL,
            "stdin": subprocess.DEVNULL,
            "timeout": 60,
            "check": False,
        }
        try:
            if subprocess.run([str(COMPILER), str(src), "-o", str(ll)], **quiet).returncode != 0:
                continue
            if subprocess.run([CLANG, "-O0", "-w", str(ll), "-o", str(mine), "-lm"], **quiet).returncode != 0:
                continue
            if (
                subprocess.run(
                    [CLANG, "-std=c11", "-O0", "-w", str(src), "-o", str(ref), "-lm"],
                    **quiet,
                ).returncode
                != 0
            ):
                continue
        except subprocess.TimeoutExpired:
            continue
        dt, _rc = time_call([str(mine)], timeout=30)
        ours += dt
        dt, _rc = time_call([str(ref)], timeout=30)
        clang += dt
        ran += 1
    return {"files": ran, "ours_s": round(ours, 4), "clang_s": round(clang, 4)}


def time_awk_compile() -> dict:
    script = ROOT / "scripts" / "build_onetrueawk.sh"
    src = ROOT / "tests" / "external" / "onetrueawk"
    if not src.is_dir() or not script.is_file():
        return {"status": "not-fetched"}
    work = ROOT / "build" / "perf-awk"
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    # Generate awkgram.tab.c and proctab.c before the clock starts.
    subprocess.run(
        [str(script), "clang", str(work / "prep")],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        stdin=subprocess.DEVNULL,
        timeout=300,
        check=False,
    )
    shutil.rmtree(work / "prep", ignore_errors=True)
    start = time.perf_counter()
    clang = subprocess.run(
        [str(script), "clang", str(work / "clang")],
        text=True,
        capture_output=True,
        stdin=subprocess.DEVNULL,
        timeout=300,
        check=False,
    )
    clang_s = time.perf_counter() - start
    start = time.perf_counter()
    ours = subprocess.run(
        [str(script), "ccomp", str(work / "ccomp")],
        text=True,
        capture_output=True,
        stdin=subprocess.DEVNULL,
        timeout=300,
        check=False,
    )
    ours_s = time.perf_counter() - start
    return {
        "status": "ok" if clang.returncode == 0 and ours.returncode == 0 else "compile-failed",
        "clang_s": round(clang_s, 4),
        "ours_s": round(ours_s, 4),
        "clang_rc": clang.returncode,
        "ours_rc": ours.returncode,
        "clang_awk": str(work / "clang" / "awk"),
        "ours_awk": str(work / "ccomp" / "awk"),
        "ours_stderr": ours.stderr[-500:],
    }


def time_awk_suite(clang_awk: Path, our_awk: Path) -> dict:
    src = ROOT / "tests" / "external" / "onetrueawk"
    if not (src / "testdir" / "REGRESS").is_file():
        return {"status": "not-fetched"}

    def suite(binary: Path) -> tuple[float, int]:
        env = os.environ.copy()
        env["oldawk"] = str(binary)
        env["awk"] = str(binary)
        start = time.perf_counter()
        try:
            proc = subprocess.run(
                [str(src / "testdir" / "REGRESS")],
                cwd=src / "testdir",
                env=env,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                stdin=subprocess.DEVNULL,
                timeout=180,
                check=False,
            )
            code = proc.returncode
        except subprocess.TimeoutExpired:
            return time.perf_counter() - start, 124
        # bugs-fixed/REGRESS wants ../a.out. Time the same programs directly.
        for awk in sorted((src / "bugs-fixed").glob("*.awk")):
            cmd = [str(binary), "-f", str(awk)]
            data = awk.with_suffix(".in")
            if data.is_file():
                cmd.append(str(data))
            try:
                subprocess.run(
                    cmd,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    stdin=subprocess.DEVNULL,
                    timeout=30,
                    check=False,
                )
            except subprocess.TimeoutExpired:
                return time.perf_counter() - start, 124
        return time.perf_counter() - start, code

    row: dict = {"status": "ok", "runner": "testdir/REGRESS and bugs-fixed/*.awk"}
    if clang_awk.is_file():
        clang_s, clang_rc = suite(clang_awk)
        row["clang_s"] = round(clang_s, 4)
        row["clang_testdir_rc"] = clang_rc
    else:
        row["clang_s"] = None
    if our_awk.is_file():
        our_s, our_rc = suite(our_awk)
        row["ours_s"] = round(our_s, 4)
        row["ours_testdir_rc"] = our_rc
    else:
        row["ours_s"] = None
        row["status"] = "clang-only"
    return row


def time_volume(clang_awk: Path, our_awk: Path) -> dict:
    volume = ROOT / "tests" / "external" / "awk-volume-tests"
    if not (volume / "run.sh").is_file():
        return {"status": "not-available"}
    rows = {}
    for label, binary in (("clang", clang_awk), ("ours", our_awk)):
        if not binary.is_file():
            rows[label] = {"status": "no-binary"}
            continue
        env = {**os.environ, "AWKS": "onetrue", "ONETRUE_CMD": str(binary)}
        start = time.perf_counter()
        try:
            proc = subprocess.run(
                ["./run.sh"],
                cwd=volume,
                env=env,
                stdin=subprocess.DEVNULL,
                timeout=180,
                check=False,
            )
            rows[label] = {"s": round(time.perf_counter() - start, 4), "rc": proc.returncode}
        except subprocess.TimeoutExpired:
            rows[label] = {"s": round(time.perf_counter() - start, 4), "rc": 124}
    rows["status"] = "ok"
    return rows


def markdown(report: dict) -> str:
    c = report["compile"]["harness"]
    a = report["compile"]["onetrueawk"]
    r = report["run"]["harness"]
    suite = report["run"]["onetrueawk_suite"]
    vol = report["run"]["volume"]
    lines = [
        "| Measurement | This compiler | Clang -O0 |",
        "| --- | --- | --- |",
        f"| Harness compile ({c['files']} files) | {c['ours_s']} s | {c['clang_s']} s |",
    ]
    if a.get("status") == "ok":
        lines.append(f"| One True AWK compile | {a['ours_s']} s | {a['clang_s']} s |")
    else:
        lines.append(
            f"| One True AWK compile | {a.get('status', 'n/a')} | {a.get('clang_s', 'n/a')} s |"
        )
    lines.append(f"| Harness run ({r['files']} programs) | {r['ours_s']} s | {r['clang_s']} s |")
    if suite.get("clang_s") is not None:
        ours_cell = suite["ours_s"] if suite.get("ours_s") is not None else "did not build"
        lines.append(f"| One True AWK test suite | {ours_cell} | {suite['clang_s']} s |")
    else:
        lines.append("| One True AWK test suite | not run | not run |")
    if vol.get("status") == "ok" and "ours" in vol and "s" in vol.get("ours", {}):
        lines.append(
            f"| Shakespeare volume | {vol['ours']['s']} s | {vol['clang']['s']} s |"
        )
    else:
        lines.append("| Shakespeare volume | not available in this run | not available in this run |")
    return "\n".join(lines)


def main() -> int:
    if not COMPILER.is_file():
        print(f"compiler not found: {COMPILER}")
        return 1
    if shutil.which(CLANG) is None:
        print(f"clang not found: {CLANG}")
        return 1
    with tempfile.TemporaryDirectory(prefix="perf-") as tmp:
        work = Path(tmp)
        harness_c = compile_harness(work)
        if harness_c["ours_fail"] or harness_c["clang_fail"]:
            print(f"harness compile failures: {harness_c}")
            return 1
        harness_r = run_harness(work)
    awk_c = time_awk_compile()
    clang_awk = Path(awk_c.get("clang_awk", ""))
    our_awk = Path(awk_c.get("ours_awk", ""))
    suite = time_awk_suite(clang_awk, our_awk) if clang_awk.is_file() else {"status": "not-run"}
    volume = time_volume(clang_awk, our_awk) if clang_awk.is_file() else {"status": "not-run"}
    report = {
        "machine": {
            "uname": " ".join(platform.uname()),
            "system": platform.system(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "clang": clang_version(),
            "note": (
                "CI runs this script on ubuntu-24.04 and uploads reports/perf.json. "
                "A number in STATUS.md from another machine is labeled as such."
            ),
        },
        "flags": FLAGS,
        "compile": {"harness": harness_c, "onetrueawk": awk_c},
        "run": {"harness": harness_r, "onetrueawk_suite": suite, "volume": volume},
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    table = markdown(report)
    print(table)
    print(f"\nwrote {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
