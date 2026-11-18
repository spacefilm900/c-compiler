#!/usr/bin/env python3
"""Build Lua 5.4 with this compiler and with Clang, then compare.

Fetch the inputs first:

    scripts/fetch_lua.sh

The program is the pinned Lua release (tests/external/lua/lua-5.4.7) and
its official test suite (lua-5.4.7-tests), both from lua.org and checked by
SHA-256. Neither is patched. Two configurations are built:

- release: the interpreter `lua` and the compiler `luac`, as the upstream
  Makefile's `linux` target builds them (LUA_COMPAT_5_3, LUA_USE_LINUX,
  linked with -lm -ldl -Wl,-E; no readline). On macOS it is the
  `macosx` target's configuration instead: LUA_USE_MACOSX, linked with -lm
  (the dynamic loader is in the C library, and Apple's linker exports a
  program's symbols without -E); no readline.
- ltests: the same interpreter with the test suite's internal checks built
  in (LUA_USER_H="ltests.h" plus ltests.c), which turns on Lua's own
  assertions and adds the `T` library the suite uses for white-box tests.

For each optimisation level the harness does three graded steps, per
configuration:

1. Per-file: every translation unit is compiled by this compiler to LLVM
   IR, and Clang turns that IR into an object at the same level. Each file
   is recorded as ok, reject (a diagnostic), ice (the compiler crashed),
   bad-ir (Clang refused the IR), or timeout.
2. Mixed: if some files failed, they are replaced by Clang-compiled
   objects so the rest can still be run and compared.
3. Full: if every file compiled, the program is built from this
   compiler's objects only.

The reference is the same sources compiled by Clang with the same -std and
level. Then, with both builds:

- Test suite, three ways, each in a fresh copy of the test directory:
  `user` is `lua -e "_U=true" all.lua` (the suite's documented mode for
  user builds: soft, portable, no internal tests); `portable` is
  `lua -e "_port=true" all.lua` (every portable test, including the long
  ones); `internal` runs the ltests build with `-e "_port=true"`, so the
  internal `T` tests and Lua's assertions are on. The suite prints random
  seeds and timings, so its output is not compared byte for byte; the
  outcome is: exit status, "final OK !!!", and the list of test files
  reached.
- Workloads: every harness/lua/*.lua script, written for this harness,
  run by both `lua` builds; stdout, stderr and exit status must match
  byte for byte. Run times are recorded.
- Bytecode: `luac -s` of every workload and every test-suite file, by both
  `luac` builds; the output files must be identical.

Exit status is 0 only when every requested level built with this compiler
alone (both configurations) and every comparison matched.

Env: CC_UNDER_TEST, CLANG, LUA_ROOT (default tests/external/lua).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPILER = Path(os.environ.get("CC_UNDER_TEST", ROOT / "build" / "compiler"))
CLANG = os.environ.get("CLANG", "clang")
VERSION = "5.4.7"
LUA_ROOT = Path(os.environ.get("LUA_ROOT", ROOT / "tests" / "external" / "lua"))
SRC = LUA_ROOT / f"lua-{VERSION}" / "src"
TESTS = LUA_ROOT / f"lua-{VERSION}-tests"
WORKLOADS = ROOT / "harness" / "lua"
MACOS = sys.platform == "darwin"
DEFINES = ["-DLUA_COMPAT_5_3", "-DLUA_USE_MACOSX" if MACOS else "-DLUA_USE_LINUX"]
LTESTS_DEFINES = ['-DLUA_USER_H="ltests.h"']
LIBS = ["-lm"] if MACOS else ["-lm", "-ldl", "-Wl,-E"]
SUITES = (
    ("user", "release", "_U=true"),
    ("portable", "release", "_port=true"),
    ("internal", "ltests", "_port=true"),
)
FINAL_OK = "final OK !!!"
FILE_LINE = re.compile(r"\*\*\*\*\* FILE '([^']+)'\*\*\*\*\*")


def makefile_objects(var: str) -> list[str]:
    text = (SRC / "Makefile").read_text()
    match = re.search(rf"^{var}=\s*(.*)$", text, re.M)
    if not match:
        raise SystemExit(f"cannot find {var} in the Lua Makefile")
    return [n[:-2] for n in match.group(1).split()]


def config_files(config: str) -> list[Path]:
    """Translation units of a configuration, in the Makefile's order."""
    base = makefile_objects("CORE_O") + makefile_objects("LIB_O")
    files = [SRC / f"{n}.c" for n in base + makefile_objects("LUA_O")]
    if config == "release":
        return files + [SRC / f"{n}.c" for n in makefile_objects("LUAC_O")]
    return files + [TESTS / "ltests" / "ltests.c"]


