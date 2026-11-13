#!/usr/bin/env python3
"""Build DOOM (doomgeneric) with this compiler and with Clang, then compare.

Fetch the inputs first:

    scripts/fetch_doomgeneric.sh
    scripts/fetch_freedoom.sh

The program is the pinned doomgeneric tree plus two files of this
repository: the headless backend harness/doom/dg_headless.c and the
fixed-address allocator harness/doom/dg_alloc.c. The backend has no
window, a fake clock, and no input. It plays DEMO1 of freedoom1.wad with -timedemo (one
game tic per frame), hashes every frame, and exits after --frames frames.

For each optimisation level the harness does three graded steps:

1. Per-file: every translation unit is compiled by this compiler
   (-std=gnu11) to LLVM IR, and Clang turns that IR into an object at the
   same level. Each file is recorded as ok, reject (a diagnostic), ice
   (the compiler crashed), bad-ir (Clang refused the IR), or timeout.
2. Mixed: if some files failed, they are replaced by Clang-compiled
   objects so the rest can still be run and compared.
3. Full: if every file compiled, the program is built from this
   compiler's objects only.

The reference is the same sources compiled by Clang (-std=gnu11) at the
same level. DOOM draws a few frames from memory it never initialised (the
first screen wipe, and a handful of stray pixels later), and that memory
holds old heap addresses. dg_alloc.c puts every allocation at a fixed
address, so those frames are the same from run to run and from build to
build; see the comment at the top of that file.

Exit status is 0 only when every requested level built with this compiler
alone and every frame hash matched the reference.

Env: CC_UNDER_TEST, CLANG, DOOMGENERIC_ROOT, DOOM_WAD.
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
DG_ROOT = Path(
    os.environ.get("DOOMGENERIC_ROOT", ROOT / "tests" / "external" / "doomgeneric")
)
SRC = DG_ROOT / "doomgeneric"
WAD = Path(
    os.environ.get("DOOM_WAD", ROOT / "tests" / "external" / "freedoom" / "freedoom1.wad")
)
BACKEND = ROOT / "harness" / "doom" / "dg_headless.c"
ALLOCATOR = ROOT / "harness" / "doom" / "dg_alloc.c"
DEFINES = ["-DNORMALUNIX", "-DLINUX", "-D_DEFAULT_SOURCE"]
DEMO = "demo1"


def sources() -> list[Path]:
    """The upstream Makefile's object list, minus its X11 backend."""
    text = (SRC / "Makefile").read_text()
    match = re.search(r"^SRC_DOOM\s*=\s*(.*)$", text, re.M)
    if not match:
        raise SystemExit("cannot find SRC_DOOM in the doomgeneric Makefile")
    names = [n for n in match.group(1).split() if n != "doomgeneric_xlib.o"]
    return [SRC / (n[:-2] + ".c") for n in names] + [BACKEND, ALLOCATOR]


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


def clang_obj(src: Path, obj: Path, opt: str) -> dict:
    cmd = [CLANG, "-std=gnu11", f"-{opt}", "-w", *DEFINES, "-I", str(SRC), "-c", str(src), "-o", str(obj)]
    rc, err, secs = run(cmd, 300)
    return {"file": src.name, "status": "ok" if rc == 0 else "clang-fail", "seconds": secs, "stderr": err[-2000:]}


def ours_obj(src: Path, obj: Path, opt: str) -> dict:
    ll = obj.with_suffix(".ll")
    cmd = [str(COMPILER), "-std=gnu11", *DEFINES, "-I", str(SRC), str(src), "-o", str(ll)]
    rc, err, front = run(cmd, 300)
    result = {"file": src.name, "frontend_seconds": front, "backend_seconds": 0.0}
    if rc != 0:
        if rc == 124:
            status = "timeout"
        elif rc == 101 or "panicked" in err:
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
    rc, err, _ = run([CLANG, "-o", str(exe), *map(str, objs), "-lm"], 300)
    return rc, err


def play(exe: Path, log: Path, frames: int, timeout: float) -> dict:
    home = Path(tempfile.mkdtemp(prefix="doom-home-"))
    cmd = [str(exe), "-iwad", str(WAD), "-timedemo", DEMO, "-nogui", "--frames", str(frames), "--log", str(log)]
    env = dict(os.environ, HOME=str(home))
    try:
        rc, err, secs = run(cmd, timeout, env=env, cwd=str(home))
    finally:
        shutil.rmtree(home, ignore_errors=True)
    return {"rc": rc, "seconds": secs, "stderr": err[-2000:]}


def read_log(log: Path) -> tuple[list[str], str | None]:
    if not log.is_file():
        return [], None
    hashes: list[str] = []
    total = None
    for line in log.read_text().splitlines():
        parts = line.split()
        if len(parts) == 3 and parts[0] == "total":
            total = parts[2]
        elif len(parts) == 2:
            hashes.append(parts[1])
    return hashes, total


