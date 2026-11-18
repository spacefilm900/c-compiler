# Quality phase D: stage dumps and a test-case reducer

Branch `quality-d-tooling`, from `main` at `2846773` (pull request 46 merged), measured on the x86_64 Linux box (Rust 1.85.1, Clang 19.1.7, GCC 14.2.0, Python 3.13.5) on 2026-11-18. This is the last part of step 8 in `PLAN.md`, item (f): "an option to dump each stage ... and a test-case reducer that shrinks a failing C program to a minimal one". Internal errors that name the phase and line were part B.

Everything here is original. The dumps are written from this compiler's own data structures; the reducer is written from the published description of delta debugging (A. Zeller and R. Hildebrandt, "Simplifying and Isolating Failure-Inducing Input", IEEE TSE 28(2), 2002). No compiler or reducer source was read. Clang and GCC are used only as black-box oracles: the reducer runs them and compares results.

## Summary in plain English

- **Every stage can now be printed.** `-E` already printed the preprocessed text. New options print the token stream, the syntax tree, the checked tree with types and conversions, and the IR, and stop there.
- **A reducer shrinks a failing program to a few lines.** On a real wrong-code bug from before pull request 46, it turned a 118-line program into 14 lines, unattended, in about 80 seconds. On a second real bug it went from 39 lines to 2.
- **Normal compilation is unchanged:** 13,172 compilations gave the same exit status, messages and IR as `main`, and SQLite, Lua and DOOM give identical IR.
- **`docs/DEBUGGING.md`** explains how to go from a failing program to a fix with a regression test.
- **CI** runs the new golden tests and the reducer's unit tests in the existing Linux job, adding no macOS minutes.

## 1. Stage dumps

| Option | Stops after | Prints |
|---|---|---|
| `-E` (also `--dump-pp`) | preprocessing | the preprocessed text (unchanged) |
| `--dump-tokens` | preprocessing | one line per phase-7 token: `FILE:LINE:COL KIND 'SPELLING'`, plus the value and type of a constant and the decoded bytes of a string |
| `--dump-ast` | parsing | the syntax tree, one node per line, with `@LINE:COL` |
| `--dump-sema` | semantic analysis | the same tree plus each expression's type (`: T`), implicit conversions (`-> T`), resolved slots and calls, member offsets and bit-field positions, folded `case` values and checked initializers; then record layouts, emitted globals with their constant values, string literals and external declarations |
| `--dump-ir` | IR generation | the IR, exactly as `-o` writes it |

A dump goes to standard output, or to `-o FILE`. Warnings and errors are reported as in a normal compilation, with the same exit status (1 for an error, 3 for an internal error). Only one stage may be asked for. The formats are documented in `docs/DEBUGGING.md` section 3.

How it is built: `lib.rs` gains `Stage` and `dump_stage`, which run the normal pipeline (`compile_impl`) with a stop point. A normal compilation passes no stop point, so it runs the same code as before. `ast/dump.rs` became a small `Dumper` with two switches (positions, and sema details); the parser unit tests still get exactly the old plain tree from `ast::dump`. `token::dump_tokens` is new. The sema dump computes the conversion shown for a binary operator with the same rules the compiler uses (`TypeName::usual`, `float::usual_real`), and reads everything else from what semantic analysis stored.

Golden tests: `harness/dumps/` holds four small programs (scalars and control flow; records, bit-fields, unions and static addresses; the preprocessor, constants, `enum`, `sizeof` and `_Generic`; GNU extensions with `-std=gnu11`) and the expected dump of each stage, 20 files. `harness/run_dump_tests.py` checks each dump byte for byte with `-target x86_64-linux-gnu`. It also checks that `--dump-ir` equals the IR written by `-o`, that `-o` and standard output give the same dump, and that a second run gives the same bytes: 24 checks. `--update` rewrites the expected files. Four new Rust unit tests cover the option parsing and conflicts, that each dump stops at its stage (a program sema rejects still dumps its tokens and tree), the contents of each dump, and index designators and `va_arg` types in the tree.

## 2. The reducer

`tools/reduce/reduce.py INPUT.c --mode MODE [-o OUT.c]`. The input is never modified. Full option list in `tools/reduce/README.md`.

Modes (built-in predicates):

