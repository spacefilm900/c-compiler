#!/usr/bin/env python3
"""Shrink a C file while it stays "interesting".

The reducer repeats four passes until none of them removes anything:

  1. lines: delta debugging over the lines of the file;
  2. decls: over the top-level declarations (a function definition, a
     declaration up to its `;`, or a preprocessor directive);
  3. blocks: inside every balanced `{ ... }`, over its statements, and
     inside every `{ ... }` or `( ... )` without statements, over the
     comma-separated items (initializer lists, enumerators, arguments,
     parameters);
  4. tokens: over the individual tokens.

Each pass uses ddmin, the minimizing delta debugging algorithm described in
A. Zeller and R. Hildebrandt, "Simplifying and Isolating Failure-Inducing
Input", IEEE TSE 28(2), 2002. This is an original implementation written
from that published description; it does not use or follow the source of
any existing reducer.

A candidate is interesting when a predicate holds. Built-in predicates
(--mode):

  ice         this compiler stops with an internal compiler error (exit
              status 3); with --message, its stderr contains that text
  crash       this compiler panics (exit 101) or is killed by a signal;
              --message filters stderr as for ice
  miscompile  Clang compiles and runs the file; this compiler compiles it,
              Clang links the IR, and the program's exit status or stdout
              differs from Clang's. The reference must compile without the
              warnings in UB_WARNINGS, and a build with
              -fsanitize=address,undefined (by --ub-cc: Clang, or GCC when
              Clang has no sanitizer runtime) must give the same result
              (turn off with --no-ub-check), so a reduction does not drift
              into undefined behaviour
  reject      this compiler reports an error (exit 1) and Clang accepts the
              file (-fsyntax-only; with -pedantic-errors for -std=c11);
              --message filters this compiler's stderr
  accept      this compiler accepts the file and Clang rejects it;
              --message filters Clang's stderr
  bad-ir      this compiler accepts the file and writes IR that Clang (the
              LLVM backend) rejects; --message filters Clang's stderr
  cmd         --cmd COMMAND exits 0. COMMAND runs in a shell in a scratch
              directory; `{}` in it is replaced by the candidate's path,
              otherwise the path is appended. The path is also in
              $REDUCE_FILE

--clang-valid adds a condition to ice, crash and cmd: Clang must accept the
candidate as well, so the result stays a valid C program.

Every command has a time limit (--timeout), runs in its own process group
(killed with all its descendants when the time is up), and may write at
most --output-limit bytes to stdout or stderr and files of at most
--file-limit bytes; a candidate that exceeds any of these is not
interesting. Results are cached by the SHA-256 of the candidate text,
so no candidate is tested twice. The input file is never modified; the
result goes to -o (default INPUT.reduced.c).

Usage:
  python3 tools/reduce/reduce.py INPUT.c --mode ice --message "in irgen"
  python3 tools/reduce/reduce.py INPUT.c --mode miscompile -o small.c
  python3 tools/reduce/reduce.py INPUT.c --mode cmd --cmd "$PWD/check.sh {}"
Exit status: 0 when a result was written, 2 when the input itself is not
interesting (or on a usage error).
"""
import argparse
import hashlib
import os
import resource
import shlex
import signal
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# ---------------------------------------------------------------------------
# ddmin


def ddmin(items, test, on_progress=None):
    """A 1-minimal subsequence of `items` for which `test` holds.

    `test(subsequence)` must hold for `items` itself (the caller checks
    this). The result is a subsequence (order kept) such that removing any
    single element makes `test` fail, as in Zeller and Hildebrandt's
    ddmin. A final check tries the empty sequence.
    """
    items = list(items)
    granularity = 2
    while len(items) >= 2:
        chunks = split(items, granularity)
        reduced = False
        # Reduce to a subset.
        for chunk in chunks:
            if test(chunk):
                items, granularity, reduced = chunk, 2, True
                break
        # Reduce to a complement. With two chunks a complement is the
        # other subset, which was just tested.
        if not reduced and granularity > 2:
            for index in range(len(chunks)):
                complement = [item for other, chunk in enumerate(chunks)
                              if other != index for item in chunk]
                if test(complement):
                    items, granularity, reduced = complement, max(granularity - 1, 2), True
                    break
        if reduced:
            if on_progress:
                on_progress(items)
            continue
        if granularity >= len(items):
            break
        granularity = min(len(items), granularity * 2)
    if len(items) == 1 and test([]):
        items = []
    return items


