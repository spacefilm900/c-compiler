#!/usr/bin/env python3
"""Run the Writing a C Compiler suite against build/compiler.

The suite is fetched by scripts/fetch_wacc_tests.sh into tests/external/wacc
(gitignored). This runner does not call the suite's test_compiler script.

A valid program is compiled with this compiler, linked with clang, and
compared to expected_results.json (return code and stdout). If that file has
no entry, clang -std=c11 is the oracle. The book's expected results are for
x86_64 (a signed `char`, x86-64 assembly helpers), so for any other target
(read from `clang -dumpmachine`) Clang is the oracle for every program and
the tests that link the book's x86-64 assembly are skipped. A library test compiles one
translation unit here and the other files with clang. An invalid program
must be rejected with a non-zero exit and must not crash or panic.

Chapters 19 and 20 are skipped. Chapter 13 (floating point) is run,
including its NaN tests. Goto, unions, and switch are run. Structs
passed or returned by value are run.
harness/wacc_known_failures.txt lists the remaining failures. A test that is
not listed must pass. A listed test that passes is reported, not failed.
A compiler crash or panic fails the run even when the test is listed.

Env: CC_UNDER_TEST, CLANG, TIMEOUT (seconds per command, default 10),
WACC_ROOT (the fetched suite, default tests/external/wacc).
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "harness"))
from target_arch import arch_of, clang_triple  # noqa: E402
COMPILER = Path(os.environ.get("CC_UNDER_TEST", ROOT / "build" / "compiler"))
CLANG = os.environ.get("CLANG", "clang")
TIMEOUT = float(os.environ.get("TIMEOUT", "10"))
SUITE = Path(os.environ.get("WACC_ROOT", ROOT / "tests" / "external" / "wacc"))
KNOWN_PATH = ROOT / "harness" / "wacc_known_failures.txt"

# Part I, long, unsigned, pointers, arrays, characters, void/sizeof, structs.
INCLUDED_CHAPTERS = set(range(1, 19))
CHAPTER_SKIP = {
    19: "chapter skipped: part III optimizations inspect the book's assembly",
    20: "chapter skipped: part III register allocation inspects the book's assembly",
}
# Extra-credit features with no frontend support. Bitwise, compound
# assignment, increment, floating point, NaN, unions, switch, and goto
# are implemented, so those tests are run.
UNSUPPORTED_EXTRA = set()
INVALID_MARKERS = (
    "invalid_lex",
    "invalid_parse",
    "invalid_semantics",
    "invalid_declarations",
    "invalid_types",
    "invalid_labels",
    "invalid_struct_tags",
)
ASM_SUFFIX = "_osx.s" if platform.system().lower() == "darwin" else "_linux.s"


def chapter_of(rel: str) -> int:
    return int(rel.split("/", 1)[0].split("_", 1)[1])


def props_key(rel: str) -> str:
    """expected_results.json key. A *_client.c file shares the library's key."""
    parent, name = rel.rsplit("/", 1) if "/" in rel else ("", rel)
    stem, dot, suffix = name.rpartition(".")
    if stem.endswith("_client"):
        stem = stem[: -len("_client")]
    name = f"{stem}{dot}{suffix}"
    return f"{parent}/{name}" if parent else name


def is_invalid(rel: str) -> bool:
    return any(part in INVALID_MARKERS for part in rel.split("/"))


def skip_reason(rel: str, tags: set[str]) -> str | None:
    chapter = chapter_of(rel)
    if chapter in CHAPTER_SKIP:
        return CHAPTER_SKIP[chapter]
    if is_invalid(rel):
        return None
    missing = sorted(UNSUPPORTED_EXTRA.intersection(tags))
    if missing:
        return "extra-credit not started: " + ", ".join(missing)
    return None


def load_known(path: Path) -> dict[str, str]:
    known: dict[str, str] = {}
    if not path.is_file():
        return known
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        text = line.strip()
        if not text or text.startswith("#"):
            continue
        rel, _, reason = text.partition(" ")
        reason = reason.strip() or "known"
        if rel in known:
            sys.exit(f"{path}:{lineno}: duplicate entry for {rel}")
        known[rel] = reason
    return known


