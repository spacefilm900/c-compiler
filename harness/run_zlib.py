#!/usr/bin/env python3
"""Build zlib with this compiler and with Clang, then compare.

Fetch the input first:

    scripts/fetch_zlib.sh

The program is the pinned zlib release (tests/external/zlib/zlib-1.3.1,
checked by SHA-256, not patched), built as its Makefile does on Linux
(-D_LARGEFILE64_SOURCE=1 -DHAVE_HIDDEN, here with -std=gnu11): the library
objects, test/example.c and test/minigzip.c. For each optimisation level:

1. Per-file: every translation unit is compiled by this compiler to LLVM
   IR and Clang turns the IR into an object at the same level; each file is
   ok, reject, ice, bad-ir or timeout. Failed files are replaced by Clang's
   objects (a mixed build) so the rest can still be compared.
2. zlib's own tests, as `make test` runs them: `example` (exit 0, and the
   same output as Clang's build) and `echo hello world | minigzip |
   minigzip -d`.
3. A corpus round trip: every corpus file (generated here, deterministic:
   text, random bytes, runs, a mixture, empty and one-byte files, and this
   repository's own sources) is compressed by both minigzip builds with
   each of -1, -6, -9, -9 -f (filtered), -1 -h (Huffman only) and -r
   (run length). The two compressed streams must be identical bytes, and each
   build must decompress the other's stream back to the original.

Exit status is 0 only when every level built with this compiler alone and
every comparison matched.

Env: CC_UNDER_TEST, CLANG, ZLIB_ROOT (default tests/external/zlib).
"""

from __future__ import annotations

import argparse
import json
import os
import random
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
VERSION = "1.3.1"
ZLIB_ROOT = Path(os.environ.get("ZLIB_ROOT", ROOT / "tests" / "external" / "zlib"))
SRC = ZLIB_ROOT / f"zlib-{VERSION}"
STD = "gnu11"
FLAGS = ["-D_LARGEFILE64_SOURCE=1", "-DHAVE_HIDDEN"]
LIB = ["adler32", "crc32", "deflate", "infback", "inffast", "inflate", "inftrees", "trees", "zutil",
       "compress", "uncompr", "gzclose", "gzlib", "gzread", "gzwrite"]
PROGRAMS = {"example": "test/example.c", "minigzip": "test/minigzip.c"}
MODES = [["-1"], ["-6"], ["-9"], ["-9", "-f"], ["-1", "-h"], ["-r"]]


def run(cmd: list[str], timeout: float, cwd: Path | None = None, data: bytes | None = None) -> dict:
    start = time.perf_counter()
    try:
        proc = subprocess.run(cmd, capture_output=True, input=data if data is not None else b"",
                              timeout=timeout, check=False, cwd=cwd)
        rc, out, err, timed_out = proc.returncode, proc.stdout, proc.stderr, False
    except subprocess.TimeoutExpired as exc:
        rc, out, err, timed_out = 124, exc.stdout or b"", (exc.stderr or b"") + b"timed out\n", True
    return {"rc": rc, "stdout": out, "stderr": err, "seconds": time.perf_counter() - start, "timed_out": timed_out}


def text(res: dict) -> str:
    return (res["stdout"] + res["stderr"]).decode("utf-8", "replace")


def sources() -> list[tuple[str, Path]]:
    return [(n, SRC / f"{n}.c") for n in LIB] + [(n, SRC / p) for n, p in PROGRAMS.items()]


def clang_obj(src: Path, obj: Path, opt: str) -> dict:
    res = run([CLANG, f"-std={STD}", f"-{opt}", "-w", *FLAGS, "-I", str(SRC), "-c", str(src), "-o", str(obj)], 300)
    return {"file": src.name, "status": "ok" if res["rc"] == 0 else "clang-fail", "seconds": res["seconds"],
            "stderr": text(res)[-2000:]}


def ours_obj(src: Path, obj: Path, opt: str) -> dict:
    ll = obj.with_suffix(".ll")
    res = run([str(COMPILER), f"-std={STD}", *FLAGS, "-I", str(SRC), str(src), "-o", str(ll)], 300)
    out = {"file": src.name, "frontend_seconds": res["seconds"], "backend_seconds": 0.0}
    if res["rc"] != 0:
        err = text(res)
        if res["timed_out"]:
            status = "timeout"
        elif res["rc"] < 0 or res["rc"] == 101 or "panicked" in err:
            status = "ice"
        else:
            status = "reject"
        out.update(status=status, stderr=err.replace(f"{ROOT}/", "")[-2000:])
        return out
    back = run([CLANG, f"-{opt}", "-w", "-c", str(ll), "-o", str(obj)], 300)
    out["backend_seconds"] = back["seconds"]
    out.update(status="ok" if back["rc"] == 0 else "bad-ir", stderr="" if back["rc"] == 0 else text(back)[-2000:])
    return out