def split(items, parts):
    """`items` in `parts` contiguous chunks of nearly equal size."""
    chunks, start = [], 0
    for index in range(parts):
        end = start + (len(items) - start) // (parts - index)
        chunks.append(items[start:end])
        start = end
    return [chunk for chunk in chunks if chunk]


# ---------------------------------------------------------------------------
# A small C tokenizer that keeps the text around each token


class Token:
    """One token, with the whitespace and comments before it (`space`).
    `kind` is 'pp' (a whole preprocessor directive line), 'ident',
    'number', 'string', 'char' or 'punct'."""
    __slots__ = ("kind", "text", "space")

    def __init__(self, kind, text, space):
        self.kind, self.text, self.space = kind, text, space

    def __repr__(self):
        return f"Token({self.kind!r}, {self.text!r})"


PUNCTUATORS = sorted([
    "%:%:", "...", "<<=", ">>=", "->", "++", "--", "<<", ">>", "<=", ">=", "==",
    "!=", "&&", "||", "*=", "/=", "%=", "+=", "-=", "&=", "^=", "|=", "##",
    "<:", ":>", "<%", "%>", "%:",
], key=len, reverse=True)
PREFIXES = ("u8", "u", "U", "L")


def tokenize(text):
    """(tokens, trailing space). Joining each token's space and text, then
    the trailing space, gives back `text` exactly."""
    tokens, space, pos, n = [], [], 0, len(text)
    line_start = True  # only whitespace or comments since the last newline
    while pos < n:
        char = text[pos]
        if char in " \t\r\f\v":
            space.append(char)
            pos += 1
            continue
        if char == "\n":
            space.append(char)
            pos += 1
            line_start = True
            continue
        if char == "\\" and text.startswith("\n", pos + 1):
            space.append("\\\n")
            pos += 2
            continue
        if text.startswith("/*", pos):
            end = text.find("*/", pos + 2)
            end = n if end < 0 else end + 2
            space.append(text[pos:end])
            pos = end
            continue
        if text.startswith("//", pos):
            end = text.find("\n", pos)
            end = n if end < 0 else end
            space.append(text[pos:end])
            pos = end
            continue
        start = pos
        if char == "#" and line_start:
            pos = directive_end(text, pos)
            kind = "pp"
        elif char.isalpha() or char == "_":
            while pos < n and (text[pos].isalnum() or text[pos] == "_"):
                pos += 1
            kind = "ident"
            if pos < n and text[pos] in "\"'" and text[start:pos] in PREFIXES:
                kind, pos = quoted(text, pos)
        elif char.isdigit() or (char == "." and pos + 1 < n and text[pos + 1].isdigit()):
            pos += 1
            while pos < n:
                if text[pos] in "eEpP" and pos + 1 < n and text[pos + 1] in "+-":
                    pos += 2
                elif text[pos].isalnum() or text[pos] in "._":
                    pos += 1
                else:
                    break
            kind = "number"
        elif char in "\"'":
            kind, pos = quoted(text, pos)
        else:
            kind = "punct"
            match = next((p for p in PUNCTUATORS if text.startswith(p, pos)), char)
            pos += len(match)
        tokens.append(Token(kind, text[start:pos], "".join(space)))
        space = []
        line_start = False
    return tokens, "".join(space)


def quoted(text, pos):
    """End of the string or character literal whose quote is at `pos`."""
    quote, n = text[pos], len(text)
    pos += 1
    while pos < n and text[pos] != quote and text[pos] != "\n":
        pos += 2 if text[pos] == "\\" else 1
    return ("string" if quote == '"' else "char"), min(pos + 1, n)


