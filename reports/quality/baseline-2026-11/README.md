# Quality baseline, November 2026 (step 8, part A)

Measured on `main` at `f11131e` on 2026-11-16, on the x86_64 Linux box (Rust 1.85.1, Clang 19.1.7). This part only measures: no compiler code changed. "Non-test code" means `src/` without `#[cfg(test)]` modules and the test-only `target_tests.rs`.

## Summary in plain English

- **The tests reach most of the code.** Nine lines in ten of the compiler run under at least one test (89.6%), and 95.5% of functions. The Rust unit tests alone reach 85.1%; the outside test suites alone reach 80.9%. The gaps are small and scattered. The biggest are ARM64 (`aarch64_va_class`) and Apple variadic paths that the Linux runs cannot reach, a byte-blob printer, and a few constant-folding branches.
- **Bad input does not crash the compiler.** It has 333 places that would crash if an assumption failed (`unwrap`, `expect`, `panic!`, `unreachable!`; there are no `todo!` or `unimplemented!`). Then 225,740 broken C programs, made by randomly damaging 2,876 real test files, were fed to it. Not one crashed, hung or hit any of those places: each got a normal error message. On reading, 293 of the 333 are safe by design. 104 are checked a few lines earlier. The other 189 rely on an earlier stage having already rejected bad input, and the damaged programs back that up. The other 40 are "unclear", nearly all in `irgen.rs`, which trusts `sema.rs`. None was shown to be reachable.
- **The code is large but not tangled.** It has 1,152 functions. Only 26 are over 100 lines and 2 are over 200: `check_stmt` in `sema.rs` (246) and `gen_expr` in `irgen.rs` (210). The three big files hold 21 of the 26.
- **The unit tests are weak at checking exact output.** Mutation testing makes small deliberate bugs and checks whether any test notices them. In about 1.5 hours on 794 sampled bugs, the unit tests caught 70% of those that built. The rate is much the same in every file: 68% in `irgen.rs` and `sema.rs`, 71% in `parser.rs`, 74% in `pp.rs`. The unit tests often check only that IR is produced, not that it is right. The outside suite does not fill the gap either: it caught only 4 of 20 sampled bugs that the unit tests had missed.

## Headline numbers

| Measure | Value |
|---|---|
| Line coverage, unit tests + suites (unit alone; suites alone) | **89.6%** (85.1%; 80.9%) of 22,466 lines |
| Function coverage, unit tests + suites | **95.5%** of 1,554 |
| `sema.rs` / `parser.rs` / `irgen.rs` line coverage | 89.2% / 91.3% / 92.5% |
| Functions over 100 / over 200 lines | **26 / 2** (of 1,152) |
| Functions with cyclomatic complexity over 25 / over 50 | 17 / 2 |
| Crash points (unwrap, expect, panic!, unreachable!) | **333**: (a) invariant 293, (b) reachable from bad input **0**, (c) unclear 40 |
| ... of which on lines no test reaches | 18 |
| Fuzzed bad inputs / crashes / hangs | 225,740 / **0** / 0 |
| Mutation sample: caught / missed / timeout / unviable | 794 mutants: **513 caught / 230 missed / 13 timeout / 38 unviable**, so **70%** of viable mutants were killed by the unit tests |

## 1. Tools and versions

| Tool | Version | Note |
|---|---|---|
| cargo-llvm-cov | 0.6.15 | with the `llvm-tools-preview` rustup component for Rust 1.85.1 |
| cargo-mutants | 26.0.0 | 27.x needs Rust 1.88 |
| complexity | `tools/quality/complexity.py` | rust-code-analysis-cli 0.0.25 would not build: its tree-sitter C++ grammar needs C++ standard headers the box lacks, and installing them would have upgraded held packages |

Install: `rustup component add llvm-tools-preview --toolchain 1.85.1`, `cargo install cargo-llvm-cov --locked --version 0.6.15`, `cargo install cargo-mutants --locked --version 26.0.0`. None of this runs in CI.

## 2. Coverage

`tools/quality/coverage.sh` runs the unit tests under `cargo llvm-cov`. It then builds an instrumented `compiler` (`cargo llvm-cov show-env`) and runs the main differential suite (179/179 passed), WACC (1,384 passed) and GCC torture (1,333 passed, 413 known failures) with it, and merges the two profiles. Line and function coverage only: branch coverage needs a nightly toolchain.

