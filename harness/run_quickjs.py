#!/usr/bin/env python3
"""Build QuickJS with this compiler and with Clang, then compare.

Fetch the inputs first:

    scripts/fetch_quickjs.sh

The program is the pinned QuickJS release (tests/external/quickjs/
quickjs-2026-06-04, from bellard.org, checked by SHA-256) and, for
--test262, the test262 revision its Makefile names, prepared with
QuickJS's own tests/test262.patch. QuickJS is only a test subject: it is
built and run, and nothing from it goes into the compiler.

The build follows the upstream Makefile's Linux configuration (read for its
flags only): -std=gnu11 -D_GNU_SOURCE -DCONFIG_VERSION="2026-06-04", linked
with -lm -lpthread -ldl, and -rdynamic so the test modules can be loaded.
Programs: qjsc, qjs (with repl.c, which qjsc generates from repl.js) and
run-test262; plus the two native test modules tests/bjson.so and
examples/point.so.

For each optimisation level:

1. Per-file: every translation unit is compiled by this compiler to LLVM
   IR and Clang turns that IR into an object at the same level. Each file
   is ok, reject (a diagnostic), ice (the compiler crashed), bad-ir (Clang
   refused the IR) or timeout. Failed files are replaced by Clang's
   objects (a mixed build) so the rest can still be compared.
2. qjsc output: both qjsc builds compile repl.js and every tests/*.js file
   to C (`qjsc -c`); the outputs must be identical bytes. This compares the
   bytecode the two builds generate.
3. Tests: the Makefile's `test` list (tests/test_*.js, test_bjson.js and
   examples/test_point.js) run by both qjs builds, each with its own native
   modules; exit status, stdout and stderr must match byte for byte, and
   the Clang build must pass.
4. test262 (with --test262 LEVEL): run-test262 -a with test262.conf by both
   builds; the sets of failing tests must be the same. The count of
   upstream's known failures (test262_errors.txt) is reported.

Exit status is 0 only when every level built with this compiler alone and
every comparison matched.

Env: CC_UNDER_TEST, CLANG, QUICKJS_ROOT (default tests/external/quickjs).
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COMPILER = Path(os.environ.get("CC_UNDER_TEST", ROOT / "build" / "hcc"))
CLANG = os.environ.get("CLANG", "clang")
VERSION = "2026-06-04"
QJS_ROOT = Path(os.environ.get("QUICKJS_ROOT", ROOT / "tests" / "external" / "quickjs"))
SRC = QJS_ROOT / f"quickjs-{VERSION}"
STD = "gnu11"
DEFINES = ["-D_GNU_SOURCE", f'-DCONFIG_VERSION="{VERSION}"']
LIBS = ["-lm", "-lpthread", "-ldl", "-rdynamic"]
LIB_UNITS = ["quickjs", "dtoa", "libregexp", "libunicode", "cutils", "quickjs-libc"]
PROGRAMS = {
    "qjsc": LIB_UNITS + ["qjsc"],
    "qjs": LIB_UNITS + ["qjs", "repl"],
    "run-test262": LIB_UNITS + ["run-test262"],
}
MODULES = {"bjson": "tests/bjson.c", "point": "examples/point.c"}
# The Makefile's `test` target, in its order.
TESTS = [
    ["tests/test_closure.js"],
    ["tests/test_language.js"],
    ["--std", "tests/test_builtin.js"],
    ["tests/test_loop.js"],
    ["tests/test_bigint.js"],
    ["tests/test_cyclic_import.js"],
    ["tests/test_worker.js"],
    ["tests/test_std.js"],
    ["tests/test_rw_handler.js"],
    ["tests/test_bjson.js"],
    ["examples/test_point.js"],
]


def run(cmd: list[str], timeout: float, cwd: Path | None = None) -> dict:
    start = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd, capture_output=True, stdin=subprocess.DEVNULL, timeout=timeout, check=False, cwd=cwd
        )
        rc, out, err, timed_out = proc.returncode, proc.stdout, proc.stderr, False
    except subprocess.TimeoutExpired as exc:
        rc, out, err, timed_out = 124, exc.stdout or b"", (exc.stderr or b"") + b"timed out\n", True
    return {"rc": rc, "stdout": out, "stderr": err, "seconds": time.perf_counter() - start, "timed_out": timed_out}


def text(res: dict) -> str:
    return (res["stdout"] + res["stderr"]).decode("utf-8", "replace")


def unit_flags(pic: bool) -> list[str]:
    """A native module is built position-independent and with
    JS_SHARED_LIBRARY, which names its init function js_init_module."""
    return [*DEFINES, *(["-DJS_SHARED_LIBRARY"] if pic else []), "-I", str(SRC)]


def clang_obj(src: Path, obj: Path, opt: str, pic: bool) -> dict:
    cmd = [CLANG, f"-std={STD}", f"-{opt}", "-w", *unit_flags(pic)]
    res = run(cmd + (["-fPIC"] if pic else []) + ["-c", str(src), "-o", str(obj)], 600)
    return {"file": src.name, "status": "ok" if res["rc"] == 0 else "clang-fail", "seconds": res["seconds"],
            "stderr": text(res)[-2000:]}


def ours_obj(src: Path, obj: Path, opt: str, pic: bool) -> dict:
    ll = obj.with_suffix(".ll")
    res = run([str(COMPILER), f"-std={STD}", *unit_flags(pic), str(src), "-o", str(ll)], 600)
    out = {"file": src.name, "frontend_seconds": res["seconds"], "backend_seconds": 0.0}
    err = text(res)
    if res["rc"] != 0:
        if res["timed_out"]:
            status = "timeout"
        elif res["rc"] < 0 or res["rc"] == 101 or "panicked" in err:
            status = "ice"
        else:
            status = "reject"
        out.update(status=status, stderr=err.replace(f"{ROOT}/", "")[-2000:])
        return out
    back = run([CLANG, f"-{opt}", "-w", *(["-fPIC"] if pic else []), "-c", str(ll), "-o", str(obj)], 600)
    out["backend_seconds"] = back["seconds"]
    out.update(status="ok" if back["rc"] == 0 else "bad-ir", stderr="" if back["rc"] == 0 else text(back)[-2000:])
    return out


def link(objs: list[Path], exe: Path, shared: bool = False) -> dict:
    return run([CLANG, *(["-shared"] if shared else []), "-o", str(exe), *map(str, objs), *LIBS], 300)


def build(opt: str, out: Path, jobs: int) -> dict:
    """Both builds at one level. Clang's qjsc generates repl.c for both, so
    the qjs programs embed the same bytecode; step 2 compares qjsc itself."""
    dirs = {"ref": out / f"clang-{opt}", "ours": out / f"ours-{opt}"}
    for d in dirs.values():
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
    units = sorted({u for stems in PROGRAMS.values() for u in stems if u != "repl"})
    sources = [(SRC / f"{u}.c", u, False) for u in units]
    sources += [(SRC / path, name, True) for name, path in MODULES.items()]

    def compile_all(kind: str, items: list[tuple[Path, str, bool]]) -> list[dict]:
        fn = clang_obj if kind == "ref" else ours_obj
        with ThreadPoolExecutor(max_workers=jobs) as pool:
            return list(pool.map(lambda s: fn(s[0], dirs[kind] / f"{s[1]}.o", opt, s[2]), items))

    ref = compile_all("ref", sources)
    bad = [r for r in ref if r["status"] != "ok"]
    if bad:
        raise SystemExit(f"clang failed to compile {bad[0]['file']}:\n{bad[0]['stderr']}")
    ours = compile_all("ours", sources)
    ref_qjsc = dirs["ref"] / "qjsc"
    res = link([dirs["ref"] / f"{s}.o" for s in PROGRAMS["qjsc"]], ref_qjsc)
    if res["rc"] != 0:
        raise SystemExit(f"clang reference qjsc failed to link:\n{text(res)}")
    repl = out / f"repl-{opt}.c"
    res = run([str(ref_qjsc), "-s", "-c", "-o", str(repl), "-m", "repl.js"], 120, cwd=SRC)
    if res["rc"] != 0:
        raise SystemExit(f"clang reference qjsc failed on repl.js:\n{text(res)}")
    ref += compile_all("ref", [(repl, "repl", False)])
    ours += compile_all("ours", [(repl, "repl", False)])
    failed = [r["file"] for r in ours if r["status"] != "ok"]
    result: dict = {
        "opt": opt, "files": ours, "failed_files": failed, "build": "mixed" if failed else "full",
        "dirs": {k: str(v) for k, v in dirs.items()},
        "clang_compile_seconds": sum(r["seconds"] for r in ref),
        "ours_frontend_seconds": sum(r["frontend_seconds"] for r in ours),
        "ours_compile_seconds": sum(r["frontend_seconds"] + r["backend_seconds"] for r in ours),
    }

    def obj(kind: str, stem: str) -> Path:
        mixed = kind == "ours" and any(f.startswith(stem + ".") or f == f"repl-{opt}.c" and stem == "repl"
                                       for f in failed)
        return dirs["ref" if mixed else kind] / f"{stem}.o"

    link_ok = True
    for kind in ("ref", "ours"):
        for exe, stems in PROGRAMS.items():
            res = link([obj(kind, s) for s in stems], dirs[kind] / exe)
            if res["rc"] != 0:
                if kind == "ref":
                    raise SystemExit(f"clang reference {exe} failed to link:\n{text(res)}")
                link_ok = False
                result["link_stderr"] = result.get("link_stderr", "") + text(res)[-2000:]
        for name in MODULES:
            res = link([obj(kind, name)], dirs[kind] / f"{name}.so", shared=True)
            if res["rc"] != 0:
                if kind == "ref":
                    raise SystemExit(f"clang reference {name}.so failed to link:\n{text(res)}")
                link_ok = False
                result["link_stderr"] = result.get("link_stderr", "") + text(res)[-2000:]
    result["link_ok"] = link_ok
    return result


def qjsc_inputs() -> list[Path]:
    return [SRC / "repl.js"] + sorted((SRC / "tests").glob("*.js"))


def compare_qjsc(ref: Path, ours: Path, scratch: Path) -> dict:
    """`qjsc -c` of every input by both builds; the C files must be equal.
    repl.js is compiled as a module, as the Makefile does; for the rest qjsc
    decides. An input the Clang build rejects must be rejected the same way
    (status and message) by ours."""
    scratch.mkdir(parents=True, exist_ok=True)
    same, rejected, differ = 0, [], []
    for i, src in enumerate(qjsc_inputs()):
        outs = []
        for tag, exe in (("ref", ref), ("ours", ours)):
            dest = scratch / f"{i}-{tag}.c"
            dest.unlink(missing_ok=True)
            module = ["-m"] if src.name == "repl.js" else []
            res = run([str(exe), "-c", "-o", str(dest), *module, src.name], 120, cwd=src.parent)
            ok = res["rc"] == 0 and not res["timed_out"] and dest.is_file()
            outs.append(dest.read_bytes() if ok else (res["rc"], res["stdout"], res["stderr"], res["timed_out"]))
        if outs[0] == outs[1] and not (isinstance(outs[0], tuple) and outs[0][3]):
            same += 1
            if isinstance(outs[0], tuple):
                rejected.append(src.name)
        else:
            differ.append(src.name)
    return {"files": same + len(differ), "matching": same, "rejected_by_both": rejected, "differ": differ}


def run_test(exe: Path, modules: Path, args: list[str]) -> dict:
    """One test in a scratch copy of the tree, with this build's native
    modules where the tests load them from."""
    work = Path(tempfile.mkdtemp(prefix="qjs-test-"))
    try:
        tree = work / "q"
        shutil.copytree(SRC, tree, ignore=shutil.ignore_patterns("test262"), symlinks=True)
        shutil.copy(modules / "bjson.so", tree / "tests" / "bjson.so")
        shutil.copy(modules / "point.so", tree / "examples" / "point.so")
        return run([str(exe), *args], 600, cwd=tree)
    finally:
        shutil.rmtree(work, ignore_errors=True)


def first_difference(a: bytes, b: bytes) -> str:
    x, y = a.decode("utf-8", "replace").splitlines(), b.decode("utf-8", "replace").splitlines()
    for i in range(max(len(x), len(y))):
        p = x[i] if i < len(x) else "<end>"
        q = y[i] if i < len(y) else "<end>"
        if p != q:
            return f"line {i + 1}: clang {p[:200]!r} / ours {q[:200]!r}"
    return "same lines, different bytes"


def test262(exe: Path, tag: str, threads: int) -> dict:
    """run-test262 -a over the whole configured tree. The errors file it
    writes (-u) lists every failing test; the config file must sit in the
    source tree because its paths are relative to it."""
    errors = Path(tempfile.mkdtemp(prefix="t262-")) / "errors.txt"
    shutil.copy(SRC / "test262_errors.txt", errors)
    conf = SRC / f"test262-hcc-{tag}.conf"
    conf.write_text(
        "".join(f"errorfile={errors}\n" if line.startswith("errorfile=") else line
                for line in (SRC / "test262.conf").read_text().splitlines(keepends=True))
    )
    try:
        res = run([str(exe), "-c", conf.name, "-a", "-T", str(threads), "-C", "-u"], 3600, cwd=SRC)
    finally:
        conf.unlink(missing_ok=True)
    lines = text(res).strip().splitlines()
    summary = next((ln for ln in reversed(lines) if ln.startswith("Result:")), lines[-1] if lines else "")
    failing = sorted(set(errors.read_text().splitlines())) if errors.is_file() else []
    shutil.rmtree(errors.parent, ignore_errors=True)
    return {"rc": res["rc"], "summary": summary, "failing": failing, "seconds": res["seconds"],
            "timed_out": res["timed_out"]}


def level(opt: str, out: Path, jobs: int, with_test262: bool, threads: int) -> dict:
    b = build(opt, out, jobs)
    ref_dir, our_dir = Path(b["dirs"]["ref"]), Path(b["dirs"]["ours"])
    result: dict = {"opt": opt, "build": b}
    if not b["link_ok"]:
        result.update(match=False, full=False, tests=[], qjsc={"files": 0, "matching": 0, "differ": ["(not built)"]})
        return result
    result["qjsc"] = compare_qjsc(ref_dir / "qjsc", our_dir / "qjsc", out / f"qjsc-{opt}")
    rows = []
    for index, args in enumerate(TESTS):
        order = [("ours", our_dir), ("ref", ref_dir)] if index % 2 else [("ref", ref_dir), ("ours", our_dir)]
        res = {tag: run_test(d / "qjs", d, args) for tag, d in order}
        ref, ours = res["ref"], res["ours"]
        if ref["rc"] != 0 or ref["timed_out"]:
            raise SystemExit(f"clang reference failed {' '.join(args)}:\n{text(ref)[-1500:]}")
        row = {"test": " ".join(args), "clang_seconds": ref["seconds"], "ours_seconds": ours["seconds"],
               "ours_rc": ours["rc"],
               "match": not ours["timed_out"] and (ours["rc"], ours["stdout"], ours["stderr"])
               == (ref["rc"], ref["stdout"], ref["stderr"])}
        if not row["match"]:
            row["first_difference"] = first_difference(ref["stdout"] + ref["stderr"], ours["stdout"] + ours["stderr"])
        rows.append(row)
    result["tests"] = rows
    if with_test262:
        ref = test262(ref_dir / "run-test262", f"clang-{opt}", threads)
        ours = test262(our_dir / "run-test262", f"ours-{opt}", threads)
        known = sorted(set((SRC / "test262_errors.txt").read_text().splitlines()))
        result["test262"] = {
            "clang": {k: v for k, v in ref.items() if k != "failing"} | {"failing": len(ref["failing"])},
            "ours": {k: v for k, v in ours.items() if k != "failing"} | {"failing": len(ours["failing"])},
            "upstream_known": len(known),
            "only_clang_fails": sorted(set(ref["failing"]) - set(ours["failing"])),
            "only_ours_fails": sorted(set(ours["failing"]) - set(ref["failing"])),
        }
        result["test262"]["match"] = (not ours["timed_out"] and not ref["timed_out"]
                                      and ref["failing"] == ours["failing"])
    result["full"] = b["build"] == "full"
    result["match"] = (all(r["match"] for r in rows) and not result["qjsc"]["differ"]
                       and result.get("test262", {"match": True})["match"])
    return result


def summary(res: dict) -> str:
    b = res["build"]
    total = len(b["files"])
    lines = [
        f"{res['opt']}: full={'yes' if res['full'] else 'no'} match={'yes' if res['match'] else 'no'}",
        f"    build: {b['build']}, {total - len(b['failed_files'])}/{total} files,"
        f" link={'ok' if b['link_ok'] else 'FAIL'}; compile ours {b['ours_compile_seconds']:.1f}s"
        f" (frontend {b['ours_frontend_seconds']:.1f}s) clang {b['clang_compile_seconds']:.1f}s",
    ]
    for f in b["files"]:
        if f["status"] != "ok":
            first = (f.get("stderr") or "").strip().splitlines()
            lines.append(f"      {f['status']}: {f['file']}: {first[0] if first else ''}")
    q = res["qjsc"]
    lines.append(
        f"    qjsc -c: {q['matching']}/{q['files']} identical"
        + (f" (rejected by both: {q['rejected_by_both']})" if q.get("rejected_by_both") else "")
        + (f"; differ {q['differ']}" if q["differ"] else "")
    )
    for t in res["tests"]:
        lines.append(
            f"    test {t['test']}: match={'yes' if t['match'] else 'no'}"
            f" ours {t['ours_seconds']:.2f}s clang {t['clang_seconds']:.2f}s"
            + (f" {t['first_difference']}" if t.get("first_difference") else "")
        )
    if "test262" in res:
        t = res["test262"]
        lines.append(
            f"    test262: match={'yes' if t['match'] else 'no'}; clang {t['clang']['summary']} ({t['clang']['seconds']:.0f}s);"
            f" ours {t['ours']['summary']} ({t['ours']['seconds']:.0f}s); upstream known failures {t['upstream_known']}"
        )
        for name in ("only_ours_fails", "only_clang_fails"):
            for line in t[name][:20]:
                lines.append(f"      {name}: {line}")
    return "\n".join(lines)


def verdict(levels: list[dict]) -> tuple[bool, str]:
    ok = bool(levels) and all(lvl["full"] and lvl["match"] for lvl in levels)
    return ok, "QUICKJS: PASS" if ok else "QUICKJS: FAIL"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--opt", action="append", choices=["O0", "O1", "O2", "O3"], help="levels (default O0 and O2)")
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 4, help="parallel compiles")
    ap.add_argument("--test262", action="append", choices=["O0", "O1", "O2", "O3"], default=[],
                    help="also run test262 with both builds at this level (repeatable)")
    ap.add_argument("--threads", type=int, default=os.cpu_count() or 4, help="run-test262 threads")
    ap.add_argument("--out", type=Path, default=ROOT / "build" / "quickjs", help="work directory")
    ap.add_argument("--json", type=Path, help="write the full results here")
    args = ap.parse_args()
    if not (SRC / "quickjs.c").is_file():
        print(f"QuickJS sources not found under {SRC}; run scripts/fetch_quickjs.sh", file=sys.stderr)
        return 2
    if args.test262 and not (SRC / "test262" / "harness" / "assert.js").is_file():
        print("test262 not found; run scripts/fetch_quickjs.sh without QUICKJS_NO_TEST262", file=sys.stderr)
        return 2
    args.out.mkdir(parents=True, exist_ok=True)
    levels = []
    for opt in args.opt or ["O0", "O2"]:
        res = level(opt, args.out, args.jobs, opt in args.test262, args.threads)
        print(summary(res), flush=True)
        levels.append(res)
    ok, line = verdict(levels)
    if args.json:
        args.json.write_text(json.dumps({"version": VERSION, "levels": levels}, indent=2, default=str))
    print(line)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