def directive_end(text, pos):
    """End of the directive line at `pos`: the newline that is not escaped
    by a backslash, which is not part of the token."""
    n = len(text)
    while pos < n:
        if text[pos] == "\\" and text.startswith("\n", pos + 1):
            pos += 2
        elif text[pos] == "\n":
            return pos
        elif text.startswith("/*", pos):
            end = text.find("*/", pos + 2)
            pos = n if end < 0 else end + 2
        else:
            pos += 1
    return n


def render(tokens, keep, tail):
    """The text of the tokens whose indices are in `keep` (sorted). A token
    whose neighbour was removed takes the space of the first removed token
    after the previous kept one, so a line break is not lost; two tokens
    never touch where they did not touch before, and a directive always
    starts on its own line."""
    out = []
    previous = -1
    for index in keep:
        token = tokens[index]
        space = token.space
        if index != previous + 1:
            removed_space = tokens[previous + 1].space
            space = removed_space if removed_space else space
            if not space and previous >= 0:
                space = " "
        if previous >= 0 and "\n" not in space and (
                token.kind == "pp" or tokens[previous].kind == "pp"):
            space = "\n" + space
        out.append(space)
        out.append(token.text)
        previous = index
    out.append(tail if previous == len(tokens) - 1 else (tail or "\n"))
    return "".join(out)


# ---------------------------------------------------------------------------
# Units each pass removes, as lists of token index ranges

OPEN = {"(": ")", "[": "]", "{": "}", "<%": "%>", "<:": ":>"}
CLOSE = {value: key for key, value in OPEN.items()}


def match_brackets(tokens):
    """{open index: close index} for balanced brackets; unbalanced ones are
    ignored."""
    pairs, stack = {}, []
    for index, token in enumerate(tokens):
        if token.kind != "punct":
            continue
        if token.text in OPEN:
            stack.append(index)
        elif token.text in CLOSE:
            if stack and OPEN.get(tokens[stack[-1]].text) == token.text:
                pairs[stack.pop()] = index
            else:
                stack.clear()
    return pairs


def top_level_units(tokens):
    """Ranges (start, end) of top-level declarations: a directive; a
    function definition, which ends at the `}` of a brace block that follows
    `)`; or anything else up to and including a `;` outside brackets."""
    pairs = match_brackets(tokens)
    units, start, index = [], 0, 0
    while index < len(tokens):
        token = tokens[index]
        if token.kind == "pp":
            if start < index:
                units.append((start, index))
            units.append((index, index + 1))
            start = index = index + 1
            continue
        if token.kind == "punct" and token.text in OPEN and index in pairs:
            close = pairs[index]
            is_body = token.text in ("{", "<%") and index > 0 and tokens[index - 1].text == ")"
            index = close + 1
            if is_body and not (index < len(tokens) and tokens[index].text in (";", ",")):
                units.append((start, index))
                start = index
            continue
        index += 1
        if token.kind == "punct" and token.text == ";":
            units.append((start, index))
            start = index
    if start < len(tokens):
        units.append((start, len(tokens)))
    return units


def inner_units(tokens, open_index, close_index, pairs):
    """The items between a bracket pair: statements for a brace block that
    holds a `;`, otherwise comma-separated items (each with its comma)."""
    body = range(open_index + 1, close_index)
    has_semi = False
    index = open_index + 1
    while index < close_index:
        if index in pairs:
            index = pairs[index] + 1
            continue
        if tokens[index].text == ";":
            has_semi = True
        index += 1
    statements = has_semi and tokens[open_index].text in ("{", "<%")
    units, start, index = [], body.start, body.start
    while index < close_index:
        token = tokens[index]
        if index in pairs:
            inner_close = pairs[index]
            index = inner_close + 1
            # In a statement list, `{ ... }` ends a statement (`if (x) { }`).
            if statements and token.text in ("{", "<%"):
                units.append((start, index))
                start = index
            continue
        index += 1
        if (statements and token.text == ";") or (not statements and token.text == ","):
            units.append((start, index))
            start = index
    if start < close_index:
        units.append((start, close_index))
    return units


# ---------------------------------------------------------------------------
# The reducer