def load_suite(suite: Path) -> tuple[dict, dict[str, set[str]], dict, dict, set[str]]:
    try:
        expected = json.loads((suite / "expected_results.json").read_text(encoding="utf-8"))
        props = json.loads((suite / "test_properties.json").read_text(encoding="utf-8"))
    except OSError as err:
        sys.exit(
            f"cannot read the test suite in {suite} ({err}). "
            "Run scripts/fetch_wacc_tests.sh first."
        )
    tags = {key: set(value) for key, value in props["extra_credit_tests"].items()}
    return expected, tags, props["libs"], props["assembly_libs"], set(props["requires_mathlib"])


def discover(suite: Path, tags: dict[str, set[str]]) -> list[dict]:
    tests_dir = suite / "tests"
    found = []
    for path in sorted(tests_dir.rglob("*.c")):
        rel = path.relative_to(tests_dir).as_posix()
        if "helper_libs" in rel.split("/"):
            continue
        chapter = chapter_of(rel)
        if chapter not in INCLUDED_CHAPTERS and chapter not in CHAPTER_SKIP:
            sys.exit(f"unexpected chapter directory: {rel}")
        kind = "invalid" if is_invalid(rel) else "valid"
        if chapter in CHAPTER_SKIP:
            kind = "skipped"
        # A *_client.c file is not listed on its own in test_properties.json.
        # The extra-credit tags live on the library path.
        key = props_key(rel)
        tagged = tags.get(rel, set()) | tags.get(key, set())
        found.append(
            {
                "rel": rel,
                "chapter": chapter,
                "kind": kind,
                "path": str(path),
                "skip": skip_reason(rel, tagged),
            }
        )
    return found


def partner_of(path: Path) -> Path | None:
    if "libraries" not in path.parts:
        return None
    stem = path.stem
    if stem.endswith("_client"):
        other = path.with_name(stem[: -len("_client")] + ".c")
    else:
        other = path.with_name(stem + "_client.c")
    if not other.is_file():
        return None
    return other


def extra_sources(rel: str, suite: Path, c_libs: dict, asm_libs: dict) -> list[str]:
    key = props_key(rel)
    paths = []
    for name in c_libs.get(key, []):
        paths.append(str(suite / "tests" / name))
    for name in asm_libs.get(key, []):
        paths.append(str(suite / "tests" / f"{name}{ASM_SUFFIX}"))
    return paths


def clip(text: str, limit: int = 240) -> str:
    flat = " ".join(text.split())
    if len(flat) > limit:
        return flat[:limit] + "..."
    return flat


def is_compiler_crash(rc, err: str) -> bool:
    if isinstance(rc, str):
        return False
    if rc < 0:
        return True
    low = err.lower()
    return (
        "panicked at" in low
        or "stack backtrace" in low
        or "fatal runtime error" in low
        or "internal compiler error" in low
    )


def run_cmd(cmd: list[str], timeout: float, cwd: str) -> tuple:
    try:
        proc = subprocess.run(
            cmd,
            cwd=cwd,
            capture_output=True,
            timeout=timeout,
            encoding="utf-8",
            errors="replace",
        )
        return proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired as err:
        out = err.stdout or ""
        err_text = err.stderr or ""
        if isinstance(out, bytes):
            out = out.decode("utf-8", "replace")
        if isinstance(err_text, bytes):
            err_text = err_text.decode("utf-8", "replace")
        return "timeout", out, err_text
    except OSError as err:
        return "oserror", "", str(err)


def compile_c(clang: str, source: str, obj: str, timeout: float, cwd: str) -> tuple:
    return run_cmd(
        [clang, "-w", "-std=c11", "-c", source, "-o", obj],
        timeout,
        cwd,
    )