def config_flags(config: str) -> list[str]:
    flags = list(DEFINES)
    if config == "ltests":
        flags += LTESTS_DEFINES + ["-I", str(TESTS / "ltests")]
    return flags + ["-I", str(SRC)]


def programs(config: str) -> dict[str, list[str]]:
    """Executable name to the stems of the objects it links."""
    files = [f.stem for f in config_files(config)]
    if config == "release":
        return {"lua": [f for f in files if f != "luac"], "luac": [f for f in files if f != "lua"]}
    return {"lua": files}


def run(cmd: list[str], timeout: float, **kw) -> tuple[int, str, float]:
    start = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            errors="replace",
            stdin=subprocess.DEVNULL,
            timeout=timeout,
            check=False,
            **kw,
        )
        rc, err = proc.returncode, proc.stderr
    except subprocess.TimeoutExpired:
        rc, err = 124, f"timed out after {timeout}s\n"
    return rc, err, time.perf_counter() - start


def run_bytes(cmd: list[str], timeout: float, cwd: Path | None = None) -> dict:
    """Run a program, keeping stdout and stderr as bytes."""
    start = time.perf_counter()
    timed_out = False
    try:
        proc = subprocess.run(
            cmd, capture_output=True, stdin=subprocess.DEVNULL, timeout=timeout, check=False, cwd=cwd
        )
        rc, out, err = proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired as exc:
        timed_out = True
        rc, out, err = 124, exc.stdout or b"", (exc.stderr or b"") + f"timed out after {timeout}s\n".encode()
    return {"rc": rc, "stdout": out, "stderr": err, "seconds": time.perf_counter() - start, "timed_out": timed_out}


def clang_obj(src: Path, obj: Path, opt: str, std: str, flags: list[str]) -> dict:
    cmd = [CLANG, f"-std={std}", f"-{opt}", "-w", *flags, "-c", str(src), "-o", str(obj)]
    rc, err, secs = run(cmd, 300)
    return {"file": src.name, "status": "ok" if rc == 0 else "clang-fail", "seconds": secs, "stderr": err[-2000:]}


def ours_obj(src: Path, obj: Path, opt: str, std: str, flags: list[str]) -> dict:
    ll = obj.with_suffix(".ll")
    cmd = [str(COMPILER), f"-std={std}", *flags, str(src), "-o", str(ll)]
    rc, err, front = run(cmd, 300)
    result = {"file": src.name, "frontend_seconds": front, "backend_seconds": 0.0}
    if rc != 0:
        if rc == 124:
            status = "timeout"
        elif rc < 0 or rc == 101 or "panicked" in err:
            # A negative status is a signal (SIGSEGV, SIGABRT): a crash.
            status = "ice"
        else:
            status = "reject"
        result.update(status=status, stderr=err.replace(f"{ROOT}/", "")[-2000:])
        return result
    rc, err, back = run([CLANG, f"-{opt}", "-w", "-c", str(ll), "-o", str(obj)], 300)
    result["backend_seconds"] = back
    if rc != 0:
        result.update(status="bad-ir", stderr=err[-2000:])
        return result
    result.update(status="ok", stderr="")
    return result


def link(objs: list[Path], exe: Path) -> tuple[int, str]:
    rc, err, _ = run([CLANG, "-o", str(exe), *map(str, objs), *LIBS], 300)
    return rc, err