class Reducer:
    """Applies the passes to `text` with `predicate(text) -> bool`."""

    def __init__(self, predicate, log=None, max_tests=None):
        self.predicate = predicate
        self.cache = {}
        self.tests = 0
        self.hits = 0
        self.log = log or (lambda message: None)
        self.max_tests = max_tests

    def interesting(self, text):
        key = hashlib.sha256(text.encode("utf-8", "surrogateescape")).hexdigest()
        if key in self.cache:
            self.hits += 1
            return self.cache[key]
        if self.max_tests is not None and self.tests >= self.max_tests:
            return False
        self.tests += 1
        result = bool(self.predicate(text))
        self.cache[key] = result
        return result

    def reduce(self, text, passes=("lines", "decls", "blocks", "tokens"), max_rounds=20):
        if not self.interesting(text):
            raise ValueError("the input is not interesting")
        for round_number in range(1, max_rounds + 1):
            before = text
            for name in passes:
                size = len(text)
                text = getattr(self, "pass_" + name)(text)
                self.log(f"round {round_number} {name}: {size} -> {len(text)} bytes, "
                         f"{count_lines(text)} lines, {self.tests} tests")
            if text == before:
                break
        return text

    def pass_lines(self, text):
        lines = text.splitlines(keepends=True)
        kept = ddmin(range(len(lines)),
                     lambda keep: self.interesting("".join(lines[i] for i in keep)))
        return "".join(lines[i] for i in kept)

    def reduce_units(self, text, tokens, tail, units):
        """ddmin over `units` (token ranges); tokens outside them stay."""
        if not units:
            return text, False
        in_unit = set()
        for start, end in units:
            in_unit.update(range(start, end))
        fixed = [i for i in range(len(tokens)) if i not in in_unit]

        def candidate(chosen):
            keep = set(fixed)
            for unit in chosen:
                keep.update(range(*units[unit]))
            return render(tokens, sorted(keep), tail)

        kept = ddmin(range(len(units)), lambda chosen: self.interesting(candidate(chosen)))
        if len(kept) == len(units):
            return text, False
        return candidate(kept), True

    def pass_decls(self, text):
        tokens, tail = tokenize(text)
        result, _ = self.reduce_units(text, tokens, tail, top_level_units(tokens))
        return result

    def pass_blocks(self, text):
        position = 0
        while True:
            tokens, tail = tokenize(text)
            pairs = match_brackets(tokens)
            opens = sorted(pairs)
            if position >= len(opens):
                return text
            open_index = opens[position]
            units = inner_units(tokens, open_index, pairs[open_index], pairs)
            text, _ = self.reduce_units(text, tokens, tail, units)
            # Removing items inside this pair leaves the earlier pairs and this
            # one where they were, so the next pair is the next position.
            position += 1

    def pass_tokens(self, text):
        tokens, tail = tokenize(text)
        units = [(index, index + 1) for index in range(len(tokens))]
        result, _ = self.reduce_units(text, tokens, tail, units)
        return result


def count_lines(text):
    return len(text.splitlines())


# ---------------------------------------------------------------------------
# Built-in predicates

# Clang warnings that usually mean a reduction made the program undefined.
UB_WARNINGS = [
    "-Werror=uninitialized", "-Werror=sometimes-uninitialized", "-Werror=return-type",
    "-Werror=implicit-function-declaration", "-Werror=implicit-int", "-Werror=int-conversion",
    "-Werror=incompatible-pointer-types", "-Werror=array-bounds", "-Werror=division-by-zero",
    "-Werror=shift-count-overflow", "-Werror=shift-count-negative", "-Werror=main-return-type",
    "-Werror=format", "-Werror=null-dereference",
]


def kill_group(proc):
    """Kill the process group `proc` leads (any descendants still running)
    and reap `proc`."""
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        pass
    proc.wait()


# A sanitizer that finds an error exits with this status (set through the
# documented ASAN_OPTIONS and UBSAN_OPTIONS `exitcode` option), and prints
# one of these markers on stderr.
SANITIZER_EXIT = 86
SANITIZER_ENV = {
    "ASAN_OPTIONS": f"exitcode={SANITIZER_EXIT}:detect_leaks=0",
    "UBSAN_OPTIONS": f"exitcode={SANITIZER_EXIT}:halt_on_error=1:print_stacktrace=0",
}
SANITIZER_MARKERS = (b"runtime error:", b"ERROR: AddressSanitizer", b"ERROR: LeakSanitizer",
                     b"SUMMARY: UndefinedBehaviorSanitizer")


