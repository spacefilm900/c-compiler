# Quality phase C: split big files and functions (step 8, part C)

Branch `quality-c-split`, from `main` at `6a8776c`, measured on the x86_64 Linux box (Rust 1.85.1, Clang 19.1.7) on 2026-11-17. Part B is in [../phase-b/README.md](../phase-b/README.md).

## Summary in plain English

- **The five largest source files are now directories of smaller files.** `sema.rs` (8,766 lines), `parser.rs` (7,786), `irgen.rs` (6,528), `pp.rs` (3,819) and `ast.rs` (3,559) each became a module directory. The largest source file in `src/` is now `lexer.rs` at 1,535 lines, and the largest file from the split is `sema/init.rs` at 1,385. Each split is its own commit and only moves code.
- **No function is longer than 120 lines.** Before, 14 functions were longer than 120 lines, and the two longest were 246 and 216 lines. Each was broken into named helpers, one per construct or step. Functions over 100 lines went from 27 to 16, and functions with cyclomatic complexity over 25 went from 17 to 13. None is now over 50 (before: 2).
- **Statement lowering in `irgen` now passes errors back with `Result`.** Part B's "record the first error and carry on with a placeholder" stays only in expression lowering, where every caller needs an operand back.
- **No change in behaviour.** The old and new compilers produced byte-identical IR, messages and exit status for all 13,148 compilations (every `.c` file in the test suites, in strict and GNU mode). Crash points are the same 285 sites. On part B's 17 targeted functions, the unit tests miss exactly the same 87 planted bugs as before.

## Headline numbers

Function figures cover non-test code, measured with `tools/quality/complexity.py`. Raw data: [functions-before.csv](functions-before.csv), [functions-after.csv](functions-after.csv), [files-before.csv](files-before.csv), [files-after.csv](files-after.csv), [complexity-before.txt](complexity-before.txt), [complexity-after.txt](complexity-after.txt).

| Measure | Before (`6a8776c`) | After |
|---|---:|---:|
| Largest source file | `sema.rs` 8,766 lines | `lexer.rs` 1,535 lines |
| Largest file from the split | | `sema/init.rs` 1,385 lines |
| Functions | 1,164 | 1,217 |
| Longest function | `check_stmt` 246 lines | `parse_member_list` 120 lines |
| Functions over 120 lines | 14 | **0** |
| Functions over 100 / over 200 lines | 27 / 2 | **16 / 0** |
| Functions over 50 lines | 130 | 136 |
| Cyclomatic complexity over 10 / 25 / 50 | 150 / 17 / 2 | 149 / **13 / 0** |
| Highest cyclomatic complexity | `type_only` 53 | `keyword` 49 (`token.rs`, one flat `match`, unchanged) |
| Crash points (unwrap, expect, panic!, unreachable!, assert!) | 285 | 285 |
| Unit tests | 196 | 198 |
| Mutants caught on part B's 17 targeted functions | 219 / 87 / 2 (72%) | 221 / 87 / 0 (72%) |
| Output differences, old vs new compiler | | **0** of 13,148 compilations |

Function length distribution (non-test functions):

| Lines | Before | After |
|---|---:|---:|
| 1–25 | 849 | 871 |
| 26–50 | 185 | 210 |
| 51–100 | 103 | 120 |
| 101–120 | 13 | 16 |
| 121–200 | 12 | 0 |
| over 200 | 2 | 0 |

The count of functions over 50 lines went up a little because each long function became one shorter dispatcher plus several helpers, and some helpers are 50 to 100 lines. The 16 functions between 101 and 120 lines (for example `convert`, `gen_expr`, `finish`, `eval_ice`, `dump_stmt`) are mostly a single `match` with one arm per case. Splitting them further would scatter the cases without making any one of them clearer, so they were left alone. `parse_member_list` grew from 118 to 120 lines only because rustfmt wrapped its signature once it gained `pub(super)`.

## 1. File splits (pure moves)

