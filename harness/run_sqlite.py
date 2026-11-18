#!/usr/bin/env python3
"""Build the SQLite amalgamation with this compiler and with Clang, then compare.

Fetch the inputs first:

    scripts/fetch_sqlite.sh

The program is the pinned SQLite amalgamation (sqlite3.c and shell.c) from
sqlite.org, checked by SHA-256 and SHA3-256. It is not patched. One
configuration is built: the command-line shell, with FTS5, RTree and the
math functions enabled, SQLITE_THREADSAFE=0, and _DEFAULT_SOURCE so the
POSIX declarations the shell uses are visible. It is linked with
-lm -ldl -lpthread, and without readline.

For each optimisation level the harness does three graded steps:

1. Per-file: each translation unit is compiled by this compiler to LLVM
   IR, and Clang turns that IR into an object at the same level. Each file
   is recorded as ok, reject (a diagnostic), ice (the compiler crashed),
   bad-ir (Clang refused the IR), or timeout. Wall time and peak RSS are
   recorded for this compiler's frontend, for Clang turning the IR into an
   object, and for Clang compiling the C directly.
2. Mixed: if some files failed, they are replaced by Clang-compiled
   objects so the rest can still be run and compared.
3. Full: if every file compiled, the program is built from this
   compiler's objects only.

The frontend does not take -O, so its IR is produced once and reused at
every level; the measured frontend time is included at each level. The
two compilers take turns to go first: Clang first at the first level, this
compiler first at the next, and the same alternation on the workloads.

The reference is the same sources compiled by Clang with the same -std,
defines, and level. Then both shells run every harness/sqlite/*.sql
script: stdout, stderr, exit status, and the database file must match
byte for byte. A run that timed out or failed to write its database never
counts as a match, even if both sides did the same thing. Old database
files and captured output are deleted before each run.

Exit status is 0 only when every requested level built with this compiler
alone and every comparison matched.

Env: CC_UNDER_TEST, CLANG, SQLITE_ROOT (default tests/external/sqlite).
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPILER = Path(os.environ.get("CC_UNDER_TEST", ROOT / "build" / "compiler"))
CLANG = os.environ.get("CLANG", "clang")
VERSION = "3.54.1"
AMALGAMATION = "sqlite-amalgamation-3540100"
SQLITE_ROOT = Path(os.environ.get("SQLITE_ROOT", ROOT / "tests" / "external" / "sqlite"))
SRC = SQLITE_ROOT / AMALGAMATION
WORKLOADS = ROOT / "harness" / "sqlite"
SOURCES = ("sqlite3.c", "shell.c")
DEFINES = [
    "-D_DEFAULT_SOURCE",
    "-DSQLITE_THREADSAFE=0",
    "-DSQLITE_ENABLE_FTS5",
    "-DSQLITE_ENABLE_RTREE",
    "-DSQLITE_ENABLE_MATH_FUNCTIONS",
]
LIBS = ["-lm", "-ldl", "-lpthread"]
COMPILE_TIMEOUT = 1800
WORKLOAD_TIMEOUT = 180


def flags() -> list[str]:
    return list(DEFINES) + ["-I", str(SRC)]


def _wait4_main() -> int:
    """Single-threaded helper: fork, exec, wait4, write a result JSON."""
    payload = json.loads(sys.stdin.read())
    cmd = payload["cmd"]
    timeout = float(payload["timeout"])
    cwd = payload.get("cwd")
    out_path = payload["out"]
    err_path = payload["err"]
    start = time.perf_counter()
    timed_out = False
    pid = os.fork()
    if pid == 0:
        try:
            if cwd:
                os.chdir(cwd)
            os.dup2(os.open(out_path, os.O_WRONLY | os.O_TRUNC), 1)
            os.dup2(os.open(err_path, os.O_WRONLY | os.O_TRUNC), 2)
            os.execvp(cmd[0], cmd)
        except OSError:
            os._exit(127)
        os._exit(127)
    deadline = start + timeout
    ru = None
    status = None
    while True:
        wpid, status, ru = os.wait4(pid, os.WNOHANG)
        if wpid == pid:
            break
        if time.perf_counter() >= deadline:
            timed_out = True
            try:
                os.kill(pid, signal.SIGKILL)
            except OSError:
                pass
            _, status, ru = os.wait4(pid, 0)
            break
        time.sleep(0.02)
    if timed_out:
        rc = 124
    elif os.WIFEXITED(status):
        rc = os.WEXITSTATUS(status)
    elif os.WIFSIGNALED(status):
        rc = -os.WTERMSIG(status)
    else:
        rc = 1
    json.dump(
        {
            "rc": rc,
            "seconds": time.perf_counter() - start,
            "timed_out": timed_out,
            # ru_maxrss is in kilobytes on Linux and in bytes on macOS.
            "max_rss_kb": (int(ru.ru_maxrss) // (1024 if sys.platform == "darwin" else 1)
                           if ru is not None else 0),
        },
        sys.stdout,
    )
    return 0


def run_wait4(cmd: list[str], timeout: float, cwd: Path | None = None) -> dict:
    """Run a command, capturing bytes, wall time, and peak RSS via wait4.

    The fork happens in a fresh helper process so it is safe when this
    module is used from a multithreaded test or compile pool.
    """
    out_fd, out_path = tempfile.mkstemp(prefix="cc-sq-out-")
    err_fd, err_path = tempfile.mkstemp(prefix="cc-sq-err-")
    os.close(out_fd)
    os.close(err_fd)
    payload = {
        "cmd": cmd,
        "timeout": timeout,
        "cwd": str(cwd) if cwd else None,
        "out": out_path,
        "err": err_path,
    }
    try:
        proc = subprocess.run(
            [sys.executable, str(Path(__file__).resolve()), "--wait4"],
            input=json.dumps(payload).encode(),
            capture_output=True,
            check=False,
            timeout=timeout + 30,
        )
        meta = json.loads(proc.stdout.decode() or "{}")
    except (subprocess.TimeoutExpired, json.JSONDecodeError):
        meta = {"rc": 124, "seconds": timeout, "timed_out": True, "max_rss_kb": 0}
    stdout = Path(out_path).read_bytes()
    stderr = Path(err_path).read_bytes()
    Path(out_path).unlink(missing_ok=True)
    Path(err_path).unlink(missing_ok=True)
    if meta.get("timed_out"):
        meta["rc"] = 124
        stderr += f"timed out after {timeout}s\n".encode()
    meta["stdout"] = stdout
    meta["stderr"] = stderr
    return meta


def run_text(cmd: list[str], timeout: float) -> tuple[int, str, float, int, bool]:
    res = run_wait4(cmd, timeout)
    err = res["stderr"].decode("utf-8", "replace")
    return res["rc"], err, res["seconds"], res["max_rss_kb"], res["timed_out"]


def clang_obj(src: Path, obj: Path, opt: str, std: str) -> dict:
    cmd = [CLANG, f"-std={std}", f"-{opt}", "-w", *flags(), "-c", str(src), "-o", str(obj)]
    rc, err, secs, rss, timed_out = run_text(cmd, COMPILE_TIMEOUT)
    status = "ok" if rc == 0 and not timed_out else ("timeout" if timed_out or rc == 124 else "clang-fail")
    return {
        "file": src.name,
        "status": status,
        "seconds": secs,
        "max_rss_kb": rss,
        "stderr": err[-2000:],
    }


def ours_obj(src: Path, obj: Path, opt: str, std: str, ll: Path) -> dict:
    result = {
        "file": src.name,
        "frontend_seconds": 0.0,
        "frontend_max_rss_kb": 0,
        "backend_seconds": 0.0,
        "backend_max_rss_kb": 0,
    }
    if not ll.is_file():
        cmd = [str(COMPILER), f"-std={std}", *flags(), str(src), "-o", str(ll)]
        rc, err, front, rss, timed_out = run_text(cmd, COMPILE_TIMEOUT)
        result["frontend_seconds"] = front
        result["frontend_max_rss_kb"] = rss
        if rc != 0 or timed_out:
            if timed_out or rc == 124:
                status = "timeout"
            elif rc < 0 or rc == 101 or "panicked" in err:
                status = "ice"
            else:
                status = "reject"
            result.update(status=status, stderr=err.replace(f"{ROOT}/", "")[-2000:])
            ll.unlink(missing_ok=True)
            return result
    rc, err, back, rss, timed_out = run_text([CLANG, f"-{opt}", "-w", "-c", str(ll), "-o", str(obj)], COMPILE_TIMEOUT)
    result["backend_seconds"] = back
    result["backend_max_rss_kb"] = rss
    if rc != 0 or timed_out:
        result.update(status="bad-ir", stderr=err[-2000:])
        return result
    result.update(status="ok", stderr="")
    return result


def link(objs: list[Path], exe: Path) -> tuple[int, str]:
    rc, err, _, _, _ = run_text([CLANG, "-o", str(exe), *map(str, objs), *LIBS], 300)
    return rc, err


def build(opt: str, std: str, out: Path, jobs: int, ir_dir: Path, ours_first: bool, prev_ours: list[dict] | None) -> dict:
    """Graded build at one level, both compilers."""
    ref_dir = out / f"clang-{opt}"
    our_dir = out / f"ours-{opt}"
    for d in (ref_dir, our_dir):
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
    files = [SRC / name for name in SOURCES]
    prev_by_name = {r["file"]: r for r in (prev_ours or [])}

    def one_clang(src: Path) -> dict:
        return clang_obj(src, ref_dir / f"{src.stem}.o", opt, std)

    def one_ours(src: Path) -> dict:
        ll = ir_dir / f"{src.stem}.ll"
        result = ours_obj(src, our_dir / f"{src.stem}.o", opt, std, ll)
        if src.name in prev_by_name and prev_by_name[src.name]["status"] == "ok" and ll.is_file():
            # Frontend was reused; report the same frontend cost as the first level.
            result["frontend_seconds"] = prev_by_name[src.name]["frontend_seconds"]
            result["frontend_max_rss_kb"] = prev_by_name[src.name]["frontend_max_rss_kb"]
            result["ir_reused"] = True
        return result

    if ours_first:
        with ThreadPoolExecutor(max_workers=jobs) as pool:
            ours = list(pool.map(one_ours, files))
        with ThreadPoolExecutor(max_workers=jobs) as pool:
            ref = list(pool.map(one_clang, files))
    else:
        with ThreadPoolExecutor(max_workers=jobs) as pool:
            ref = list(pool.map(one_clang, files))
        with ThreadPoolExecutor(max_workers=jobs) as pool:
            ours = list(pool.map(one_ours, files))

    bad_ref = [r["file"] for r in ref if r["status"] != "ok"]
    if bad_ref:
        raise SystemExit(f"clang failed to compile {bad_ref}:\n{ref[0]['stderr']}")
    failed = [r["file"] for r in ours if r["status"] != "ok"]
    result: dict = {
        "files": ours,
        "clang_files": ref,
        "failed_files": failed,
        "build": "mixed" if failed else "full",
        "clang_compile_seconds": sum(r["seconds"] for r in ref),
        "clang_max_rss_kb": max((r["max_rss_kb"] for r in ref), default=0),
        "ours_frontend_seconds": sum(r["frontend_seconds"] for r in ours),
        "ours_frontend_max_rss_kb": max((r["frontend_max_rss_kb"] for r in ours), default=0),
        "ours_compile_seconds": sum(r["frontend_seconds"] + r["backend_seconds"] for r in ours),
        "ours_max_rss_kb": max((max(r["frontend_max_rss_kb"], r["backend_max_rss_kb"]) for r in ours), default=0),
        "first": "ours" if ours_first else "clang",
        "ref_dir": str(ref_dir),
        "our_dir": str(our_dir),
    }
    rc, err = link([ref_dir / f"{Path(n).stem}.o" for n in SOURCES], ref_dir / "sqlite3")
    if rc != 0:
        raise SystemExit(f"clang reference sqlite3 failed to link:\n{err}")
    objs = [(ref_dir if f"{s}" in failed else our_dir) / f"{Path(s).stem}.o" for s in SOURCES]
    # failed holds 'sqlite3.c' style names
    objs = [(ref_dir if name in failed else our_dir) / f"{Path(name).stem}.o" for name in SOURCES]
    rc, err = link(objs, our_dir / "sqlite3")
    result["link_ok"] = rc == 0
    if rc != 0:
        result["link_stderr"] = err[-2000:]
    return result


def first_difference(ref: bytes, got: bytes) -> str:
    a, b = ref.decode("utf-8", "replace").splitlines(), got.decode("utf-8", "replace").splitlines()
    for i in range(max(len(a), len(b))):
        x = a[i] if i < len(a) else "<end>"
        y = b[i] if i < len(b) else "<end>"
        if x != y:
            return f"line {i + 1}: clang {x[:200]!r} / ours {y[:200]!r}"
    if ref != got:
        return "same lines, different bytes"
    return "identical text, other difference"


def file_digest(path: Path) -> str | None:
    if not path.is_file():
        return None
    import hashlib

    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def run_workload(exe: Path, script: Path, scratch: Path, timeout: float) -> dict:
    """Run one SQL script against a fresh database. Stale files are removed."""
    scratch.mkdir(parents=True, exist_ok=True)
    db = scratch / (script.stem + ".db")
    db.unlink(missing_ok=True)
    # Also drop leftover journal/wal files from a previous interrupted run.
    for extra in (str(db) + "-journal", str(db) + "-wal", str(db) + "-shm"):
        Path(extra).unlink(missing_ok=True)
    cmd = [str(exe), "-batch", "-init", os.devnull, "-bail", str(db)]
    # SQL on stdin, so a leftover .sqliterc cannot interfere (-init /dev/null).
    start = time.perf_counter()
    timed_out = False
    try:
        proc = subprocess.run(
            cmd,
            input=script.read_bytes(),
            capture_output=True,
            timeout=timeout,
            check=False,
            cwd=scratch,
        )
        rc, out, err = proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired as exc:
        timed_out = True
        rc, out, err = 124, exc.stdout or b"", (exc.stderr or b"") + f"timed out after {timeout}s\n".encode()
    return {
        "rc": rc,
        "stdout": out,
        "stderr": err,
        "seconds": time.perf_counter() - start,
        "timed_out": timed_out,
        "db": str(db),
        "db_exists": db.is_file(),
        "db_size": db.stat().st_size if db.is_file() else 0,
        "db_sha256": file_digest(db),
    }


def compare_workloads(ref_exe: Path, our_exe: Path | None, out: Path, ours_first_start: bool) -> list[dict]:
    rows = []
    scripts = sorted(WORKLOADS.glob("*.sql"))
    for index, script in enumerate(scripts):
        ours_first = ours_first_start if index % 2 == 0 else not ours_first_start
        scratch_ref = out / "work" / f"{script.stem}-ref"
        scratch_ours = out / "work" / f"{script.stem}-ours"
        shutil.rmtree(scratch_ref, ignore_errors=True)
        shutil.rmtree(scratch_ours, ignore_errors=True)
        if ours_first and our_exe is not None:
            ours = run_workload(our_exe, script, scratch_ours, WORKLOAD_TIMEOUT)
        ref = run_workload(ref_exe, script, scratch_ref, WORKLOAD_TIMEOUT)
        row = {
            "workload": script.name,
            "clang_seconds": ref["seconds"],
            "rc": ref["rc"],
            "first": "ours" if ours_first and our_exe is not None else "clang",
            "stdout_bytes": len(ref["stdout"]),
            "clang_db_sha256": ref["db_sha256"],
            "clang_db_size": ref["db_size"],
        }
        if our_exe is not None:
            if not (ours_first):
                ours = run_workload(our_exe, script, scratch_ours, WORKLOAD_TIMEOUT)
            row["ours_seconds"] = ours["seconds"]
            row["ours_rc"] = ours["rc"]
            row["ours_db_sha256"] = ours["db_sha256"]
            row["ours_db_size"] = ours["db_size"]
            output_match = (
                not ref["timed_out"]
                and not ours["timed_out"]
                and ref["rc"] == 0
                and ours["rc"] == 0
                and (ours["stdout"], ours["stderr"]) == (ref["stdout"], ref["stderr"])
            )
            db_match = (
                ref["db_exists"]
                and ours["db_exists"]
                and ref["db_sha256"] is not None
                and ref["db_sha256"] == ours["db_sha256"]
            )
            # A timeout, a missing database, or a non-zero exit on either side is never a match.
            row["output_match"] = output_match
            row["db_match"] = db_match
            row["match"] = output_match and db_match
            if not output_match:
                row["first_difference"] = first_difference(
                    ref["stdout"] + ref["stderr"], ours["stdout"] + ours["stderr"]
                )
            elif not db_match:
                row["first_difference"] = (
                    f"database files differ (clang {ref['db_size']} bytes {ref['db_sha256']};"
                    f" ours {ours['db_size']} bytes {ours['db_sha256']})"
                )
        else:
            row["match"] = False
            row["output_match"] = False
            row["db_match"] = False
        rows.append(row)
    return rows


def level(opt: str, std: str, out: Path, jobs: int, ir_dir: Path, ours_first: bool, prev_ours: list[dict] | None) -> dict:
    b = build(opt, std, out, jobs, ir_dir, ours_first, prev_ours)
    result: dict = {"opt": opt, "build": b}
    ref_exe = Path(b["ref_dir"]) / "sqlite3"
    our_exe = Path(b["our_dir"]) / "sqlite3" if b["link_ok"] else None
    result["workloads"] = compare_workloads(ref_exe, our_exe, out / opt, ours_first)
    result["match"] = b["link_ok"] and all(r["match"] for r in result["workloads"])
    result["full"] = b["build"] == "full" and b["link_ok"]
    return result


def verdict(levels: list[dict]) -> tuple[bool, str]:
    ok = bool(levels) and all(lvl["full"] and lvl["match"] for lvl in levels)
    return ok, "SQLITE: PASS" if ok else "SQLITE: FAIL"


def summary(res: dict) -> str:
    b = res["build"]
    total = len(b["files"])
    lines = [
        f"{res['opt']}: full={'yes' if res['full'] else 'no'} match={'yes' if res['match'] else 'no'}"
        f" first={b['first']}",
        f"    build: {b['build']}, {total - len(b['failed_files'])}/{total} files,"
        f" link={'ok' if b['link_ok'] else 'FAIL'}; compile ours {b['ours_compile_seconds']:.1f}s"
        f" (frontend {b['ours_frontend_seconds']:.1f}s, peak {b['ours_frontend_max_rss_kb']} KB)"
        f" clang {b['clang_compile_seconds']:.1f}s (peak {b['clang_max_rss_kb']} KB)",
    ]
    for f in b["files"]:
        if f["status"] != "ok":
            first = (f.get("stderr") or "").strip().splitlines()
            lines.append(f"      {f['status']}: {f['file']}: {first[0] if first else ''}")
        else:
            lines.append(
                f"      ok: {f['file']}: frontend {f['frontend_seconds']:.2f}s"
                f" {f['frontend_max_rss_kb']} KB, backend {f['backend_seconds']:.2f}s"
                f" {f['backend_max_rss_kb']} KB"
            )
    for c in b["clang_files"]:
        lines.append(f"      clang: {c['file']}: {c['seconds']:.2f}s {c['max_rss_kb']} KB")
    for w in res["workloads"]:
        extra = f" {w['first_difference']}" if w.get("first_difference") else ""
        db = " db=yes" if w.get("db_match") else " db=no"
        lines.append(
            f"    workload {w['workload']}: match={'yes' if w['match'] else 'no'}{db}"
            f" ours {w.get('ours_seconds', 0):.2f}s clang {w['clang_seconds']:.2f}s"
            + extra
        )
    return "\n".join(lines)


def metadata(std: str) -> dict:
    def out(cmd: list[str]) -> str:
        try:
            return subprocess.run(cmd, capture_output=True, text=True, check=False).stdout.strip()
        except OSError:
            return ""

    pin = SQLITE_ROOT / ".pin"
    parts = pin.read_text().split() if pin.is_file() else ["", "", ""]
    version = out([CLANG, "--version"])
    return {
        "date": time.strftime("%Y-%m-%d"),
        "compiler_commit": out(["git", "-C", str(ROOT), "rev-parse", "--short", "HEAD"]),
        "sqlite_version": parts[0] if parts else VERSION,
        "zip_sha256": parts[1] if len(parts) > 1 else "",
        "zip_sha3_256": parts[2] if len(parts) > 2 else "",
        "std": std,
        "defines": " ".join(DEFINES),
        "clang": version.splitlines()[0] if version else "",
    }


def yes(flag: bool) -> str:
    return "yes" if flag else "no"


def ratio(ours: float, theirs: float) -> str:
    return f"{ours:.2f} s vs {theirs:.2f} s ({ours / theirs:.2f}x)" if theirs else "n/a"


def kb(n: int) -> str:
    if n >= 1024:
        return f"{n / 1024:.1f} MiB"
    return f"{n} KiB"


def render_scorecard(payload: dict, title: str, notes: str = "") -> str:
    meta = payload.get("meta", {})
    levels = payload["levels"]
    head = " | ".join(lvl["opt"] for lvl in levels)
    rule = " | ".join("---" for _ in levels)

    def row(label: str, cell) -> str:
        return f"| {label} | " + " | ".join(cell(lvl) for lvl in levels) + " |"

    def files_cell(lvl: dict) -> str:
        b = lvl["build"]
        return f"{len(b['files']) - len(b['failed_files'])} / {len(b['files'])}"

    def work_cell(lvl: dict) -> str:
        rows = lvl["workloads"]
        return f"{sum(1 for r in rows if r['match'])} / {len(rows)}"

    def db_cell(lvl: dict) -> str:
        rows = lvl["workloads"]
        return f"{sum(1 for r in rows if r.get('db_match'))} / {len(rows)}"

    def work_time(lvl: dict) -> str:
        rows = [r for r in lvl["workloads"] if "ours_seconds" in r]
        return ratio(sum(r["ours_seconds"] for r in rows), sum(r["clang_seconds"] for r in rows))

    def compile_time(lvl: dict) -> str:
        b = lvl["build"]
        return ratio(b["ours_compile_seconds"], b["clang_compile_seconds"])

    def compile_mem(lvl: dict) -> str:
        b = lvl["build"]
        return f"ours {kb(b['ours_frontend_max_rss_kb'])} (frontend), Clang {kb(b['clang_max_rss_kb'])}"

    lines = [
        f"# {title}",
        "",
        f"- Date: {meta.get('date', '')}",
        f"- Compiler commit: `{meta.get('compiler_commit', '')}`",
        f"- SQLite: {meta.get('sqlite_version', '')} amalgamation from sqlite.org"
        f" (`{AMALGAMATION}.zip` SHA-256 `{meta.get('zip_sha256', '')}`,"
        f" SHA3-256 `{meta.get('zip_sha3_256', '')}`)",
        f"- Language mode: `-std={meta.get('std', '')}` for this compiler and for the reference;"
        f" `{meta.get('defines', '')}`, linked with `-lm -ldl -lpthread`, no readline",
        f"- Reference: {meta.get('clang', 'clang')}, same flags and optimisation level",
        f"- Compile jobs: {payload.get('jobs', '')}",
        "",
        f"| | {head} |",
        f"|---| {rule} |",
        row("Builds with this compiler alone", lambda l: yes(l["full"])),
        row("Files compiled (`sqlite3.c`, `shell.c`)", files_cell),
        row("Build compared", lambda l: "full" if l["full"] else "mixed"),
        row("Workloads matching Clang byte for byte", work_cell),
        row("Database files identical", db_cell),
        row("Matches Clang", lambda l: yes(l["full"] and l["match"])),
        row("Compile time, ours vs Clang", compile_time),
        row("Compile peak memory", compile_mem),
        row("Workload run time, ours vs Clang", work_time),
        "",
        "Compile time is this compiler producing LLVM IR plus Clang turning that",
        "IR into an object, against Clang compiling the C directly. The frontend",
        "does not take `-O`, so its IR is produced once and reused at every",
        "level; the measured frontend time is included at each level. Run times",
        "are wall-clock times of all the workloads at that level. The two",
        "builds take turns to run first: Clang first at the first level and on",
        "every other workload in name order, this compiler first for the rest.",
        "",
    ]
    lines += file_table(levels)
    lines += workload_table(levels)
    failed = [
        (lvl["opt"], f)
        for lvl in levels
        for f in lvl["build"]["files"]
        if f["status"] != "ok"
    ]
    if failed:
        lines += ["## Files this compiler did not build", ""]
        for opt, f in failed:
            first = (f.get("stderr") or "").strip().splitlines()
            lines.append(f"- {opt} `{f['file']}`: {f['status']}: `{first[0] if first else ''}`")
        lines.append("")
    if notes:
        lines += [notes.rstrip(), ""]
    return "\n".join(lines)


def file_table(levels: list[dict]) -> list[str]:
    if not levels:
        return []
    names = [f["file"] for f in levels[0]["build"]["files"]]
    opts = [lvl["opt"] for lvl in levels]
    lines = [
        "## Per-file compile time and peak memory",
        "",
        "| File | " + " | ".join(f"{o} ours (front+back, peak) | {o} Clang" for o in opts) + " |",
        "|---| " + " | ".join("--- | ---" for _ in opts) + " |",
    ]
    for name in names:
        cells = []
        for lvl in levels:
            ours = next(f for f in lvl["build"]["files"] if f["file"] == name)
            clang = next(f for f in lvl["build"]["clang_files"] if f["file"] == name)
            if ours["status"] == "ok":
                cells.append(
                    f"{ours['frontend_seconds']:.2f} s + {ours['backend_seconds']:.2f} s,"
                    f" {kb(ours['frontend_max_rss_kb'])}"
                )
            else:
                cells.append(f"{ours['status']} ({ours['frontend_seconds']:.2f} s, {kb(ours['frontend_max_rss_kb'])})")
            cells.append(f"{clang['seconds']:.2f} s, {kb(clang['max_rss_kb'])}")
        lines.append(f"| `{name}` | " + " | ".join(cells) + " |")
    lines.append("")
    return lines


def workload_table(levels: list[dict]) -> list[str]:
    names = [w["workload"] for w in levels[0]["workloads"]] if levels else []
    if not names:
        return []
    opts = [lvl["opt"] for lvl in levels]
    lines = [
        "## Workloads",
        "",
        "| Script | Output bytes | "
        + " | ".join(f"{o} matches | {o} db | {o} ours vs Clang" for o in opts)
        + " |",
        "|---|---| " + " | ".join("--- | --- | ---" for _ in opts) + " |",
    ]
    for name in names:
        cells = []
        size = ""
        for lvl in levels:
            w = next(r for r in lvl["workloads"] if r["workload"] == name)
            size = str(w["stdout_bytes"])
            cells.append(yes(w["match"]))
            cells.append(yes(w.get("db_match", False)))
            cells.append(f"{w.get('ours_seconds', 0):.2f} s / {w['clang_seconds']:.2f} s")
        lines.append(f"| `{name}` | {size} | " + " | ".join(cells) + " |")
    lines.append("")
    return lines


def strip_dirs(payload: dict) -> None:
    for lvl in payload["levels"]:
        lvl["build"].pop("ref_dir", None)
        lvl["build"].pop("our_dir", None)
        for w in lvl["workloads"]:
            # Keep hashes and sizes; drop machine-local paths if any leaked.
            pass


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--opt", action="append", choices=["O0", "O1", "O2", "O3"], help="repeatable; default O0 and O2")
    ap.add_argument("--std", default="gnu11", choices=["c11", "gnu11"], help="language mode for both compilers")
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 1)
    ap.add_argument("--out", type=Path, default=ROOT / "build" / "sqlite")
    ap.add_argument("--json", type=Path, help="write the full results here")
    ap.add_argument("--scorecard", type=Path, help="write a Markdown scorecard here")
    ap.add_argument("--title", default="SQLite scorecard")
    ap.add_argument("--notes", type=Path, help="Markdown appended to the scorecard (defects)")
    ap.add_argument("--from-json", type=Path, help="render --scorecard from saved results; do not run")
    ap.add_argument(
        "--ci",
        action="store_true",
        help="lean run: -O0 only (used by CI; the full scorecard is still O0 and O2 locally)",
    )
    args = ap.parse_args()
    notes = args.notes.read_text() if args.notes else ""

    if args.from_json:
        if not args.scorecard:
            ap.error("--from-json needs --scorecard")
        payload = json.loads(args.from_json.read_text())
        args.scorecard.write_text(render_scorecard(payload, args.title, notes))
        return 0
    args.out = args.out.resolve()

    for name in SOURCES:
        path = SRC / name
        if not path.is_file():
            print(f"missing {path}. Run scripts/fetch_sqlite.sh first.", file=sys.stderr)
            return 2
    if not COMPILER.is_file():
        print(f"missing {COMPILER}. Run make first.", file=sys.stderr)
        return 2
    if not any(WORKLOADS.glob("*.sql")):
        print(f"missing workloads in {WORKLOADS}", file=sys.stderr)
        return 2

    opts = args.opt or (["O0"] if args.ci else ["O0", "O2"])
    ir_dir = args.out / "ir"
    shutil.rmtree(ir_dir, ignore_errors=True)
    ir_dir.mkdir(parents=True, exist_ok=True)
    results = []
    prev_ours = None
    for index, opt in enumerate(opts):
        ours_first = index % 2 == 1
        res = level(opt, args.std, args.out, args.jobs, ir_dir, ours_first, prev_ours)
        print(summary(res), flush=True)
        prev_ours = res["build"]["files"]
        results.append(res)
    payload = {"meta": metadata(args.std), "jobs": args.jobs, "levels": results}
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
    if len(sys.argv) > 1 and sys.argv[1] == "--wait4":
        sys.exit(_wait4_main())
    sys.exit(main())