class Runner:
    """Runs this compiler and Clang on candidates in a scratch directory."""

    def __init__(self, args):
        self.compiler = str(Path(args.compiler).resolve())
        self.clang = args.clang
        self.std = args.std
        self.flags = shlex.split(args.flags or "")
        self.clang_flags = shlex.split(args.clang_flags or "")
        self.timeout = args.timeout
        self.output_limit = args.output_limit
        self.file_limit = max(args.file_limit, args.output_limit + 1)
        self.message = args.message
        self.ub_check = args.ub_check
        self.ub_cc = args.ub_cc
        self.clang_valid = args.clang_valid
        self.cmd = args.cmd
        self.mode = args.mode
        self.include = [f"-I{Path(args.input).resolve().parent}"]
        self.scratch = tempfile.mkdtemp(prefix="reduce-")
        self.count = 0

    def pick_ub_cc(self, log):
        """Choose the sanitizer compiler: --ub-cc, or the first of Clang and
        GCC that builds and runs a sanitized program. Turns the check off,
        with a message, when neither can."""
        if self.mode != "miscompile" or not self.ub_check:
            return
        candidates = [self.ub_cc] if self.ub_cc != "auto" else [self.clang, "gcc"]
        probe = Path(self.scratch) / "probe.c"
        probe.write_text("int main(void) { return 0; }\n")
        for cc in candidates:
            exe = probe.with_suffix(".exe")
            status, _, _ = self.run([cc, "-fsanitize=address,undefined", str(probe), "-o",
                                     str(exe)])
            if status == 0 and self.run([str(exe)])[0] == 0:
                self.ub_cc = cc
                log(f"UB check: {cc} -fsanitize=address,undefined")
                return
        log("UB check: no compiler here can build with -fsanitize=address,undefined; "
            "continuing without it (pass --no-ub-check to silence this)")
        self.ub_check = False

    def close(self):
        shutil.rmtree(self.scratch, ignore_errors=True)

    def run(self, argv, env=None, shell=False):
        """(exit status, stdout, stderr). The status is None when the
        command could not run, timed out, or wrote more than
        --output-limit bytes to either stream; no predicate counts that as
        interesting. Output goes to files, not pipes, so memory stays
        bounded, and the command gets its own process group, which is
        killed on a timeout so no descendant outlives it."""
        out_path = Path(self.scratch) / "stdout.txt"
        err_path = Path(self.scratch) / "stderr.txt"
        limit = self.output_limit
        with open(out_path, "wb") as out_file, open(err_path, "wb") as err_file:
            try:
                proc = subprocess.Popen(argv, stdout=out_file, stderr=err_file,
                                        stdin=subprocess.DEVNULL, cwd=self.scratch, env=env,
                                        shell=shell, start_new_session=True,
                                        preexec_fn=self.limit_files)
            except OSError:
                return None, b"", b""
            try:
                status = proc.wait(timeout=self.timeout)
            except subprocess.TimeoutExpired:
                status = None
            finally:
                kill_group(proc)
        sizes = (out_path.stat().st_size, err_path.stat().st_size)
        if status is None or max(sizes) > limit:
            return None, b"", b""
        return status, out_path.read_bytes(), err_path.read_bytes()

    def limit_files(self):
        """In the child: no file (stdout and stderr included) may grow past
        --file-limit, so a runaway program cannot fill the disk."""
        resource.setrlimit(resource.RLIMIT_FSIZE, (self.file_limit, self.file_limit))

    def write(self, text):
        self.count += 1
        path = Path(self.scratch) / f"c{self.count}.c"
        path.write_text(text, errors="surrogateescape")
        return path

    def ours(self, path, *extra):
        return self.run([self.compiler, f"-std={self.std}", *self.include, *self.flags,
                         str(path), *extra])

    def clang_syntax(self, path, pedantic):
        argv = [self.clang, f"-std={self.std}", "-fsyntax-only", *self.include,
                *self.clang_flags, str(path)]
        if pedantic and self.std == "c11":
            argv.insert(2, "-pedantic-errors")
        return self.run(argv)

    def message_in(self, stream):
        return self.message is None or self.message in stream.decode(errors="replace")

    def __call__(self, text):
        path = self.write(text)
        try:
            return getattr(self, "mode_" + self.mode.replace("-", "_"))(path)
        finally:
            for leftover in Path(self.scratch).glob(path.stem + "*"):
                leftover.unlink(missing_ok=True)

    def valid_for_clang(self, path):
        return not self.clang_valid or self.clang_syntax(path, pedantic=False)[0] == 0

    def mode_ice(self, path):
        status, _, err = self.ours(path, "-o", str(path.with_suffix(".ll")))
        return status == 3 and self.message_in(err) and self.valid_for_clang(path)

    def mode_crash(self, path):
        status, _, err = self.ours(path, "-o", str(path.with_suffix(".ll")))
        # A signal or a Rust panic (101); SIGXFSZ is --file-limit, not a crash.
        crashed = status is not None and status != -signal.SIGXFSZ and (
            status < 0 or status == 101)
        return crashed and self.message_in(err) and self.valid_for_clang(path)

    def mode_reject(self, path):
        status, _, err = self.ours(path, "-o", str(path.with_suffix(".ll")))
        if status != 1 or not self.message_in(err):
            return False
        return self.clang_syntax(path, pedantic=True)[0] == 0

    def mode_accept(self, path):
        status, _, _ = self.ours(path, "-o", str(path.with_suffix(".ll")))
        if status != 0:
            return False
        status, _, err = self.clang_syntax(path, pedantic=True)
        return status not in (0, None) and self.message_in(err)

    def mode_bad_ir(self, path):
        ll = path.with_suffix(".ll")
        status, _, _ = self.ours(path, "-o", str(ll))
        if status != 0:
            return False
        status, _, err = self.run([self.clang, "-c", "-w", str(ll), "-o",
                                   str(path.with_suffix(".o"))])
        return status not in (0, None) and self.message_in(err)

    def program_result(self, exe):
        status, out, _ = self.run([str(exe)])
        return None if status is None else (status, out)

    def mode_miscompile(self, path):
        ref = path.with_suffix(".ref")
        status, _, _ = self.run([self.clang, f"-std={self.std}", "-O0", *self.include,
                                 *self.clang_flags, *UB_WARNINGS, str(path), "-o", str(ref)])
        if status != 0:
            return False
        expected = self.program_result(ref)
        if expected is None:
            return False
        if self.ub_check:
            checked = path.with_suffix(".ubsan")
            status, _, _ = self.run([self.ub_cc, f"-std={self.std}", "-O0", "-g",
                                     "-fsanitize=address,undefined",
                                     "-fno-sanitize-recover=all", *self.include,
                                     *self.clang_flags, str(path), "-o", str(checked)])
            if status != 0:
                return False
            status, out, err = self.run([str(checked)], env=dict(os.environ, **SANITIZER_ENV))
            if status is None or status == SANITIZER_EXIT or any(
                    marker in err for marker in SANITIZER_MARKERS):
                return False
            if (status, out) != expected:
                return False
        ll = path.with_suffix(".ll")
        status, _, _ = self.ours(path, "-o", str(ll))
        if status != 0:
            return False
        exe = path.with_suffix(".ours")
        status, _, _ = self.run([self.clang, "-w", str(ll), "-o", str(exe), "-lm"])
        if status != 0:
            return False
        actual = self.program_result(exe)
        return actual is not None and actual != expected

    def mode_cmd(self, path):
        quoted = shlex.quote(str(path))
        command = self.cmd.replace("{}", quoted) if "{}" in self.cmd else f"{self.cmd} {quoted}"
        env = dict(os.environ, REDUCE_FILE=str(path))
        status, _, _ = self.run(command, env=env, shell=True)
        return status == 0 and self.valid_for_clang(path)