def build(config: str, opt: str, std: str, out: Path, jobs: int) -> dict:
    """Graded build of one configuration at one level, both compilers."""
    ref_dir = out / f"clang-{opt}-{config}"
    our_dir = out / f"ours-{opt}-{config}"
    for d in (ref_dir, our_dir):
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
    files = config_files(config)
    flags = config_flags(config)
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        ref = list(pool.map(lambda s: clang_obj(s, ref_dir / f"{s.stem}.o", opt, std, flags), files))
        ours = list(pool.map(lambda s: ours_obj(s, our_dir / f"{s.stem}.o", opt, std, flags), files))
    bad_ref = [r["file"] for r in ref if r["status"] != "ok"]
    if bad_ref:
        raise SystemExit(f"clang failed to compile {bad_ref}:\n{ref[0]['stderr']}")
    failed = [r["file"] for r in ours if r["status"] != "ok"]
    result: dict = {
        "config": config,
        "files": ours,
        "failed_files": failed,
        "build": "mixed" if failed else "full",
        "clang_compile_seconds": sum(r["seconds"] for r in ref),
        "ours_frontend_seconds": sum(r["frontend_seconds"] for r in ours),
        "ours_compile_seconds": sum(r["frontend_seconds"] + r["backend_seconds"] for r in ours),
        "ref_dir": str(ref_dir),
        "our_dir": str(our_dir),
    }
    link_ok = True
    for exe, stems in programs(config).items():
        rc, err = link([ref_dir / f"{s}.o" for s in stems], ref_dir / exe)
        if rc != 0:
            raise SystemExit(f"clang reference {exe} failed to link:\n{err}")
        objs = [(ref_dir if f"{s}.c" in failed else our_dir) / f"{s}.o" for s in stems]
        rc, err = link(objs, our_dir / exe)
        if rc != 0:
            link_ok = False
            result.setdefault("link_stderr", "")
            result["link_stderr"] += err[-2000:]
    result["link_ok"] = link_ok
    return result


def suite_outcome(res: dict) -> dict:
    text = res["stdout"].decode("utf-8", "replace") + res["stderr"].decode("utf-8", "replace")
    files = FILE_LINE.findall(text)
    ok = not res.get("timed_out", False) and res["rc"] == 0 and FINAL_OK in text
    tail = "" if ok else text.strip()[-1500:]
    return {
        "rc": res["rc"],
        "final_ok": ok,
        "timed_out": res.get("timed_out", False),
        "files": files,
        "seconds": res["seconds"],
        "tail": tail,
    }


def run_suite(exe: Path, setting: str, timeout: float) -> dict:
    """all.lua in a fresh copy of the test directory (the suite writes files)."""
    work = Path(tempfile.mkdtemp(prefix="lua-tests-"))
    try:
        shutil.copytree(TESTS, work / "t")
        res = run_bytes([str(exe), "-e", setting, "all.lua"], timeout, cwd=work / "t")
    finally:
        shutil.rmtree(work, ignore_errors=True)
    return suite_outcome(res)


def luac_files() -> list[Path]:
    return sorted(WORKLOADS.glob("*.lua")) + sorted(TESTS.glob("*.lua"))


def compare_luac(ref: Path, ours: Path, scratch: Path) -> dict:
    """`luac -s` of every Lua file by both builds. A file matches only when
    both runs exit 0 within the time limit, both write their output, and
    the two outputs are identical; anything else is a difference."""
    scratch.mkdir(parents=True, exist_ok=True)
    same, differ = 0, []
    for i, src in enumerate(luac_files()):
        outs = []
        for tag, exe in (("ref", ref), ("ours", ours)):
            dest = scratch / f"{i}-{tag}.luac"
            dest.unlink(missing_ok=True)
            res = run_bytes([str(exe), "-s", "-o", str(dest), str(src)], 60)
            ok = res["rc"] == 0 and not res["timed_out"] and dest.is_file()
            outs.append(dest.read_bytes() if ok else None)
        if outs[0] is not None and outs[0] == outs[1]:
            same += 1
        else:
            differ.append(src.name)
    return {"files": same + len(differ), "matching": same, "differ": differ}


