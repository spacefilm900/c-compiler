#!/usr/bin/env python3
"""Local Csmith differential campaign.

This is the repository's campaign runner. It is original code, written
to replace the challenge pack's tool for in-tree use. It always links
executables with -lm, at both -O0 and -O2.

Default mode is gnu11: the candidate is given -std=gnu11, and GCC and
Clang are invoked in their default GNU mode, without -pedantic-errors.
Csmith is given size caps and --no-packed-struct so programs usually
finish in seconds and layouts stay comparable.

A result is a valid differential comparison only when GCC and Clang
agree on a completed run and the candidate also runs to completion
(agreement or a wrong-output mismatch).

Usage (from the repository root, after `make` and `scripts/fetch_csmith.sh`):

  python3 tools/csmith_campaign.py \\
    --compiler build/compiler \\
    --seed-start 1 --count 500 \\
    --opt O0 --opt O2 \\
    --out /tmp/csmith-coverage

Shards use different `--seed-start` / `--count` values and the same
`--out`. Re-running a range skips seeds that already have a complete
result. `--summarize DIR` rebuilds SUMMARY.json from the seed tree.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import resource
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent

DEFAULT_CSMITH_OPTIONS = [
    "--no-packed-struct",
    "--no-argc",
    "--max-funcs",
    "4",
    "--max-block-depth",
    "3",
    "--max-block-size",
    "3",
    "--max-expr-complexity",
    "4",
    "--max-array-len-per-dim",
    "4",
    "--max-array-dim",
    "2",
    "--concise",
]

VALID_COMPARISON_STATUSES = frozenset(
    {"agreement", "candidate-wrong-output"}
)

DEFAULT_COMPILER = ROOT / "build" / "compiler"
DEFAULT_CSMITH = (
    ROOT / "tests" / "external" / "csmith" / "install" / "bin" / "csmith"
)
DEFAULT_INCLUDE = (
    ROOT / "tests" / "external" / "csmith" / "install" / "include" / "csmith-2.3.0"
)
# Fall back to the build tree's runtime headers, which is what the CI
# runner uses if the install include path is missing.
DEFAULT_INCLUDE_FALLBACK = (
    ROOT / "tests" / "external" / "csmith" / "src" / "runtime"
)

GENERATE_TIMEOUT = 30.0
COMPILE_TIMEOUT = 60.0


def reference_compile_cmd(
    compiler: str,
    source: str,
    output: str,
    opt: str,
    include: str,
    mode: str,
) -> list[str]:
    cmd = [compiler]
    if mode == "strict":
        cmd.append("-std=c11")
    cmd += [f"-{opt}", "-w"]
    if include:
        cmd += ["-I", include]
    cmd += [source, "-o", output, "-lm"]
    return cmd


def candidate_compile_cmd(
    compiler: str,
    source: str,
    output: str,
    include: str,
    mode: str,
) -> list[str]:
    cmd = [compiler]
    if mode == "gnu11":
        cmd.append("-std=gnu11")
    if include:
        cmd += ["-I", include]
    cmd += [source, "-o", output]
    return cmd


def candidate_link_cmd(
    clang: str, ir: str, output: str, opt: str
) -> list[str]:
    return [clang, f"-{opt}", "-w", ir, "-o", output, "-lm"]


def is_valid_comparison(status: str) -> bool:
    return status in VALID_COMPARISON_STATUSES


def _is_signal(code: Any) -> bool:
    if isinstance(code, int) and code < 0:
        return True
    return isinstance(code, str) and code.upper().startswith("SIG")


def _finished_run(run: dict[str, Any] | None) -> bool:
    if not run:
        return False
    code = run.get("code")
    if code in (None, "timeout", "oserror") or _is_signal(code):
        return False
    return isinstance(code, int)


def _run_crashed(run: dict[str, Any] | None) -> bool:
    return bool(run and _is_signal(run.get("code")))


def _run_signature(run: dict[str, Any] | None) -> tuple[Any, str] | None:
    if not _finished_run(run):
        return None
    digest = run.get("stdout_sha256")
    if digest:
        return (run.get("code"), digest)
    return (run.get("code"), run.get("stdout", ""))


def _is_ice(build: dict[str, Any] | None) -> bool:
    if not build:
        return False
    err = (build.get("stderr") or "").lower()
    if "panicked at" in err:
        return True
    code = build.get("code")
    if isinstance(code, int) and code < 0:
        return True
    if isinstance(code, str) and code not in ("timeout", "oserror", ""):
        # A signal name from the runner, e.g. "SIGSEGV".
        if code.upper().startswith("SIG"):
            return True
    return False


def classify_seed(
    *,
    gcc_build: dict[str, Any] | None,
    clang_build: dict[str, Any] | None,
    candidate_build: dict[str, Any] | None,
    candidate_link: dict[str, Any] | None,
    gcc_run: dict[str, Any] | None,
    clang_run: dict[str, Any] | None,
    candidate_run: dict[str, Any] | None,
) -> str:
    if _is_ice(candidate_build) or _is_ice(candidate_link):
        return "candidate-crash-or-ice"

    gcc_compiled = bool(gcc_build and gcc_build.get("code") == 0)
    clang_compiled = bool(clang_build and clang_build.get("code") == 0)
    cand_compiled = bool(candidate_build and candidate_build.get("code") == 0)
    cand_linked = bool(candidate_link and candidate_link.get("code") == 0)

    gcc_build_timeout = bool(gcc_build and gcc_build.get("code") == "timeout")
    clang_build_timeout = bool(
        clang_build and clang_build.get("code") == "timeout"
    )

    if gcc_build_timeout or clang_build_timeout:
        return "reference-timeout"

    if not gcc_compiled and not clang_compiled:
        return "invalid-program"

    gcc_done = gcc_compiled and _finished_run(gcc_run)
    clang_done = clang_compiled and _finished_run(clang_run)
    gcc_timeout = gcc_compiled and gcc_run is not None and gcc_run.get("code") == "timeout"
    clang_timeout = (
        clang_compiled and clang_run is not None and clang_run.get("code") == "timeout"
    )

    if gcc_timeout or clang_timeout or not gcc_done or not clang_done:
        # One reference rejected the program while the other ran, or a
        # run did not finish. Signal deaths are not timeouts.
        if gcc_compiled and clang_compiled:
            if _run_crashed(gcc_run) or _run_crashed(clang_run):
                return "reference-crash"
            return "reference-timeout"
        return "invalid-program"

    gcc_sig = _run_signature(gcc_run)
    clang_sig = _run_signature(clang_run)
    if gcc_sig != clang_sig:
        return "reference-disagreement"

    if not cand_compiled:
        if candidate_build and candidate_build.get("code") == "timeout":
            return "candidate-timeout"
        return "candidate-rejects-valid-gnu-c"
    if not cand_linked:
        if _is_ice(candidate_link):
            return "candidate-crash-or-ice"
        if candidate_link and candidate_link.get("code") == "timeout":
            return "candidate-timeout"
        return "candidate-bad-ir"
    if not _finished_run(candidate_run):
        if _run_crashed(candidate_run) or _is_ice(candidate_run):
            return "candidate-crash-or-ice"
        return "candidate-timeout"
    if _run_signature(candidate_run) != gcc_sig:
        return "candidate-wrong-output"
    return "agreement"


@dataclass
class CampaignConfig:
    compiler: str
    csmith: str
    gcc: str
    clang: str
    csmith_include: str
    seed_start: int
    count: int
    opts: list[str]
    timeout: float
    out: Path
    mode: str = "gnu11"
    csmith_options: list[str] = field(
        default_factory=lambda: list(DEFAULT_CSMITH_OPTIONS)
    )
    keep_binaries: bool = True


def _limit_program(cpu: int) -> None:
    resource.setrlimit(resource.RLIMIT_CPU, (cpu, cpu))
    resource.setrlimit(resource.RLIMIT_AS, (1 << 31, 1 << 31))


def call(
    cmd: list[str],
    cwd: Path,
    timeout: float,
    *,
    limit: bool = False,
    cpu: int = 0,
    truncate: int = 4000,
) -> dict[str, Any]:
    rec: dict[str, Any] = {"cmd": cmd}
    try:
        proc = subprocess.run(
            cmd,
            cwd=cwd,
            capture_output=True,
            timeout=timeout,
            preexec_fn=(lambda: _limit_program(cpu)) if limit else None,
        )
        rec["code"] = proc.returncode
        rec["stdout_sha256"] = hashlib.sha256(proc.stdout or b"").hexdigest()
        rec["stderr_sha256"] = hashlib.sha256(proc.stderr or b"").hexdigest()
        stdout = proc.stdout.decode(errors="replace")
        stderr = proc.stderr.decode(errors="replace")
        if truncate and len(stdout) > truncate:
            stdout = stdout[-truncate:]
        if truncate and len(stderr) > truncate:
            stderr = stderr[-truncate:]
        rec["stdout"] = stdout
        rec["stderr"] = stderr
    except subprocess.TimeoutExpired as exc:
        rec["code"] = "timeout"
        out_b = exc.stdout or b""
        err_b = exc.stderr or b""
        rec["stdout_sha256"] = hashlib.sha256(out_b).hexdigest()
        rec["stderr_sha256"] = hashlib.sha256(err_b).hexdigest()
        rec["stdout"] = out_b.decode(errors="replace")[-truncate:]
        rec["stderr"] = err_b.decode(errors="replace")[-truncate:]
    except OSError as exc:
        rec["code"] = "oserror"
        rec["stdout"] = ""
        rec["stderr"] = str(exc)
    return rec


def write_json(path: Path, obj: Any) -> None:
    """Write JSON by replacing a temporary file in the same directory.

    Each call gets its own temporary name, so concurrent shards writing the
    same destination never replace or truncate each other's partial file.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix=path.name + ".", suffix=".tmp", dir=path.parent)
    tmp = Path(name)
    try:
        os.fchmod(fd, 0o644)
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.write(json.dumps(obj, indent=2) + "\n")
        tmp.replace(path)
    except BaseException:
        tmp.unlink(missing_ok=True)
        raise


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def _complete(seed_dir: Path, opts: list[str]) -> bool:
    source = seed_dir / "source.c"
    if not source.is_file():
        return False
    return all((seed_dir / opt / "result.json").is_file() for opt in opts)