def build(opt: str, out: Path, jobs: int) -> dict:
    dirs = {"ref": out / f"clang-{opt}", "ours": out / f"ours-{opt}"}
    for d in dirs.values():
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        ref = list(pool.map(lambda s: clang_obj(s[1], dirs["ref"] / f"{s[0]}.o", opt), sources()))
        ours = list(pool.map(lambda s: ours_obj(s[1], dirs["ours"] / f"{s[0]}.o", opt), sources()))
    bad = [r for r in ref if r["status"] != "ok"]
    if bad:
        raise SystemExit(f"clang failed to compile {bad[0]['file']}:\n{bad[0]['stderr']}")
    failed = {r["file"] for r in ours if r["status"] != "ok"}
    result: dict = {"opt": opt, "files": ours, "failed_files": sorted(failed),
                    "build": "mixed" if failed else "full", "dirs": {k: str(v) for k, v in dirs.items()},
                    "clang_compile_seconds": sum(r["seconds"] for r in ref),
                    "ours_compile_seconds": sum(r["frontend_seconds"] + r["backend_seconds"] for r in ours)}
    names = {name: path.name for name, path in sources()}
    link_ok = True
    for kind, d in dirs.items():
        for exe in PROGRAMS:
            objs = [(dirs["ref"] if kind == "ours" and names[n] in failed else d) / f"{n}.o" for n in LIB + [exe]]
            res = run([CLANG, "-o", str(d / exe), *map(str, objs)], 300)
            if res["rc"] != 0:
                if kind == "ref":
                    raise SystemExit(f"clang reference {exe} failed to link:\n{text(res)}")
                link_ok = False
                result["link_stderr"] = result.get("link_stderr", "") + text(res)[-2000:]
    result["link_ok"] = link_ok
    return result


def corpus() -> list[tuple[str, bytes]]:
    """Deterministic inputs: the same bytes on every run and machine, apart
    from the repository sources, which both builds see alike."""
    rng = random.Random(20261124)
    words = [b"alpha", b"beta", b"gamma", b"delta", b"compress", b"zlib", b"\n", b" ", b"{", b"}"]
    text_like = b"".join(rng.choice(words) for _ in range(200_000))
    noise = bytes(rng.getrandbits(8) for _ in range(300_000))
    runs = b"".join(bytes([rng.randrange(4)]) * rng.randrange(1, 300) for _ in range(5_000))
    skewed = bytes(min(255, int(rng.expovariate(0.05))) for _ in range(300_000))
    rust = b"".join(p.read_bytes() for p in sorted((ROOT / "src").rglob("*.rs")))[:3_000_000]
    items = [
        ("empty", b""),
        ("one-byte", b"x"),
        ("zeros-1M", bytes(1 << 20)),
        ("text-like", text_like),
        ("random-300k", noise),
        ("runs", runs),
        ("skewed", skewed),
        ("mixed", (text_like[:100_000] + noise[:50_000] + runs[:100_000]) * 3),
        ("hcc-sources", rust),
        ("zlib.h", (SRC / "zlib.h").read_bytes()),
    ]
    return items


def round_trip(ref_dir: Path, our_dir: Path) -> dict:
    rows = []
    for name, data in corpus():
        for mode in MODES:
            a = run([str(ref_dir / "minigzip"), *mode], 120, data=data)
            b = run([str(our_dir / "minigzip"), *mode], 120, data=data)
            ok_a = a["rc"] == 0 and not a["timed_out"]
            ok_b = b["rc"] == 0 and not b["timed_out"]
            same = ok_a and ok_b and a["stdout"] == b["stdout"]
            back_ours = run([str(our_dir / "minigzip"), "-d"], 120, data=a["stdout"]) if ok_a else None
            back_ref = run([str(ref_dir / "minigzip"), "-d"], 120, data=b["stdout"]) if ok_b else None
            row = {
                "input": name, "mode": " ".join(mode), "bytes": len(data),
                "compressed": len(a["stdout"]) if ok_a else None,
                "identical_stream": same,
                "ours_inflates_clang": bool(back_ours) and back_ours["rc"] == 0 and back_ours["stdout"] == data,
                "clang_inflates_ours": bool(back_ref) and back_ref["rc"] == 0 and back_ref["stdout"] == data,
            }
            if not ok_a:
                raise SystemExit(f"clang minigzip {' '.join(mode)} failed on {name}:\n{text(a)[-1000:]}")
            row["match"] = row["identical_stream"] and row["ours_inflates_clang"] and row["clang_inflates_ours"]
            rows.append(row)
    return {"cases": len(rows), "matching": sum(r["match"] for r in rows),
            "differ": [f"{r['input']} {r['mode']}" for r in rows if not r["match"]], "rows": rows}


