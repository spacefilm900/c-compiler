#!/usr/bin/env python3
"""Run the adversarial challenge kits (tests/adversarial/) on a real
platform: macOS, Windows (MSVC or MinGW-w64), or Linux, optionally under a
sanitizer.

The kits' own runner.py runs only on native Linux. This driver reads the
same manifest.json and review-tests/ without changing them, and for every
case builds two programs on the host:

  candidate  each translation unit through this compiler (IR), then the
             host Clang for the object at -O0 or -O2;
  reference  every translation unit through the host Clang (the
             black-box oracle) at the same level.

A two-translation-unit case also links this compiler's object with
Clang's, both ways ("mixed-hcc-first", "mixed-hcc-second"). A case passes
when every candidate build prints the same exit status, standard output
and standard error as the reference. If the reference cannot be built or
run, the candidate must match the kit's expected result instead, and the
case is reported as "reference blocked". The kit's expected results were
written on x86-64 Linux; a reference that differs from them on this
platform (a layout case on LLP64 Windows, say) is counted under
"reference differs from the kit", not as a failure. A required-diagnostic
case passes when this compiler rejects it with an error and no crash;
a strict-probe case must also be rejected at -std=c11.

<threads.h>: when the host C library has none (macOS, Windows, or Linux
with --force-threads-shim), the candidate uses this compiler's
include/threads.h and lib/threads.c compiled by this compiler; the
reference uses the same header (from a directory holding only that file,
so Clang's own <stdatomic.h> stays in use) and lib/threads.c compiled by
Clang. That comparison is "shared fallback runtime": both sides run the
project's runtime, built by different compilers.

Stress: a case the kit marks "stress" is run --stress-repetitions more
times (its candidate native build), and every run must match.

Sanitizers (--sanitize, Linux or macOS host with Clang's runtimes):
  address, thread  Both builds are instrumented. Clang instruments a
                   function only when it carries the sanitize_address or
                   sanitize_thread attribute (LLVM LangRef), which this
                   compiler does not emit, so the driver adds the
                   attribute to every function definition in this
                   compiler's IR before Clang compiles it. Known-positive
                   controls (tools/platform/controls/) are compiled by
                   this compiler first; the run stops unless the
                   sanitizer reports them and the objects reference the
                   sanitizer's entry points. A sanitizer report from the
                   candidate fails the case unless the reference reports
                   too (then it is "sanitizer also reports the
                   reference", for example TSan on a stand-alone fence).
                   A fatal signal or a failure of the sanitizer's own
                   runtime is not a report: such a reference is blocked
                   and such a control is not detected. Use thread with
                   --force-threads-shim on glibc (see SANITIZER_CRASH).
  undefined        UBSan checks are inserted by the C frontend, so LLVM
                   cannot add them to this compiler's IR. Only the
                   reference is built with -fsanitize=undefined (plus
                   address); this checks the test corpus itself. The
                   candidate is built plainly and compared as usual.

Usage:
  challenge_native.py --kit DIR [--kit DIR ...] --opt O0 [--opt O2]
                      --out DIR [--target TRIPLE] [--case ID ...]
                      [--stress-repetitions N] [--jobs N] [--timeout S]
                      [--sanitize address|thread|undefined]
                      [--force-threads-shim] [--known-failures FILE]

--target is needed only for Windows (x86_64-pc-windows-msvc or
x86_64-w64-windows-gnu; on Linux those run under Wine as in
harness/windows_x64.py). Otherwise both compilers use the host's default.
--known-failures FILE lists "CASE-ID [O0|O2]  # reason" lines; a listed
failure is reported but does not fail the run, and a listed case that
passes is reported so the line can be removed.

Writes OUT/results.json and OUT/summary.md. Exit status 0 when every case
passed (or is a listed known failure), 1 otherwise, 2 on a setup error.
Env: CC_UNDER_TEST (default build/hcc), CLANG (default clang).
"""
import argparse
import concurrent.futures
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "harness"))
import windows_x64 as win  # noqa: E402

