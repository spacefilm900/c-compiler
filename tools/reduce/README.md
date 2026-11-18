# Test-case reducer (step 8, part D)

`reduce.py` shrinks a C file while a predicate ("interestingness test")
keeps holding, so a bug found in a large program becomes a few lines that
can go into a regression test. It is written from the published
description of delta debugging (Zeller and Hildebrandt, "Simplifying and
Isolating Failure-Inducing Input", IEEE TSE 2002), not from the source of
any existing reducer. The input file is never modified.

```sh
python3 tools/reduce/reduce.py INPUT.c --mode MODE [options] [-o OUT.c]
```

## Modes

| `--mode` | Interesting when |
|---|---|
| `ice` | this compiler exits with status 3 (internal compiler error); `--message` must appear in its stderr |
| `crash` | this compiler panics (exit 101) or dies from a signal; `--message` filters stderr |
| `miscompile` | Clang builds and runs the program, this compiler's build (IR linked by Clang) gives a different exit status or stdout. The reference must have none of the warnings in `UB_WARNINGS` and must give the same result under `-fsanitize=address,undefined` (Clang, or GCC if Clang has no sanitizer runtime; `--ub-cc`, `--no-ub-check`) |
| `reject` | this compiler reports an error (exit 1) and Clang accepts the file (`-fsyntax-only`, plus `-pedantic-errors` for `-std=c11`); `--message` filters our stderr |
| `accept` | this compiler accepts the file and Clang rejects it; `--message` filters Clang's stderr |
| `bad-ir` | this compiler accepts the file and Clang cannot compile the IR it wrote; `--message` filters Clang's stderr |
| `cmd` | `--cmd COMMAND` exits 0. `{}` is replaced by the candidate path (or the path is appended); `$REDUCE_FILE` holds it too. Runs in a scratch directory |

`--clang-valid` additionally requires Clang to accept every candidate in
the `ice`, `crash` and `cmd` modes, so the result stays valid C.
`--std gnu11` selects GNU C for both compilers; `--flags` and
`--clang-flags` pass more options; the input's directory is on the include
path of both.

## Passes

Repeated until a whole round removes nothing (at most `--max-rounds`):

1. `lines`: ddmin over lines.
2. `decls`: ddmin over top-level declarations: a directive, a function
   definition, or a declaration up to its `;`.
3. `blocks`: for each balanced `{ }` with statements, ddmin over the
   statements (an `if (...) { }` or a nested block is one item); for any
   other `{ }` or `( )`, ddmin over its comma-separated items.
4. `tokens`: ddmin over single tokens.

`--passes lines,decls` picks a subset. Each command has a `--timeout`
(default 10 s); a candidate that times out is not interesting. Every
candidate's result is cached by its SHA-256, so nothing is tested twice.
`--max-tests` caps the work.

## Demo

`demo/run_demo.sh` builds the compiler at `0cab101` (before pull request
46) and reduces two real bugs that pull request 46 fixed:

| Input | Mode | Before | After |
|---|---|---|---|
| `demo/bitfield_mix.c` | `miscompile` | 118 lines | 14 lines (1,803 tests, about 70 s) |
| `demo/wide_address.c` | `bad-ir` | 39 lines | 2 lines (260 tests, about 1 s) |

It then checks that the current `build/compiler` no longer shows either
bug. `OLD_COMPILER=path` skips the build.

## Tests

`python3 -m unittest tools/reduce/test_reduce.py`: ddmin on synthetic
predicates (1-minimality, complements, order), the tokenizer (exact round
trip, directives, literals), the unit splitters, the cache, timeouts, and
each built-in mode with stand-in compilers. CI runs them in the Linux job.