def own_tests(ref_dir: Path, our_dir: Path) -> list[dict]:
    rows = []
    outs = {}
    for tag, d in (("ref", ref_dir), ("ours", our_dir)):
        work = Path(tempfile.mkdtemp(prefix="zlib-example-"))
        try:
            outs[tag] = run([str(d / "example")], 120, cwd=work)
        finally:
            shutil.rmtree(work, ignore_errors=True)
    if outs["ref"]["rc"] != 0:
        raise SystemExit(f"clang reference example failed:\n{text(outs['ref'])}")
    rows.append({"test": "example", "match": (outs["ours"]["rc"], outs["ours"]["stdout"], outs["ours"]["stderr"])
                 == (outs["ref"]["rc"], outs["ref"]["stdout"], outs["ref"]["stderr"]) and not outs["ours"]["timed_out"]})
    hello = b"hello world\n"
    packed = run([str(our_dir / "minigzip")], 60, data=hello)
    unpacked = run([str(our_dir / "minigzip"), "-d"], 60, data=packed["stdout"])
    rows.append({"test": "echo hello world | minigzip | minigzip -d",
                 "match": packed["rc"] == 0 and unpacked["rc"] == 0 and unpacked["stdout"] == hello})
    return rows


def level(opt: str, out: Path, jobs: int) -> dict:
    b = build(opt, out, jobs)
    result: dict = {"opt": opt, "build": b, "full": b["build"] == "full"}
    if not b["link_ok"]:
        result.update(match=False, tests=[], round_trip={"cases": 0, "matching": 0, "differ": ["(not built)"]})
        return result
    ref_dir, our_dir = Path(b["dirs"]["ref"]), Path(b["dirs"]["ours"])
    result["tests"] = own_tests(ref_dir, our_dir)
    result["round_trip"] = round_trip(ref_dir, our_dir)
    result["match"] = all(t["match"] for t in result["tests"]) and not result["round_trip"]["differ"]
    return result


def summary(res: dict) -> str:
    b = res["build"]
    total = len(b["files"])
    lines = [f"{res['opt']}: full={'yes' if res['full'] else 'no'} match={'yes' if res['match'] else 'no'}",
             f"    build: {b['build']}, {total - len(b['failed_files'])}/{total} files, link={'ok' if b['link_ok'] else 'FAIL'};"
             f" compile ours {b['ours_compile_seconds']:.1f}s clang {b['clang_compile_seconds']:.1f}s"]
    for f in b["files"]:
        if f["status"] != "ok":
            first = (f.get("stderr") or "").strip().splitlines()
            lines.append(f"      {f['status']}: {f['file']}: {first[0] if first else ''}")
    for t in res["tests"]:
        lines.append(f"    test {t['test']}: match={'yes' if t['match'] else 'no'}")
    rt = res["round_trip"]
    lines.append(f"    round trip: {rt['matching']}/{rt['cases']} cases identical and inverse"
                 + (f"; differ {rt['differ'][:20]}" if rt["differ"] else ""))
    return "\n".join(lines)


def verdict(levels: list[dict]) -> tuple[bool, str]:
    ok = bool(levels) and all(lvl["full"] and lvl["match"] for lvl in levels)
    return ok, "ZLIB: PASS" if ok else "ZLIB: FAIL"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--opt", action="append", choices=["O0", "O1", "O2", "O3"], help="levels (default O0 and O2)")
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    ap.add_argument("--out", type=Path, default=ROOT / "build" / "zlib")
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()
    if not (SRC / "zlib.h").is_file():
        print(f"zlib sources not found under {SRC}; run scripts/fetch_zlib.sh", file=sys.stderr)
        return 2
    # The tests run from temporary directories, so a relative --out would
    # no longer find the executables.
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    levels = []
    for opt in args.opt or ["O0", "O2"]:
        res = level(opt, args.out, args.jobs)
        print(summary(res), flush=True)
        levels.append(res)
    ok, line = verdict(levels)
    if args.json:
        args.json.write_text(json.dumps({"version": VERSION, "levels": levels}, indent=2, default=str))
    print(line)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