def compare(ref: list[str], got: list[str]) -> dict:
    same = 0
    first_bad = None
    for i, h in enumerate(ref):
        if i < len(got) and got[i] == h:
            same += 1
        elif first_bad is None:
            first_bad = i + 1
    return {"reference_frames": len(ref), "frames": len(got), "matching": same, "first_mismatch": first_bad}


def level(opt: str, files: list[Path], out: Path, frames: int, jobs: int) -> dict:
    ref_dir = out / f"clang-{opt}"
    our_dir = out / f"ours-{opt}"
    for d in (ref_dir, our_dir):
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)

    def obj_for(d: Path, src: Path) -> Path:
        return d / (src.stem + ".o")

    with ThreadPoolExecutor(max_workers=jobs) as pool:
        ref = list(pool.map(lambda s: clang_obj(s, obj_for(ref_dir, s), opt), files))
        ours = list(pool.map(lambda s: ours_obj(s, obj_for(our_dir, s), opt), files))
    bad_ref = [r["file"] for r in ref if r["status"] != "ok"]
    if bad_ref:
        raise SystemExit(f"clang failed to compile {bad_ref}")

    result: dict = {"opt": opt, "files": ours}
    result["clang_compile_seconds"] = sum(r["seconds"] for r in ref)
    result["ours_frontend_seconds"] = sum(r["frontend_seconds"] for r in ours)
    result["ours_compile_seconds"] = sum(r["frontend_seconds"] + r["backend_seconds"] for r in ours)
    failed = [r["file"] for r in ours if r["status"] != "ok"]
    result["failed_files"] = failed

    ref_exe = ref_dir / "doom"
    rc, err = link([obj_for(ref_dir, s) for s in files], ref_exe)
    if rc != 0:
        raise SystemExit(f"clang reference failed to link:\n{err}")
    ref_log = ref_dir / "frames.log"
    ref_run = play(ref_exe, ref_log, frames, 600)
    ref_hashes, ref_total = read_log(ref_log)
    if ref_run["rc"] != 0 or ref_total is None:
        raise SystemExit(f"clang reference run failed (rc {ref_run['rc']}):\n{ref_run['stderr']}")
    result["clang_run_seconds"] = ref_run["seconds"]
    result["reference_total"] = ref_total

    mixed = bool(failed)
    result["build"] = "mixed" if mixed else "full"
    objs = [obj_for(ref_dir if s.name in failed else our_dir, s) for s in files]
    exe = our_dir / "doom"
    rc, err = link(objs, exe)
    if rc != 0:
        result.update(link_ok=False, link_stderr=err[-2000:])
        return result
    result["link_ok"] = True
    log = our_dir / "frames.log"
    got_run = play(exe, log, frames, 600)
    hashes, total = read_log(log)
    result["ours_run_seconds"] = got_run["seconds"]
    result["run_rc"] = got_run["rc"]
    result["run_stderr"] = got_run["stderr"] if got_run["rc"] != 0 else ""
    result["total"] = total
    result["compare"] = compare(ref_hashes, hashes)
    result["match"] = total == ref_total and got_run["rc"] == 0 and hashes == ref_hashes
    return result


def summary(res: dict) -> str:
    c = res.get("compare", {})
    lines = [
        f"{res['opt']}: build={res['build']} failed_files={len(res['failed_files'])}"
        f" frames={c.get('frames', 0)}/{c.get('reference_frames', 0)}"
        f" matching={c.get('matching', 0)} first_mismatch={c.get('first_mismatch')}"
        f" match={'yes' if res.get('match') else 'no'}",
        f"    compile: ours {res['ours_compile_seconds']:.1f}s (frontend {res['ours_frontend_seconds']:.1f}s)"
        f" clang {res['clang_compile_seconds']:.1f}s;"
        f" run: ours {res.get('ours_run_seconds', 0):.2f}s clang {res['clang_run_seconds']:.2f}s",
    ]
    for f in res["files"]:
        if f["status"] != "ok":
            first = (f.get("stderr") or "").strip().splitlines()
            lines.append(f"    {f['status']}: {f['file']}: {first[0] if first else ''}")
    if res.get("run_rc"):
        lines.append(f"    run exited {res['run_rc']}: {res['run_stderr'].strip()[-300:]}")
    return "\n".join(lines)


def metadata() -> dict:
    def out(cmd: list[str]) -> str:
        try:
            return subprocess.run(cmd, capture_output=True, text=True, check=False).stdout.strip()
        except OSError:
            return ""

    pin = DG_ROOT / ".pin"
    return {
        "date": time.strftime("%Y-%m-%d"),
        "compiler_commit": out(["git", "-C", str(ROOT), "rev-parse", "--short", "HEAD"]),
        "doomgeneric_pin": pin.read_text().strip() if pin.is_file() else "",
        "wad": WAD.name,
        "wad_sha256": out(["sha256sum", str(WAD)]).split(" ")[0],
        "clang": out([CLANG, "--version"]).splitlines()[0] if out([CLANG, "--version"]) else "",
        "demo": DEMO,
    }