def parse_args(argv):
    parser = argparse.ArgumentParser(
        description="Shrink a C file while a predicate holds (see the module docstring).")
    parser.add_argument("input", help="the C file to reduce (not modified)")
    parser.add_argument("-o", "--output", help="where to write the result "
                        "(default INPUT.reduced.c)")
    parser.add_argument("--mode", required=True,
                        choices=["ice", "crash", "miscompile", "reject", "accept", "bad-ir", "cmd"])
    parser.add_argument("--message", help="text the relevant stderr must contain")
    parser.add_argument("--cmd", help="for --mode cmd: shell command, exit 0 = interesting")
    parser.add_argument("--compiler", default=str(ROOT / "build" / "compiler"),
                        help="this compiler (default build/compiler)")
    parser.add_argument("--clang", default=os.environ.get("CLANG", "clang"))
    parser.add_argument("--std", default="c11", choices=["c11", "gnu11"])
    parser.add_argument("--flags", help="extra flags for this compiler, e.g. '-target ...'")
    parser.add_argument("--clang-flags", help="extra flags for Clang")
    parser.add_argument("--timeout", type=float, default=10.0,
                        help="seconds per command (default 10)")
    parser.add_argument("--output-limit", type=int, default=16 << 20,
                        help="bytes a command may write to stdout or stderr before the "
                        "candidate counts as uninteresting (default 16 MiB)")
    parser.add_argument("--file-limit", type=int, default=1 << 30,
                        help="largest file a command may write (default 1 GiB)")
    parser.add_argument("--no-ub-check", dest="ub_check", action="store_false",
                        help="miscompile: skip the sanitizer build of the reference")
    parser.add_argument("--ub-cc", default="auto",
                        help="miscompile: compiler for the sanitizer build (default: Clang, "
                        "else GCC)")
    parser.add_argument("--clang-valid", action="store_true",
                        help="ice, crash, cmd: Clang must accept every candidate too")
    parser.add_argument("--passes", default="lines,decls,blocks,tokens",
                        help="comma-separated passes, in order (default all four)")
    parser.add_argument("--max-rounds", type=int, default=20)
    parser.add_argument("--max-tests", type=int, help="stop testing after this many")
    parser.add_argument("-q", "--quiet", action="store_true")
    args = parser.parse_args(argv)
    if args.mode == "cmd" and not args.cmd:
        parser.error("--mode cmd needs --cmd")
    passes = [name.strip() for name in args.passes.split(",") if name.strip()]
    unknown = [name for name in passes if name not in ("lines", "decls", "blocks", "tokens")]
    if unknown:
        parser.error(f"unknown pass: {', '.join(unknown)}")
    args.passes = passes
    return args