COMPILER = Path(os.environ.get("CC_UNDER_TEST", ROOT / "build" / "hcc"))
CLANG = os.environ.get("CLANG", "clang")
THREADS_RUNTIME = ROOT / "lib" / "threads.c"
THREADS_HEADER = ROOT / "include" / "threads.h"
CONTROLS = Path(__file__).resolve().parent / "controls"
SANITIZER_ATTR = {"address": "sanitize_address", "thread": "sanitize_thread"}
SANITIZER_REPORT = re.compile(rb"(ERROR|WARNING): (AddressSanitizer|ThreadSanitizer|LeakSanitizer)"
                              rb"|runtime error:|SUMMARY: UndefinedBehaviorSanitizer")
# A fatal signal under the sanitizer, or its runtime failing (a nested
# failure, an internal CHECK), as opposed to a finding such as a data race
# or a buffer overflow. A reference that ends this way is "blocked", and a
# control that ends this way has not been detected. ThreadSanitizer
# (LLVM 18 on the runner, LLVM 19 locally) ends this way in every thread
# that glibc's own thrd_create starts, while threads from pthread_create
# work: apparently glibc's thrd_create does not reach the pthread_create
# interceptor, so the new thread has no sanitizer state. ThreadSanitizer
# therefore runs only with --force-threads-shim (lib/threads.c, which
# calls pthread_create).
SANITIZER_CRASH = re.compile(rb"Sanitizer:DEADLYSIGNAL|Sanitizer: nested bug|Sanitizer: CHECK failed")
# One finding: "ThreadSanitizer: data race", "AddressSanitizer:
# heap-buffer-overflow on address", "UBSan: runtime error".
REPORT_KIND = re.compile(rb"(?:ERROR|WARNING): (\w+Sanitizer): ([A-Za-z][\w-]*(?: [A-Za-z][\w-]*)*)")


def report_kinds(err):
    """The kinds of sanitizer finding in standard error, as strings."""
    kinds = {f"{m.group(1).decode()}: {m.group(2).decode()}" for m in REPORT_KIND.finditer(err)}
    if b"runtime error:" in err:
        kinds.add("UBSan: runtime error")
    if not kinds and SANITIZER_REPORT.search(err):
        kinds.add("unrecognised report")
    return kinds


CRASH_WORDS = ("internal compiler error", "segmentation fault", "panicked at", "stack overflow",
               "killed signal", "please submit a bug report")
