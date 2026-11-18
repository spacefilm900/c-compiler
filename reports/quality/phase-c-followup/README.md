# Quality phase C follow-up: fix the bugs found during review

Branch `fix-review-bugs`, from `main` at `0cab101` (pull request 45 merged), measured on the x86_64 Linux box (Rust 1.85.1, Clang 19.1.7, GCC 14.2.0) on 2026-11-18. Phase C is in [../phase-c/README.md](../phase-c/README.md); its section 7 lists the findings fixed here.

Every fix was written from C11 (N1570), the LLVM LangRef and the SysV AMD64 ABI. Clang and GCC were used only as black-box oracles.

## Summary in plain English

- **The three wrong-code bugs from phase C's review are fixed, and each has a regression test.**
- **All 17 "Minor" CodeRabbit findings on pull request 45 were real, and all 17 are fixed.** Phase C's report said 15. CodeRabbit marked 18 findings Minor. One of them, about `rust_scan`, was already fixed in pull request 45, which leaves 17.
- **No change in exit status or IR across the 13,148 tested compilations** (the test suites, so none of them hit the fixed bugs). Messages changed in 4, all from the `void *` / function pointer fix (section 4). The fixed programs themselves of course get different IR.
- **Review of this pull request found five more defects, all fixed** (section 6), including one wrong-code bug on AArch64 Linux that CI caught.

## 1. The three review bugs

| Bug | Reproducer | Before | Now | Test |
|---|---|---|---|---|
| a. An address converted to an integer wider than a pointer in a static initializer | `int g; __int128 a = (__int128)&g;` | IR that LLVM rejects (`ptrtoint ... to i64` used as `i128`) | error: `initializer of 'a' converts an address to an integer wider than a pointer, which is not a constant expression` (Clang also rejects it) | sema unit tests; `fffacec` |
| b. Floating compound assignment to a bit-field | `struct S { unsigned a:4, b:4; } s = {1,7}; s.a += 1.0;` | `b` became 0 | `b` stays 7 | `tests/147_bitfield_float_compound.c`; `7ebfef2` |
| c. A designator that initializes a whole subobject again | `struct S { int a[2]; } s = { .a = {1,2}, .a = {3} };` | `s.a[1]` was 2 | `s.a[1]` is 0 | `tests/148_designator_replaces_subobject.c`; `16ffcc1` |

Details:

- **a.** C11 6.6p10 lets an implementation accept other forms of constant expression. An address converted to a pointer-sized integer (64 bits on every supported target) is still accepted, as before. A wider (or narrower) integer is now rejected in `sema/init.rs`. `irgen/consts.rs` `render_const` reports an internal compiler error if a `PtrToInt` constant that is not 64 bits ever reaches it.
- **b.** `gen_compound_assign` computed the result correctly, but on the floating path it then did an ordinary store of the whole storage unit. Both the floating and the integer path now convert the result to the bit-field's declared type and go through `store_bitfield` (6.5.16.2p3, 6.3.1.2, 6.3.1.4). Converting to the declared type, not the promoted `int`, also fixes two more cases. One was `_Bool` bit-fields: `b += 0.25` gave 0 and now gives 1. The other was a `long long` right operand on `char` and `long long` bit-fields. The test covers `unsigned`, signed, `_Bool` and `long long` bit-fields with `float`, `double` and `long long` operands, and checks that neighbouring bit-fields keep their values.
- **c.** C11 6.7.9p19: a brace-enclosed list for a subobject initializes that whole subobject, and members it does not name are zero. So a later `.a = {3}` replaces everything an earlier initializer stored inside `a`. Shaping now records how far each subobject's stores reach (`init::Reach`). When a designator or a positional item opens a brace list for a whole aggregate, `Sema::replace_subobject` drops the earlier stores inside it and moves them to `overridden`. Their expressions are still type-checked (and constant-folded for static storage), but they are not evaluated at run time, which 6.7.9p19 footnote 151 allows ("might not be evaluated at all"). Element designators (`.a[1] = 5`) and brace elision still keep earlier values, as 6.7.9p19 requires. The test checks 15 values, and Clang and GCC agree on all of them.

## 2. Triage of the 17 Minor findings