def missing_tools(args):
    """The compilers this mode needs that cannot be found."""
    needed = []
    if args.mode != "cmd":
        needed.append(args.compiler)
    if args.mode in ("miscompile", "reject", "accept", "bad-ir") or args.clang_valid:
        needed.append(args.clang)
    return [tool for tool in needed if not (shutil.which(tool) or (
        Path(tool).is_file() and os.access(tool, os.X_OK)))]


def main(argv=None):
    args = parse_args(argv)
    source = Path(args.input)
    text = source.read_text(errors="surrogateescape")
    output = Path(args.output) if args.output else source.with_suffix(".reduced.c")
    log = (lambda message: None) if args.quiet else (
        lambda message: print(message, file=sys.stderr, flush=True))
    missing = missing_tools(args)
    if missing:
        print(f"not found: {', '.join(missing)}", file=sys.stderr)
        return 2
    runner = Runner(args)
    runner.pick_ub_cc(log)
    reducer = Reducer(runner, log=log, max_tests=args.max_tests)
    started = time.monotonic()
    try:
        result = reducer.reduce(text, passes=args.passes, max_rounds=args.max_rounds)
    except ValueError as err:
        print(f"{source}: {err} for --mode {args.mode}; nothing to reduce", file=sys.stderr)
        return 2
    finally:
        runner.close()
    output.write_text(result, errors="surrogateescape")
    print(f"{source}: {count_lines(text)} lines, {len(text)} bytes -> "
          f"{count_lines(result)} lines, {len(result)} bytes in {output} "
          f"({reducer.tests} tests, {reducer.hits} cache hits, "
          f"{time.monotonic() - started:.1f} s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
