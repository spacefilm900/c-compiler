# Quality phase B: internal errors instead of crashes, tighter unit tests (step 8, part B)

Branch `quality-b-diagnostics`, from `main` at `fff6635`, measured on the x86_64 Linux box (Rust 1.85.1, Clang 19.1.7) on 2026-11-17. Part A's baseline is in [../baseline-2026-11/README.md](../baseline-2026-11/README.md).

## Summary in plain English

- **The compiler no longer has "unclear" crash points.** Part A found 40 places where the compiler would crash if an earlier stage had missed a bad program. All 40 are gone. Most now print a proper message, such as `t.c:3:5: internal compiler error: 'break' is not inside a loop or switch (in irgen)`, and the compiler exits with status 3 instead of crashing. The rest were rewritten so that there is nothing left to crash on.
- **One of them could really be hit.** In GNU mode, a `case` label of a `switch` placed inside a statement expression (`switch (x) { case 0: ({ case 1: ; }); }`) crashed the compiler. GCC's manual forbids it, and GCC and Clang reject it. It is now a normal error: `'case' label jumps into a statement expression from its switch`.
- **Fewer crash points overall:** 333 → 284 (not counting `assert!`s, which went from 7 to 1). `irgen.rs` went from 134 to 89.
- **The unit tests now check exact output where they did not.** On the 17 functions chosen from the part A mutation sample, the unit tests caught 46% of deliberately planted bugs before and 72% after. Of the 54 planted bugs part A's sample had shown slipping through in those functions, 50 are now caught; the other 4 change nothing that a program can observe.
- **No change for valid programs.** The old and new compilers produced byte-identical output, messages and exit status for every one of 13,148 compilations (all `.c` files in the test suites, in strict and GNU mode).

## Headline numbers

| Measure | Before (`fff6635`) | After |
|---|---:|---:|
| Crash points (unwrap, expect, panic!, unreachable!) | 333 | **284** |
| ... of which (c) unclear | 40 | **0** |
| ... in `irgen.rs` | 134 | 89 |
| `assert!` / `assert_eq!` in non-test code | 7 | 1 |
| Crash points on lines the unit tests never run | 96 (5 unclear) | 85 (0 unclear) |
| Mutants caught on the 17 targeted functions (caught / missed / timeout) | 143 / 170 / 2: **46%** | 219 / 87 / 2: **72%** |
| Part A sample misses in those functions now caught | 0 of 54 | **50 of 54** |
| Unit tests | 167 | 196 |
| Valid-program output differences, old vs new compiler | | **0** of 13,148 compilations |

## 1. How an internal error works

- `Diagnostic` has an optional `phase`. `Diagnostic::internal(file, line, column, phase, message)` prints `file:line:column: internal compiler error: message (in phase)`, or `file: internal compiler error: ...` when no position is known (a file-scope initializer). The driver exits with status 3 (`diag::EXIT_INTERNAL_ERROR`); an ordinary error still exits 1 and a usage error 2.
- `irgen::render` returns `Result<String, Diagnostic>`. The free functions (`render_function`, `render_global`, `render_typed_const`, `render_const`) pass an `Ice` up through `Result`.
- Inside the big `Gen` lowering functions, rewriting every signature to return `Result` would have been a large restructuring, which belongs to part C. Instead `Gen::ice` records the first error at the current statement or expression and returns a placeholder operand, so lowering finishes the function, and `render_function` then returns the error instead of the IR. Later, follow-on problems in the same function do not replace the first message.
- The position comes from the expression being lowered when there is one, or else from the current statement. `lib.rs` takes the file name from the token at that position, as semantic analysis does, so an error inside an included header names the header.
- Semantic analysis's two "expression has no type" errors are internal errors too (phase `sema`).

## 2. The 40 unclear crash points

Source: [../baseline-2026-11/crash-points.csv](../baseline-2026-11/crash-points.csv), category c.

