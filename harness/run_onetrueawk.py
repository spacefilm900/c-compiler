#!/usr/bin/env python3
"""Build One True AWK with this compiler and with Clang, then compare.

scripts/build_onetrueawk.sh generates the parser with bison, builds
maketab with the host compiler, and compiles the awk sources at -O0.
The test is the upstream testdir plus bugs-fixed. testdir compares two
awk binaries. bugs-fixed compares each program's output. CI fails when
the two builds disagree, or when this compiler cannot build awk.

Env: CC_UNDER_TEST, CLANG, OTAWK_ROOT.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = Path(os.environ.get("OTAWK_ROOT", ROOT / "tests" / "external" / "onetrueawk"))
BUILD = ROOT / "scripts" / "build_onetrueawk.sh"


def build(mode: str, dest: Path) -> subprocess.CompletedProcess[str]:
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    return subprocess.run([str(BUILD), mode, str(dest)], text=True, capture_output=True)


def bugs_fixed(binary: Path) -> dict[str, tuple[int, bytes, bytes]]:
    """Run each bugs-fixed/*.awk the way that directory's REGRESS does.

    An optional `.in` file is an awk input operand, not stdin.
    """
    out: dict[str, tuple[int, bytes, bytes]] = {}
    for src in sorted((SRC / "bugs-fixed").glob("*.awk")):
        data = src.with_suffix(".in")
        cmd = [str(binary), "-f", str(src)]
        if data.is_file():
            cmd.append(str(data))
        proc = subprocess.run(cmd, capture_output=True, timeout=30)
        out[src.name] = (proc.returncode, proc.stdout, proc.stderr)
    return out


def testdir_compare(clang_awk: Path, our_awk: Path) -> tuple[int, str]:
    """Use the upstream testdir driver with oldawk=clang and awk=ours."""
    env = os.environ.copy()
    env["oldawk"] = str(clang_awk)
    env["awk"] = str(our_awk)
    env["PATH"] = str(SRC / "testdir") + os.pathsep + env.get("PATH", "")
    proc = subprocess.run(
        [str(SRC / "testdir" / "REGRESS")],
        cwd=SRC / "testdir",
        text=True,
        capture_output=True,
        env=env,
        timeout=180,
    )
    text = proc.stdout + proc.stderr
    return proc.returncode, text


def main() -> int:
    if not SRC.is_dir():
        sys.exit(f"One True AWK not found: {SRC}. Run scripts/fetch_onetrueawk.sh first.")
    if not (ROOT / "build" / "compiler").is_file() and not os.environ.get("CC_UNDER_TEST"):
        sys.exit("build/compiler not found. Run make first.")

    with tempfile.TemporaryDirectory(prefix="otawk-") as tmp:
        work = Path(tmp)
        clang_dir = work / "clang"
        our_dir = work / "ccomp"
        clang = build("clang", clang_dir)
        if clang.returncode != 0:
            sys.stdout.write(clang.stdout)
            sys.stderr.write(clang.stderr)
            sys.exit("clang failed to build One True AWK")
        ours = build("ccomp", our_dir)
        sys.stdout.write(ours.stdout)
        sys.stderr.write(ours.stderr)
        if ours.returncode != 0:
            print("\nOne True AWK did not build with this compiler.")
            print("The diagnostics above are language gaps, not edits to the awk sources.")
            return 1

        clang_awk = clang_dir / "awk"
        our_awk = our_dir / "awk"
        code, text = testdir_compare(clang_awk, our_awk)
        bad = [line for line in text.splitlines() if "BAD" in line]
        if bad or code != 0:
            print("testdir disagreed with the clang-built awk:")
            print("\n".join(bad[:40]))
            if code != 0:
                print(f"testdir REGRESS exited {code}")
            return 1
        print("testdir: clang-built awk and this compiler's awk agree")

        try:
            clang_bugs = bugs_fixed(clang_awk)
            our_bugs = bugs_fixed(our_awk)
        except subprocess.TimeoutExpired as err:
            print(f"bugs-fixed timed out: {err}")
            return 1
        disagree = []
        for name in sorted(set(clang_bugs) | set(our_bugs)):
            if clang_bugs.get(name) != our_bugs.get(name):
                disagree.append(name)
        if disagree:
            print("bugs-fixed disagreed:")
            for name in disagree:
                print(f"  {name}")
            return 1
        print(f"bugs-fixed: {len(clang_bugs)} programs agree")
    return 0


if __name__ == "__main__":
    sys.exit(main())