| # | Finding (file) | Real? | Fix | Test |
|---|---|---|---|---|
| 1 | A bit-field's `bit_offset` truncated to `u32` (`ast/record.rs`) | yes | `u32::try_from`; a bit-field that would start at or past bit 2^32 is "too large" | `ast/tests.rs`, `parser/tests.rs` |
| 2 | The bit cursor used saturating arithmetic, so it could quietly stop at its maximum (`ast/record.rs`) | yes | checked arithmetic, and an error if it overflows | same |
| 3 | `#line` took any number, not a digit sequence (`pp/directive.rs`) | yes | 6.10.4p3: only a digit sequence, read as decimal (so `#line 010` is line 10, and `0x10` is an error) | pp unit test |
| 4 | Unsigned character constants in `#if` were signed (`pp/ifexpr.rs`) | yes | 6.10.1p4: `u'…'`, `U'…'` (and `L'…'` where `wchar_t` is unsigned) act as `uintmax_t` | pp unit test |
| 5 | A `##` that came from a macro argument was treated as the paste operator (`pp/expand.rs`) | yes | only a `##` written in the replacement list pastes (6.10.3.3) | pp unit test with the 6.10.3.3p4 `hash_hash` example |
| 6 | `&1[a]` (subscript operands swapped) was not folded in a static initializer (`sema/image.rs`) | yes | the integer operand is the index whichever side it is on (6.5.2.1p2) | `tests/149_static_address_initializers.c` |
| 7 | `void *` = function pointer was accepted in strict mode (`sema/convert.rs`) | yes | 6.5.16.1p1 pairs `void *` only with object pointers. Now an error with `-std=c11`, and a warning with `-std=gnu11` (the existing convention for GNU extensions) | sema unit test |
| 8 | `?:` with `void *` and a function pointer (`sema/convert.rs`) | yes | 6.5.15p3, same treatment as 7. The GNU result is `void *` | same |
| 9 | A block-scope function redeclaration did not keep the composite type (`sema/decl.rs`) | yes | 6.2.7p4: the local and file-scope entries take the composite type | `block_function_redeclaration_keeps_the_composite_type` |
| 10 | An enum specifier did not count as the type specifier, so `enum E int x;` was accepted (`parser/specs.rs`) | yes | 6.7.2p2 | parser unit test |
| 11 | `_Noreturn` (and `inline`) were accepted on objects, typedefs and tag-only declarations (`parser/specs.rs`) | yes | 6.7.4p2: "`'_Noreturn'` can only appear on functions" | parser unit test |
| 12 | Pointer + pointer was accepted in an unevaluated operand (`sizeof (p + p)`) (`parser/uneval.rs`) | yes | 6.5.6p2 checked there too | `unevaluated_operand_types_follow_c11` |
| 13 | Typing an unevaluated call ignored a local that shadows a library forwarder (`parser/uneval.rs`) | yes, but only reachable through reserved `__builtin_…` names | fixed so it matches sema | same |
| 14 | Typing an unevaluated `?:` on pointers did not form the composite type (`parser/uneval.rs`) | yes | shares sema's `merge_pointers`, now in `ast/compat.rs` | same |
| 15 | `a ?: b` (GNU, omitted middle operand) was not folded in floating constant expressions (`sema/consteval.rs`) | yes | `CondShort` handled in `has_real` and `eval_num` | `tests/gnu/15_cond_short_floating_constant.c` |
| 16 | A static `_Bool` initialized from an address was rejected (`sema/init.rs`) | yes | 6.3.1.2: a non-null address is 1. A `weak` symbol's address may be null, so it is not folded and stays an error | `tests/149_static_address_initializers.c`, sema unit tests |
| 17 | `va_arg` of a 16-byte-aligned record of one SSE eightbyte read it from the overflow area at 8-byte alignment (`irgen/vaarg.rs`) | yes. CodeRabbit labelled it AAPCS64, but the defect is on the SysV path. The AArch64 path already aligned to 16 | the `Fp` class carries its stack alignment and size | `tests/xabi/05_va_aligned_fp_def.c` / `_use.c` (cross-ABI against Clang) |

## 3. Seen while fixing, not fixed

These are recorded here and in `STATUS.md`. None is a wrong-code bug. (A fifth item, a saturating `u32` bit position for static arrays of bit-field structs past 512 MiB, was wrong code and is fixed; see section 6.)

- **A pointer assigned to, or initializing, a `_Bool` bit-field is rejected.** Clang accepts it. Plain `_Bool` objects are fine.
- **`# ##` in a function-like macro is accepted.** It should be a diagnostic, because `#` must be followed by a parameter (6.10.3.2p1).
- **`int *p = (int *)(__int128)&g;` is accepted** at file scope. GCC accepts it and Clang rejects it. 6.6p10 leaves this to the implementation.
- **Bit-field position limit.** A bit-field at or past byte 512 MiB of its record (bit 2^32) is now an error ("too large"). Clang accepts it. This is an implementation limit, and before this fix such a record was laid out wrong.

## 4. Output differences, old vs new compiler