| What happened | Sites |
|---|---|
| Now an internal error at the source position (`irgen.rs`: `break`/`continue` with no target, switch with no promoted type, case with no value or not collected, struct value with no address, address of a non-lvalue, struct-returning call with no value, missing `sret` attribute, struct return from a scalar function, bit-field access on a non-bit-field, void value used, label address outside a function, unary `+`/`-`/`~` on a non-integer, `sizeof` of an incomplete type, string literal with no symbol, member with no offset or index, pointer arithmetic or difference on a bad pointer, compound assignment and `++`/`--` on a non-arithmetic operand, malformed `__builtin_nans` tag, overflow builtin result type, global initializer of the wrong kind; `render_function`'s missing signature) | 33 |
| The check was already made one line earlier, so the code now uses `if let` / `let ... else` and nothing can crash (`aapcs64::classify`, `abi::classify_for`, `convert` float to `_Bool`, `gen_compound_assign` and `gen_incdec` pointer and float cases) | 6 |
| `sema::integer_ty` removed: `require_integer` now returns the integer type it checked | 1 |

The six `assert!`s in `start`, `br`, `br_cond`, `gen_switch` and `gen_ternary` became internal errors as well, as did a few category (a) neighbours on the same paths (the `unreachable!`s in `render_typed_const`, `render_const`, `materialize_call` and `gen_ternary`).

**Reachable from user input: one.** The fuzzer had not found it. While writing tests, `case`/`default` inside a statement expression inside its switch reached `take_switch_label`, which assumed every label had been collected. The GCC manual ("Statements and Declarations in Expressions") says a switch may not jump into a statement expression; GCC 14 and Clang 19 reject it (black box). Semantic analysis now hides the enclosing switch while it checks a statement expression and reports the error; `break` out of one still reaches the enclosing switch. Tests: a lib unit test for the errors and the valid neighbours, and `tests/gnu/14_switch_in_statement_expr.c` (nested switch, `break` and `continue` out of a statement expression) in the differential harness.

Crash points now: [crash-points.csv](crash-points.csv), [crash-points-summary.md](crash-points-summary.md). The hand review of part A is carried over with its line numbers moved to the new code ([crash-points-review.csv](crash-points-review.csv); all 68 entries still match).

"Lines the unit tests never run" uses the part A unit-test profile (`../baseline-2026-11/unit.lcov.gz`) for before and a fresh `cargo llvm-cov --lib` profile for after. It is not the "18" in part A's README, which used a narrower rule that was not recorded.

## 3. Tests added

- `src/irgen/ice_tests.rs` (18 tests): check a valid program, damage the checked syntax tree the way a bug in semantic analysis could (a `break` moved out of its loop, a case with no value, a member with no index or offset, a void call used as a value, a global initializer of the wrong kind, operands of the wrong type, an error inside one arm of `?:`, and so on), and expect the exact message and position. About half of the internal-error sites are reached by these tests; the rest guard paths that cannot be set up from a checked tree without rebuilding large parts of it.
- `src/irgen/precise_tests.rs` (8 tests): pin exact IR or exact run output (Clang's) for code the mutation sample showed no test checked: `u"..."`/`U"..."` strings used as pointers, union and bit-field record types, GNU case ranges (listed, tested as a range, empty, one value; signed and unsigned; four widths), struct values from comma and statement expressions, a statement expression's own locals, integer to floating conversions, `x ?: y`, Apple arm64 `long double` conversions, constant folding in static initializers, enumerators and array bounds, and unsigned case-range ordering.
- `diag.rs` and `driver.rs`: message format and exit status. `lib.rs`: the statement-expression switch rule.
- `convert` had a second, unreachable copy of its floating-to-floating branch. It is removed; it produced 5 mutants that no test could ever catch.

## 4. Mutation testing on the targeted functions

cargo-mutants 26.0.0, unit tests only (`-- --lib`), 3 jobs. The targeted set is every mutant in these 17 functions, chosen because the part A sample showed misses there that affect real output: `render_string`, `Gen::gen_switch`, `Gen::convert`, `Gen::record_ptr`, `Gen::gen_stmt_expr_body`, `Gen::gen_cond_short`, `Gen::test_nonzero` (it has no mutants under this regex), `case_le`, `ByteImage::zero`, `ByteImage::write`, `compare`, `arith`, `eval_shift` (all `sema.rs`), and `ice_arith`, `ice_compare`, `eval_ice_bin`, `ice_shift_count`, `bits_ice` (`parser.rs`).

```sh
cargo mutants --file src/irgen.rs --file src/sema.rs --file src/parser.rs \
  --re 'in (render_string|Gen::gen_switch|Gen::convert|Gen::record_ptr|Gen::gen_stmt_expr_body|Gen::gen_cond_short|Gen::test_nonzero|case_le|ByteImage::zero|ByteImage::write|compare|arith|eval_shift|ice_arith|ice_compare|eval_ice_bin|ice_shift_count|bits_ice)$' \
  -j 3 -- --lib
```

Before ran on `fff6635` (315 mutants), after on this branch (308: the removed duplicate branch took 7 with it). `gen_switch` was rerun after one more test was added (19 of 19 caught). Lists: [mutants/](mutants/).

| Function | Before: caught / missed | After: caught / missed |
|---|---:|---:|
| `render_string` | 1 / 7 | 8 / 0 |
| `Gen::gen_switch` | 1 / 17 | 19 / 0 |
| `Gen::convert` | 18 / 11 | 17 / 4 |
| `Gen::record_ptr` | 5 / 2 | 7 / 0 |
| `Gen::gen_stmt_expr_body` | 1 / 2 | 3 / 0 |
| `Gen::gen_cond_short` | 1 / 0 | 1 / 0 |
| `case_le` | 1 / 5 | 6 / 0 |
| `ByteImage::write` | 1 / 8 | 1 / 8 |
| `ByteImage::zero` | 1 / 4 | 1 / 4 |
| `compare` | 13 / 23 | 19 / 17 |
| `arith` (2 timeouts both times) | 24 / 14 | 26 / 12 |
| `eval_shift` | 15 / 2 | 16 / 1 |
| `ice_arith` | 17 / 34 | 38 / 13 |
| `ice_compare` | 16 / 20 | 19 / 17 |
| `eval_ice_bin` | 20 / 12 | 25 / 7 |
| `ice_shift_count` | 4 / 4 | 7 / 1 |
| `bits_ice` | 4 / 5 | 6 / 3 |
| **Total** | **143 / 170 (46%)** | **219 / 87 (72%)** |

The 4 part A sample misses that survive are `convert`'s `<` to `<=` and `>` to `>=` (twice), which only change comparisons of two equal widths or ranks that an earlier `return` already handled, and `ByteImage::write` losing its `Bytes` arm, which the general arm then handles with the same result. Most of the remaining misses in `compare`, `ice_compare` and `ice_arith` are operators not yet exercised at every width (`<=`, `>=`, `|` at `unsigned long`, and so on); `ByteImage::zero` and `write` need relocation images that overlap earlier writes. Those are left for later.

## 5. No output change for valid programs

`/workspace/scratch/phaseb/irdiff.py` (not committed) ran the `fff6635` compiler and this branch's compiler on every `.c` file under `tests/`, WACC, GCC torture, Csmith, One True AWK and the C11 challenge, with no flag and with `-std=gnu11`: 6,574 files, 13,148 compilations (9,536 accepted, 3,612 rejected). IR, messages and exit status were identical for all of them. The new statement-expression error is not triggered by any of them. Local checks: `cargo fmt`, `cargo clippy -D warnings`, `cargo test --lib` (196 passed), `make` and `harness/run_tests.py` (180/180, one new file). CI runs WACC, torture, Csmith, DOOM, Lua, SQLite and One True AWK.