def level(opt: str, std: str, out: Path, jobs: int, run_jobs: int, suites: list[str]) -> dict:
    builds = {c: build(c, opt, std, out, jobs) for c in ("release", "ltests")}
    result: dict = {"opt": opt, "builds": builds}
    tasks = []
    for index, (name, config, setting) in enumerate(SUITES):
        if name not in suites:
            continue
        b = builds[config]
        pair = [(name, "ref", Path(b["ref_dir"]) / "lua", setting)]
        if b["link_ok"]:
            pair.append((name, "ours", Path(b["our_dir"]) / "lua", setting))
        # Alternate which build runs first (with --run-jobs 1 they run in
        # this order).
        tasks += pair[::-1] if index % 2 == 1 else pair
    with ThreadPoolExecutor(max_workers=max(1, run_jobs)) as pool:
        done = dict(zip([(t[0], t[1]) for t in tasks], pool.map(lambda t: run_suite(t[2], t[3], 1800), tasks)))
    suite_rows = []
    for name, config, setting in SUITES:
        if name not in suites:
            continue
        ref = done[(name, "ref")]
        if not ref["final_ok"]:
            raise SystemExit(f"clang reference failed the {name} suite at {opt}:\n{ref['tail']}")
        ours = done.get((name, "ours"))
        row = {"suite": name, "config": config, "setting": setting, "ref": ref, "ours": ours}
        row["match"] = bool(ours) and ours["final_ok"] and ours["files"] == ref["files"]
        suite_rows.append(row)
    result["suites"] = suite_rows

    rel = builds["release"]
    ref_lua, our_lua = Path(rel["ref_dir"]) / "lua", Path(rel["our_dir"]) / "lua"
    rows = []
    for index, script in enumerate(sorted(WORKLOADS.glob("*.lua"))):
        # Alternate which build runs first, so neither one always runs on a
        # warmer (or busier) machine.
        ours_first = index % 2 == 1 and rel["link_ok"]
        if ours_first:
            ours = run_bytes([str(our_lua), script.name], 600, cwd=WORKLOADS)
        ref = run_bytes([str(ref_lua), script.name], 600, cwd=WORKLOADS)
        row = {
            "workload": script.name,
            "clang_seconds": ref["seconds"],
            "rc": ref["rc"],
            "first": "ours" if ours_first else "clang",
        }
        if rel["link_ok"]:
            if not ours_first:
                ours = run_bytes([str(our_lua), script.name], 600, cwd=WORKLOADS)
            row["ours_seconds"] = ours["seconds"]
            row["ours_rc"] = ours["rc"]
            # A run that timed out never matches, even if both did.
            row["match"] = (
                not ref["timed_out"]
                and not ours["timed_out"]
                and (ours["rc"], ours["stdout"], ours["stderr"]) == (ref["rc"], ref["stdout"], ref["stderr"])
            )
            if not row["match"]:
                row["first_difference"] = first_difference(ref["stdout"] + ref["stderr"], ours["stdout"] + ours["stderr"])
        else:
            row["match"] = False
        row["stdout_bytes"] = len(ref["stdout"])
        rows.append(row)
    result["workloads"] = rows
    if rel["link_ok"]:
        result["luac"] = compare_luac(
            Path(rel["ref_dir"]) / "luac", Path(rel["our_dir"]) / "luac", out / f"luac-{opt}"
        )
    else:
        result["luac"] = {"files": len(luac_files()), "matching": 0, "differ": ["(not built)"]}
    result["match"] = (
        all(b["link_ok"] for b in builds.values())
        and all(r["match"] for r in suite_rows)
        and all(r["match"] for r in rows)
        and not result["luac"]["differ"]
    )
    result["full"] = all(b["build"] == "full" for b in builds.values())
    return result


def first_difference(ref: bytes, got: bytes) -> str:
    a, b = ref.decode("utf-8", "replace").splitlines(), got.decode("utf-8", "replace").splitlines()
    for i in range(max(len(a), len(b))):
        x = a[i] if i < len(a) else "<end>"
        y = b[i] if i < len(b) else "<end>"
        if x != y:
            return f"line {i + 1}: clang {x[:200]!r} / ours {y[:200]!r}"
    return "same lines, different bytes"