def generate_source(cfg: CampaignConfig, seed: int, seed_dir: Path) -> dict[str, Any]:
    seed_dir.mkdir(parents=True, exist_ok=True)
    source = seed_dir / "source.c"
    if source.is_file() and source.stat().st_size > 0:
        gen_path = seed_dir / "generation.json"
        if gen_path.is_file():
            return json.loads(gen_path.read_text(encoding="utf-8"))
    cmd = [
        cfg.csmith,
        "--seed",
        str(seed),
        *cfg.csmith_options,
        "-o",
        str(source),
    ]
    gen = call(cmd, seed_dir, GENERATE_TIMEOUT, truncate=2000)
    (seed_dir / "generation.json").write_text(
        json.dumps(gen, indent=2) + "\n", encoding="utf-8"
    )
    if gen["code"] == 0 and source.is_file():
        (seed_dir / "source.sha256").write_text(
            _sha256(source) + "\n", encoding="utf-8"
        )
    return gen


def run_one_opt(
    cfg: CampaignConfig, seed: int, seed_dir: Path, opt: str
) -> dict[str, Any]:
    work = seed_dir / opt
    work.mkdir(parents=True, exist_ok=True)
    source = seed_dir / "source.c"
    include = cfg.csmith_include
    gcc_exe = work / "gcc.exe"
    clang_exe = work / "clang.exe"
    cand_ir = work / "candidate.ll"
    cand_exe = work / "candidate.exe"
    wall = cfg.timeout * 2
    cpu = max(1, int(cfg.timeout))

    gcc_build = call(
        reference_compile_cmd(
            cfg.gcc, str(source), str(gcc_exe), opt, include, cfg.mode
        ),
        work,
        COMPILE_TIMEOUT,
    )
    clang_build = call(
        reference_compile_cmd(
            cfg.clang, str(source), str(clang_exe), opt, include, cfg.mode
        ),
        work,
        COMPILE_TIMEOUT,
    )
    cand_build = call(
        candidate_compile_cmd(
            cfg.compiler, str(source), str(cand_ir), include, cfg.mode
        ),
        work,
        COMPILE_TIMEOUT,
    )

    cand_link: dict[str, Any] | None = None
    if cand_build.get("code") == 0 and cand_ir.is_file():
        cand_link = call(
            candidate_link_cmd(cfg.clang, str(cand_ir), str(cand_exe), opt),
            work,
            COMPILE_TIMEOUT,
        )

    gcc_run = (
        call([str(gcc_exe)], work, wall, limit=True, cpu=cpu)
        if gcc_build.get("code") == 0 and gcc_exe.exists()
        else None
    )
    clang_run = (
        call([str(clang_exe)], work, wall, limit=True, cpu=cpu)
        if clang_build.get("code") == 0 and clang_exe.exists()
        else None
    )
    cand_run = (
        call([str(cand_exe)], work, wall, limit=True, cpu=cpu)
        if cand_link and cand_link.get("code") == 0 and cand_exe.exists()
        else None
    )

    status = classify_seed(
        gcc_build=gcc_build,
        clang_build=clang_build,
        candidate_build=cand_build,
        candidate_link=cand_link,
        gcc_run=gcc_run,
        clang_run=clang_run,
        candidate_run=cand_run,
    )
    rec = {
        "seed": seed,
        "opt": opt,
        "mode": cfg.mode,
        "status": status,
        "valid_comparison": is_valid_comparison(status),
        "source_sha256": _sha256(source) if source.is_file() else "",
        "runs": {
            "gcc": {"build": gcc_build, "run": gcc_run},
            "clang": {"build": clang_build, "run": clang_run},
            "candidate": {
                "build": cand_build,
                "link": cand_link,
                "run": cand_run,
            },
        },
    }
    write_json(work / "result.json", rec)
    if not cfg.keep_binaries:
        for path in (gcc_exe, clang_exe, cand_exe):
            if path.exists():
                path.unlink()
    return rec