| Mode | Interesting when |
|---|---|
| `ice` | this compiler exits 3; `--message` must be in its stderr |
| `crash` | this compiler panics (exit 101) or is killed by a signal (other than `SIGXFSZ` from `--file-limit`) |
| `miscompile` | the program's exit status or stdout differs between Clang's build and ours (our IR linked by Clang). Guarded against undefined behaviour: the reference must compile without a list of UB-related Clang warnings, and a `-fsanitize=address,undefined` build must agree and report nothing (its exit status is set to 86 through `ASAN_OPTIONS` / `UBSAN_OPTIONS`, and its stderr is searched for sanitizer reports). The box's Clang has no sanitizer runtime, so the reducer picks GCC for that build automatically (`--ub-cc`) |
| `reject` | we report an error and Clang accepts (`-fsyntax-only`, plus `-pedantic-errors` in C11 mode) |
| `accept` | we accept and Clang rejects |
| `bad-ir` | we accept and Clang cannot compile our IR |
| `cmd` | any shell command exits 0 (`{}` is the candidate path) |

`--clang-valid` keeps every candidate acceptable to Clang in the `ice`, `crash` and `cmd` modes. `--std gnu11`, `--flags` and `--clang-flags` pass options through.

Passes, repeated until a round removes nothing: ddmin over lines; over top-level declarations (directive, function definition, or declaration to its `;`); inside each balanced `{ }` over its statements, and inside other `{ }` and `( )` over comma-separated items (initializers, enumerators, arguments, parameters); then over single tokens. A small tokenizer keeps each token's preceding whitespace and comments, so the unreduced text renders back exactly; removed tokens never let two neighbours merge into one, and directives stay on their own lines. Every command has a timeout (default 10 s) and runs in its own process group, which is killed with all its descendants when the time is up. Its stdout and stderr go to files limited to `--output-limit` (16 MiB, checked while it runs), and no file it writes may pass `--file-limit` (1 GiB, through `RLIMIT_FSIZE`, lowered to an inherited hard limit). A command stopped by `SIGXFSZ` has no result. A timeout, a limit, or a command that cannot be started is never interesting, and `reduce.py` checks before reducing that the compilers it needs exist (exit 2 otherwise). Each candidate's result is cached by SHA-256, so nothing is tested twice.