| Old file | Lines | New directory | Files | Lines now | `mod.rs` |
|---|---:|---|---:|---:|---:|
| `src/sema.rs` | 8,766 | `src/sema/` | 10 | 8,963 | 497 |
| `src/parser.rs` | 7,786 | `src/parser/` | 11 | 8,036 | 635 |
| `src/irgen.rs` | 6,528 | `src/irgen/` | 12 | 7,454 | 382 |
| `src/pp.rs` | 3,819 | `src/pp/` | 7 | 3,915 | 528 |
| `src/ast.rs` | 3,559 | `src/ast/` | 7 | 3,588 | 29 |

`irgen/` "lines now" includes `ice_tests.rs` and `precise_tests.rs`, which were already separate files. Most of the growth in the other directories comes from the helpers added later, plus a `use super::*;` line and an `impl` header in each file.

- `sema/`: `decl.rs`, `init.rs`, `image.rs` (static initializer bytes), `stmt.rs`, `expr.rs`, `call.rs`, `convert.rs`, `consteval.rs`, `tests.rs`.
- `parser/`: `decl.rs`, `specs.rs`, `record.rs`, `attrs.rs`, `declarator.rs`, `stmt.rs`, `expr.rs`, `ice.rs` (integer constant expressions), `uneval.rs` (operands that are not evaluated), `tests.rs`.
- `irgen/`: `module.rs`, `consts.rs`, `stmt.rs`, `call.rs`, `access.rs`, `vaarg.rs`, `expr.rs`, `inline_builtins.rs`, `tests.rs`, plus the existing `ice_tests.rs` and `precise_tests.rs`.
- `pp/`: `predefined.rs`, `expand.rs`, `directive.rs`, `ifexpr.rs`, `scan.rs`, `tests.rs`.
- `ast/`: `types.rs`, `record.rs`, `compat.rs`, `tree.rs`, `dump.rs`, `tests.rs`.

How the moves were made and checked:

- Each child file starts with `use super::*;`, and the parent declares the child and re-exports it (`pub use` where the child has public items). Items that were private became `pub(super)`, the narrowest visibility that still lets the sibling files use them. Nothing outside the crate sees a change, and an ambiguous name between two glob imports is a compile error, not a silent rebinding.
- A script (not committed) checked that every item of the old file appears in the new files with the same text, ignoring whitespace, trailing commas and the added `pub(super)`. There were 0 missing items in all five splits.
- The quality tools (`complexity.py`, `crash_points.py`, `lcov_summary.py`, `panic_fuzz.py`) and the `Makefile` were first taught to follow nested module directories, with a unit test. Their output on the old layout was unchanged.
- After the five moves, the 13,148-compilation comparison already showed 0 differences.

## 2. Function splits

Each of the 14 functions over 120 lines was split into helpers, keeping the order of every side effect: the same fresh LLVM names in the same order, the same diagnostics, and the same first error.

| Function | Before: file | lines / cc | After: file | lines / cc |
|---|---|---:|---|---:|
| `check_stmt` | `src/sema.rs` | 246 / 42 | `src/sema/stmt.rs` | 108 / 26 |
| `gen_expr` | `src/irgen.rs` | 216 / 52 | `src/irgen/expr.rs` | 119 / 30 |
| `gen_inline_builtin` | `src/irgen.rs` | 192 / 28 | `src/irgen/inline_builtins.rs` | 76 / 16 |
| `parse_specs_inner` | `src/parser.rs` | 175 / 44 | `src/parser/specs.rs` | 5 / 2 |
| `dump_expr` | `src/ast.rs` | 171 / 42 | `src/ast/dump.rs` | 107 / 29 |
| `target_macros` | `src/pp.rs` | 168 / 11 | `src/pp/predefined.rs` | 78 / 7 |
| `render_module` | `src/irgen.rs` | 164 / 29 | `src/irgen/module.rs` | 39 / 6 |
| `check_uneval` | `src/parser.rs` | 154 / 48 | `src/parser/uneval.rs` | 92 / 28 |
| `check_call` | `src/sema.rs` | 151 / 34 | `src/sema/call.rs` | 82 / 18 |
| `parse_builtin_offsetof` | `src/parser.rs` | 148 / 14 | `src/parser/expr.rs` | 35 / 3 |
| `type_only` | `src/parser.rs` | 137 / 53 | `src/parser/uneval.rs` | 66 / 26 |
| `define_record` | `src/parser.rs` | 126 / 23 | `src/parser/record.rs` | 76 / 15 |
| `declare_global` | `src/sema.rs` | 124 / 25 | `src/sema/decl.rs` | 62 / 13 |
| `gen_overflow` | `src/irgen.rs` | 122 / 15 | `src/irgen/inline_builtins.rs` | 50 / 9 |