# A function definition line: the signature up to the parameter list's
# closing parenthesis, then any function attributes, then "{".
DEFINE = re.compile(r"^(define\b[^\n]*\))([^()\n]*?)([ \t]*\{)[ \t]*$", re.M)


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class Runner:
    def __init__(self, args, tmp):
        self.args = args
        self.tmp = tmp
        self.target = args.target
        self.windows = args.target in win.TARGETS
        self.sanitize = args.sanitize
        self.timeout = args.timeout
        self.shim_dir = tmp / "threads-shim-include"
        self.shim_dir.mkdir()
        shutil.copyfile(THREADS_HEADER, self.shim_dir / "threads.h")
        self.env = dict(os.environ)
        self.env["LC_ALL"] = "C"
        if self.windows and not win.NATIVE:
            self.env.update(win.wine_env())
        if self.sanitize == "thread":
            # Each TSan report fails the program, so a report cannot hide
            # behind an expected exit status of 0.
            self.env.setdefault("TSAN_OPTIONS", "halt_on_error=1 exitcode=66 second_deadlock_stack=1")
        if self.sanitize in ("address", "undefined"):
            self.env.setdefault("ASAN_OPTIONS", "detect_leaks=1:abort_on_error=0:exitcode=66")
            self.env.setdefault("UBSAN_OPTIONS", "print_stacktrace=1:halt_on_error=1:exitcode=66")

    # -- commands ---------------------------------------------------------
    def run(self, cmd, cwd=None, timeout=None):
        start = time.monotonic()
        try:
            p = subprocess.run([str(c) for c in cmd], cwd=cwd, env=self.env, capture_output=True,
                               timeout=timeout or self.timeout)
            return dict(cmd=[str(c) for c in cmd], rc=p.returncode, out=p.stdout, err=p.stderr,
                        seconds=round(time.monotonic() - start, 3), timeout=False)
        except subprocess.TimeoutExpired as e:
            return dict(cmd=[str(c) for c in cmd], rc=None, out=e.stdout or b"", err=e.stderr or b"",
                        seconds=round(time.monotonic() - start, 3), timeout=True)
        except OSError as e:
            return dict(cmd=[str(c) for c in cmd], rc=None, out=b"", err=str(e).encode(),
                        seconds=0.0, timeout=False)

    def needs_shim(self):
        if self.args.force_threads_shim or self.windows or sys.platform == "darwin":
            return True
        return not Path("/usr/include/threads.h").is_file()

    def target_flags(self):
        return ["-target", self.target] if self.target else []

    def ours_cmd(self, src, ll, mode, defines):
        cmd = [COMPILER, *self.target_flags()]
        if self.windows:
            cmd += win.ours_flags(self.target)
        if mode != "c11":
            cmd.append(f"-std={mode}")
        return cmd + defines + [src, "-o", ll]

    def ll_obj_cmd(self, ll, obj, opt):
        cmd = [CLANG, *self.target_flags(), "-w", f"-{opt}"]
        if self.sanitize in SANITIZER_ATTR:
            cmd.append(f"-fsanitize={self.sanitize}")
        return cmd + ["-c", ll, "-o", obj]

    def ref_obj_cmd(self, src, obj, mode, opt, includes, defines):
        cmd = [CLANG, *self.target_flags()]
        if self.windows:
            cmd += win.clang_flags(self.target)
        cmd += ["-w", f"-std={mode}", f"-{opt}", *includes, *defines]
        if self.sanitize in SANITIZER_ATTR:
            cmd.append(f"-fsanitize={self.sanitize}")
        elif self.sanitize == "undefined":
            cmd += ["-fsanitize=undefined,address", "-fno-sanitize-recover=all"]
        return cmd + ["-c", src, "-o", obj]

    def link_cmd(self, objs, exe, opt, pthread, reference, latomic=False):
        if self.windows:
            return win.link_command(objs, exe, self.target)
        cmd = [CLANG, *self.target_flags(), f"-{opt}", *objs, "-lm"]
        if latomic and reference and sys.platform.startswith("linux"):
            # Clang calls libatomic for an atomic it does not inline (a
            # 16-byte one without -mcx16); this compiler must not need it.
            cmd.append("-latomic")
        if pthread:
            cmd.append("-pthread")
        if self.sanitize in SANITIZER_ATTR:
            cmd.append(f"-fsanitize={self.sanitize}")
        elif self.sanitize == "undefined" and reference:
            cmd.append("-fsanitize=undefined,address")
        return cmd + ["-o", exe]

    def exe_name(self, base):
        return base.with_suffix(".exe")

    def execute(self, exe, cwd):
        cmd = win.run_command(exe) if self.windows else [exe]
        return self.run(cmd, cwd=cwd)

    def instrument(self, ll):
        """Add the sanitizer attribute to every function definition."""
        attr = SANITIZER_ATTR.get(self.sanitize)
        if not attr:
            return 0
        text = ll.read_text(errors="replace")
        def add(m):
            # Attributes follow the parameter list (LangRef "Functions");
            # anything else there (a section, metadata) stays after them.
            return f"{m.group(1)} {attr}{m.group(2)}{m.group(3)}"
        total = len(re.findall(r"^define\b", text, re.M))
        text, count = DEFINE.subn(add, text)
        ll.write_text(text)
        return count if count == total else -1

    # -- one build --------------------------------------------------------
    def build(self, wd, tag, case, srcs, choices, opt, logs):
        """choices[i] True: this compiler builds srcs[i]. Returns (exe, error)."""
        mode = case["mode"]
        shim = self.uses_threads(srcs) and self.needs_shim()
        defines = ["-D__HCC_THREADS_FORCE_SHIM"] if shim and self.args.force_threads_shim else []
        units = list(zip(srcs, choices))
        if shim:
            # The fallback runtime is built by the same compiler as the
            # case's first translation unit.
            units.append((THREADS_RUNTIME, choices[0]))
        objs = []
        for i, (src, ours) in enumerate(units):
            obj = wd / f"{tag}-{i}.o"
            runtime = src == THREADS_RUNTIME
            if ours:
                ll = wd / f"{tag}-{i}.ll"
                r = self.run(self.ours_cmd(src, ll, "c11" if runtime else mode, defines))
                logs.append(r)
                if r["rc"] != 0 or not ll.is_file():
                    return None, f"{tag}: this compiler failed on {Path(src).name} ({r['rc']}): {short(r['err'])}"
                if self.sanitize in SANITIZER_ATTR and self.instrument(ll) < 0:
                    return None, f"{tag}: could not add {SANITIZER_ATTR[self.sanitize]} to {Path(src).name}"
                r = self.run(self.ll_obj_cmd(ll, obj, opt))
            else:
                includes = [] if runtime or not shim else ["-I", str(self.shim_dir)]
                r = self.run(self.ref_obj_cmd(src, obj, "c11" if runtime else mode, opt, includes, defines))
            logs.append(r)
            if r["rc"] != 0 or not obj.is_file():
                who = "LLVM on this compiler's IR" if ours else "the Clang reference"
                return None, f"{tag}: {who} failed on {Path(src).name} ({r['rc']}): {short(r['err'])}"
            objs.append(obj)
        exe = self.exe_name(wd / tag)
        pthread = not self.windows and ("-pthread" in case["flags"] or shim)
        # Any Clang-built unit (a mixed build) brings the reference's
        # sanitizer runtime and library calls with it.
        r = self.run(self.link_cmd(objs, exe, opt, pthread, reference=not all(choices),
                                   latomic="-latomic" in case["flags"]))
        logs.append(r)
        if r["rc"] != 0 or not exe.is_file():
            return None, f"{tag}: link failed ({r['rc']}): {short(win.link_messages(r['out'], r['err']))}"
        return exe, None

    @staticmethod
    def uses_threads(srcs):
        return any(b"<threads.h>" in Path(s).read_bytes() for s in srcs)

    def rejects(self, src, mode, wd, tag, logs):
        dest = wd / f"{tag}.ll"
        r = self.run(self.ours_cmd(src, dest, mode, []))
        logs.append(r)
        err = r["err"].decode("utf-8", "replace").lower()
        return (not r["timeout"] and isinstance(r["rc"], int) and 0 < r["rc"] < 100 and r["rc"] != 2
                and "error:" in err and not dest.exists() and not any(w in err for w in CRASH_WORDS))

    # -- one case ---------------------------------------------------------
    def run_case(self, kit, case, opt):
        wd = self.tmp / kit.name / opt / case["id"]
        wd.mkdir(parents=True)
        logs, checks, notes = [], {}, []
        srcs = [kit / "review-tests" / case["id"] / f for f in case["files"]]
        result = dict(kit=kit.name, id=case["id"], title=case["title"], group=case["group"], opt=opt)
        if case["kind"] == "reject":
            checks["required-diagnostic"] = self.rejects(srcs[0], case["mode"], wd, "reject", logs)
            return self.finish(result, checks, notes, logs)
        ref_exe, ref_err = self.build(wd, "reference", case, srcs, [False] * len(srcs), opt, logs)
        reference = None
        if ref_exe:
            reference = self.execute(ref_exe, wd)
            logs.append(reference)
            if reference["timeout"] or reference["rc"] is None:
                ref_err = "reference did not complete"
                reference = None
            elif self.sanitize and SANITIZER_CRASH.search(reference["err"]):
                ref_err = "the sanitizer's runtime failed in the reference: " + short(reference["err"])
                reference = None
        if reference is None:
            notes.append(f"reference blocked: {ref_err}")
            result["reference"] = "blocked"
        else:
            result["reference"] = "ran"
            if not self.matches_expected(reference, case):
                notes.append("reference differs from the kit's expected result")
                result["reference_differs_from_kit"] = True
            if self.sanitize and SANITIZER_REPORT.search(reference["err"]):
                notes.append("the sanitizer also reports the reference")
                result["sanitizer_reports_reference"] = True
                result["reference_report_kinds"] = sorted(report_kinds(reference["err"]))
        configs = [("native", [True] * len(srcs))]
        if len(srcs) == 2:
            configs += [("mixed-hcc-first", [True, False]), ("mixed-hcc-second", [False, True])]
        native_exe = None
        for tag, choices in configs:
            exe, err = self.build(wd, tag, case, srcs, choices, opt, logs)
            if err:
                checks[tag] = False
                notes.append(err)
                continue
            if tag == "native":
                native_exe = exe
            got = self.execute(exe, wd)
            logs.append(got)
            checks[tag], why = self.compare(got, reference, case, result)
            if why:
                notes.append(f"{tag}: {why}")
        if case.get("strict_probe"):
            checks["strict-mode-reject"] = self.rejects(srcs[0], "c11", wd, "strict", logs)
        reps = self.args.stress_repetitions
        if case.get("stress") and reps and native_exe and checks.get("native"):
            bad = 0
            for i in range(reps):
                got = self.execute(native_exe, wd)
                ok, why = self.compare(got, reference, case, result)
                if not ok:
                    bad += 1
                    if bad <= 3:
                        notes.append(f"stress repetition {i + 1}: {why}")
                        logs.append(got)
            checks["stress"] = bad == 0
            result["stress"] = dict(repetitions=reps, failed=bad)
        return self.finish(result, checks, notes, logs)

    def matches_expected(self, got, case):
        exp = case["expected"]
        return (got["rc"] == exp["exit"] and normal(got["out"]) == exp["stdout"]
                and normal(got["err"]) == exp["stderr"])

    def compare(self, got, reference, case, result):
        if got["timeout"] or got["rc"] is None:
            return False, f"did not complete ({'timeout' if got['timeout'] else short(got['err'])})"
        if self.sanitize and SANITIZER_REPORT.search(got["err"]):
            # Excused only when every kind of finding is one the reference
            # has too (TSan on a stand-alone fence, say); a different kind
            # of finding still fails the case.
            extra = report_kinds(got["err"]) - set(result.get("reference_report_kinds", []))
            if not extra:
                return True, None
            return False, f"sanitizer report not in the reference ({', '.join(sorted(extra))}): " + \
                short(got["err"], 600)
        if reference is None or self.sanitize:
            # Under a sanitizer, standard error carries the runtime's own
            # messages, so both sides are held to the kit's result. A
            # reference ended or changed by its own sanitizer report is not
            # an expected result: a report-free candidate must match the kit.
            want = case["expected"]
            if (reference is not None and result.get("reference_differs_from_kit")
                    and not result.get("sanitizer_reports_reference")):
                same = got["rc"] == reference["rc"] and got["out"] == reference["out"]
                return same, None if same else describe(got, reference["rc"], reference["out"])
            if self.matches_expected(got, case):
                return True, None
            return False, describe(got, want["exit"], want["stdout"].encode())
        if (got["rc"], got["out"], got["err"]) == (reference["rc"], reference["out"], reference["err"]):
            return True, None
        return False, describe(got, reference["rc"], reference["out"], reference["err"])

    def finish(self, result, checks, notes, logs):
        result["checks"] = checks
        result["passed"] = bool(checks) and all(checks.values())
        result["notes"] = notes
        result["commands"] = [dict(cmd=r["cmd"], rc=r["rc"], timeout=r["timeout"], seconds=r["seconds"],
                                   stdout=r["out"].decode("utf-8", "replace")[:2000],
                                   stderr=r["err"].decode("utf-8", "replace")[:4000]) for r in logs]
        return result

    # -- sanitizer controls -------------------------------------------------
    def controls(self):
        """Known-positive controls: this compiler's code must be instrumented."""
        if self.sanitize not in SANITIZER_ATTR:
            return []
        name = {"address": "asan_overflow.c", "thread": "tsan_race.c"}[self.sanitize]
        src = CONTROLS / name
        wd = self.tmp / "controls"
        wd.mkdir()
        logs = []
        case = dict(mode="c11", flags=["-pthread"])
        exe, err = self.build(wd, "control", case, [src], [True], "O0", logs)
        if err:
            return [f"control {name}: {err}"]
        obj = (wd / "control-0.o").read_bytes()
        prefix = b"__asan_" if self.sanitize == "address" else b"__tsan_"
        problems = []
        if prefix not in obj:
            problems.append(f"control {name}: this compiler's object does not reference {prefix.decode()}*")
        got = self.execute(exe, wd)
        if not SANITIZER_REPORT.search(got["err"]) or SANITIZER_CRASH.search(got["err"]):
            problems.append(f"control {name}: no sanitizer finding (exit {got['rc']}): {short(got['err'])}")
        # Regression: a struct parameter in registers, this compiler's
        # callee and Clang's caller (see store_param in src/irgen/call.rs).
        pair = [CONTROLS / "param_spill_def.c", CONTROLS / "param_spill_use.c"]
        exe, err = self.build(wd, "spill", case, pair, [True, False], "O0", logs)
        if err:
            return problems + [f"control param_spill: {err}"]
        got = self.execute(exe, wd)
        if got["rc"] != 0 or normal(got["out"]) != "42\n" or SANITIZER_REPORT.search(got["err"]):
            problems.append(f"control param_spill: exit {got['rc']} stdout {got['out'][:40]!r} "
                            f"stderr {short(got['err'])}")
        return problems