def verdict(levels: list[dict]) -> tuple[bool, str]:
    ok = bool(levels) and all(lvl["full"] and lvl["match"] for lvl in levels)
    return ok, "LUA: PASS" if ok else "LUA: FAIL"


def summary(res: dict) -> str:
    lines = [f"{res['opt']}: full={'yes' if res['full'] else 'no'} match={'yes' if res['match'] else 'no'}"]
    for b in res["builds"].values():
        total = len(b["files"])
        lines.append(
            f"    build {b['config']}: {b['build']}, {total - len(b['failed_files'])}/{total} files,"
            f" link={'ok' if b['link_ok'] else 'FAIL'}; compile ours {b['ours_compile_seconds']:.1f}s"
            f" (frontend {b['ours_frontend_seconds']:.1f}s) clang {b['clang_compile_seconds']:.1f}s"
        )
        for f in b["files"]:
            if f["status"] != "ok":
                first = (f.get("stderr") or "").strip().splitlines()
                lines.append(f"      {f['status']}: {f['file']}: {first[0] if first else ''}")
    for s in res["suites"]:
        ours = s["ours"]
        lines.append(
            f"    suite {s['suite']}: clang {'OK' if s['ref']['final_ok'] else 'FAIL'} {s['ref']['seconds']:.2f}s;"
            f" ours {('OK' if ours['final_ok'] else 'FAIL (rc %d)' % ours['rc']) if ours else 'not built'}"
            f" {ours['seconds'] if ours else 0:.2f}s; match={'yes' if s['match'] else 'no'}"
        )
        if ours and not ours["final_ok"]:
            lines.append("      " + ours["tail"][-600:].replace("\n", "\n      "))
    for w in res["workloads"]:
        lines.append(
            f"    workload {w['workload']}: match={'yes' if w['match'] else 'no'}"
            f" ours {w.get('ours_seconds', 0):.2f}s clang {w['clang_seconds']:.2f}s"
            + (f" {w['first_difference']}" if w.get("first_difference") else "")
        )
    lc = res["luac"]
    lines.append(f"    luac -s: {lc['matching']}/{lc['files']} identical" + (f"; differ {lc['differ']}" if lc["differ"] else ""))
    return "\n".join(lines)


def metadata(std: str) -> dict:
    def out(cmd: list[str]) -> str:
        try:
            return subprocess.run(cmd, capture_output=True, text=True, check=False).stdout.strip()
        except OSError:
            return ""

    pin = LUA_ROOT / ".pin"
    parts = pin.read_text().split() if pin.is_file() else ["", "", ""]
    version = out([CLANG, "--version"])
    return {
        "date": time.strftime("%Y-%m-%d"),
        "compiler_commit": out(["git", "-C", str(ROOT), "rev-parse", "--short", "HEAD"]),
        "lua_version": parts[0],
        "lua_sha256": parts[1] if len(parts) > 1 else "",
        "tests_sha256": parts[2] if len(parts) > 2 else "",
        "std": std,
        "clang": version.splitlines()[0] if version else "",
    }


def yes(flag: bool) -> str:
    return "yes" if flag else "no"


def ratio(ours: float, theirs: float) -> str:
    return f"{ours:.2f} s vs {theirs:.2f} s ({ours / theirs:.2f}x)" if theirs else "n/a"