- Per file: [coverage-combined.txt](coverage-combined.txt) (also `coverage-unit.txt`, `coverage-suites.txt` and the `.csv` files).
- Largest uncovered regions in `sema.rs`, `parser.rs` and `irgen.rs`: at the end of `coverage-combined.txt`; every run of 3 or more uncovered lines is in [uncovered-combined.csv](uncovered-combined.csv). The unit-test-only view is [uncovered-unit-top.txt](uncovered-unit-top.txt).
- Raw data: `unit.lcov.gz`, `suites.lcov.gz`. Suite logs: `suite-*.log`.

The largest uncovered runs with all tests together:

- **irgen.rs:** `aarch64_va_class` (32 and 9 lines), `aapcs64_va_arg_addr` (14), which are ARM64-only and reached only by the ARM64 IR runner and the macOS job; `render_byte_blob` (28); `write_const_bytes` (12, 11, 8); `convert` (13); `spill` (12).
- **parser.rs:** `eval_ice` (15, a `?:` branch of integer constant expressions), `parse_enum_specifier` (10), `parse_builtin_offsetof` (7), `check_uneval` (7, 6).
- **sema.rs:** `ByteImage::write` (18), `eval_num` (15), `eval_const_expr` (15), `finish` (10), `check_compound_assign` (10), `check_inline_builtin` (10).

## 3. Size and complexity

`tools/quality/complexity.py` approximates McCabe complexity from the text: 1 + `if`, `while`, `for`, match arms after the first, `&&`, `||`. Data: [complexity-files.csv](complexity-files.csv), [complexity-functions.csv](complexity-functions.csv), [complexity-top.txt](complexity-top.txt) (the top 25 by length and by complexity).

| File | Non-test lines | Functions | Longest | Over 100 lines |
|---|---:|---:|---:|---:|
| sema.rs | 7,005 | 219 | 246 | 7 |
| parser.rs | 6,444 | 212 | 175 | 7 |
| irgen.rs | 4,969 | 146 | 210 | 7 |
| pp.rs | 3,467 | 157 | 168 | 1 |
| ast.rs | 3,353 | 179 | 171 | 2 |

The longest functions: `sema::check_stmt` 246 lines (complexity 41), `irgen::gen_expr` 210 (51), `irgen::gen_inline_builtin` 192, `parser::parse_specs_inner` 175, `ast::dump_expr` 171. The most complex: `parser::type_only` 53, `irgen::gen_expr` 51, `token::keyword` 49 (a flat keyword table, harmless), `parser::check_uneval` 48.

## 4. Crash points

`tools/quality/crash_points.py` lists every `.unwrap()`, `.expect(..)`, `panic!`, `unreachable!`, `todo!`, `unimplemented!` and `assert*!` in non-test code. Comments and strings are skipped, and so is the parser's own `self.expect(..)`, which returns an error. Data: [crash-points.csv](crash-points.csv) (file, line, kind, function, category, reason, code) and [crash-points-summary.md](crash-points-summary.md). The 7 `assert!` / `assert_eq!` sites are in the CSV, but the counts here leave them out.

Categories: (a) a true invariant, (b) reachable from bad input, (c) unclear. Categories come from rules in the script plus a hand review of 68 sites, recorded in [crash-points-review.csv](crash-points-review.csv). Two kinds of (a) are worth telling apart:

- **Checked locally (104):** the value was tested or pushed a few lines earlier, or it is a write into a `String`.
- **Relies on an earlier phase (189):** for example `irgen.rs` expecting a type that `sema.rs` always sets, or an `unreachable!` arm of a match that was already narrowed. These are set by rule and not hand-checked one by one.

(b) is set only by evidence. `tools/quality/panic_fuzz.py` compiled 2,616 to 2,876 seed files from `tests/`, WACC, GCC torture and the C11 challenge v0.6. It used single and stacked token-level damage, in strict C11 and in `-std=gnu11`, including stray bytes that are not valid UTF-8. That made 225,740 runs over three rounds (`fuzz/`). It found no panic, stack overflow, signal or hang. The 3 timeouts in an early round came from machine load and did not recur on an identical rerun. So (b) is 0.