def link_and_run(
    clang: str,
    objects: list[str],
    needs_lm: bool,
    timeout: float,
    cwd: str,
    tmp: Path,
) -> tuple:
    exe = tmp / "prog"
    # libm is linked for every program. Chapter 13 calls copysign and
    # fma from tests the suite does not list in requires_mathlib.
    del needs_lm
    cmd = [clang, "-w", "-o", str(exe), *objects, "-lm"]
    rc, _out, err = run_cmd(cmd, timeout, cwd)
    if rc != 0:
        return None, f"link failed ({rc}): {clip(err)}"
    rc, out, err = run_cmd([str(exe)], timeout, cwd)
    if rc == "timeout":
        return None, "program timed out"
    if rc == "oserror":
        return None, f"could not run the program: {clip(err)}"
    if isinstance(rc, int) and rc < 0:
        return None, f"program killed by signal {-rc}"
    return {"return_code": rc, "stdout": out, "stderr": err}, None


def oracle_result(task: dict, tmp: Path) -> tuple:
    """Compile every translation unit with clang and run the program."""
    objects = []
    sources = [task["path"]]
    if task["partner"]:
        sources.append(task["partner"])
    index = 0
    for source in sources + task["extras"]:
        if source.endswith(".c"):
            obj = str(tmp / f"oracle{index}.o")
            index += 1
            rc, _out, err = compile_c(task["clang"], source, obj, task["timeout"], task["cwd"])
            if rc != 0:
                return None, f"oracle compile failed ({rc}): {clip(err)}"
            objects.append(obj)
        else:
            objects.append(source)
    return link_and_run(
        task["clang"],
        objects,
        task["needs_lm"],
        task["timeout"],
        task["cwd"],
        tmp,
    )


def finish(task: dict, status: str, detail: str = "") -> dict:
    return {
        "rel": task["rel"],
        "chapter": task["chapter"],
        "kind": task["kind"],
        "status": status,
        "detail": detail,
    }


def execute(task: dict) -> dict:
    try:
        return run_one(task)
    except Exception as err:  # a runner bug, not a compiler panic
        return finish(task, "fail", f"runner error: {err}")


def run_one(task: dict) -> dict:
    with tempfile.TemporaryDirectory(prefix="wacc-") as directory:
        tmp = Path(directory)
        ll = tmp / "out.ll"
        rc, _out, err = run_cmd(
            [task["compiler"], task["path"], "-o", str(ll)],
            task["timeout"],
            task["cwd"],
        )
        if is_compiler_crash(rc, err):
            return finish(task, "panic", f"compiler crashed ({rc}): {clip(err)}")
        if rc == "timeout":
            if task["kind"] == "invalid":
                return finish(task, "panic", "compiler timed out on invalid input")
            return finish(task, "fail", "compiler timed out")
        if rc == "oserror":
            return finish(task, "fail", f"could not run the compiler: {clip(err)}")
        if task["kind"] == "invalid":
            if rc == 0:
                return finish(task, "fail", "compiler accepted an invalid program")
            return finish(task, "pass")
        if rc != 0:
            return finish(task, "fail", f"compiler rejected: {clip(err)}")
        if not ll.is_file():
            return finish(task, "fail", "compiler exited 0 but wrote no IR")

        objects = [str(ll)]
        index = 0
        others = []
        if task["partner"]:
            others.append(task["partner"])
        others.extend(task["extras"])
        for source in others:
            if source.endswith(".c"):
                obj = str(tmp / f"other{index}.o")
                index += 1
                other_rc, _o, other_err = compile_c(
                    task["clang"], source, obj, task["timeout"], task["cwd"]
                )
                if other_rc != 0:
                    return finish(
                        task,
                        "fail",
                        f"clang failed to compile {Path(source).name} ({other_rc}): {clip(other_err)}",
                    )
                objects.append(obj)
            else:
                if not Path(source).is_file():
                    return finish(task, "fail", f"missing linked file {source}")
                objects.append(source)

        got, problem = link_and_run(
            task["clang"],
            objects,
            task["needs_lm"],
            task["timeout"],
            task["cwd"],
            tmp,
        )
        if problem:
            return finish(task, "fail", problem)
        expected = task["expected"]
        if expected is None:
            expected, problem = oracle_result(task, tmp)
            if problem:
                return finish(task, "fail", problem)
        exp_rc = expected["return_code"]
        exp_out = expected.get("stdout", "")
        if got["return_code"] != exp_rc or got["stdout"] != exp_out:
            return finish(
                task,
                "fail",
                "mismatch: "
                f"got exit={got['return_code']} stdout={got['stdout'][:120]!r}; "
                f"expected exit={exp_rc} stdout={exp_out[:120]!r}",
            )
        if got["stderr"]:
            return finish(task, "fail", f"unexpected stderr: {clip(got['stderr'])}")
        return finish(task, "pass")