def render_scorecard(payload: dict, title: str, notes: str = "") -> str:
    """A Markdown scorecard in the PLAN.md format for big programs."""
    meta = payload.get("meta", {})
    levels = payload["levels"]
    head = " | ".join(lvl["opt"] for lvl in levels)
    rule = " | ".join("---" for _ in levels)

    def row(label: str, cell) -> str:
        return f"| {label} | " + " | ".join(cell(lvl) for lvl in levels) + " |"

    def files_cell(lvl: dict, config: str) -> str:
        b = lvl["builds"][config]
        return f"{len(b['files']) - len(b['failed_files'])} / {len(b['files'])}"

    def suite_cell(lvl: dict, name: str) -> str:
        s = next((s for s in lvl["suites"] if s["suite"] == name), None)
        if s is None:
            return "not run"
        ours = s["ours"]
        if not ours:
            return "not built"
        state = "OK" if ours["final_ok"] else f"FAIL (exit {ours['rc']}, after {len(ours['files'])} files)"
        return f"{state}; Clang OK"

    def work_cell(lvl: dict) -> str:
        rows = lvl["workloads"]
        return f"{sum(1 for r in rows if r['match'])} / {len(rows)}"

    def suite_time(lvl: dict) -> str:
        ours = sum(s["ours"]["seconds"] for s in lvl["suites"] if s["ours"])
        return ratio(ours, sum(s["ref"]["seconds"] for s in lvl["suites"] if s["ours"]))

    def work_time(lvl: dict) -> str:
        rows = [r for r in lvl["workloads"] if "ours_seconds" in r]
        return ratio(sum(r["ours_seconds"] for r in rows), sum(r["clang_seconds"] for r in rows))

    def compile_time(lvl: dict) -> str:
        bs = lvl["builds"].values()
        return ratio(sum(b["ours_compile_seconds"] for b in bs), sum(b["clang_compile_seconds"] for b in bs))

    lines = [
        f"# {title}",
        "",
        f"- Date: {meta.get('date', '')}",
        f"- Compiler commit: `{meta.get('compiler_commit', '')}`",
        f"- Lua: {meta.get('lua_version', '')} from lua.org (`lua-{meta.get('lua_version', '')}.tar.gz`"
        f" SHA-256 `{meta.get('lua_sha256', '')}`)",
        f"- Test suite: `lua-{meta.get('lua_version', '')}-tests.tar.gz` (SHA-256 `{meta.get('tests_sha256', '')}`)",
        f"- Language mode: `-std={meta.get('std', '')}` for this compiler and for the reference;"
        f" `{' '.join(DEFINES)}`, linked with `{' '.join(LIBS)}`, no readline",
        f"- Reference: {meta.get('clang', 'clang')}, same flags and optimisation level",
        f"- Compile jobs: {payload.get('jobs', '')}; test-suite runs at once: {payload.get('run_jobs', 1)}",
        "",
        f"| | {head} |",
        f"|---| {rule} |",
        row("Builds with this compiler alone (both configurations)", lambda l: yes(l["full"] and all(b["link_ok"] for b in l["builds"].values()))),
        row("Files compiled, release (`lua`, `luac`)", lambda l: files_cell(l, "release")),
        row("Files compiled, ltests (internal checks on)", lambda l: files_cell(l, "ltests")),
        row("Build compared", lambda l: "full" if l["full"] else "mixed"),
        row("Test suite, user mode (`_U=true`)", lambda l: suite_cell(l, "user")),
        row("Test suite, portable mode (`_port=true`)", lambda l: suite_cell(l, "portable")),
        row("Test suite, internal (ltests build)", lambda l: suite_cell(l, "internal")),
        row("Workloads matching Clang byte for byte", work_cell),
        row("`luac -s` output identical", lambda l: f"{l['luac']['matching']} / {l['luac']['files']}"),
        row("Matches Clang", lambda l: yes(l["full"] and l["match"])),
        row("Compile time, ours vs Clang", compile_time),
        row("Test-suite run time, ours vs Clang", suite_time),
        row("Workload run time, ours vs Clang", work_time),
        "",
        "Compile time is the sum over all files of both configurations: this",
        "compiler plus Clang turning its IR into an object, against Clang",
        "compiling the C directly. Run times are wall-clock times of the",
        "three test-suite runs and of all the workloads at that level. The",
        "two builds take turns to run first: Clang first for the user and",
        "internal suites and for every other workload in name order, this",
        "compiler first for the rest.",
        "",
    ]
    lines += workload_table(levels)
    failed = [
        (lvl["opt"], b["config"], f)
        for lvl in levels
        for b in lvl["builds"].values()
        for f in b["files"]
        if f["status"] != "ok"
    ]
    if failed:
        lines += ["## Files this compiler did not build", ""]
        for opt, config, f in failed:
            first = (f.get("stderr") or "").strip().splitlines()
            lines.append(f"- {opt} {config} `{f['file']}`: {f['status']}: `{first[0] if first else ''}`")
        lines.append("")
    if notes:
        lines += [notes.rstrip(), ""]
    return "\n".join(lines)