def _empty_totals(opts: list[str]) -> dict[str, Any]:
    return {
        opt: {
            "agreement": 0,
            "candidate-wrong-output": 0,
            "candidate-crash-or-ice": 0,
            "candidate-rejects-valid-gnu-c": 0,
            "candidate-bad-ir": 0,
            "candidate-timeout": 0,
            "reference-timeout": 0,
            "reference-crash": 0,
            "reference-disagreement": 0,
            "invalid-program": 0,
            "generation-failed": 0,
        }
        for opt in opts
    }


def summarize_tree(out: Path, opts: list[str] | None = None) -> dict[str, Any]:
    seeds_root = out / "seeds"
    records: list[dict[str, Any]] = []
    found_opts: set[str] = set(opts or [])
    if seeds_root.is_dir():
        for seed_dir in sorted(seeds_root.iterdir(), key=lambda p: _seed_key(p.name)):
            if not seed_dir.is_dir():
                continue
            for child in seed_dir.iterdir():
                result_path = child / "result.json" if child.is_dir() else None
                if result_path and result_path.is_file():
                    rec = json.loads(result_path.read_text(encoding="utf-8"))
                    records.append(
                        {
                            "seed": rec.get("seed"),
                            "opt": rec.get("opt", child.name),
                            "status": rec.get("status"),
                            "valid_comparison": rec.get("valid_comparison"),
                        }
                    )
                    found_opts.add(rec.get("opt", child.name))
            gen_path = seed_dir / "generation.json"
            src = seed_dir / "source.c"
            if gen_path.is_file() and not src.is_file():
                gen = json.loads(gen_path.read_text(encoding="utf-8"))
                if gen.get("code") != 0:
                    seed_id = (
                        int(seed_dir.name)
                        if seed_dir.name.isdigit()
                        else seed_dir.name
                    )
                    have = {rec["opt"] for rec in records if rec["seed"] == seed_id}
                    for opt in sorted(found_opts or {"O0"}):
                        if opt in have:
                            continue
                        records.append(
                            {
                                "seed": seed_id,
                                "opt": opt,
                                "status": "generation-failed",
                                "valid_comparison": False,
                            }
                        )
    use_opts = opts or sorted(found_opts) or ["O0", "O2"]
    totals = _empty_totals(use_opts)
    valid = {opt: 0 for opt in use_opts}
    for rec in records:
        opt = rec["opt"]
        status = rec["status"]
        if opt not in totals:
            totals[opt] = _empty_totals([opt])[opt]
            valid[opt] = 0
        totals[opt].setdefault(status, 0)
        totals[opt][status] += 1
        if rec.get("valid_comparison") or is_valid_comparison(status):
            valid[opt] += 1
    summary = {
        "totals": totals,
        "valid_comparisons": valid,
        "records": records,
        "seed_count": len({rec["seed"] for rec in records}),
    }
    write_json(
        out / "SUMMARY.json",
        {k: summary[k] for k in ("totals", "valid_comparisons", "seed_count")}
        | {"record_count": len(records)},
    )
    return summary


