# Hardening, mutation testing (2026-11-23/24)

Step 14 of `PLAN.md`. Tool: [cargo-mutants](https://mutants.rs) 26.0.0, used as
a third-party tool only. It was run locally on the 8-core development box, not in CI.
The oracle is the unit test suite (`cargo test --lib`). The C harness, WACC and
GCC torture suites are too slow to run once per mutant, so they were run only
before and after the new tests were added (see "Regression results").

## Scope

There are seven files: constant folding, conversions, static images, layout, and the
arithmetic and conversion paths of the IR generator.

| File | What it holds |
| --- | --- |
| `src/sema/consteval.rs` | sema's integer and real constant folder |
| `src/sema/convert.rs` | constant conversions between arithmetic types |
| `src/sema/image.rs` | byte images of static unions and bit-field records, address constants |
| `src/float.rs` | IEEE folding: arithmetic, comparisons, hex/decimal rounding, long double formats |
| `src/parser/ice.rs` | the parser's integer-constant-expression folder (array bounds, enumerators, `_Static_assert`) |
| `src/irgen/expr.rs` | IR generation for expressions: arithmetic, conversions, member loads |
| `src/ast/record.rs` | struct and union layout (System V, Apple arm64, MSVC) |

## Commands

```sh
# pass 1, on main f51cc91
cargo mutants -j4 -o OUT \
  --file src/sema/consteval.rs --file src/sema/convert.rs --file src/sema/image.rs \
  --file src/float.rs --file src/parser/ice.rs --file src/irgen/expr.rs \
  --file src/ast/record.rs -- --lib
# later passes: same command plus --iterate on a copy of the previous OUT,
# which skips mutants already caught or unviable
```

## Results

**Pass 1** ran on `main` (`f51cc91`) before any new tests were added. It took 1 h 42 min.

| File | Caught | Missed | Timeout | Unviable |
| --- | --- | --- | --- | --- |
| `ast/record.rs` | 143 | 34 | 0 | 31 |
| `float.rs` | 307 | 120 | 0 | 8 |
| `irgen/expr.rs` | 110 | 15 | 6 | 2 |
| `parser/ice.rs` | 136 | 70 | 4 | 7 |
| `sema/consteval.rs` | 137 | 62 | 0 | 18 |
| `sema/convert.rs` | 39 | 6 | 0 | 2 |
| `sema/image.rs` | 195 | 58 | 0 | 9 |
| **Total (1519)** | **1067** | **365** | **10** | **77** |

Pass 1 caught 74.0% of the 1442 viable mutants (caught / (caught + missed + timeout)).

Each later pass re-ran only the mutants that were still alive, using `--iterate`. It also mutated
any code that had changed. Pass 3 and pass 4 include the new parser code for
brace-elided lengths (bug 1 below), so they contain mutants that pass 1 did not have.

| Pass | Tree | Mutants run | Caught | Missed | Timeout | Unviable |
| --- | --- | --- | --- | --- | --- | --- |
| 2 | first tests (`9540aed`) | 441 | 219 | 201 | 5 | 16 |
| 3 | both fixes, more tests (`e82db1e`) | 416 | 245 | 161 | 5 | 5 |
| 4 | final tests (`d8d7041`), only `consteval.rs`, `image.rs` and `ice.rs` | 176 | 80 | 96 | 0 | 0 |

Pass 4 only re-ran the pass-3 survivors in the three files whose tests changed after pass 3.
It selected them with `--re`, matching on the mutation text and ignoring the line, so it also re-ran
some twins of mutants that were already caught. The other four files keep their pass-3 outcome.
Their tests did not change after pass 3. A full `--iterate` pass on every file would have taken
about 90 minutes.

`8673c86` added 14 more expressions after pass 4 started. Three mutants were then
checked by hand and are caught: `<` to `<=` in `ice_compare`, the `&&` mask in
`eval_ice_bin`, and the `INT_MAX` limit in `bits_ice`.

**Before and after, for the 1519 mutants of pass 1.** Each pass-1 survivor was matched to its
latest outcome by file and mutation text.

| | Caught | Missed | Timeout | Unviable | Removed with dead code |
| --- | --- | --- | --- | --- | --- |
| Before (pass 1, `f51cc91`) | 1067 | 365 | 10 | 77 | — |
| After (final tests) | 1328 | 96 | 5 | 77 | 13 |

The new tests killed 261 of the 375 survivors, and the caught rate went from **74.0% to 92.9%**
of viable mutants. The five timeouts are `Gen::gen_assign`, `gen_compound_assign` and `gen_incdec` returning an empty
or junk value. The generated code then loops, which the test timeout stops, so in practice
they are caught. The new parser code for bug 1 added mutants of its own. 13 of its survivors
are listed below.

Survivors left per file, from pass-1 mutants and new code:

| File | Missed |
| --- | --- |
| `ast/record.rs` | 10 |
| `float.rs` | 23 |
| `irgen/expr.rs` | 4 (+5 timeouts) |
| `parser/ice.rs` | 20 (10 old, 13 new code, minus 3 checked by hand) |
| `sema/consteval.rs` | 12 |
| `sema/convert.rs` | 5 |
| `sema/image.rs` | 50 |

## Bugs found

1. **sizeof of an array completed by its initializer was rejected in an ICE.**
   For example, `struct P { char x, y; } a[] = {1, 2, 3}; char q[sizeof a];` failed with
   "invalid application of 'sizeof' to an incomplete type". C11 6.7.9p22 completes the type
   from the initializer, so `sizeof a` is a constant (6.6p6). The parser's ICE folder only
   counted top-level items when the initializer was not brace-elided. It now counts
   brace-elided items and `[k] =` designators the way 6.7.9p20 assigns them to subobjects
   (`parser/ice.rs` `elided_len`/`fill_subobject`). A string literal at a pointer member counts as
   one scalar. This was found from the surviving `infer_init_len` mutants.
   The tests are `tests/163_sizeof_brace_elided_array.c` and `sema::tests::infers_array_lengths_like_clang`,
   plus `parser_folds_sizeof_of_brace_elided_arrays`.
2. **A union bit-field took the size of its declared type.** On System V and Apple,
   `union { char a; int : 3; }` had size 4. Clang and GCC give 1: a bit-field in a union
   adds only the bytes its width needs. This was found by the layout probes written for
   `ast/record.rs` survivors. It is fixed in `layout_union_members`. The tests are `tests/164_union_bit_field_size.c`
   and `sema::tests::union_bit_fields_take_the_bytes_of_their_width`. A packed union with a
   bit-field now reaches the existing "bit-fields in a packed or aligned union are
   not supported" error. Before the fix it got a wrong layout.
3. **Dead code.** `ast::layout_union` had no callers. Its 13 surviving mutants
   showed this, and it was removed.

No wrong code was found in the folders, image builder or IR generator. Every
new test value was checked against Clang.

## Known limitation (not fixed)

The parser cannot type an initializer item that is a struct or union *value*. Examples are
a variable `s` of struct type, and `[k].m =` designator chains. An array whose length comes from
such items still gets its length from sema. Using `sizeof` of that array in an ICE context
is therefore still rejected. Clang accepts both of these:

```c
struct P { int x, y; };
int f(void) { struct P s = {1, 2}; struct P v[] = {s, s}; char q[sizeof v]; return sizeof q; }
struct P w[] = {[1].x = 1}; char r[sizeof w];
```

The error is "invalid application of 'sizeof' to an incomplete type". The test
`parser_folds_sizeof_of_brace_elided_arrays` pins the current behaviour, so a later fix shows up.
A fix needs the parser to know the type of every identifier and member, which is sema's job.
Another option is to fold such bounds after sema.

## Tests added

All expected values come from Clang 18 (black box). `sema/tests.rs`:

- `folds_every_comparison_and_operator_like_clang`: 77 initializers through sema's folder,
  and 33 of them again as array bounds through the parser's.
- `folds_shifts_limits_and_conditionals_like_clang`: 75 more, in gnu11. They cover shifts by 0
  and by unsigned counts, values at exactly `INT_MAX`/`INT_MIN`, unsigned negation and `~`,
  narrowing casts, `?:` and `?:`-omitted operand types and truncated conditions, and
  unevaluated `1 / 0`.
- `array_bounds_reject_overflow_and_undefined_operations`
- `infers_array_lengths_like_clang` (16 brace-elided and designated shapes)
- `parser_folds_sizeof_of_brace_elided_arrays`
- `bit_field_layouts_match_clang_on_sysv_and_msvc`, with probes on x86-64 and aarch64
  Linux, x86-64 Windows (MSVC) and Apple arm64
- `union_bit_fields_take_the_bytes_of_their_width`

`float.rs`:

- each float/double operator and comparison, NaN included
- hex constants rounding to nearest-even at the extremes
- float-to-integer limits
- the long double formats (binary128, x87, MSVC double)
- wide integer conversions and exponent limits

`irgen/tests.rs`:

- `float_compares_mixed_conditions_and_packed_members_match_clang`, which checks the unaligned loads
  of packed members
- `static_union_and_bit_field_images_match_clang`, a run against Clang covering overlapping designators,
  addresses in unions and nested records, overridden members, and straddling bit-fields

There are also two strict C11 harness tests: `tests/163`, `tests/164`.

## Surviving mutants

Each remaining survivor was read and sorted into one of the groups below. "Equivalent" means
no C program can tell the mutant from the original.

- **Equivalent: disjoint bits.** `|` to `^` where the operands cannot share a set bit. These are in
  `widen_f32_bits`, `pack`, `ieee_nan_bits`, `hex_float_bits`, `arith` and `ice_arith`.
- **Equivalent: masks applied twice.** An `&` mask whose caller masks again, or whose value is
  already masked. Examples are `eval_real_unary` and `negate` in sema, `eval_ice_unary`, and `shift_count`.
  Also `mant <<=` after a carry has cleared the low bits.
- **Equivalent: a fast path.** `has_real` and the `Int`/`Sizeof`/`CondShort` arms of `eval_num`
  only choose between the integer and real folders. Both give the same value for an integer expression.
- **Equivalent: diagnostics that sema repeats.** The parser's `ice_shift_count` and the
  `fold_array_bound` size limit. When the parser does not reject a bad bound, sema
  rejects it with the same message.
- **Equivalent: integer arms only.** In an ICE `?:`, both arms are integers. A floating arm is not
  an ICE operand (6.6p6), so the `is_integer()` guards in `eval_ice` cannot be told apart.
- **Unreachable or defensive.**
  - `Gen::spill`: a member base is always an lvalue or a record.
  - The `Decay` arms of `gen_addr` and `is_string_literal_arg`, and the array/function check in `gen_expr`.
  - `ByteImage::write` for `Bytes`/`RelocBytes` values at an offset. Only a struct-valued constant
    such as a GNU compound literal would reach it, and hcc rejects those in static initializers.
  - `reloc_width`: switching union members clears the whole image first.
  - `Debug` formatting, and `StructRef::body_key`, which is a hash key.
- **Seen only in another harness.** The MSVC bit-field offsets in `ms_layout_members` (`/` to `*` or `%`) and
  `unadjusted_align` are checked by the Wine cross harness, not by unit tests.
- **Real gaps left open.** These are mostly `sema/image.rs`: `eval_ptr` guards for pointer ± integer and
  pointer casts, `const_index` bounds, and `ByteImage::slice` overlap checks. Also the
  `&&`/`||` guards in the new `fill_subobject`/`infer_init_len` code (they only fall back to sema),
  `can_assign`/`cond_type` in `convert.rs`, and `real_is_zero` for quad. None of these was shown to
  be wrong code. They are still worth tests if this area changes.

## Regression results (final tree)

On `d8d7041`, plus `8673c86` for the lib tests:

| Suite | Result |
| --- | --- |
| `cargo test --lib` | 323 passed |
| `harness/run_tests.py` | 210/210 |
| WACC | 1384 passed |
| GCC torture | 1378 passed, 368 known, 0 unexpected |
| Windows x64 (`llc -filetype=obj`) | 165 accepted, 0 failed |
| aarch64 Linux / arm64 Apple / x86-64 Apple (on `545b1d8`) | 210/210, 208 + 2 known, 206 + 2 known |

A merge simulation of `main` + #64 + #65 + #66 + this branch was clean in every order tried.
It passed 335 lib tests and harness 220/220.

## Rerun

Pass 1 took 1 h 42 min with `-j4`. Each `--iterate` pass takes 30–60 min. To check whether the
slower suites catch a sample of the unit-test survivors, run
`tools/quality/check_missed.py --sample N` on a `mutants.out` directory.