New helpers, by function:

- `check_stmt`: `check_return`, `check_switch`, `check_case`, `note_case_range`, `check_for`, `check_goto_ptr`.
- `gen_expr`: `value_at`, `gen_unary`, `gen_binary`, `gen_assign`, `gen_sizeof`.
- `gen_inline_builtin`: `int_result`, `gen_count_bits`, `gen_ffs`, `gen_clrsb`, `gen_fp_classify`.
- `gen_overflow`: `overflow_operand`, `overflow_magnitude`, `overflow_mul`.
- `parse_specs_inner`: `parse_spec_word`, `parse_spec_ident`, `parse_spec_other`, `finish_specs`, `SpecFlags::has_type_specifier`.
- `dump_expr`: `dump_node`, `dump_generic`, `dump_call`, `unary_label`, `incdec_label`.
- `target_macros`: `float_macros`.
- `render_module`: `render_types_and_strings`, `render_lowering_declares`, `render_special_arrays`.
- `check_uneval`: `check_uneval_call`, `check_uneval_builtin_call`.
- `type_only`: `unary_type_only`, `bin_type_only`, `cond_type_only`, `call_type_only`, `member_type_only`.
- `parse_builtin_offsetof`: `parse_offsetof_designator`, `offsetof_member`, `offsetof_index`.
- `check_call`: `check_builtin_call`, `check_call_arity`, `check_call_args`.
- `define_record`: `record_being_defined`, `natural_layout_spec`, `attributed_layout_spec`.
- `declare_global`: `bind_global_declarator`, `compose_with_prior`.

## 3. Result propagation in statement lowering

Part B made the large `Gen` functions record the first internal error and return a placeholder, so that their signatures did not have to change. Part C changes this where it is clean:

- In `irgen/stmt.rs`, `gen_compound`, `gen_stmt`, `gen_if`, `gen_while`, `gen_do`, `gen_for`, `gen_switch` and `collect_switch_targets` now return `Result<(), Ice>`, and `take_switch_label` returns `Result<String, Ice>`. An error in a statement ends lowering of its function at once, and `render_function` passes it up with `?`.
- `Gen::fail` and `Gen::fail_at` build the `Ice` to return. If an expression error was already recorded, they return that one instead, so the first error is still the one reported.
- A statement expression (`({ ... })`) is lowered from inside expression lowering, so `gen_stmt_expr_body` turns a statement error back into a recorded error and stops.
- Expression lowering (`gen_expr` and the functions it calls) keeps the record-and-placeholder pattern. Every caller needs an operand back, and the expression functions call one another about 200 times. Changing all of those to `?` would be a large rewrite that changes nothing a user can see. `start`, `br` and `br_cond` keep it too.
- Two new tests in `irgen/ice_tests.rs` pin this down. One checks that an expression error before a statement error is the one reported. The other checks that a statement error stops lowering of the rest of its function.

Valid programs never reach these paths, so output is unchanged.

## 4. Crash points

