# Hardening: coverage of hcc (step 1 of the second hardening campaign)

Measured on the x86_64 Linux box, 2026-11-23, with `tools/quality/branch_coverage.sh`.
"Before" is `main` at `f51cc91`; "after" is this branch with the new tests and fixes.
Only non-test code counts (`#[cfg(test)]` modules and `target_tests.rs` are left out).

## Summary in plain English

- **The tests now reach 92.7% of hcc's lines (was 90.8%), 96.3% of its functions
  (was 95.7%) and 83.4% of its branches (was 80.3%).** Branch coverage is new:
  it counts both directions of every `if`, `match` arm and `&&`/`||`.
- **The worst area was compile-time `long double` arithmetic** (`fp80.rs`,
  58.7% of lines, 35.8% of branches). New tests lifted it to 90.1% / 74.9% and
  **found three bugs** there, plus one in `float`/`double` folding.
- **Tests for other gaps found one more bug and one wrong message:** the old
  `offsetof` idiom `(long)&((struct S *)0)->m` was rejected as "not a constant",
  and an enumerator such as `enum { E = 1 / 0 }` was reported as an "array size"
  error.
- What is still uncovered is mostly error messages for rare misuse, ARM64- and
  Apple-only paths that the Linux runs reach only through the cross harness, and a
  few defensive branches that look unreachable (listed below).

## Bugs found and fixed (tests first)

| # | Bug | Test |
|---|---|---|
| 1 | `10.0L / 0x1p-16445L` (divide by a subnormal `long double`) folded to `+0` instead of `+inf`: the quotient overflowed a shift | `fp80::tests::division_by_a_subnormal_keeps_every_quotient_bit`, `tests/hosted/09_long_double_folding.c` |
| 2 | `long double` arithmetic on a NaN (x87 and binary128) returned the default NaN instead of passing the NaN operand through with its sign and payload, as Clang does | `fp80`/`fp128` NaN tests, test 09 |
| 3 | `long double` to/from `double`/`float` dropped NaN payloads | `fp80::tests::nans_keep_sign_and_payload_through_arithmetic_and_conversion`, test 09 |
| 4 | `float`/`double` folding of an invalid operation (`0.0/0.0`, `inf*0`) gave the host CPU's negative NaN, so `printf("%f", 0.0/0.0)` printed `-nan` where Clang's build prints `nan`, and the output depended on the machine hcc ran on | `float::tests::invalid_operations_fold_to_the_positive_default_nan`, test 09 |
| 5 | Address constants built from an integer pointer (`&((struct S *)0)->m`, `&((T *)0x40)[3]`, `((struct S *)0x100)->b`, and the cast of such an address to `long`) were rejected; C11 6.6p9 allows `[]`, `->`, `.`, `&` and `*` there | `sema::tests::address_constants_through_integer_pointers`, `tests/160_constant_forms.c` |
| 6 | Enumerator values and bit-field widths reported constant errors as "array size divides by zero" / "overflows" | `parser::tests::array_bound_errors_name_the_problem` |

Two cases where Clang accepts and hcc rejects were checked against the standard
and left as they are: `enum { E = 1 ? -1 : 0u }` (value not representable as
`int`, C11 6.7.2.2p2) and `(unsigned char)(long)(int *)0x1234` in an arithmetic
constant expression (pointer casts are not allowed there, 6.6p8). `#if (1, 2)`
and an unsuffixed decimal too big for `intmax_t` in `#if` are also rejected,
correctly (6.6p3, 6.4.4p2).

## New tests

- `tests/hosted/09_long_double_folding.c`: about 150 `long double` constants
  (conversions to and from every integer and floating type, rounding ties,
  overflow, subnormals, NaN payloads, comparisons, `__int128`), compared byte for
  byte with Clang. It runs on every target: x87 on x86-64 Linux and MinGW,
  binary128 on AArch64 Linux, `double` on Apple and MSVC.
- `tests/160_constant_forms.c`: array-bound constant forms (`?:`, unary `+`/`-`,
  every comparison, unsigned wrap-around, unevaluated division by zero), mixed
  integer and floating static initializers, union constants with floating, array
  and bit-field members and addresses, and pointer constants from integers.
- `tests/hosted/10_pp_if_arithmetic.c`: `#if` arithmetic with signed and unsigned
  operands (C11 6.10.1p4).
- Unit tests for the diagnostics nobody exercised: `#if` errors, array-bound
  errors, the `<stdatomic.h>` constraints, argument counts, `va_start` misuse.

## Numbers

All runs together (unit tests plus every suite):

| Measure | Before | After |
|---|---:|---:|
| Lines | 90.8% (24,665 of 27,151) | **92.7%** (25,194 of 27,182) |
| Functions | 95.7% | **96.3%** |
| Branches | 80.3% (6,135 of 7,644) | **83.4%** (6,394 of 7,664) |
| Unit tests alone: lines / branches | 86.6% / 75.2% | 87.8% / 76.7% |
| Suites alone: lines / branches | 84.0% / 70.8% | 86.7% / 75.0% |