Tests: `tools/reduce/test_reduce.py`, 37 unit tests in about 11 s: ddmin on synthetic predicates (the result holds, is 1-minimal, keeps order, needs complements, can be empty), the tokenizer's exact round trip, the unit splitters, the cache (no candidate tested twice), `--max-tests`, timeouts (and that a timeout kills the command's children), the output limit (also on a command that keeps running), the file-size limit (`SIGXFSZ` is not a result, and the reducer's own writes are not limited) and an inherited lower hard limit, limits out of range, a missing compiler, an uninteresting input (exit 2), a sanitizer report rejecting a `miscompile` candidate, and each mode with stand-in compilers: `ice` with a message filter, `crash` by signal (and that an ordinary error is not a crash), `miscompile`, `reject` (the result still passes `clang -std=c11 -pedantic-errors`), and `cmd` with and without `{}`.

## 3. Demo on real bugs

`tools/reduce/demo/run_demo.sh` builds the compiler at `0cab101` (before pull request 46) and reduces two of the bugs pull request 46 fixed, then checks that the current compiler no longer shows them. The results are in `demo/`.

| Input | Mode | Before | After | Tests run | Time |
|---|---|---|---|---|---|
| `tools/reduce/demo/bitfield_mix.c`, a mixed program (accounts, Fibonacci, rotations, a `switch`, a union, a bit-field struct) | `miscompile` | 118 lines, 2,879 bytes | **14 lines**, 362 bytes | 1,803 (plus 2,221 cache hits) | 80 s |
| `tools/reduce/demo/wide_address.c`, a set of static initializers | `bad-ir` | 39 lines, 1,120 bytes | **2 lines**, 57 bytes | 260 | 1 s |

The first result keeps only the bit-field struct, the function with `flags->level += step;` (a `double`), and a `printf` of the neighbouring field, which the old compiler zeroed (bug b). The second is exactly the reproducer in the follow-up report: `int counter; __int128 wide_address = (__int128)&counter;` (bug a). The current compiler prints the right value for the first and rejects the second with a diagnostic, and the demo checks both.

## 4. Verification

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --offline --all-targets -- -D warnings` | clean |
| `cargo test --offline --lib` | **213 passed** (209 on `main`, 4 new) |
| `make` and `python3 harness/run_tests.py` | **187/187** |
| `python3 harness/run_dump_tests.py` | **24/24** |
| `python3 -m unittest tools/reduce/test_reduce.py` | **37 passed** |
| `tools/reduce/demo/run_demo.sh` | ok (section 3): the current compiler compiles both reduced files and agrees with Clang on the first, and rejects the second with the expected error (exit 1) |
| Old (`2846773`) vs new compiler on every `.c` file under `tests/`, WACC, GCC torture, Csmith, One True AWK and the C11 challenge, no flag and `-std=gnu11` | **13,172 compilations, 0 differences** in exit status, messages or IR (9,558 accepted, 3,614 rejected) |
| Same, on SQLite 3.54.1 (`sqlite3.c`, `shell.c`), Lua 5.4.7 (34 files) and DOOM (95 files), `-std=gnu11` with `-D_DEFAULT_SOURCE` (`sqlite3.c` also with `-DSQLITE_THREADSAFE=0`) | **0 differences.** Identical IR for both SQLite files, all 34 Lua files and 84 DOOM files. The other 11 DOOM files are front ends for SDL, Allegro, X11, Windows and other platforms whose headers are not installed here (and `doomgeneric_linuxvt.c`, which redefines a kernel header macro); both compilers reject them with the same message |

Earlier phases compared 13,148 compilations (6,574 files, 2 modes each). This run has 6,586 files (13,172 compilations): those 6,574, the 8 test files pull request 46 added, and the 4 new `harness/dumps/` programs. So the 13,148 compilations of the earlier phases are all included, with 0 differences. The comparison script is the same `irdiff.py` as parts B and C (`/workspace/scratch/phased/irdiff.py`, not committed).

Compile time: `sqlite3.c` (whole compilation, 5 alternating runs) took a median of 1.49 s on `main` and 1.53 s on this branch. The only new work on the normal path is three comparisons of the stop point, so this is probably code layout or noise; CI's `tools/perf_guard.py` still checks the budget.

Function size: `tools/quality/complexity.py` reports no new function over 100 lines. The dumper is split by kind of node; the longest new function is `Dumper::postfix` at 82 lines, and `driver::parse_args` grew from 110 to 114 lines. (`sema/init.rs` `fold_one`, now 123 lines, grew past 120 in pull request 46 and is unchanged here.)

## 5. CI

The `check` job (Linux) runs one new step after the existing script tests: `python3 harness/run_dump_tests.py` and `python3 -m unittest tools/reduce/test_reduce.py`, a few seconds in total. Nothing is added to the macOS workflow.

## 6. Limits and notes

- The sema dump shows conversions where the tree implies them: arithmetic operators and comparisons, shifts, unary operators, assignment, initialization, call arguments and `return`. It does not spell out the conversions inside a compound assignment (`a += b`) or a conditional's condition, which IR generation applies directly.
- File-scope initializers in the sema tree appear as parsed; their folded values are under `globals`.
- The reducer is sequential. A miscompile reduction costs about four compiler runs per candidate, so large inputs are slow; reducing the preprocessed file and then the lines pass first is the cheap path. The token pass can leave odd spacing (`flags-> mode`), which is harmless.
- The reducer cannot edit inside a preprocessor directive; it removes whole directive lines.

## 7. Step 8 overall

With part D, every item of step 8 in `PLAN.md` is done:

| Item | Where | Before → after |
|---|---|---|
| (a) coverage | part A | baseline 89.6% lines, 95.5% functions (unit tests and suites together) |
| (b) complexity | parts A, C | functions over 120 lines 14 → 0 (one at 123 since pull request 46); over 200: 2 → 0 |
| (c) mutation testing | parts A, B | the 17 targeted functions: 46% → 72% of planted bugs caught |
| (d) split the largest files | part C | largest file 8,766 → 1,535 lines |
| (e) crash points to diagnostics | parts B, C | unclear crash points 40 → 0; 333 → 285 sites; internal errors exit 3 with the phase and line |
| (f) diagnosis aids | parts B, D | stage dumps, a reducer, `docs/DEBUGGING.md` |

Each part kept the compiler's output byte for byte on the benchmark set, except for the deliberate fixes in the phase C follow-up. Next is step 9, Linux on ARM.