`tools/quality/crash_points.py` finds the same 285 sites before and after: 284 crash points plus 1 `assert!`. Comparing `(kind, category, code)`, all but four sites match text for text. The four are `.expect("typed")` calls in `gen_expr` that moved into the new helpers and now read `operand.ty()` or `value_at(...)` instead of `expr.ty()`. They are the same checks. The tool's own rules, without part B's hand review, give 217 (a) and 68 (c) both times. Lists: [crash-points-before.csv](crash-points-before.csv), [crash-points-after.csv](crash-points-after.csv), [crash-points-summary.md](crash-points-summary.md).

## 5. Mutation testing on part B's targeted functions

The same 17 functions and the same command as part B, with the file paths pointing at the new directories:

```sh
cargo mutants --file 'src/irgen/*.rs' --file 'src/sema/*.rs' --file 'src/parser/*.rs' \
  --re 'in (render_string|Gen::gen_switch|Gen::convert|Gen::record_ptr|Gen::gen_stmt_expr_body|Gen::gen_cond_short|Gen::test_nonzero|case_le|ByteImage::zero|ByteImage::write|compare|arith|eval_shift|ice_arith|ice_compare|eval_ice_bin|ice_shift_count|bits_ice)$' \
  -j 3 -- --lib
```

Result: 308 mutants (the same 308 as part B), 221 caught, 87 missed, 0 timeouts, in 21 minutes. Lists: [mutants/](mutants/).

- The 87 missed mutants are exactly part B's final 87, compared by mutation and function, ignoring file and line. No function's count changed.
- Part B's two timeouts (`delete match arm BinOp::Mul` and `BinOp::Div` in `arith`) are now caught: a unit test fails before the timeout. The mutated code is the same, so this is a timing difference, not new test strength.
- `Gen::gen_switch` is 19 of 19 caught, as in part B's rerun after its last test was added. Part B's main run, before that test, had 17 of 19.

| Function | Part B after: caught / missed | Part C: caught / missed |
|---|---:|---:|
| `render_string` | 8 / 0 | 8 / 0 |
| `Gen::gen_switch` | 19 / 0 | 19 / 0 |
| `Gen::convert` | 17 / 4 | 17 / 4 |
| `Gen::record_ptr` | 7 / 0 | 7 / 0 |
| `Gen::gen_stmt_expr_body` | 3 / 0 | 3 / 0 |
| `Gen::gen_cond_short` | 1 / 0 | 1 / 0 |
| `case_le` | 6 / 0 | 6 / 0 |
| `ByteImage::write` | 1 / 8 | 1 / 8 |
| `ByteImage::zero` | 1 / 4 | 1 / 4 |
| `compare` | 19 / 17 | 19 / 17 |
| `arith` | 26 / 12 (2 timeouts) | 28 / 12 |
| `eval_shift` | 16 / 1 | 16 / 1 |
| `ice_arith` | 38 / 13 | 38 / 13 |
| `ice_compare` | 19 / 17 | 19 / 17 |
| `eval_ice_bin` | 25 / 7 | 25 / 7 |
| `ice_shift_count` | 7 / 1 | 7 / 1 |
| `bits_ice` | 6 / 3 | 6 / 3 |
| **Total** | **219 / 87 (72%)** | **221 / 87 (72%)** |

Part C was not meant to add tests for these, so the remaining misses are as part B describes them.

## 6. No output change

`/workspace/scratch/phasec/irdiff.py` (not committed; the same script as part B) ran the `6a8776c` compiler and this branch's compiler on every `.c` file under `tests/`, WACC, GCC torture, Csmith, One True AWK and the C11 challenge, with no flag and with `-std=gnu11`. That is 6,574 files and 13,148 compilations (9,536 accepted, 3,612 rejected). IR, messages and exit status were identical for every one, both after the file moves and at the final commit.

Local checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib` (198 passed), `make` and `harness/run_tests.py` (180/180), and `python3 -m unittest tools/quality/test_quality_tools.py`. CI runs WACC, torture, Csmith, DOOM, Lua, SQLite and One True AWK.