def build_tasks(cases: list[dict], suite: Path, expected, c_libs, asm_libs, mathlibs) -> list[dict]:
    tasks = []
    for case in cases:
        if case["skip"]:
            continue
        path = Path(case["path"])
        partner = partner_of(path)
        key = props_key(case["rel"])
        tasks.append(
            {
                **case,
                "partner": str(partner) if partner else None,
                "extras": extra_sources(case["rel"], suite, c_libs, asm_libs),
                "expected": expected.get(key),
                "needs_lm": key in mathlibs,
                "compiler": str(COMPILER),
                "clang": CLANG,
                "timeout": TIMEOUT,
                "cwd": str(ROOT),
            }
        )
    return tasks


def classify(result: dict, known: dict[str, str]) -> dict:
    status = result["status"]
    if status == "pass" and result["rel"] in known:
        result["status"] = "known_pass"
        result["reason"] = known[result["rel"]]
    elif status == "fail" and result["rel"] in known:
        result["status"] = "known_fail"
        result["reason"] = known[result["rel"]]
    elif status == "panic" and result["rel"] in known:
        result["reason"] = known[result["rel"]]
    return result


def print_table(rows: list[tuple]) -> None:
    headers = (
        "chapter",
        "run",
        "passed",
        "known failures",
        "skipped",
        "now passing",
        "unexpected",
    )
    print("\t".join(headers))
    for row in rows:
        print("\t".join(str(cell) for cell in row))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--chapter", type=int, action="append", dest="chapters")
    parser.add_argument("--only", action="append", help="run tests whose path contains this text")
    parser.add_argument("--jobs", type=int, default=0)
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--list", action="store_true", help="print tests and skip reasons, then exit")
    args = parser.parse_args()

    if not (COMPILER.is_file() and os.access(COMPILER, os.X_OK)):
        sys.exit(f"compiler not found or not executable: {COMPILER}")
    if shutil.which(CLANG) is None:
        sys.exit(f"clang not found: {CLANG}")
    if not SUITE.is_dir():
        sys.exit(f"test suite not found: {SUITE}. Run scripts/fetch_wacc_tests.sh.")

    expected, tag_map, c_libs, asm_libs, mathlibs = load_suite(SUITE)
    known = load_known(KNOWN_PATH)
    cases = discover(SUITE, tag_map)
    triple = clang_triple(CLANG)
    arch = arch_of(triple)
    if arch != "x86_64":
        print(f"target {triple or 'unknown'}: clang is the oracle for every program")
        expected = {}
        for case in cases:
            if not case["skip"] and case["kind"] == "valid" and asm_libs.get(props_key(case["rel"])):
                case["skip"] = f"links the book's x86-64 assembly helper (target is {arch or 'unknown'})"

    if args.chapters or args.only:
        wanted = set(args.chapters or [])
        needles = args.only or []

        def selected(case: dict) -> bool:
            if wanted and case["chapter"] not in wanted:
                return False
            if needles and not any(needle in case["rel"] for needle in needles):
                return False
            return True

        cases = [case for case in cases if selected(case)]

    if args.list:
        for case in cases:
            mark = case["skip"] or case["kind"]
            print(f"{case['rel']}\t{mark}")
        return 0

    tasks = build_tasks(cases, SUITE, expected, c_libs, asm_libs, mathlibs)
    jobs = args.jobs or min(8, os.cpu_count() or 1)
    results = []
    if tasks:
        with ProcessPoolExecutor(max_workers=jobs) as pool:
            futures = [pool.submit(execute, task) for task in tasks]
            done = 0
            for future in as_completed(futures):
                result = classify(future.result(), known)
                results.append(result)
                done += 1
                if result["status"] in {"fail", "panic", "known_pass"}:
                    print(f"{result['status'].upper()} {result['rel']}: {result['detail']}", flush=True)
                if done % 50 == 0 or done == len(tasks):
                    print(f"... {done}/{len(tasks)}", file=sys.stderr, flush=True)

    by_chapter: dict[int, dict[str, int]] = {}
    skip_reasons: dict[str, int] = {}
    for case in cases:
        row = by_chapter.setdefault(
            case["chapter"],
            {"run": 0, "passed": 0, "known": 0, "skipped": 0, "now": 0, "fail": 0, "panic": 0},
        )
        if case["skip"]:
            row["skipped"] += 1
            skip_reasons[case["skip"]] = skip_reasons.get(case["skip"], 0) + 1
    for result in results:
        row = by_chapter[result["chapter"]]
        row["run"] += 1
        status = result["status"]
        if status == "pass":
            row["passed"] += 1
        elif status == "known_fail":
            row["known"] += 1
        elif status == "known_pass":
            row["now"] += 1
        elif status == "panic":
            row["panic"] += 1
        else:
            row["fail"] += 1

    print()
    table = []
    totals = {"run": 0, "passed": 0, "known": 0, "skipped": 0, "now": 0, "fail": 0, "panic": 0}
    for chapter in sorted(by_chapter):
        row = by_chapter[chapter]
        for key in totals:
            totals[key] += row[key]
        table.append(
            (
                chapter,
                row["run"],
                row["passed"],
                row["known"],
                row["skipped"],
                row["now"],
                row["fail"] + row["panic"],
            )
        )
    table.append(
        (
            "total",
            totals["run"],
            totals["passed"],
            totals["known"],
            totals["skipped"],
            totals["now"],
            totals["fail"] + totals["panic"],
        )
    )
    print_table(table)

    if skip_reasons:
        print("\nSkip reasons:")
        for reason in sorted(skip_reasons):
            print(f"  {skip_reasons[reason]:4d}  {reason}")

    categories: dict[str, int] = {}
    for result in results:
        if result["status"] == "known_fail":
            categories[result["reason"]] = categories.get(result["reason"], 0) + 1
    if categories:
        print("\nKnown-failure categories:")
        for reason in sorted(categories, key=lambda item: (-categories[item], item)):
            print(f"  {categories[reason]:4d}  {reason}")

    unexpected = [result for result in results if result["status"] in {"fail", "panic"}]
    unexpected.sort(key=lambda result: result["rel"])
    if unexpected:
        print("\nUnexpected failures:")
        for result in unexpected:
            print(f"  {result['status'].upper()} {result['rel']}: {result['detail']}")

    now_passing = sorted(result["rel"] for result in results if result["status"] == "known_pass")
    if now_passing:
        print("\nListed tests that now pass (remove them from harness/wacc_known_failures.txt):")
        for rel in now_passing:
            print(f"  {rel}")

    ran = {result["rel"] for result in results}
    full_run = not args.chapters and not args.only
    stale = sorted(rel for rel in known if rel not in ran) if full_run else []
    if stale:
        print("\nKnown-failure entries that were not run:")
        for rel in stale:
            print(f"  {rel}")

    if args.json_out:
        args.json_out.write_text(
            json.dumps({"results": results, "table": table}, indent=2) + "\n",
            encoding="utf-8",
        )

    if unexpected or stale:
        print(
            f"\n{totals['passed']} passed, {totals['known']} known failures, "
            f"{len(unexpected)} unexpected, {totals['skipped']} skipped"
        )
        return 1
    print(
        f"\n{totals['passed']} passed, {totals['known']} known failures, "
        f"{totals['now']} now passing, {totals['skipped']} skipped"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