def yes(flag: bool) -> str:
    return "yes" if flag else "no"


def render_scorecard(payload: dict, title: str, notes: str = "") -> str:
    """A Markdown scorecard in the PLAN.md format for big programs."""
    meta = payload.get("meta", {})
    levels = payload["levels"]
    head = " | ".join(level["opt"] for level in levels)
    rule = " | ".join("---" for _ in levels)

    def row(label: str, cell) -> str:
        return f"| {label} | " + " | ".join(cell(level) for level in levels) + " |"

    def ratio(ours: float, theirs: float) -> str:
        return f"{ours:.2f} s vs {theirs:.2f} s ({ours / theirs:.2f}x)" if theirs else "n/a"

    total = payload["files"]
    lines = [
        f"# {title}",
        "",
        f"- Date: {meta.get('date', '')}",
        f"- Compiler commit: `{meta.get('compiler_commit', '')}`",
        f"- doomgeneric: `{meta.get('doomgeneric_pin', '')}`",
        f"- WAD: `{meta.get('wad', '')}` (SHA-256 `{meta.get('wad_sha256', '')}`), demo `{meta.get('demo', '')}` with `-timedemo`",
        f"- Reference: {meta.get('clang', 'clang')}, `-std=gnu11`, same optimisation level",
        f"- Frames requested: {payload['frames']}; translation units: {total}; parallel jobs: {payload.get('jobs', '')}",
        "",
        f"| | {head} |",
        f"|---| {rule} |",
        row("Builds with this compiler alone", lambda l: yes(l["build"] == "full" and l.get("link_ok", False))),
        row("Files compiled by this compiler", lambda l: f"{total - len(l['failed_files'])} / {total}"),
        row("Build compared", lambda l: l["build"]),
        row("Frames compared", lambda l: str(l.get("compare", {}).get("reference_frames", 0))),
        row("Frames matching Clang", lambda l: str(l.get("compare", {}).get("matching", 0))),
        row("First mismatching frame", lambda l: str(l.get("compare", {}).get("first_mismatch") or "none")),
        row("Matches Clang", lambda l: yes(bool(l.get("match")) and l["build"] == "full")),
        row("Compile time, ours vs Clang", lambda l: ratio(l["ours_compile_seconds"], l["clang_compile_seconds"])),
        row("of which our frontend", lambda l: f"{l['ours_frontend_seconds']:.2f} s"),
        row("Run time, ours vs Clang", lambda l: ratio(l.get("ours_run_seconds", 0.0), l["clang_run_seconds"])),
        "",
        "Compile time is the sum over all files of this compiler plus Clang turning",
        "its IR into an object, against Clang compiling the C directly. Run time is",
        "the whole demo run, including hashing every frame.",
        "",
    ]
    failed = [(level["opt"], f) for level in levels for f in level["files"] if f["status"] != "ok"]
    if failed:
        lines += ["## Files this compiler did not build", ""]
        for opt, f in failed:
            first = (f.get("stderr") or "").strip().splitlines()
            lines.append(f"- {opt} `{f['file']}`: {f['status']}: `{first[0] if first else ''}`")
        lines.append("")
    if notes:
        lines += [notes.rstrip(), ""]
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--opt", action="append", choices=["O0", "O1", "O2", "O3"], help="repeatable; default O0 and O2")
    ap.add_argument("--frames", type=int, default=2100, help="frames to hash (default 2100, one minute of game time)")
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 1)
    ap.add_argument("--out", type=Path, default=ROOT / "build" / "doom")
    ap.add_argument("--json", type=Path, help="write the full results here")
    ap.add_argument("--scorecard", type=Path, help="write a Markdown scorecard here")
    ap.add_argument("--title", default="DOOM scorecard")
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

    for path, hint in ((SRC / "d_main.c", "scripts/fetch_doomgeneric.sh"), (WAD, "scripts/fetch_freedoom.sh")):
        if not path.is_file():
            print(f"missing {path}. Run {hint} first.", file=sys.stderr)
            return 2
    if not COMPILER.is_file():
        print(f"missing {COMPILER}. Run make first.", file=sys.stderr)
        return 2

    files = sources()
    results = []
    ok = True
    for opt in args.opt or ["O0", "O2"]:
        res = level(opt, files, args.out, args.frames, args.jobs)
        print(summary(res), flush=True)
        results.append(res)
        ok = ok and res["build"] == "full" and bool(res.get("match"))
    payload = {
        "meta": metadata(),
        "frames": args.frames,
        "jobs": args.jobs,
        "files": len(files),
        "levels": results,
    }
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(payload, indent=2) + "\n")
    if args.scorecard:
        args.scorecard.parent.mkdir(parents=True, exist_ok=True)
        args.scorecard.write_text(render_scorecard(payload, args.title, notes))
    print("DOOM: PASS" if ok else "DOOM: FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