`/workspace/scratch/fixbugs/irdiff.py` (not committed; the same script as phases B and C) compared the `0cab101` compiler with this branch's compiler. It ran both on every `.c` file under `tests/`, WACC, GCC torture, Csmith, One True AWK and the C11 challenge, once with no flag and once with `-std=gnu11`. That is 6,574 files and 13,148 compilations: 9,536 accepted and 3,612 rejected by both. **Exit status and IR were identical for every compilation.** Messages differed in 4 compilations, all from the `void *` / function pointer fix (triage 7 and 8):

| File | Mode | Old | New |
|---|---|---|---|
| torture `20041212-1.c` | strict | error at 15:12 (`!=` of `void *` and a function pointer) | error at 10:10, earlier (assigning `void *(*)(void)` to `void *`) |
| torture `pr17377.c` | strict | error at 51:13 (`!=`) | error at 24:14, earlier (assigning `void *(*)(int)` to `void *`) |
| torture `20041212-1.c` | gnu11 | one warning | the same warning plus one for the assignment, same IR |
| torture `pr17377.c` | gnu11 | one warning | the same warning plus one for the assignment, same IR |

Both torture files were already rejected in strict mode and accepted in GNU mode, so their status in the torture list does not change.

The same comparison with `-target aarch64-linux-gnu` gives these 4 plus 2 more: torture `pr78436.c` in both modes, where a `signed char` bit-field is now read with `sext` instead of `zext` (section 6). The program's result does not change because it only compares the field with 0.

Real programs, built with `-std=gnu11`, old vs new compiler:

- **SQLite** `sqlite3.c` and `shell.c`: identical IR. `sqlite3.c` gets 24 new warnings. They come from the math-function table, which stores function pointers in a `void *` field.
- **Lua 5.4.7:** 34 files, no differences.
- **DOOM:** 95 files. Identical IR for all of them. Two files get new GNU-mode warnings: `m_menu.c` (6) and `p_saveg.c` (2).

**Compile time.** `sqlite3.c`'s frontend went from 1.34 s to 1.41 s (`perf stat`, 8 runs each). All of the difference is in sema. The cause is the first warning in a file: when sema reports its first diagnostic, it builds a lookup table from source positions to file names once (`Sema::locs`, unchanged code). `sqlite3.c` had no warnings before this branch and now has 24, so it pays for that table, about 0.1 s. Any GNU-mode warning would cost the same. The new checks themselves do not cost measurable time. A 20,000-element designated initializer takes the same time on both compilers.

## 5. Local checks

- `cargo fmt --all -- --check` clean
- `cargo clippy --all-targets -- -D warnings` clean
- `cargo test --lib`: 209 passed (198 on `0cab101`)
- `make` and `python3 harness/run_tests.py`: 187/187, including the new tests 147–151, `gnu/15` and `xabi/05`

CI runs WACC, torture, Csmith, DOOM, Lua, SQLite and One True AWK.

## 6. Fixes from the review of this pull request

CodeRabbit and CI on this pull request found these. Each is fixed with a test.

| Finding | Fix | Test |
|---|---|---|
| CI (arm64 under qemu): `tests/147` printed 254 for a `signed char` bit-field holding -2 | `irgen/access.rs` `container_type` used plain `char` for a signed 8-bit storage unit, and plain `char` is unsigned on AArch64 Linux, so every read of a `signed char` bit-field zero-extended. This was already on `main`; the new compound-assignment path made it visible. The unit is now `signed char` | `tests/150_signed_char_bitfield.c` |
| A static array of bit-field structs past 512 MiB: the bit position saturated at `u32::MAX` and the bit landed in an earlier element | `BitStore::abs_bit` is now `u64`, built with checked arithmetic ("object is too large to initialize" on overflow), and `ByteImage::pack_bits` works in `u64` | sema unit test `bit_positions_past_two_to_the_32_stay_in_place` |
| `_Bool b = (_Bool)&g;` was rejected as "truncates an address" | An explicit `(_Bool)` cast of an address is its truth value (6.3.1.2), as the implicit conversion already was; a weak symbol still is not folded | sema unit tests, `tests/151` |
| `1 ? (int *)p : (void *)0` had type `void *` | 6.5.15p6: a null pointer constant takes the other operand's type, checked before `void *` merging, in sema (already wrong on `main`) and in the parser's typing for `_Generic` / `_Static_assert` / `typeof` | parser unit test, `tests/151` |
| The parser's typing of `?:` accepted `void` with a non-`void` operand, and two different record types | 6.5.15p3: void only with void, and a record only with a compatible record, as sema already required | parser unit test |

Also from the review: `tests/xabi/05_va_aligned_fp_def.c` no longer includes `<stdarg.h>`; it uses `__builtin_va_list` and the `__builtin_va_*` builtins. The README wording about overridden initializers and the IR comparison was corrected.