| File | Total | a | c |
|---|---:|---:|---:|
| irgen.rs | 141 | 104 | 37 |
| sema.rs | 124 | 123 | 1 |
| parser.rs | 32 | 32 | 0 |
| pp.rs | 22 | 22 | 0 |
| others | 14 | 12 | 2 |

Of the (c) sites, 37 are in `irgen.rs`, which trusts that `sema.rs` rejected the case: `break` outside a loop, an aggregate with no address, a missing member offset, and so on. 18 crash points sit on lines no test runs. Fixing those is the cheapest win for part B.

Not counted: slice indexing, integer overflow in debug builds and `RefCell` borrows, which can also panic.

## 5. Mutation testing (bounded sample)

cargo-mutants 26.0.0 with `cargo test` (the unit tests) as the test. The baseline build took 5 s and the tests 18 s, and the timeout was set automatically. Two runs, in `mutants-core/` and `mutants-front/`, about 94 minutes of wall time in total:

1. **`irgen.rs`, first half**: `--file sema.rs parser.rs irgen.rs --shard 0/8`, 42 minutes. cargo-mutants' default "slice" sharding gave the first 424 of 3,392 mutants, which all fall in `irgen.rs`, lines 39 to about 2,980. Result: **279 caught, 131 missed, 5 timeouts, 9 unviable**, so 68% of viable mutants were killed.
2. **`sema.rs`, `parser.rs`, `pp.rs` spread evenly**: `--shard 0/10 --sharding round-robin`, 370 mutants, 52 minutes. Result: **234 caught, 99 missed, 8 timeouts, 29 unviable**, so 70% killed. By file: `sema.rs` 79/38/2/13 (68%), `parser.rs` 77/33/5/10 (71%), `pp.rs` 78/28/1/6 (74%), as caught/missed/timeout/unviable. Several `pp.rs` timeouts are mutants that make the preprocessor loop forever.

**Does the outside suite catch what the unit tests miss?** `tools/quality/check_missed.py` applied 20 evenly spaced missed `irgen.rs` mutants, rebuilt the compiler, and ran `harness/run_tests.py`. Only **4 of 20** were caught ([mutants-core/suite-check.csv](mutants-core/suite-check.csv)). Of the 16 that slipped through, 6 are ARM64 or Apple-only code, which the Linux suite never runs, and 10 are real gaps in the x86_64 tests. The full run with WACC and torture was not repeated for these.

Representative missed mutants that show real test gaps:

- `irgen.rs:373` `render_string`, `unit == 2` to `!=`: `u"..."` and `U"..."` string literals used as pointers get the wrong element type. Every test initialises an array from them instead (a `const char16_t *p = u"ab";` test would catch it).
- `irgen.rs:379` `render_string` byte packing (`*` to `/`): wide strings used as pointers again; the unit tests check that IR is produced, not what it says.
- `irgen.rs:2069`, `2075` `gen_switch`: range and table bounds in `switch` lowering.
- `irgen.rs:1814`, `1854` `convert`: two branches of integer and floating conversion.
- `irgen.rs:1084` `record_ptr`: a struct value from a comma expression, `(a, s).x`.
- `sema.rs:5289` `ByteImage::zero` and `5321` `ByteImage::write`: parts of static initializer images.
- `pp.rs:1850` `expect_end` returning `Ok(())`: extra tokens after a directive are not diagnosed by any unit test.

Full lists: `mutants-*/missed.txt`; `outcomes.json.gz` has every mutant.

## 6. Rerunning

```sh
tools/quality/coverage.sh                       # OUT=dir SUITES="tests wacc torture"
python3 tools/quality/complexity.py --csv f.csv --files files.csv
python3 tools/quality/crash_points.py --overrides reports/quality/baseline-2026-11/crash-points-review.csv --csv cp.csv --summary cp.md
python3 tools/quality/panic_fuzz.py --compiler build/compiler --corpus tests --out /tmp/fuzz [--stack 3] [--arg=-std=gnu11]
cargo mutants --file src/sema.rs --shard 0/10 --sharding round-robin -j 4
python3 tools/quality/check_missed.py --mutants-out mutants.out --scratch /tmp/mc --csv check.csv
python3 -m unittest tools/quality/test_quality_tools.py
```