def workload_table(levels: list[dict]) -> list[str]:
    names = [w["workload"] for w in levels[0]["workloads"]] if levels else []
    if not names:
        return []
    opts = [lvl["opt"] for lvl in levels]
    lines = [
        "## Workloads",
        "",
        "| Script | Output bytes | " + " | ".join(f"{o} matches | {o} ours vs Clang" for o in opts) + " |",
        "|---|---| " + " | ".join("--- | ---" for _ in opts) + " |",
    ]
    for name in names:
        cells = []
        size = ""
        for lvl in levels:
            w = next(r for r in lvl["workloads"] if r["workload"] == name)
            size = str(w["stdout_bytes"])
            cells.append(yes(w["match"]))
            cells.append(f"{w.get('ours_seconds', 0):.2f} s / {w['clang_seconds']:.2f} s")
        lines.append(f"| `{name}` | {size} | " + " | ".join(cells) + " |")
    lines.append("")
    return lines


def strip_dirs(payload: dict) -> None:
    """Drop machine-local paths before saving."""
    for lvl in payload["levels"]:
        for b in lvl["builds"].values():
            b.pop("ref_dir", None)
            b.pop("our_dir", None)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--opt", action="append", choices=["O0", "O1", "O2", "O3"], help="repeatable; default O0 and O2")
    ap.add_argument("--std", default="gnu11", choices=["c11", "gnu11"], help="language mode for both compilers")
    ap.add_argument(
        "--suite",
        action="append",
        choices=[s[0] for s in SUITES],
        help="test-suite mode to run (repeatable; default all three)",
    )
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 1)
    ap.add_argument(
        "--run-jobs",
        type=int,
        default=1,
        help="test-suite runs at once (default 1, so run times are not skewed by sharing the machine)",
    )
    ap.add_argument("--out", type=Path, default=ROOT / "build" / "lua")
    ap.add_argument("--json", type=Path, help="write the full results here")
    ap.add_argument("--scorecard", type=Path, help="write a Markdown scorecard here")
    ap.add_argument("--title", default="Lua scorecard")
    ap.add_argument("--notes", type=Path, help="Markdown appended to the scorecard (defects)")
    ap.add_argument("--from-json", type=Path, help="render --scorecard from saved results; do not run")
    args = ap.parse_args()
    notes = args.notes.read_text() if args.notes else ""

    if args.from_json:
        if not args.scorecard:
            ap.error("--from-json needs --scorecard")
        payload = json.loads(args.from_json.read_text())
        args.scorecard.write_text(render_scorecard(payload, args.title, notes))
        return 0
    args.out = args.out.resolve()

    for path in (SRC / "lua.c", TESTS / "all.lua"):
        if not path.is_file():
            print(f"missing {path}. Run scripts/fetch_lua.sh first.", file=sys.stderr)
            return 2
    if not COMPILER.is_file():
        print(f"missing {COMPILER}. Run make first.", file=sys.stderr)
        return 2

    suites = args.suite or [s[0] for s in SUITES]
    results = []
    for opt in args.opt or ["O0", "O2"]:
        res = level(opt, args.std, args.out, args.jobs, args.run_jobs, suites)
        print(summary(res), flush=True)
        results.append(res)
    payload = {
        "meta": metadata(args.std),
        "jobs": args.jobs,
        "run_jobs": args.run_jobs,
        "levels": results,
    }
    strip_dirs(payload)
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(payload, indent=2) + "\n")
    if args.scorecard:
        args.scorecard.parent.mkdir(parents=True, exist_ok=True)
        args.scorecard.write_text(render_scorecard(payload, args.title, notes))
    ok, text = verdict(results)
    print(text)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