def normal(data):
    return data.decode("utf-8", "replace").replace("\r\n", "\n")


def short(data, n=300):
    if isinstance(data, bytes):
        data = data.decode("utf-8", "replace")
    return data.strip().replace("\r", "")[:n]


def describe(got, exit_, out, err=None):
    text = f"got exit {got['rc']} stdout {got['out'][:120]!r}, expected exit {exit_} stdout {out[:120]!r}"
    if err is not None and got["err"] != err:
        text += f"; stderr {got['err'][:120]!r} vs {err[:120]!r}"
    return text


def load_known(path):
    known = {}
    if not path:
        return known
    for line in Path(path).read_text().splitlines():
        body = line.split("#", 1)[0].split()
        if not body:
            continue
        opts = body[1:] or ["O0", "O2"]
        for opt in opts:
            known[(body[0], opt)] = line.split("#", 1)[1].strip() if "#" in line else ""
    return known


def tool_version(cmd):
    try:
        p = subprocess.run(cmd, capture_output=True, timeout=60)
        return (p.stdout or p.stderr).decode("utf-8", "replace").strip().splitlines()[0]
    except (OSError, subprocess.TimeoutExpired, IndexError):
        return "unavailable"


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--kit", action="append", required=True, type=Path)
    p.add_argument("--opt", action="append", choices=["O0", "O2"], required=True)
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--target")
    p.add_argument("--case", action="append")
    p.add_argument("--stress-repetitions", type=int, default=0)
    p.add_argument("--jobs", type=int, default=os.cpu_count() or 2)
    p.add_argument("--timeout", type=float, default=60)
    p.add_argument("--sanitize", choices=["address", "thread", "undefined"])
    p.add_argument("--force-threads-shim", action="store_true")
    p.add_argument("--known-failures")
    p.add_argument("--title", default="")
    args = p.parse_args()
    # A level, kit or case given twice would share a scratch directory.
    args.opt = list(dict.fromkeys(args.opt))
    args.kit = list({k.resolve(): k for k in args.kit}.values())
    if args.case:
        args.case = list(dict.fromkeys(args.case))
    if args.target and args.target in win.TARGETS:
        missing = win.available(args.target)
        if missing:
            print(f"setup: {missing}", file=sys.stderr)
            return 2
        win.start_server()
    if not COMPILER.is_file() and not Path(f"{COMPILER}.exe").is_file():
        print(f"setup: {COMPILER} not found (run make)", file=sys.stderr)
        return 2
    known = load_known(args.known_failures)
    args.out.mkdir(parents=True, exist_ok=True)
    tmp = Path(tempfile.mkdtemp(prefix="challenge-native-"))
    try:
        return run_all(args, known, tmp)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def run_all(args, known, tmp):
    runner = Runner(args, tmp)
    problems = runner.controls()
    if problems:
        for line in problems:
            print(f"SETUP FAIL {line}")
        (args.out / "summary.md").write_text("# Sanitizer controls failed\n\n" + "\n".join(f"- {x}" for x in problems) + "\n")
        return 2
    jobs = []
    for kit in args.kit:
        kit = kit.resolve()
        cases = json.loads((kit / "manifest.json").read_text())
        for case in cases:
            if args.case and case["id"] not in args.case:
                continue
            for opt in args.opt:
                jobs.append((kit, case, opt))
    if not jobs:
        print("setup: no cases selected", file=sys.stderr)
        return 2
    start = time.monotonic()
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        results = list(pool.map(lambda j: runner.run_case(*j), jobs))
    seconds = round(time.monotonic() - start, 1)
    failed, known_hit, fixed = [], [], []
    for r in results:
        key = (r["id"], r["opt"])
        if not r["passed"] and key in known:
            r["known_failure"] = known[key]
            known_hit.append(r)
        elif not r["passed"]:
            failed.append(r)
        elif key in known:
            fixed.append(r)
    host = f"{platform.system()} {platform.machine()} ({platform.platform()})"
    data = dict(title=args.title, host=host, target=args.target or "host default",
                sanitize=args.sanitize, force_threads_shim=args.force_threads_shim,
                threads_runtime="lib/threads.c (shared fallback runtime)" if runner.needs_shim() else "C library",
                compiler=str(COMPILER), compiler_sha256=sha256(COMPILER if COMPILER.is_file() else f"{COMPILER}.exe"),
                clang=tool_version([CLANG, "--version"]), opts=args.opt,
                stress_repetitions=args.stress_repetitions, seconds=seconds,
                kits={k.name: sha256(k / "manifest.json") for k in [x.resolve() for x in args.kit]},
                passed=sum(r["passed"] for r in results), total=len(results),
                failed=len(failed), known_failures=len(known_hit), results=results)
    (args.out / "results.json").write_text(json.dumps(data, indent=1) + "\n")
    lines = [f"# {args.title or 'Challenge kits'}", "",
             f"- Host: {host}", f"- Target: {data['target']}",
             f"- Sanitizer: {args.sanitize or 'none'}", f"- <threads.h> runtime: {data['threads_runtime']}",
             f"- Clang: {data['clang']}", f"- Stress repetitions: {args.stress_repetitions}",
             f"- Wall time: {seconds} s", "",
             "| Kit | Level | Passed | Failed | Known failures | Reference blocked | Reference differs from kit |",
             "| --- | --- | ---: | ---: | ---: | ---: | ---: |"]
    for kit in args.kit:
        for opt in args.opt:
            rs = [r for r in results if r["kit"] == kit.resolve().name and r["opt"] == opt]
            lines.append(f"| {kit.resolve().name} | {opt} | {sum(r['passed'] for r in rs)}/{len(rs)} | "
                         f"{sum(1 for r in rs if r in failed)} | {sum(1 for r in rs if r in known_hit)} | "
                         f"{sum(1 for r in rs if r.get('reference') == 'blocked')} | "
                         f"{sum(1 for r in rs if r.get('reference_differs_from_kit'))} |")
    stress = [r for r in results if "stress" in r]
    if stress:
        runs = sum(r["stress"]["repetitions"] for r in stress)
        bad = sum(r["stress"]["failed"] for r in stress)
        lines += ["", f"Stress: {len(stress)} case builds x {args.stress_repetitions} repetitions, "
                      f"{runs - bad}/{runs} runs matched."]
    for title, rs in (("Failures", failed), ("Known failures", known_hit), ("Listed but passed", fixed)):
        if rs:
            lines += ["", f"## {title}", ""]
            for r in rs:
                why = "; ".join(r["notes"][:3]) or ", ".join(k for k, v in r["checks"].items() if not v)
                lines.append(f"- {r['kit']} {r['id']} {r['opt']} ({r['title']}): {why[:400]}")
    (args.out / "summary.md").write_text("\n".join(lines) + "\n")
    for r in failed:
        print(f"FAIL {r['kit']} {r['id']} {r['opt']} {r['checks']} {'; '.join(r['notes'][:2])[:300]}")
    print(f"{data['passed']}/{data['total']} passed, {len(failed)} failed, "
          f"{len(known_hit)} known failures, {seconds} s")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