def _seed_key(name: str) -> tuple[int, str]:
    return (int(name), name) if name.isdigit() else (10**18, name)


def run_campaign(cfg: CampaignConfig) -> dict[str, Any]:
    out = cfg.out
    out.mkdir(parents=True, exist_ok=True)
    (out / "seeds").mkdir(exist_ok=True)
    options = {
        "compiler": cfg.compiler,
        "csmith": cfg.csmith,
        "gcc": cfg.gcc,
        "clang": cfg.clang,
        "csmith_include": cfg.csmith_include,
        "seed_start": cfg.seed_start,
        "count": cfg.count,
        "opts": cfg.opts,
        "timeout": cfg.timeout,
        "mode": cfg.mode,
        "csmith_options": cfg.csmith_options,
    }
    shard_name = f"shard-{cfg.seed_start}-{cfg.seed_start + cfg.count - 1}.json"
    (out / "shards").mkdir(exist_ok=True)
    (out / "shards" / shard_name).write_text(
        json.dumps(options, indent=2) + "\n", encoding="utf-8"
    )
    (out / "options.json").write_text(
        json.dumps(options, indent=2) + "\n", encoding="utf-8"
    )

    for offset in range(cfg.count):
        seed = cfg.seed_start + offset
        seed_dir = out / "seeds" / str(seed)
        if _complete(seed_dir, cfg.opts):
            print(f"SKIP {seed} already complete", flush=True)
            continue
        gen = generate_source(cfg, seed, seed_dir)
        if gen.get("code") != 0 or not (seed_dir / "source.c").is_file():
            for opt in cfg.opts:
                work = seed_dir / opt
                work.mkdir(parents=True, exist_ok=True)
                rec = {
                    "seed": seed,
                    "opt": opt,
                    "mode": cfg.mode,
                    "status": "generation-failed",
                    "valid_comparison": False,
                    "generation": gen,
                }
                write_json(work / "result.json", rec)
                print(f"SEED {seed} {opt} generation-failed", flush=True)
            continue
        for opt in cfg.opts:
            result_path = seed_dir / opt / "result.json"
            if result_path.is_file():
                try:
                    old_rec = json.loads(result_path.read_text(encoding="utf-8"))
                except json.JSONDecodeError:
                    result_path.unlink()
                else:
                    if old_rec.get("status") == "generation-failed":
                        result_path.unlink()
        for opt in cfg.opts:
            if (seed_dir / opt / "result.json").is_file():
                print(f"SKIP {seed} {opt} already complete", flush=True)
                continue
            rec = run_one_opt(cfg, seed, seed_dir, opt)
            print(f"SEED {seed} {opt} {rec['status']}", flush=True)

    return summarize_tree(out, cfg.opts)


