# Hardening, first campaign (2026-11-23)

Step 14 of `PLAN.md`. Compiler under test: `main` at `90e83d9` (the
frozen binary for every campaign below). Run locally on the 8-core
development box, not in CI. How to run each tool: `docs/FUZZING.md`.

**Result: no compiler bugs found.** No panic, no internal compiler error,
no hang, no invalid IR from an accepted input, no preprocessing round-trip
difference, and no wrong-code difference from Clang in any generator.

## Fuzzing the compiler itself (cargo-fuzz, libFuzzer with ASan)

Seeded with the 575 `.c` and `.h` files under `tests/` (external suites
excluded), with a C keyword and punctuator dictionary. Hours are each
target's own run time; several ran in parallel.

| Target | Hours | Inputs executed | Failures |
| --- | --- | --- | --- |
| `lex` | 1.0 | 7.3 million | 0 (coverage stopped growing after about 40 minutes) |
| `preprocess` | 2.4 | 0.72 million | 0 |
| `parse` (C11 and GNU11) | 2.9 | 0.90 million | 0 |
| `frontend` | 1.4 | 0.46 million | 0 |
| `frontend`, host headers (`HCC_FUZZ_HOST_HEADERS=1`) | 1.0 | 0.07 million | 0 |
| `frontend` with `llvm-as` on every accepted module | 2.9 | 0.98 million | 0 |
| `roundtrip` (IR unchanged after `-E`) | 1.6 | 0.58 million | 0 |
| **Total** | **13.2** | **11 million** | **0** |

The first ~20 minutes of `preprocess`, `parse` and `frontend` searched the
host's C library headers for every input and ran at about 15 inputs a
second; the targets now search only `include/` by default and run at
150–250 a second.

Triage of what libFuzzer reported:

- **Leaks** (LeakSanitizer, every input with a self-referencing struct):
  record types are reference counted and a record can point at itself, so
  they live until the process exits. Not a bug for a compiler process;
  leak detection is off in `tools/fuzz_run.sh`.
- **About 190 "timeouts" and "slow units"**: every one re-ran alone in
  under 1.5 seconds under ASan. The box pauses from time to time (the
  kernel log shows virtual machine snapshots), and those pauses were
  counted as timeouts.
- **One genuinely slow input** (2.5 KB, 1.4 s with `-E`): a macro that
  names itself, nested inside its own arguments. By the standard's
  expansion rules the output grows exponentially (4.5 MB here), and Clang
  takes 0.9 s on the same file. Not a bug.

## Random programs compared with Clang

| Generator | Programs | Compared | Agree | Differences |
| --- | --- | --- | --- | --- |
| Csmith (CI size caps, `-std=gnu11`), seeds 200001–203000 | 3,000 | 2,772 at `-O0`, 2,771 at `-O2` | all | 0 |
| YARPGen 2 (`--std=c`, about 8,000 lines each), seeds 1–2000 | 2,000 | 1,694 | all | 0 |
| Constant folding (`tools/constfold_fuzz.py`, new) | 20,000 | 12,756 | all | 0 |
| Record layout, C11 (`tools/layout_fuzz.py`, new) | 5,000 | 4,816 | all | 0 |
| Record layout, GNU11 with `packed`/`aligned` | 5,000 | 4,817 | all | 0 |

Programs not compared, and why:

- Csmith: Clang and GCC did not both finish (mostly the CPU limit on
  programs that loop for a long time); one candidate "timeout" was a box
  pause: the program compiled in 0.12 s when re-run.
- YARPGen: the Clang `-O0` reference crashed or ran out of memory (some
  programs keep very large arrays on the stack). One candidate "timeout"
  re-ran in 7 s and matched. Each run is now capped at 1 GiB of stack and
  4 GiB of address space so one program cannot take the box's memory.
- Constant folding: the expression has undefined behaviour (Clang's
  `-fsanitize=undefined` stops it) or is not a constant expression, so
  Clang rejects the static initializer.
- Layout: Clang rejected the declarations (for example `_Alignas` weaker
  than the member's type).

A further Csmith run with Csmith's own (larger) default program sizes,
seeds 300001–300600, was still running when this report was written.

## What this means and what comes next

The front end did not crash on 11 million mutated inputs, and five
independent program generators found no wrong code. The remaining items
of step 14 are coverage of the whole test set (to see what none of this
reaches), mutation testing on the core, and the new big program (QuickJS
with its test suite, then zlib).