Per module, after (before in brackets):

| Module | Lines | Line % | Function % | Branches | Branch % |
|---|---:|---:|---:|---:|---:|
| sema | 6,434 | 91.4 (89.1) | 96.7 | 1,842 | 83.9 (81.6) |
| parser | 5,906 | 92.3 (91.9) | 96.2 | 1,790 | 82.6 (82.2) |
| irgen | 5,216 | 94.8 | 96.6 | 1,100 | 86.5 |
| pp | 2,818 | 91.6 (89.4) | 96.2 | 968 | 78.9 (77.0) |
| ast | 2,152 | 92.7 | 94.5 | 430 | 87.0 |
| lexer | 871 | 97.2 | 100 | 212 | 90.6 |
| fp80 | 720 | 90.1 (58.7) | 100 (86.0) | 386 | 74.9 (35.8) |
| float | 563 | 94.8 (88.2) | 100 | 190 | 92.1 (83.7) |
| abi | 480 | 93.3 | 97.3 | 184 | 79.9 |
| fp128 | 321 | 94.4 (89.9) | 100 | 114 | 92.1 (86.8) |
| driver | 276 | 84.1 | 90.5 | 108 | 76.9 |
| others (bigint, builtins, target, aapcs64, win64, lib, token, ...) | 1,425 | 92-100 | | | |

What each suite reaches on its own (lines / functions / branches): differential
harness 73.4 / 82.0 / 62.1; GCC torture 63.6 / 72.8 / 51.0; IR checks for the
other targets 71.3 / 79.4 / 58.5; AArch64 harness 71.1; Apple arm64 68.5; Intel
macOS 73.5; challenge kits v0.7-v0.9 54.1; WACC 43.4; stage dumps 40.7.

Data: `coverage/before/` and `coverage/after/` (`modules-*.csv`, `coverage-*.csv`
per file for all / unit / suites, and `after/uncovered-all.csv`, every run of 3 or
more uncovered lines).

## Least-covered areas now

| File | Lines % | Branch % | What is missing |
|---|---:|---:|---|
| `irgen/consts.rs` | 72.4 | 55.6 | `render_byte_blob` and the floating and array arms of `write_const_bytes`. Semantic analysis already turns every union and bit-field record into bytes, so these look unreachable; kept as defensive code |
| `irgen/atomic.rs` | 93.5 | 72.7 | rarer `_Atomic` compound operations on floating and pointer types |
| `parser/specs.rs` | 88.2 | 73.3 | error paths for odd specifier combinations |
| `parser/ice.rs` | 88.9 | 73.1 | GNU `x ?: y` inside an array bound; error wording for rare operators |
| `pp/directive.rs` | 88.4 | 74.0 | `#line`/`#pragma` error paths, skipping in false groups |
| `fp80.rs` | 90.1 | 74.9 | `long double` to integer for values that do not fit (rejected anyway), pseudo-denormals |
| `driver.rs` | 84.1 | 76.9 | `-D'F(a)=...'` function-like macros on the command line, `--time-phases`, write errors |
| `timing.rs`, `fxhash.rs` | 28, 69 | | the `--time-phases` report and a hash helper only used on some paths |

ARM64 and Apple variadic code (`aapcs64.rs`, the Apple paths in `irgen`) is
reached by the cross harness under qemu; the real-Mac and Windows paths are run
by the weekly job on real machines, but not measured for coverage there.

## How it is measured

`tools/quality/branch_coverage.sh` builds hcc with `-C instrument-coverage
-Zcoverage-options=branch` on a nightly rustc (branch coverage is unstable in
1.85.1; hcc itself is still built and tested with 1.85.1). It runs the unit tests,
then an instrumented hcc under each suite into its own profile, and merges them
with `llvm-profdata` and `llvm-cov export`. It drives the instrumentation directly
because cargo-llvm-cov 0.6.15 (the last release for our Rust version) does not find
test binaries in the newer nightly build directory. `tools/quality/lcov_summary.py`
now also counts branches and rolls files up into modules.

Suites: unit tests (317), differential harness (211), stage dumps (24), IR checks
for five other targets, the AArch64 / Apple-arm64 / Intel-macOS harness runs
under qemu or the stand-in, WACC (1,384), GCC torture (1,378 + 368 known failures)
and the three challenge kits (245 cases, `-O0`). A local run takes about 15 minutes
on 8 cores.

```sh
rustup component add llvm-tools-preview --toolchain nightly
scripts/fetch_wacc_tests.sh && scripts/fetch_gcc_torture.sh
OUT=/tmp/cov tools/quality/branch_coverage.sh          # SUITES="tests wacc" for a subset
```

## Weekly coverage job

`weekly-platforms.yml` has a fifth job, `coverage` (Linux only, 45-minute cap,
about $0.15 a run, about $0.65 a month), that runs the same script every Sunday
and by hand (`gh workflow run weekly-platforms.yml -f jobs=coverage`). It puts the
per-module table on the run's summary page and uploads the CSVs and lcov files.
It is informational: it fails only when a suite does not finish. It is not part of
the pull-request CI.