def _resolve_include(path: Path) -> Path:
    if (path / "csmith.h").is_file():
        return path
    if (DEFAULT_INCLUDE_FALLBACK / "csmith.h").is_file():
        return DEFAULT_INCLUDE_FALLBACK
    return path


def _tool_ok(exe: str) -> bool:
    return bool(shutil.which(exe) or Path(exe).is_file())


def resolve_tool(exe: str) -> str:
    path = Path(exe)
    if path.is_file():
        return str(path.resolve())
    found = shutil.which(exe)
    return found or exe


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n", 1)[0])
    p.add_argument("--compiler", "--candidate", dest="compiler", default=str(DEFAULT_COMPILER))
    p.add_argument("--csmith", default=str(DEFAULT_CSMITH))
    p.add_argument("--gcc", default="gcc")
    p.add_argument("--clang", default="clang")
    p.add_argument("--csmith-include", default=str(DEFAULT_INCLUDE))
    p.add_argument("--seed-start", type=int, default=1)
    p.add_argument("--count", type=int, default=100)
    p.add_argument(
        "--opt",
        action="append",
        choices=["O0", "O2"],
        dest="opts",
        help="Optimisation level. Repeat to run more than one. Default: O0 and O2.",
    )
    p.add_argument("--timeout", type=float, default=8.0, help="CPU run timeout in seconds (wall timeout is twice this).")
    p.add_argument("--out", default="csmith-campaign-results")
    p.add_argument("--mode", choices=["gnu11", "strict"], default="gnu11")
    p.add_argument(
        "--csmith-option",
        action="append",
        default=[],
        help="Extra Csmith option. Repeatable. Added after the default size caps.",
    )
    p.add_argument(
        "--no-default-csmith-options",
        action="store_true",
        help="Do not pass the default size caps or --no-packed-struct.",
    )
    p.add_argument(
        "--summarize",
        metavar="DIR",
        help="Rebuild SUMMARY.json from DIR and exit.",
    )
    p.add_argument(
        "--discard-binaries",
        action="store_true",
        help="Delete executables after each seed (source and result.json are kept).",
    )
    return p.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.summarize:
        summary = summarize_tree(Path(args.summarize).resolve())
        print(json.dumps({"totals": summary["totals"], "valid_comparisons": summary["valid_comparisons"]}, indent=2))
        return 0
    for exe in (args.compiler, args.csmith, args.gcc, args.clang):
        if not _tool_ok(exe):
            raise SystemExit(f"Not found: {exe}")
    include = _resolve_include(Path(args.csmith_include))
    if not (include / "csmith.h").is_file():
        raise SystemExit(f"csmith.h not found in {include}")
    opts = args.opts or ["O0", "O2"]
    csmith_options = [] if args.no_default_csmith_options else list(DEFAULT_CSMITH_OPTIONS)
    csmith_options.extend(args.csmith_option)
    cfg = CampaignConfig(
        compiler=resolve_tool(args.compiler),
        csmith=resolve_tool(args.csmith),
        gcc=resolve_tool(args.gcc),
        clang=resolve_tool(args.clang),
        csmith_include=str(include.resolve()),
        seed_start=args.seed_start,
        count=args.count,
        opts=opts,
        timeout=args.timeout,
        out=Path(args.out).resolve(),
        mode=args.mode,
        csmith_options=csmith_options,
        keep_binaries=not args.discard_binaries,
    )
    summary = run_campaign(cfg)
    print(
        json.dumps(
            {
                "totals": summary["totals"],
                "valid_comparisons": summary["valid_comparisons"],
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
