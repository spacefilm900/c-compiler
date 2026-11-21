# STATUS

The program is `hcc`, the Haviland C Compiler. `make` writes `build/hcc`.

## Current milestone

**Flexible array members, `u`/`U`/`u8` literals, `long double` aggregate returns, and nested bit-field initializers, on top of variadic definitions, `goto`, designated initializers, bit-fields, and `volatile`.** A function may be defined with `...`. `<stdarg.h>` implements `va_list`, `va_start`, `va_arg`, `va_end`, and `va_copy` with `__builtin_va_list` and the matching builtins. `va_arg` follows the System V AMD64 register-save area: general registers at offsets 0..48, XMM registers at 48..176, then the overflow area. `float` promotes to `double`. An aggregate uses the same classification as a call. `llvm.va_start`, `llvm.va_end`, and `llvm.va_copy` are emitted; LLVM's `va_arg` instruction is not. `goto` and labels are function scope, including a forward jump. Designated initializers (`.field` and `[const]`) are accepted, and the next positional initializer continues at the next subobject. Bit-fields pack least-significant bit first in a storage unit of the declared type. `volatile` is `load volatile` and `store volatile`.

`float` and `double` stay as on `main`. Decimal and hexadecimal floating constants are lexed, including an `f`/`F` suffix. The usual arithmetic conversions, integer/float conversions, and the `float` to `double` default argument promotion are applied, so `printf("%f", x)` passes a `double`. Arithmetic, comparisons, unary `+`/`-`, `!`, compound assignment, `++`/`--`, `?:`, casts, and the conditions of `if` / `while` / `for` / `&&` / `||` accept floating operands. `switch` still requires an integer. File-scope and `static` objects may have constant floating initializers, and floating members are allowed in structs, unions, and arrays. Scalar `float` and `double` parameters and returns use XMM registers. An SSE eightbyte of an aggregate is `float`, `double`, or `<2 x float>`. INTEGER still wins over SSE in the same eightbyte. A lone integer eightbyte stays an integer when general registers run out. An SSE eightbyte, or a mixed aggregate that is short a register of either class, is `byval` and does not consume the registers that remain. `long double` is the 80-bit x87 format, stored in 16 bytes. A scalar parameter is passed in memory and a scalar return uses the x87 stack. `va_arg` of `long double` reads the overflow area, 16-byte aligned. An x87 aggregate is passed in memory. A return whose eightbytes are `X87` then `X87UP` (a struct or union that is one `long double`) is `x86_fp80` in `%st0`. Any other aggregate that contains `long double` is memory (`sret`), which is what the classification produces once the size exceeds 16 bytes or `X87UP` is not preceded by `X87`. Anonymous struct and union members (an untagged type specifier with no declarator) promote their members into the enclosing type. `L'...'` and `L"..."` are `wchar_t` literals, and `wchar_t` is `int`. `u'...'` / `u"..."` are `char16_t` (the unsigned 16-bit type). `U'...'` / `U"..."` are `char32_t` (the unsigned 32-bit type). `u8"..."` is an array of `char`. `u8'...'` is rejected; C11 has no such character constant. An unprefixed string adopts the other literal's prefix; two different non-ordinary prefixes are a diagnostic. A flexible array member is the last member of a struct that has another named member. `sizeof` excludes it. It is not initialized, and the struct is not an array element or a member of another struct. A union that contains one, recursively, has the same restriction. A file-scope initializer may nest a bit-field struct inside another struct or an array; the bit-field subobject is raw bytes and a sibling address stays a relocation. `__builtin_memcpy`, `__builtin_memmove`, `__builtin_memset`, `__builtin_signbit` / `__builtin_signbitf` / `__builtin_signbitl`, and the GCC floating-point format builtins `__builtin_nan` / `__builtin_nanf` / `__builtin_nanl`, `__builtin_nans` / `__builtin_nansf` / `__builtin_nansl`, `__builtin_inf` / `__builtin_inff` / `__builtin_infl`, and `__builtin_huge_val` / `__builtin_huge_valf` / `__builtin_huge_vall` are recognized. So are `__builtin_offsetof`, which `<stddef.h>` uses for `offsetof` and which folds to an integer constant expression, and the C11 7.12.14 comparison builtins `__builtin_isgreater`, `__builtin_isgreaterequal`, `__builtin_isless`, `__builtin_islessequal`, `__builtin_islessgreater`, and `__builtin_isunordered`. Programs can `#include` the host glibc headers for `stdio.h`, `stdlib.h`, `string.h`, `stdint.h`, `ctype.h`, `math.h`, `assert.h`, and `uchar.h`. The compiler supplies `stddef.h`, `stdarg.h`, `stdbool.h`, and `limits.h`. The Nora Sandler suite is fetched at a pinned commit and run by `harness/run_wacc.py`. It is not vendored.

## External suite

`main` at `0c30dc9` (`float` and `double`) has **80** unit tests, **110/110** harness programs, and **1354 executed, 1354 passed, 0 known failures, 263 skipped**. Chapter 13 is 69/69. This step: **81** unit tests, **116/116** harness programs (three cross-ABI pairs), and **1384 executed, 1384 passed, 0 known failures, 233 skipped**, 0 unexpected, 0 panics. Chapter 13 is still 69/69, including the three NaN extra-credit tests. The 30 `goto` tests now run and pass, including `switch_goto_mid_case.c`.

Licence: **MIT**, Copyright (c) 2023 Nora Sandler. The licence allows redistribution. The tree is still fetched, not committed. Source: https://github.com/nlsandler/writing-a-c-compiler-tests. Pinned commit: `ae12014d2dec14488f3f80d14df4b6d8e4634d7d` (2026-02-17). Recorded in `tests/external/README.md`. `scripts/fetch_wacc_tests.sh` clones that commit into `tests/external/wacc/` (gitignored). CI caches that directory.

`harness/run_wacc.py` does not call the suite's `test_compiler`. A valid program is compiled with `build/hcc`, linked with `clang`, and compared with `expected_results.json` (return code and stdout). If that entry is missing, `clang -std=c11` is the oracle. A `libraries/` test compiles one translation unit here and the other with clang, in both directions. Assembly helpers named in `test_properties.json` are the Linux `.s` file. Each command has a 10 second timeout. An invalid program must exit non-zero and must not crash. A compiler panic fails the run even if the test is listed in `harness/wacc_known_failures.txt`. A listed test that passes is reported so the line can be removed. An unlisted failure fails the run.

Chapters run: 1–18 (return through structs, including `long`, unsigned, pointers, arrays, character and string tests, `void` / `sizeof`, and floating point). Extra-credit bitwise, compound assignment, increment, and NaN tests are included.

Skipped, and not listed as failures:

| Skip | Tests | Why |
| --- | --- | --- |
| Chapter 19 | 167 | Part III. The runner inspects the book's own assembly for constant folding, copy propagation, dead stores, and unreachable code. It does not only run the program. |
| Chapter 20 | 66 | Part III register allocation. Same assembly inspection, including coalescing. Helper libraries are not tests. |

There is no separate function-pointer chapter in this pin. A function pointer is accepted; a test that uses one is compiled, not skipped. `chapter_18/valid/parameters/` and `params_and_returns/` are run. Integer aggregates there pass, including the library tests that link this compiler against Clang.

### Per-chapter results

Run locally with `python3 harness/run_wacc.py` after the fetch. 1384 executed, **1384 passed**, **0 known failures**, **233 skipped**, 0 unexpected, 0 compiler panics.

| Chapter | Run | Passed | Known failures | Skipped |
| --- | --- | --- | --- | --- |
| 1 | 24 | 24 | 0 | 0 |
| 2 | 19 | 19 | 0 | 0 |
| 3 | 35 | 35 | 0 | 0 |
| 4 | 43 | 43 | 0 | 0 |
| 5 | 82 | 82 | 0 | 0 |
| 6 | 68 | 68 | 0 | 0 |
| 7 | 27 | 27 | 0 | 0 |
| 8 | 98 | 98 | 0 | 0 |
| 9 | 78 | 78 | 0 | 0 |
| 10 | 72 | 72 | 0 | 0 |
| 11 | 55 | 55 | 0 | 0 |
| 12 | 38 | 38 | 0 | 0 |
| 13 | 69 | 69 | 0 | 0 |
| 14 | 78 | 78 | 0 | 0 |
| 15 | 106 | 106 | 0 | 0 |
| 16 | 96 | 96 | 0 | 0 |
| 17 | 88 | 88 | 0 | 0 |
| 18 | 308 | 308 | 0 | 0 |
| 19 | 0 | 0 | 0 | 167 |
| 20 | 0 | 0 | 0 | 66 |
| **Total** | **1384** | **1384** | **0** | **233** |

### Known-failure categories

| Category | Tests | Meaning |
| --- | --- | --- |
None. The previous `unsupported-double` list (91 tests) passes on `main` and still passes here. Chapter 13 is 69/69. This pin does not produce a `long double` outside a comment. The 30 `goto` tests pass, so they are not listed.

### Diagnostics fixed while running the suite

Both were accepted invalid programs. Neither is a new language feature.

- A dereference of a pointer to an incomplete struct, used as a value (`*p;` when `struct S` has no member list), is a diagnostic. `&*p` is still the address and does not load. `tests/91_addr_incomplete.c` returns 1, matching Clang: `p` is null and `&*p == p`.
- `extern struct S x = {1};` before `struct S` is defined is a diagnostic. The parser checks this. Semantic analysis would otherwise see a complete type, because a later definition of the same tag fills the shared struct body.

No test in the run panicked, and no executed program disagreed with `expected_results.json` except the known rejections above.

## Tests for the external suite

| Command | Toolchain | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | rustc **1.85.1** (4eb161250 2025-03-15) | clean |
| `cargo clippy --offline --all-targets -- -D warnings` | same | clean |
| `cargo test --offline --lib` | same | **81 passed, 0 failed** |
| `make` | GNU make **4.3** | `build/compiler` produced |
| `python3 harness/run_tests.py` | Ubuntu Clang **18.1.3** (`x86_64-pc-linux-gnu`), Python **3.12.3** | **118/118 passed** |
| `scripts/fetch_wacc_tests.sh` then `python3 harness/run_wacc.py` | same Clang and Python | **1384 passed, 0 known failures, 233 skipped, 0 unexpected** |

`tests/100_agg.c` covers register and memory aggregates, `f().x`, and `?:`. `tests/101_fnptr.c` covers decay, `(*fp)(...)`, arrays, struct members, a function typedef, and `qsort`. `tests/102_div.c` calls the host `div` and `ldiv`. `tests/103_float.c` covers floating arithmetic, comparisons, casts, compound assignment, `++`/`--`, globals, a `static` local, a struct member, a union, and `printf("%f", ...)`. `tests/104_libm.c` calls `sin`, `cos`, `sqrt`, `fma`, and `ldexp`. The harness links with `-lm`. `tests/xabi/01_agg_def.c` and `01_agg_use.c` are linked both ways against Clang. `tests/xabi/02_fp_def.c` and `02_fp_use.c` do the same for `float` and `double` arguments, returns, and mixed INTEGER/SSE structs, including a `byval` spill when the XMM registers are full. `tests/99_switch.c` covers fallthrough, nested switches, `break` and `continue`, Duff's device, a case inside `if`, a jump over an automatic initializer, enum and character constants, casts, `sizeof`, and every integer width from `char` through `unsigned long long`, plus `_Bool`. `tests/103_goto.c`, `tests/104_desig.c`, `tests/105_bits.c`, `tests/106_volatile.c`, and `tests/107_vararg.c` cover labels, designators, bit-fields, volatile loads and stores, and `<stdarg.h>`. `tests/108_float_h.c` and `tests/109_noreturn.c` cover `<float.h>` and `<stdnoreturn.h>`. `tests/xabi/02_vararg_def.c` and `02_vararg_use.c` are linked both ways against Clang. GitHub Actions for this step is not in this file yet. The switch step's Actions runs are in the switch record below.

## Stage 2, step 1 — built-in preprocessor

Done for that step.

The compiler is a Rust binary. `make` installs it at `build/compiler`. It accepts the Milestone 2 subset (LP64 integers, prototypes, globals, pointers, arrays, string literals, `const` and `volatile`, variadic calls, and simple structs) and now runs a built-in preprocessor: line splicing, comments, `#define` / `#undef` (object-like and function-like, including `__VA_ARGS__`, `#`, and `##`), `#if` / `#ifdef` / `#ifndef` / `#elif` / `#else` / `#endif`, `#include`, `#error`, `#line`, `#pragma once`, and the null directive. `-E` prints the preprocessed text. `-I`, `-D`, and `-U` are accepted. `tests/82_hello_hdr.c` is hello world through `#include <stdio.h>`. The compiler rejects other input with a diagnostic. It is not a full C compiler.

## Stage 2 plan

1. **Done.** Built-in preprocessor (this step). Real libc headers are not required yet; `include/` is a temporary stand-in. See below.
2. **Done.** `typedef`, `enum`, unions, and the declarations the host headers need. See the step 2 record below.
3. Basic standard headers are in use: the compiler supplies the freestanding ones, and glibc supplies the rest listed in the step 2 record.
4. **Done.** Passing and returning an integer aggregate by value. See the aggregate-call record below.
5. **Done.** `float`, `double`, and `long double`. See the floating-point record below.
6. **Done.** Variadic definitions and `<stdarg.h>`, `goto`, designated initializers, bit-fields, and `volatile`. See the step 6 record below.

## Milestone 2 plan

1. **Done.** Function prototypes and file-scope `int` globals (tentative definitions, `extern`, `static`, integer constant initializers, internal linkage).
2. **Done.** Remaining integer operators: bitwise, shifts, `++`/`--`, compound assignment, comma, ternary, `sizeof`.
3. **Done.** `char` / `short` / `long` / unsigned, integer promotions, conversions, and casts.
4. **Done.** Pointers and arrays.
5. **Done.** String literals and external calls such as `printf` / `puts` (hello world).
6. **Done.** Simple structs: tags, layout, member access, assignment, and brace initializers. Passing and returning an integer aggregate by value is Stage 2 step 4.

## Language

Rust, edition 2021, standard library only. No crates.io dependencies (`Cargo.lock` has none). `rust-toolchain.toml` pins **1.85.1** plus `rustfmt` and `clippy`.

## Tests for the preprocessor step

These are the results recorded for stage 2 step 1, before `tests/91_addr_incomplete.c`. GitHub Actions for that step is not in this file.

| Command | Toolchain | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | rustc **1.85.1** (4eb161250 2025-03-15) | clean |
| `cargo clippy --offline --all-targets -- -D warnings` | same | clean |
| `cargo test --offline --lib` | same | **60 passed, 0 failed** |
| `make` | GNU make **4.3** | `build/compiler` produced |
| `python3 harness/run_tests.py` | Ubuntu Clang **18.1.3** (`x86_64-pc-linux-gnu`), Python **3.12.3** | **90/90 passed** |

`cargo test` includes lexer regressions (string escapes included), preprocessor tests (splicing, object and function macros, `#` / `##`, `__VA_ARGS__`, conditionals, include guards, `#error`, and bad directives), parser AST dumps (including `int *a[3]` versus `int (*p)[3]`, a concatenated string, a variadic prototype, and `s.x` / `p->y`), negative diagnostics, a CLI check (`-E`, `-D`, `-I`), struct layout and member-lookup checks, IR checks (a constant return, globals, `declare` / `define internal`, the integer operators, pointer globals, `getelementptr`, a private string constant, `declare i32 @printf(ptr, ...)`, `call i32 (ptr, ...) @printf(...)`, `%struct.S = type { i8, i32 }`, member `getelementptr`, a struct load/store copy, assembled and executed), and a differential run of every `tests/*.c` file against `clang -std=c11`. The harness repeats that differential check through `build/compiler` (release binary) and compares exit status, stdout, and stderr. A harness program may `#include` a header. Clang uses the host headers; this compiler uses `include/` for `#include <...>`.

Harness programs: `01_return_const` (42), `02_add` (5), `03_loop` (55), `04_call` (42), `05_if` (2), `06_logic` (9), `07_while` (6), `08_cmp` (3), `09_arith` (17), `10_shadow` (4), `11_break` (25), `12_recursion` (16), `13_void` (6), `14_fallthrough` (0), `15_comment` (5), `16_decls` (12), `17_empty_params` (7), `18_forward_call` (42), `19_mutual` (2), `20_global_counter` (5), `21_static_global` (7), `22_tentative` (9), `23_shadow_global` (18), `24_const_init` (0), `25_extern_global` (8), `26_unnamed_params` (6), `27_multi_decl` (7), `28_void_forward` (9), `29_extern_def` (11), `30_bitwise` (0), `31_shifts` (0), `32_incdec` (0), `33_compound` (0), `34_ternary` (0), `35_comma` (0), `36_sizeof` (0), `37_const_ops` (0), `38_promotions` (0), `39_unsigned` (0), `40_casts` (0), `41_suffixes` (0), `42_chars` (0), `43_sizeof_types` (0), `44_globals` (0), `45_calls` (0), `46_trunc` (0), `47_const_types` (0), `48_swap` (0), `49_arith` (0), `50_sum` (0), `51_multi` (0), `52_pp` (0), `53_param` (0), `54_global` (0), `55_sizeof` (0), `56_cmp` (0), `57_sort` (0), `58_cast` (0), `59_cond` (0), `60_init` (0), `61_decay` (0), `62_hello` (0, prints `hello, world`), `63_printf` (0), `64_flags` (0), `65_arith` (0), `66_walk` (0), `67_strlen` (0), `68_reverse` (0), `69_format` (0), `70_io` (0), `71_strings` (0), `72_const` (0), `73_layout` (0), `74_nested` (0), `75_array` (0), `76_list` (0), `77_tree` (0), `78_copy` (0), `79_global` (0), `80_arith` (0), `81_scope` (0), `82_hello_hdr` (0, prints `hello, world` via `#include <stdio.h>`), `83_macros` (12), `84_paste` (0), `85_variadic` (10), `86_if` (0), `87_guard` (0), `88_multi` (42), `89_line` (0), `90_lib` (0, prints `4`).

`cargo test` also links two translation units both ways (this compiler's IR with a Clang object) for `char` / `unsigned char` / `short` / `unsigned short` parameters and returns. Both directions exit 0, matching Clang compiling the whole program.

Negative cases covered inside `cargo test` (each must be a non-zero diagnostic, not a successful compile): missing semicolon, missing `)`, function pointers, a file-scope variably modified type, a static or thread-local VLA, a VLA initializer, a VM struct member, a jump into the scope of a variably modified identifier, a non-positive array size, `sizeof(float)`, `sizeof(void)`, a `?:` missing `:`, old-style identifier lists (`int f(a, b);`), unnamed parameters on a definition, a non-constant `static` local initializer, an incomplete `static` local, a `static` or `extern` declaration in a `for` header, an initializer on a block-scope `extern`, a block-scope `static` function, a nested function, same-scope linkage conflicts (`extern` after a no-linkage declaration, and the reverse), conflicting types on a block-scope redeclaration, a block-scope `extern` that would give one identifier both internal and external linkage, a `do` / `while` condition that names a variable declared only in the body, function initializers, multiple storage-class specifiers, `void` variables, an incomplete local array, storage class on a parameter, `switch`, stray `else`, casts to `float`, a definition of a variadic function, `int f(...);` with no named parameter, wide string and character literals, `restrict`, a duplicate `const`, floats, invalid integer suffixes (`1Ll`, `1uu`), integer constants that fit in no target type, out-of-range hex and octal escapes, empty and multi-character character constants, unknown escapes, unterminated string literals, unterminated comments, a stray `#` that is not a directive, non-ASCII, `#error`, an unknown directive, `#else` / `#endif` without `#if`, an unterminated `#if`, a macro redefined with a different body, the wrong number of macro arguments, an include that cannot be found, and `#error` inside an included file (the diagnostic names that file), invalid type-specifier combinations (`short long`, `unsigned signed`, `long long long`, `int int`, `char int`), conflicting integer types (`char` vs `signed char`, `int` vs `unsigned`, `long` vs `long long`), incompatible pointer assignment (`int *` from `char *`, `char *` from `const char *`, `const char **` from `char **`), assignment to a const object (`const int`, `*p` for `const char *`, `char *const`, and an element of `const char[]`), a string that is too long for a character array, a string used to initialize an `int` array, arithmetic on `void *`, a subscript of a pointer to an incomplete array, a pointer compared with `0` using `<`, an array assigned to, the address of a non-lvalue, a non-constant pointer initializer, a global initializer that truncates an address (`int x = (int)&g` and `long x = (int)&g`), excess array initializers, undeclared names, redefinitions, conflicting function types (including a variadic declaration followed by a non-variadic one, and `const int` versus `int` as a return type), a variable redeclared as a function, conflicting linkage, non-constant, divide-by-zero, and overflowing global initializers, undefined shifts in a constant initializer (`1 << 32`, `1 << -1`, `-1 << 1`, `1 << 40`), `1 << 31` and `1L << 63` as overflowing constants, `++` / assignment / comma in an evaluated constant expression, a `?:` mixing `void` with an integer, `++` or compound assignment applied to a non-lvalue, bad call arity against a prototype (including too few arguments to a variadic function), a static function that is used but not defined, `break`/`continue` outside a loop, `void` returns, non-assignable expressions, calls of non-functions, and a call to a function that has not been declared yet. There is no implicit function declaration. Struct diagnostics covered the same way: passing or returning a struct by value (including an unnamed parameter `int f(struct S);`), an incomplete struct object, `sizeof` of an incomplete struct, member access on an incomplete struct, an unknown member, redefining a struct (including `struct S { struct S { int x; } in; }`), a duplicate member, a flexible array member, a bit-field, an anonymous struct, an empty struct, a designated initializer, assignment of a struct that has a const member, assignment to a member of a struct rvalue (`(s = t).x = 1`), `++` on a struct, two prototypes that each introduce their own struct tag (they are different types), and too many elements in a struct initializer.

GitHub Actions for this step is not copied here.

## Implemented

- [x] Lexer for identifiers, C11 keywords, comments, and integer constants
- [x] Recursive-descent parser and AST (C precedence, dangling `else`)
- [x] Semantic checks: scopes, types, calls, lvalues, loops
- [x] Textual LLVM IR (`alloca` / `load` / `store`, signed arithmetic, branches, calls)
- [x] Function prototypes, compatible redeclarations, and `declare` for functions with no body
- [x] File-scope `int` objects: tentative definitions, `extern`, `static`, constant initializers
- [x] Bitwise operators, shifts, `++`/`--`, compound assignment, `?:`, comma, and `sizeof`
- [x] LP64 integer types, promotions, usual arithmetic conversions, casts, and integer-constant suffixes
- [x] Pointers and arrays: declarators, arithmetic, decay, subscripts, brace initializers, and address constants
- [x] String literals, character-array initializers, and calls such as `printf` / `puts`
- [x] `const` and `volatile`, including assignment to const objects and `char *` passed to `const char *`
- [x] Variadic declarations and calls, with default argument promotions
- [x] Simple structs: tags and scope, System V layout, `.` / `->`, assignment, and brace initializers
- [x] Built-in preprocessor: splicing, macros, conditionals, `#include`, `#include_next`, `#error`, `#line`, `#pragma once`
- [x] `typedef`, including pointers, arrays, structs, and function types
- [x] Enumerations, with `int` as the compatible type
- [x] Unions: System V layout, member access, and initialization of the first member
- [x] `_Bool`, `restrict`, function pointers, and declarations of `float` / `double` / `long double`
- [x] Host glibc headers for the declarations used by the harness, plus compiler-supplied `stddef.h`, `stdarg.h`, `stdbool.h`, and `limits.h`
- [x] CLI `build/hcc INPUT.c -o OUTPUT.ll`, plus `-E`, `-I`, `-D`, and `-U`
- [x] Differential tests against Clang
- [x] Block-scope `static` objects, block-scope `extern`, block-scope function declarations, and `do` / `while`
- [x] Passing and returning structs and unions by value (System V AMD64 INTEGER and MEMORY classes)
- [x] Function pointers: decay, calls, arrays, members, typedefs, and `qsort`
- [x] `switch`, `case`, and `default`, including nested labels, fallthrough, and an LLVM `switch`
- [x] `float` and `double`: constants, conversions, operators, SSE parameters and returns, and mixed INTEGER/SSE aggregates
- [x] Compound literals, `_Generic`, `_Static_assert`, `_Alignof`, non-prototype `int f();` declarations, and `sizeof` in integer constant expressions
- [x] UTF-8 source characters and universal character names in literals

## Supported syntax

`void` and the LP64 integer types, in any legal specifier order: `char` (signed on this target), `signed char`, `unsigned char`, `short` / `signed short` / `short int` / `signed short int`, `unsigned short` / `unsigned short int`, `int` / `signed` / `signed int`, `unsigned` / `unsigned int`, `long` / `signed long` / `long int` / `signed long int`, `unsigned long` / `unsigned long int`, `long long` / `signed long long` / `long long int` / `signed long long int`, and `unsigned long long` / `unsigned long long int`. `__int128`, `signed __int128`, `unsigned __int128`, `__int128_t`, and `__uint128_t` are also accepted, in both language modes: they are 128-bit integers, size and alignment 16, lowered to LLVM `i128`. An invalid combination (`short long`, `unsigned signed`, `long long long`, `int int`, `char int`) is a diagnostic. `char`, `signed char`, and `unsigned char` are three distinct types. `long` and `long long` are both 64 bits and are still distinct types.

Those types may be used for locals, parameters, return types, prototypes, and file-scope objects, including `extern` and `static`. A later declaration must name the same type. Parameter names do not have to match. `int f(void);` and `int f() { ... }` are compatible: both have zero parameters. `int f();` is a non-prototype declaration; see Limitations. Calls check the argument count. An argument, a return value, an initializer, and the right-hand side of an assignment are converted to the destination type.

Function definitions and prototypes, including a function declaration inside a block (`int f(int);` there means `extern`); unnamed parameters on a prototype (`int f(int, int);`); file-scope objects; locals with optional initializers; block-scope `static` and `extern`; blocks; `if` / `else`; `while`; `do` / `while`; `for` (including `for (unsigned i = ...)`); `break`; `continue`; `return`; empty statements; unary `+` `-` `!` `~`; `+` `-` `*` `/` `%` `&` `|` `^` `<<` `>>`; comparisons; short-circuit `&&` and `||`; `?:`; assignment and compound assignment; prefix and postfix `++`/`--`; the comma operator; `sizeof` on an expression or on a type name such as `sizeof(unsigned long)`; explicit casts `(type)expr` and `(void)expr`; direct calls; decimal, octal, and hex constants with the suffixes below; character constants; `//` and `/* */` comments.

Sizes on `x86_64-pc-linux-gnu`: `char` / `signed char` / `unsigned char` are 1, `short` / `unsigned short` are 2, `int` / `unsigned int` are 4, and `long` / `unsigned long` / `long long` / `unsigned long long` are 8. The module triple is `x86_64-pc-linux-gnu`, and the module sets `target datalayout` to the System V AMD64 layout (`i64` aligned to 8 bytes). IR uses opaque pointers (LLVM 15+). Without that data layout, LLVM's default aligns `i64` to 4 bytes, so a later optimizer can fold the offset of a `long` after a `char` to 4. Automatic locals are allocated in the entry block. File-scope objects, and block-scope `static` objects, are LLVM globals (`global`, `internal global`, or `external global`) with natural alignment. A defined function is `define`; a prototype with no body and external linkage is `declare`. File-scope `static` is internal linkage. A block-scope `static` is also an `internal` global, with a name that is not the C identifier.

Integer promotions (C11 6.3.1.1) convert every type narrower than `int` to `int`, including `unsigned char` and `unsigned short`, because every value of those types fits in signed `int`. The usual arithmetic conversions (C11 6.3.1.8) then pick the common type. `int` with `unsigned int` is `unsigned int`, so `-1 < 1u` is false. `long` can represent every `unsigned int`, so `-1L < 1u` is true. `long long` and `unsigned long` have the same width and `long long` has greater rank, so `-1LL + 0UL` has type `unsigned long long`. Comparisons and `&&` / `||` / `!` produce `int` 0 or 1. `?:` uses the same conversions when both arms are integers, and both arms may be `void`.

Unsigned arithmetic wraps. Unsigned division, remainder, comparison, and right shift lower to `udiv`, `urem`, `icmp u*`, and `lshr`. Signed division uses `sdiv` / `srem` (toward zero). Signed `>>` is `ashr`. Bitwise `&` `|` `^` lower to `and` / `or` / `xor`. `~` is `xor` with `-1` of the promoted type. `<<` is `shl`. `?:` is a branch and a `phi`, and only the chosen arm is evaluated. Conversions are `sext`, `zext`, or `trunc`. A same-width signed/unsigned change needs no instruction.

Assignment, initialization, argument passing, and `return` convert to the destination type. Converting to an unsigned type reduces the value modulo 2^width. Converting to a signed type that cannot represent the value is implementation-defined; this target keeps the low bits and reads them as two's complement, which matches Clang here. `(char)128` is `-128`, `(char)1000` is `-24`, and `(int)2147483648L` is `-2147483648`. The conversion itself is not an overflow diagnostic. A signed operation whose mathematical result does not fit in the operation's type is still a diagnostic in a constant initializer (C11 6.6p4), including `INT_MIN / -1`, `INT_MIN % -1`, `1 << 31`, and `1L << 63`. `1u << 31` and `1ul << 63` are defined and wrap in the unsigned width.

An integer constant's type follows C11 6.4.4.1 for this LP64 target. An unsuffixed decimal constant is the first of `int`, `long`, `long long` that can hold it, so `2147483648` is `long` and `9223372036854775808` is too large. An unsuffixed octal or hex constant may also be unsigned: `0x80000000` is `unsigned int`, and `0x8000000000000000` is `unsigned long`. Suffixes are `u`/`U`, `l`/`L`, `ll`/`LL`, and the combinations `ul`/`lu` and `ull`/`llu` in either letter case for each half (`1uLL`, `1LLu`). `ll` and `LL` must be the same case, so `1Ll` is rejected. A value that fits in no type on the list is a diagnostic.

A character constant has type `int`. Simple escapes (`\' \" \? \\ \a \b \f \n \r \t \v`), octal escapes of up to three digits, and `\x` hex escapes are accepted when the escape fits in `unsigned char`. The value is that byte converted from `char` to `int`. Because `char` is signed, `'\xff'` and `'\377'` are `-1`, while `'\x41'` and `'\101'` are `65`. `sizeof('a')` is 4.

`sizeof` has type `unsigned long` (`size_t` on this target). `sizeof(char)` is 1, `sizeof(short)` is 2, `sizeof(int)` is 4, `sizeof(long)` and `sizeof(long long)` are 8, and `sizeof(sizeof(int))` is 8. The operand is not evaluated: `sizeof(h++)` does not increment `h`, and `sizeof(tick())` does not call `tick`. `sizeof(void)`, and `sizeof` applied to a `void` expression, are diagnostics because `void` is incomplete. Because the result is unsigned, `sizeof(int) - 5` wraps.

The System V AMD64 ABI requires a `char` or `short` parameter or return value to be extended to 32 bits by the caller or the callee. Signed types (`char`, `signed char`, `short`) are `signext` in the LLVM IR. Unsigned types (`unsigned char`, `unsigned short`) are `zeroext`. The attribute is written on the `define`, on the `declare`, and on the call. `int` and wider integers have no extension attribute. A unit test links this compiler's IR with a Clang object in both directions for those four narrow types.

A shift count that is negative or greater than or equal to the width of the promoted left operand is undefined. A signed left shift of a negative value is undefined. A signed left shift whose mathematical result does not fit is undefined. Constant initializers reject those cases. Runtime shifts are not diagnosed. The IR does not mask the count, so an out-of-range count is LLVM poison. An in-range left shift of a negative value is still emitted as `shl` and is not trapped. Tests stay in the defined range, except for an unevaluated shift such as `1 || (1 << 40)`, which folds to 1. Unsigned left shifts wrap. Signed right shifts are arithmetic (`-8 >> 1` is `-4`). Unsigned right shifts are logical.

Prefix and postfix `++`/`--` apply to lvalues: locals, parameters, and globals of any integer type. They follow `+= 1` and `-= 1`: the operand is promoted, 1 is an `int`, the usual arithmetic conversions apply, and the result is converted back to the object type. `unsigned char` value 255 incremented as an expression is the `int` 256; stored back, it is 0. Prefix yields the new value; postfix yields the old value. The IR loads the scalar once, after the right-hand side of a compound assignment, then stores. A program that modifies the same scalar twice with no sequence point in between is still undefined, and the tests do not do that. Signed overflow of the promoted operation is undefined.

Compound assignment (`+=` `-=` `*=` `/=` `%=` `&=` `|=` `^=` `<<=` `>>=`) evaluates the lvalue address once, evaluates the right-hand side, then loads, applies the operator, converts back, and stores. `x += x += 1` modifies the same scalar twice in one expression. C11 6.5.16p3 sequences each assignment's store after that assignment's own operand value computations, and it does not sequence the inner store before the outer load. That expression is undefined (C11 6.5p2). The tests do not use it.

The comma operator yields the right operand and has a sequence point between the operands. It is not an integer constant expression when it is evaluated. Clang accepts `int g = (1, 2);` as an extension; this compiler rejects it. A comma in an unevaluated operand, such as `sizeof(1, x)`, is allowed. Commas that separate arguments or declarators are not this operator.

File-scope `name;` and `static name;` are tentative definitions. If the translation unit never gives that object an initializer, it is emitted as zero of its type. A later initializer replaces the tentative value. `extern` without an initializer does not define the object; `extern name = expr;` does. The initializer must be an integer constant expression or a floating constant expression (C11 6.6) and is converted to the object's type, including truncation. The operators above are allowed, including casts, `?:`, `sizeof`, `~`, bitwise operators, and shifts. Assignment, `++`/`--`, function calls, and the comma operator are rejected when they are evaluated. An unevaluated operand may contain them. An evaluated division or remainder by zero is a diagnostic. `-2147483647 - 1` is the minimum `int`. `-9223372036854775807L - 1` is the minimum `long`.

A file-scope function with no storage class is treated as `extern`, so `static int f(void); int f(void) { ... }` keeps internal linkage. A file-scope object with no storage class has external linkage and does not inherit a prior `static`. The same identifier with both internal and external linkage is a diagnostic. Two bodies, or two initialized definitions, are a redefinition. A prototype parameter name is not visible outside that prototype. A local shadows a global of the same name. A global is in scope for its own initializer, but an identifier is not a constant, so `int g = g;` is rejected.

A block-scope `static` object has no linkage (C11 6.2.2p6) and static storage duration (C11 6.2.4). Each declaration is a different object, including two blocks in one function that both say `static int x`. The LLVM name is `function.x.N`, with `internal` linkage, so a file-scope `x` and another function's `static int x` do not clash at link time. No initializer means zero. An initializer is a constant expression, the same rules as a file-scope initializer, and it is the global's initializer: the function does not store it on entry. `static int a[] = {1, 2}` takes its length from the list. `static int a[]` with no size is a diagnostic, because an object with no linkage must have a complete type. `for (static int i = 0; ...)` is rejected; Clang rejects it as well.

A block-scope `extern` object, and a block-scope function declaration with no storage class or with `extern`, follow C11 6.2.2p4 and p5. The visible prior declaration supplies the linkage when that prior has internal or external linkage. A prior with no linkage, or no prior, means external linkage, and that object is not the no-linkage one it hides. `int a = 3; { extern int a; }` reads the file-scope `a` when that file-scope object has external linkage. The declaration is in scope only until the end of the block; a use after the block is undeclared unless a file-scope declaration is visible. The same scope cannot declare the name both with and without linkage (`static int x; extern int x;` and `extern int x; int x;` are diagnostics). An initializer on a block-scope declaration that has linkage is a diagnostic (C11 6.7.9p5). `static int f(void);` in a block is a diagnostic (C11 6.7.1p7). A nested function body is still rejected. A block declaration that is the only declaration of an external object or function is emitted as `external global` or `declare`, under the C name, so another translation unit can define it. When a no-linkage declaration hides a file-scope `static` and an inner `extern` would therefore have external linkage, that is both linkages in one translation unit (C11 6.2.2p7) and is a diagnostic. Clang accepts that program and binds the `extern` to the hidden `static`.

`do` statement `while (` expression `);` executes the statement, then the expression, and repeats while the expression is non-zero. The expression is not in the scope of a compound statement that is the body. `break` leaves the loop without evaluating the condition. `continue` jumps to the condition. A scalar condition is required, including a pointer. `do int x = 1; while (0);` is rejected because a declaration is not a statement.

`switch (` expression `)` statement follows C11 6.8.4.2. The controlling expression has integer type. The integer promotions are applied, and each `case` constant is an integer constant expression converted to that promoted type (C11 6.8.4.2, 6.6). Enumeration constants, character constants, casts, and `sizeof` are accepted there. A value that matches another case after that conversion, a second `default`, a `case` or `default` outside a switch, a non-integer controlling expression, and a case expression that is not an integer constant are diagnostics. `break` leaves the innermost switch or loop. `continue` does not see the switch: it jumps to the enclosing loop, and it is a diagnostic when there is no loop. A `case` or `default` may sit inside another statement in the switch body (`if`, `while`, `do`, `for`, or a nested block). A nested switch owns the labels inside it. Execution jumps to the matching label, so an automatic initializer that occurs earlier in the block does not run. That jump is not diagnosed. A label followed by a declaration (`case 1: int x = 0;`) is a diagnostic, because C11 requires a statement after the colon. The IR is an LLVM `switch` on the promoted width: `i32` when the promoted type is `int` or `unsigned int`, and `i64` for `long` and `long long`, signed or unsigned. A `char` or `short` condition is extended to `i32` before the instruction.

`int main(void)` and `int main()` are both no-parameter definitions. Reaching the closing brace of an integer function lowers to `ret` of 0 of that type. That is what C requires for `main`. A pointer-returning function that falls off the end lowers to `ret ptr null`. A `void` function that falls off the end lowers to `ret void`. The OS truncates `main`'s return value to 8 bits, so the tests keep exit codes in 0..255 and print nothing.

Pointers and arrays use the same declarator grammar as C. `*` binds less tightly than `[]` and `()`, so `int *a[3]` is an array of three `int *` and `int (*p)[3]` is a pointer to an array of three `int`. `void *` is an object pointer. `0` and `(void *)0` are null pointer constants; `(int *)0` is a null pointer of type `int *` but not a null pointer constant. Unary `&` requires an lvalue, including an array. Unary `*` requires a pointer. `&*p` and `*&x` are the address of the object; the IR does not load through the cancelled `*`. An array expression decays to a pointer to its first element except as the operand of `sizeof` or unary `&`. `a[i]` and `i[a]` are the same lvalue. A parameter declared as an array is adjusted to a pointer, so `sizeof` on `int a[10]` in a parameter list is 8. A parameter of function type is adjusted to a pointer to that function. A function designator decays to a pointer, and a call through that pointer is an indirect call.

Pointer plus or minus an integer scales by the size of the pointed-to type (`getelementptr inbounds`). The integer is sign- or zero-extended to i64. Pointer minus pointer, for compatible complete object types, has type `long` (`ptrdiff_t`) and is `ptrtoint`, `sub`, and `sdiv exact` by the element size (`char` has no division). `++`/`--` and `+=`/`-=` on a pointer add or subtract one element. Other compound assignments on a pointer are rejected. Arithmetic on `void *` is rejected. Clang warns and treats the pointee size as 1 (`-Wpointer-arith`); this compiler does not accept it. Relational `<` `<=` `>` `>=` require compatible object pointers and lower to `icmp u*`. Equality also allows `void *` with an object pointer, and a pointer with a null pointer constant. `p < 0` is rejected. Clang warns for distinct pointer types and for a pointer compared with an integer; those are errors here. A pointer is true in `if`, `!`, `&&`, `||`, and the condition of `?:` when it is not null.

`?:` follows C11 6.5.15. Both arms arithmetic: the usual arithmetic conversions. Both `void`: `void`. Both pointers to compatible types: the composite type. One arm a pointer and the other a null pointer constant: the pointer type. One arm an object pointer and the other `void *`: `void *`. Other mixes are errors.

An integer converts to a pointer by extending it to i64 (`sext` if signed, `zext` if unsigned) and `inttoptr`. A constant 0 becomes `null`. A pointer converts to an integer by `ptrtoint` directly to that integer's width. Pointer-to-pointer is a no-op in the IR (opaque `ptr`). This matches Clang on `x86_64-pc-linux-gnu`: `(int *)(int)-1` is all-ones, and `(int *)(unsigned)-1` is the low 32 bits zero-extended. Assignment allows an arithmetic conversion, a compatible pointer, `void *` to or from an object pointer, or a null pointer constant. `int *p; char *c; p = c;` is an error. Clang only warns (`-Wincompatible-pointer-types`) and still compiles.

Arrays may be locals or globals, with any supported element type, including pointers and further arrays. `int a[] = {1, 2, 3}` and `int m[][3] = {{1}, {4, 5, 6}}` take their outer length from the initializer. A brace list may be partial; the rest is zero. A nested brace initializes one subaggregate. A flat list fills subaggregates in order (`int m[2][3] = {1, 2, 3, 4}`). One pair of braces around a scalar is allowed (`int x = {1}`). An empty list and an extra initializer are errors. A local aggregate is stored as `zeroinitializer` and then the explicit elements are stored. A global or `static` object with no initializer is zero, including a null pointer. An incomplete array that is defined and never completed (`int a[];` with no later size) is an error. `extern int a[];` is an external `[0 x i32]`, which is what Clang emits.

A global initializer may be an integer constant expression, a floating constant expression, or an address constant: `&g`, `&arr[2]`, `arr + 1`, a null pointer constant, or a cast of those. The IR uses `getelementptr (i8, ptr @obj, i64 byte_offset)` and `ptrtoint` / `inttoptr` when the destination is an integer or the pointer was converted from an integer. A cast of an address through a 64-bit integer and back (`(int *)(unsigned long)&g`) is the same address. Truncating an address to 32 bits in a global initializer is rejected; the bits are not known until link time.

An array of 16 bytes or more has alignment at least 16, for locals and globals. That is the System V AMD64 rule. Smaller arrays use the element alignment. Pointers are 8-byte aligned. String-literal globals use alignment 1.

A string literal uses the same escapes as a character constant. Adjacent literals are concatenated (`"hel" "lo"`). The type is `char[N]`, where `N` counts the terminating NUL, so `sizeof "abc"` is 4 and `sizeof ""` is 1. The literal decays to `char *` except as the operand of `sizeof` or unary `&`. When the address is used, the IR is a `private unnamed_addr constant` array of `i8`, named `.str` or `.str.N`. Identical contents of the same encoding share one global. A literal used only by `sizeof`, or only to initialize a character array, does not allocate that global. Modifying a string literal is undefined and is not diagnosed; the type is `char[N]`, not `const char[N]`. `L` literals are `wchar_t` (`int`). `u` literals are `char16_t` (unsigned 16-bit). `U` literals are `char32_t` (unsigned 32-bit). `u8` strings are arrays of `char`. An unprefixed literal adopts the other prefix. Mixing `u`, `U`, `u8`, and `L` with each other is a diagnostic. `u8'...'` is rejected.

A character array (`char`, `signed char`, or `unsigned char`) may be initialized by a string, optionally in one pair of braces. `char s[] = "hi"` has length 3, including NUL. `char s[10] = "hi"` stores `'h'`, `'i'`, NUL, and zeros. `char s[2] = "hi"` stores `'h'` and `'i'` and no NUL (C11 6.7.9). A longer literal is a diagnostic. `char m[][3] = {"hi", "yo"}` takes its outer length from the list. `char *p = "hi"` and `const char *p = "hi"` are address constants. `char *p = "hi" + 1` is the same global plus one byte.

`const` and `volatile` are accepted in declaration specifiers and after `*` in a declarator. The same qualifier twice in one list is a diagnostic. Qualifiers on an array apply to the element type. What is enforced:

- Assignment, `++`/`--`, and compound assignment to a const-qualified type are diagnostics. That includes a `const int`, `*p` when `p` is `const char *`, `p` when `p` is `char *const`, and an element of `const char s[]`. Initialization is still allowed.
- `char *` may be assigned or passed to `const char *`. `const char *` may not be passed to `char *`. `char **` may not be passed to `const char **`. The same rule applies to `volatile`, and to `void *`: the destination pointee must have every qualifier of the source pointee.
- Top-level `const` on a parameter is not part of the function type, so `int f(const int)` matches `int f(int)` and `int f(int *const)` matches `int f(int *)`. `const` on a pointee is part of the type, so `const int *` does not match `int *`. A `const` return type is part of the type: `const int f(void)` does not match `int f(void)`.
- A const object is emitted as an LLVM `constant`. An array of const elements is too. `const int *` is a normal `global`, because the pointer itself can be reseated.
- Pointer comparisons ignore one level of pointee qualifiers, so `const int *` may be compared with `int *`.

A `volatile` object or member is a `load volatile` or `store volatile`. A volatile aggregate copy passes `i1 true` to `llvm.memmove`. This compiler does not otherwise optimize. A cast may drop `const` or `volatile`; the cast is accepted and a later write is not diagnosed. `restrict` is accepted on pointers and does not change the IR; see the stage 2 step 2 record. Duplicate qualifiers are errors here; Clang only warns.

A variadic declaration such as `int printf(const char *, ...);` is kept. The IR is `declare i32 @printf(ptr, ...)`, with `signext` or `zeroext` on a narrow fixed parameter or return, the same as any other function. A call is `call i32 (ptr, ...) @printf(...)`. Fixed arguments are converted to the parameter type. Further arguments get the default argument promotions: `char` and `short`, signed or unsigned, become `int`, and `float` becomes `double`. Pointers are not promoted. `long double` is not promoted. `int f(...);` is rejected because C11 requires a named parameter before `...`. A definition may use `...`. `<stdarg.h>` implements `va_start`, `va_arg`, `va_end`, and `va_copy` from the System V register-save area. See the step 6 record. A call still requires a visible declaration. C11 has no implicit declaration, and this compiler does not add one.

Hello world is `tests/62_hello.c`:

```c
int printf(const char *, ...);

int main(void) {
    printf("hello, world\n");
    return 0;
}
```

It exits 0 and prints `hello, world` followed by a newline. The harness compares that with Clang.

A struct may have a tag, or none (`struct { int x; } s` and `typedef struct { int x; } S`). A tagged struct may appear at file scope or in a block, and the tag is in its own namespace, separate from ordinary identifiers, so `struct s s;` is one type and one object. A definition in the current scope completes an incomplete tag there or creates a new type that hides an outer tag of the same name. `struct S;` in a block hides an outer `struct S` with a new incomplete type. A tag declared in a parameter list is not visible after that declarator: a prototype's tags disappear, and a definition's tags are visible in the function body only. Two prototypes `void f(struct A *p); void f(struct A *p);` therefore name different types and conflict, unless `struct A` was already declared in the enclosing scope.

Members may be any type this subset already supports: integers, pointers, arrays, and structs. A struct may be nested by naming one that is already complete, or by defining it in the member list (`struct Outer { struct Inner { int x; } in; }`). The inner tag is declared in the same scope as the outer tag. A member that is an incomplete struct or a function is a diagnostic. The last member of a struct that already has another named member may be a flexible array (`int a[]`). `sizeof` the struct excludes that array. The struct is not an array element and not a member of another struct. A union that contains such a struct, recursively, has the same restriction. Initializing the flexible member is a diagnostic. An incomplete array in any other position, including in a union, is still a diagnostic. Bit-fields (`int x : 3`) are packed least-significant bit first. See the step 6 record. An untagged struct is a distinct type. An empty member list is a diagnostic; C11's grammar requires a member, and an empty struct is a GNU extension.

`struct node;` declares an incomplete type. `struct node *` is complete, including a pointer to the struct being defined, so a linked list or tree can point at its own node type. Declaring an object of the incomplete type, or applying `sizeof` to it, is a diagnostic. `extern struct S x;` is the exception: the object may be completed in another translation unit. Completing the tag later in the same scope completes every earlier use of that tag.

Layout is the System V AMD64 rule. Each member starts at the next offset that satisfies its alignment. The struct's alignment is the strictest member alignment, at least 1, and its size is rounded up to a multiple of that alignment. An array member is aligned as its element is. A local or global array variable of 16 bytes or more still has alignment at least 16; a member does not. `struct A { char c; int i; }` is 8 bytes with `i` at offset 4. `struct B { char c; long a[2]; }` is 24 bytes with `a` at offset 8. The LLVM type is a named struct, `%struct.Tag` or `%struct.Tag.N` when the same tag is defined again in another scope, and member access is `getelementptr` with the member index. A whole-struct assignment is `llvm.memmove` of the object's size, so the copy includes padding and a self-assignment (`s = s`) is defined. An aggregate load and store would not keep the padding: LLVM may treat those bytes as `undef`.

`.` requires a struct. `->` requires a pointer to a struct. The member must be named. Qualifiers on the struct object apply to the member, so `const struct S *p; p->x` has a const type when `x` is `int`, and assigning to it is a diagnostic. A struct with a const member is not a modifiable lvalue. `++` and compound assignment still require a scalar. Unknown members are diagnosed.

A struct may be a local, a file-scope object (`extern` or `static` included), or an element of an array. A pointer to a complete struct supports the same arithmetic as any other object pointer, scaled by `sizeof` the struct. Brace initializers work for locals and globals. Missing members are zero. Nested braces initialize one member; a flat list fills inner structs and arrays in order. One expression of that struct type initializes a member as a whole, as in `struct Line line = {p, q}`. A global initializer must still be constant: integer constant expressions, floating constant expressions, address constants, and brace lists of those. Copying another global struct into a file-scope initializer is rejected, because that object is not a constant. Designated initializers (`.field` and `[const]`) are accepted. A following positional initializer continues at the next subobject. A complete struct or union may be passed or returned by value. The System V AMD64 classification is in `src/abi.rs`: at most two INTEGER eightbytes travel in registers, coerced to the integer (or `ptr`) width of the bytes those fields occupy; anything larger, or a field that is not naturally aligned, is memory (`byval` / `sret`). A call result is a temporary, so `s = f()`, `f().x`, and `c ? a : b` for the same struct type all copy those bytes with `llvm.memmove`. A prototype may name an incomplete struct or union; a definition and a call require a complete type. An SSE eightbyte is passed in an XMM register (`float`, `double`, or `<2 x float>`). A return whose eightbytes are `X87` then `X87UP` is `x86_fp80`. Other aggregates that contain `long double` are `sret`. A pointer parameter or a pointer return is unchanged.

## Limitations

- The preprocessor implements translation phases 2–4 for the directives listed below. Trigraphs are not recognized. `_Pragma` and `__DATE__` / `__TIME__` are not implemented. `#include_next` is. A `#` that survives expansion is a stray-token diagnostic. Digraphs (`<:`, `:>`, `<%`, `%>`, `%:`, `%:%:`) are recognized as the corresponding punctuators.
- An integer constant that fits in no type on the C11 6.4.4.1 list for its suffix and base is a diagnostic. A decimal constant that does not fit in signed `long long` is rejected even when it fits in `unsigned long long`.
- An untagged unnamed struct or union member is anonymous. A tagged member with no declarator is still rejected. `long double` is computed as 80-bit extended precision. No variable-length arrays. `L`, `u`, `U`, and `u8` string literals are accepted, as are `L`, `u`, and `U` character constants. `u8` character constants are rejected. A wide or Unicode character constant in a preprocessor expression has the value and type it has in program text (`L'\400'` is 256). A UTF-8 source character or a universal character name in a literal is encoded for the prefix; a character constant that would need more than one code unit is a diagnostic. Source files must be UTF-8: a raw byte that is not UTF-8 (GCC torture `20000227-1.c` has `0xFF` in a character constant) is rejected, and non-ASCII identifiers are rejected. An empty struct or union is rejected. A flexible array member that is not last, that is the only named member, that is initialized, or that makes the struct an array element or a struct member, is a diagnostic. A variadic definition with no named parameter (`int f(...)`) is rejected. `__builtin_alloca` and the other GCC builtins are not recognized, apart from the `va_*` builtins, the memory builtins, `__builtin_signbit`, and the floating-point format builtins (`__builtin_nan*`, `__builtin_nans*`, `__builtin_inf*`, `__builtin_huge_val*`).
- Multi-character character constants (`'ab'`) are rejected; Clang accepts them as an implementation-defined `int`. A hex or octal escape outside `unsigned char` is a diagnostic, matching Clang. Runtime shift counts outside `0 .. width`, signed left shifts of negative values, and signed left shifts that do not fit are undefined and are not diagnosed outside constant initializers. Signed overflow at runtime is undefined and is not diagnosed. Modifying a string literal is undefined and is not diagnosed. A cast may drop `const`.
- `goto` jumps to a label in the same function. Jumping into the scope of an automatic initializer is not diagnosed, the same as `switch`. A label followed by a declaration (`label: int x;`) is still a diagnostic. `?:` accepts two operands of the same struct or union type. `switch` is implemented, as described above. A `case` range (`case 1 ... 3`) is a GNU extension and is rejected in strict C11. Computed `goto *expr` and `&&label` are GNU extensions: they are rejected in strict C11, and accepted with `-std=gnu11` (see Lua below).
- `int f();` declares a function without a prototype (C11 6.7.6.3p14). A call converts each argument with the default argument promotions and is lowered as a call through `(...)` on x86_64, and as a fixed-argument call on the arm64 targets. A later prototype is compatible only when no parameter type changes under those promotions and there is no `...` (C11 6.7.6.3p15); a definition `int f() { ... }` has no parameters and conflicts with a later or earlier prototype that has some. Converting such a function to a pointer to a prototype is accepted, as Clang does, although 6.7.6.3p15 makes a parameter-count mismatch incompatible. An identifier list (`int f(a, b);`) and K&R definitions are rejected.
- A `for` header cannot declare a function or use `static` or `extern`. A block-scope function cannot be `static`, and a nested function definition is rejected. An initializer is rejected on a block-scope declaration that has linkage.
- A `static` function that is called but never defined is a diagnostic. An external function or object that is declared and not defined is emitted as `declare` or `external global`, so another translation unit can provide it.
- A tentative definition is completed inside this translation unit as a zero initializer of the object's type, which is the C11 6.9.2 rule and matches Clang's default (`-fno-common`). It is not a common symbol.
- Uninitialized locals are not zeroed. Reading them is undefined in C; the IR loads the alloca anyway.
- The compiler reports the first error only and does not write the `.ll` file.
- Columns count every byte as one column, including tabs.
- No optimizations are performed here. Clang can optimize and codegen the IR.

## CI and CodeRabbit

- `.github/workflows/ci.yml` runs on `push` and `pull_request` with workflow `contents: read`, cancel-in-progress for the same ref, and the runner `ubuntu-24.04`. The original job is unchanged: 20-minute timeout, Rust 1.85.1 (`rustfmt`, `clippy`), apt packages `clang`, `llvm`, `make`, and `python3`, Clang 18 or newer, then `cargo fmt --all -- --check`, `cargo clippy --offline --all-targets -- -D warnings`, `cargo test --offline --lib`, `make`, and `python3 harness/run_tests.py`.
- A second job, `writing-a-c-compiler suite`, fetches the pinned suite and runs `python3 harness/run_wacc.py`. It has a 30-minute timeout and inherits the workflow token, which is `contents: read`. Saving the suite cache does not need `actions: write`. For `push` and `pull_request`, the runner's cache mode is `write`. The cache key is the pinned commit.
- Stage 3 adds four jobs: `csmith differential` (20 minutes), `one true awk` (75 minutes), `gcc torture execute` (40 minutes), and `performance numbers` (25 minutes). They use the same runner and the same toolchain pin. The performance job's token is `contents: read` and `actions: write` so it can upload `reports/perf.json`. The other jobs stay at the workflow's `contents: read`. Fetched trees are cached by pin. The One True AWK job skips the private AWK Builder suites, naming the missing `AWK_SUITE_READ_TOKEN` secret, when that secret is unset. When the secret is set, it is passed as `GH_TOKEN`, and a clone, build, or suite failure fails the job.
- Pinned actions: `actions/checkout@11d5960a326750d5838078e36cf38b85af677262` (v4.4.0), `dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8` (v1) with toolchain `1.85.1`, `actions/cache@0400d5f644dc74513175e3cd8d07132dd4860809` (v4.2.4), and `actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02` (v4.6.2) on the performance job.
- `.coderabbit.yaml` follows CodeRabbit schema `https://coderabbit.ai/integrations/schema.v2.json`. The review profile is `CHILL`. The file does not install the GitHub App.
- Milestone 1's Actions runs and the CodeRabbit review on pull request 1 are in the record below. Step 2's Actions runs and the CodeRabbit review are in the step 2 record below. Step 3's Actions runs and the CodeRabbit review are in the step 3 record below. Step 4's Actions runs and the CodeRabbit review are in the step 4 record below. Step 5's Actions runs were not copied into that step's record. This change's Actions runs and the CodeRabbit review are in the switch record below.

## Stage 3 testing

Csmith, One True AWK, the AWK Builder suites, `gcc.c-torture/execute`, and a performance script. Pins and commands are in `tests/external/README.md`. Nothing here is a new platform. Compiler sources of GCC and Clang were not read. The torture checkout is only `gcc.c-torture/execute`.

### Csmith

Pin `30dccd73b78652c4719f36572994778a5b233a4e` (tag `csmith-2.3.0`). Every seed uses the options in `harness/run_csmith.py`: no packed structs (`#pragma pack` is ignored), `main(void)`, and size caps so a program finishes in seconds. The program is compiled with this compiler at `-std=gnu11` and with `clang -std=gnu11 -O0`, both binaries are linked with `-lm`, and then both run under an 8 second timeout, `RLIMIT_CPU`, and a 2 GiB address-space limit. Exit status and stdout are compared. The checksum line is the one Csmith's `main` prints.

CI runs the 40 seeds in `harness/csmith_seeds.txt`. The wall-clock limit is twice the CPU limit, so a program that burns CPU dies from `SIGXCPU` on both sides instead of one side winning a race with the parent's timeout. A local batch is `python3 harness/run_csmith.py --count 500`. It is not part of CI. A larger coverage campaign, at `-O0` and `-O2`, is `tools/csmith_campaign.py`; see `tests/external/README.md`.

Local result on this machine, Ubuntu Clang 18.1.3, after nested bit-field initializers: **25 passed, 15 known failures, 0 unexpected**, out of 40 seeds. The 15 failures are stricter pointer constraints than Clang's warnings, plus seeds 7, 19, and 37, which are inconclusive: both binaries are killed by the CPU limit and print no checksum, so they are not counted as a pass. Seeds 21 and 22 were a file-scope initializer of a nested bit-field struct; they pass and were removed from `harness/csmith_known_failures.txt`. No checksum mismatch in the batch.

### One True AWK

Pin `5739fd79bcfc75ba7526773d0cf634521f8aca3c` (tag `20260426`). `bison -d awkgram.y` writes the parser. `maketab` is compiled with the host `cc`, not this compiler, and writes `proctab.c`. Both awk builds use `-O0`. Clang also gets `-std=c11`. Both pass `-D_DEFAULT_SOURCE`, because this compiler defines `__STRICT_ANSI__` and glibc then hides `srandom`.

This compiler builds unmodified awk. The features that were blocking the compile are general C, not awk special cases: `__builtin_memcpy` (and `__builtin_memmove` / `__builtin_memset`), anonymous struct and union members, `long double` expressions, and `L` wide literals. Compiling awk then also required `__builtin_signbit`, `__builtin_signbitf`, and `__builtin_signbitl`, because `<math.h>`'s `signbit` expands to those names when `__GNUC__` is 4.2. The awk sources were not patched.

`bugs-fixed` is 31 programs, and this compiler's awk matches the Clang-built awk on all of them. The upstream `testdir` prints the same nine `BAD` lines for both builds. Eight are golden-file mismatches the Clang-built awk also produces here: `T.argv delete ARGV[2]`, and the `T.arnold` cases `a-format`, `decr-NF`, `ofs-rebuild`, `space`, `string-conv`, `system-status`, and `unary-plus`. The ninth is a test whose expected diagnostic contains the letters BAD; both builds miss that text. They are not differences between the two binaries. `harness/run_onetrueawk.py` compares the two lists and fails only when they differ.

### AWK Builder suites

Two private repositories, fetched with `gh` at the pins in `tests/external/README.md`. `harness/run_awk_extra.py` runs them against `AWK_CLANG` and `AWK_CCOMP`.

- POSIX challenge annotated tag `v0.6`. The tag object is `69b1f5895022956c5ebfc0d6d58fd45636b6dfaa`. The pin compared with `HEAD` is the commit that tag points at, `b46ea063c6d25372b6a2010ac537ca19a8fd1b1e`. Portable, observational, and nonportable are reported separately. A compiler defect is a per-case disagreement. One True AWK itself fails portable cases AWK-391, AWK-392, and AWK-399, so the runner's portable gate may exit nonzero. The harness still compares the two JSON files.
- Shakespeare volume test (`46701e47`): one portable program, two inputs, stdout compared to the pinned expected files.

`AWK_EXTRA_HOOK` is an optional command for a later suite. None is configured. This environment's GitHub token can read `spacefilm900/c-compiler` and cannot read the other private repositories, so the suites were not executed here. CI skips them only when `AWK_SUITE_READ_TOKEN` is unset. The One True AWK job builds awk with this compiler and compares that binary with the Clang build.

### GCC torture execute

Pin `5115c7e447fc07457443df874bf57840e8316d5f` (`releases/gcc-15.2.0`). Each program is compiled here, linked with `clang -O0 -lm`, and must exit 0. `builtins/lib` (28 files) is support code and is skipped.

`builtins/` is skipped (144 files). GCC's own harness links each of those with a `-lib.c` companion and `lib/main.c`, and the tests target compiler builtins. They are not standalone programs.

The local run before `20010325-1.c` was removed was **827 passed, 919 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped**. Thirty tests that were listed then passed: `long double` arithmetic and `va_arg`, anonymous members, `L` wide literals, and programs whose only missing piece was `__builtin_memcpy` or `__builtin_signbit`. They were removed from `harness/gcc_torture_known_failures.txt`.

CI on `fc52cef` (job `gcc torture execute`, run [42878896739](https://github.com/spacefilm900/c-compiler/actions/runs/42878896739)) printed `NOW_PASS 20010325-1.c` and `827 passed, 918 known failures, 1 unexpected, 1746 executed, 144 builtins files skipped`. That program was still listed as `wide-literal` and exits 0, so the line was removed. With that pass counted, the summary is **828 passed, 918 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped**.

After the eight C11 gaps (see that record), 72 listed tests pass and were removed, and the categories of the rest were re-derived from the current first diagnostic with `--record`. Local result: **900 passed, 846 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped**.

| Category | Tests |
| --- | --- |
| builtin | 378 |
| compile-error | 357 |
| old-style-decl | 44 |
| vla | 21 |
| nested-function | 14 |
| runtime | 13 |
| asm | 8 |
| complex | 7 |
| flexible-array | 3 |
| alignas | 1 |

The floating-point format builtins made five listed `ieee/` tests pass: `builtin-nan-1.c`, `hugeval.c`, `inf-1.c`, `inf-2.c`, and `inf-3.c`. Those lines were removed from `harness/gcc_torture_known_failures.txt`. `ieee/inf-4.c` remains listed as `builtin`. Local result: **905 passed, 841 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped**. The `builtin` category is 373 (was 378); the other categories are unchanged.

The v0.4 repairs (see that record) made twelve listed tests pass, and those lines were removed. The floating-point comparison builtins fixed eleven `builtin` entries: `ieee/compare-fp-1.c`, `ieee/compare-fp-3.c`, `ieee/compare-fp-4.c`, `ieee/fp-cmp-4.c`, `ieee/fp-cmp-4f.c`, `ieee/fp-cmp-4l.c`, `ieee/fp-cmp-5.c`, `ieee/fp-cmp-8.c`, `ieee/fp-cmp-8f.c`, `ieee/fp-cmp-8l.c`, and `ieee/pr38016.c`. `__builtin_offsetof` fixed the `compile-error` entry `pr30778.c`. Local result: **917 passed, 829 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped**. The `builtin` category is 362 and `compile-error` is 356.

`runtime` means the program compiled and did not exit 0. Several of those are bit-field width or promotion tests, or an `aligned` attribute this compiler ignores. They are listed, not treated as a reason to copy GCC.

### Performance

`harness/perf.py` writes `reports/perf.json`. CI uploads that file from `ubuntu-24.04` and does not fail on timing noise. Compile time stops at LLVM IR on both sides: this compiler writes textual IR, and Clang is timed with `-std=c11 -O0 -S -emit-llvm`. Run time is both binaries linked with `clang -O0`.

The numbers below are from the agent VM (Linux 6.12.113+, x86_64, Ubuntu Clang 18.1.3, Python 3.12.3), not from a GitHub `ubuntu-24.04` runner. The CI artifact `perf-numbers` is that runner's measurement. The harness programs are tiny, so the run-time gap is not a stable ranking.

| Measurement | This compiler | Clang -O0 |
| --- | --- | --- |
| Harness compile (115 files, both to LLVM IR) | 0.39 s | 1.86 s |
| One True AWK compile | failed after 1.44 s | 0.37 s |
| Harness run (115 programs) | 0.44 s | 0.33 s |
| One True AWK test suite | did not build | 0.40 s |
| Shakespeare volume | not run (private repo) | not run (private repo) |

### Headers added for the suites

`<float.h>` and `<stdnoreturn.h>` are freestanding headers this compiler supplies. `tests/108_float_h.c` prints the IEEE macros and matches Clang. `tests/109_noreturn.c` uses `noreturn`. An `l`/`L` constant is an 80-bit value, so a program can use `LDBL_MAX`.

The timing table above is from before this compiler could build awk. The compile now succeeds, and the two awk binaries agree on `bugs-fixed` and on `testdir`.

## Csmith coverage campaign

The in-tree runner is `tools/csmith_campaign.py`. It always links executables with `-lm`, at both `-O0` and `-O2`. Default mode is `gnu11`: the candidate is compiled with `-std=gnu11`, and GCC and Clang run in their default GNU mode, without `-pedantic-errors`. Csmith 2.3.0 is given the same size caps as `harness/run_csmith.py`, plus `--no-packed-struct` and `--no-argc`, so programs usually finish in seconds and layouts stay comparable. `--mode strict` is available for a C11 comparison. The script can be sharded with `--seed-start` and `--count` on a shared `--out` directory, and it skips a seed that already has a complete result. Command construction, the `-lm` fix, and classification are tested in `tools/test_csmith_campaign.py`, which the CI check job now runs.

The CI runner `harness/run_csmith.py` now also compiles the candidate and the Clang reference with `-std=gnu11`, and still links with `-lm`. On the 40 seeds in `harness/csmith_seeds.txt` the result was unchanged: **25 passed, 15 known failures, 0 unexpected**, in about a minute, so the known-failure list and the job's runtime stay the same. After the Csmith follow-up fixes the same batch is **34 passed, 6 known failures (all `inconclusive`), 0 unexpected**.

A local campaign of seeds 1–2000, at both `-O0` and `-O2`, was run once on 13 November 2026 (Europe/London) against compiler `e5316b1667bb4ae746bd5b713ceb3a1abf922239`, Clang 19.1.7, and GCC 14.2.0, used only as black-box oracles. Evidence is in `/workspace/csmith-coverage-2026-11-13`. Totals:

| Class | `-O0` | `-O2` |
| --- | ---: | ---: |
| Valid differential comparisons | 1,492 | 1,490 |
| `agreement` | 1,486 | 1,490 |
| `candidate-wrong-output` | 6 | 0 |
| `candidate-rejects-valid-gnu-c` | 508 | 505 |
| `reference-disagreement` | 0 | 5 |
| `candidate-crash-or-ice` / `candidate-bad-ir` / `candidate-timeout` / `reference-timeout` / `invalid-program` | 0 | 0 |

Two classes of candidate defect were recorded and not fixed:

1. **Wrong output at `-O0` only** (seeds 121, 755, 819, 1468, 1841, 1855). The final checksum matches GCC and Clang, but this compiler also prints Csmith's `transparent_crc` traces even though `print_hash_value` is 0. A minimised program (`reports/csmith-coverage/repro-struct-return-flag.c`) shows the same thing: after a discarded return of a bit-field struct, a later call that passes that 0 as the third argument (after an integer and a pointer) takes the true branch. Linking the same IR at `-O2` agrees with the references.
2. **Valid GNU C rejected** (508 seeds at `-O0`, of which 418 are incompatible pointer comparisons and 90 are incompatible pointer assignments). GCC and Clang accept and run these in GNU mode. This compiler's `-std=gnu11` still diagnoses them, because that mode currently allows only equality between a `void *` and a function pointer. Minimised reproducers: `reports/csmith-coverage/repro-pointer-compare.c` and `reports/csmith-coverage/repro-pointer-assign.c`.

The five `-O2` reference disagreements (seeds 568, 1085, 1502, 1562, 1810) are cases where GCC and Clang did not agree with each other, and are not counted as candidate defects.

Commands and options are in `tests/external/README.md`. The compiler was not changed in this step.

## Csmith follow-up fixes

Both candidate defects from the campaign are fixed, tests first, plus two small CodeRabbit leftovers from pull request 29. Clean room: the ABI change follows the System V AMD64 psABI (3.2.3, an eightbyte holds only the bytes of the object); the GNU-mode rules come from the GCC manual's description of these warnings and from GCC 14.2.0 and Clang 19.1.7 used as black boxes (which programs they accept, and the LLVM types Clang emits for a struct return). No compiler source was read.

1. **Wrong output at `-O0` (root cause).** A struct of 9 to 15 bytes that travels in two general registers had its second eightbyte typed as `i64` whenever several fields, or one bit-field storage unit, shared that eightbyte. For the 12-byte bit-field struct in the reproducer, `func_1` loaded 8 bytes from offset 8 of a 12-byte global, and the caller stored 8 bytes at offset 8 of a 12-byte temporary, overwriting the next stack slot, which held `print_hash_value`. At `-O2` the flag lived in a register, so the overrun was invisible. `src/abi.rs` now gives that piece only the bytes the object owns: `{ i64, i32 }` for 12 bytes, `{ i64, i24 }` for 11, `{ i64, i16 }` for 10, the same types Clang emits for those structs. Bit-fields were only the trigger; `struct { int a, b; short c; char d; }` had the same overrun. Tests: `tests/137_struct_return_tail.c` (crashed with `SIGSEGV` before the fix) and the `abi` unit test `last_eightbyte_never_extends_past_the_object`.
2. **GNU-mode pointer rules.** With `-std=gnu11`, a comparison of pointers to distinct types (equality for any two pointers, relational for two object pointers), and an assignment, initialization, argument, or return whose pointee differs only in integer signedness or drops a qualifier, are accepted with a warning. Deeper mismatches (`int **` from `unsigned **`, `int *` from `long *`) stay errors because GCC 14 rejects them too. Strict C11, the default, is unchanged. The parser's folded constant comparisons follow the same rule. Tests: two `sema` unit tests, and a new `tests/gnu/` harness category, where `harness/run_tests.py` compiles each file with `-std=gnu11` against `clang -std=gnu11` and also checks that the default strict mode rejects it with an `error:` and no crash.
3. **Campaign script.** `write_json` in `tools/csmith_campaign.py` writes each call to its own `mkstemp` file in the destination directory before the atomic replace, so concurrent shards never share a temporary name. The relative-path test for `resolve_tool` now passes a genuinely relative path. Both are covered in `tools/test_csmith_campaign.py`.

Rerun of Csmith seeds 1–2000 with `tools/csmith_campaign.py` (same options as the first run, compiler `c39698015d913f09a6a4fde738b4f582406a49b5`; later commits change only documents and known-failure lists). Evidence is in `/workspace/csmith-fixes-2026-11-13`; totals are in `reports/csmith-coverage/README.md`.

| Class | `-O0` before | `-O0` after | `-O2` before | `-O2` after |
| --- | ---: | ---: | ---: | ---: |
| Valid differential comparisons | 1,492 | **1,821** | 1,490 | **1,822** |
| `agreement` | 1,486 | **1,821** | 1,490 | **1,822** |
| `candidate-wrong-output` | 6 | **0** | 0 | **0** |
| `candidate-rejects-valid-gnu-c` | 508 | **0** | 505 | **0** |
| `reference-disagreement` | 0 | 0 | 5 | 0 |
| `reference-crash` | — | 179 | — | 178 |

The `reference-crash` seeds are programs whose reference binaries do not finish: GCC's binary is killed by the 8-second CPU limit in every one of them (and Clang's too, except five where it exits with `SIGSEGV` or status 16; those five include the four former `-O2` reference disagreements). The first run counted them as candidate rejects because the candidate never got to run them. They are inconclusive, not candidate defects, and no new reject or disagreement category appeared.

The CI Csmith batch is now **34 passed, 6 known failures, 0 unexpected** (40 seeds). Nine `strict-pointer` seeds pass and were removed; seeds 8, 20, and 30 now compile and join 7, 19, and 37 as `inconclusive` (both binaries killed by the CPU limit). The `strict-pointer` category is gone. Two GCC torture tests now pass in GNU mode and were removed from the known-failure list: `20001017-2.c` (`long *` parameter given `unsigned long *`) and `980617-1.c` (`unsigned int *` given `int *`).

Local results (rustc 1.85.1, Clang 19.1.7, GCC 14.2.0): `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **125 passed**, `python3 tools/test_csmith_campaign.py` **27 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **154/154 passed** (including the two `tests/gnu/` programs), `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**, `python3 harness/run_csmith.py` **34 passed, 6 known failures, 0 unexpected**, `python3 harness/run_gcc_torture.py` **919 passed, 827 known failures, 0 unexpected**, `python3 harness/run_onetrueawk.py` `bugs-fixed` 31 programs agree and `testdir` shows the same nine golden-file mismatches for both builds, and `python3 harness/perf.py` reported 0 result mismatches and 0 timeouts on 143 harness programs.

## GNU cheap tier (`-std=gnu11`)

PLAN.md step 4. Clean room: everything here comes from the public GCC manual (C extensions, attributes, and builtins), N1570, the LLVM Language Reference, and the System V psABI. GCC 14.2.0 and Clang 19.1.7 were used only as black boxes. No compiler source was read. Tests came first (`tests/gnu/03`–`08`, `tests/gnu/both/01`–`05`), then the implementation, then these documents.

1. **Language extensions, only with `-std=gnu11`.** `typeof` / `__typeof__`, statement expressions, case ranges, zero-length arrays, `x ?: y` (with `x` evaluated once), and empty initializers `{}`. Strict C11 rejects each with an error that names `-std=gnu11`. In `harness/run_tests.py`, each `tests/gnu/*.c` must be rejected in strict mode and must match `clang -std=gnu11`.
2. **Attributes, in both modes.** `__attribute__((...))` is now parsed, not skipped. It has meaning for `noreturn`, `unused`, `used`, `aligned`, `packed`, `always_inline` / `noinline`, `section`, `weak`, `deprecated`, `cold` / `hot`, `const` / `pure`, `constructor` / `destructor`, and `mode`. `format` and the other optimisation hints are ignored. An unknown attribute is ignored with a warning. `vector_size`, `cleanup`, `alias`, `ifunc`, `naked`, and `weakref` are errors, because ignoring them would silently change the program. `packed` and `aligned` change record layout. A record whose layout they change is accessed bytewise, and its loads and stores use the real alignment.
3. **Builtins, in both modes** (new module `src/builtins.rs`). The bit-counting builtins, `bswap`, `expect`, `unreachable`, `trap`, `alloca`, the classification builtins, the overflow builtins, `return_address` / `frame_address` at level 0, and constant folding for `constant_p`, `types_compatible_p`, and `choose_expr`. There are also library forwarders with standard prototypes (`__builtin_abort`, `__builtin_printf`, the string, ctype, and math functions). The same constant-foldable builtins fold in integer constant expressions. A call to a forwarder declares the library function if the program did not.
4. **Spellings and types, in both modes.** An empty declaration at file scope (a lone `;`) is accepted and ignored. `__int128` and its spellings are described under Supported syntax. `__signed__`, `__const__`, `__volatile__`, `__alignof__` (also with an expression), and `__FUNCTION__`, alongside the existing `__extension__`, `__inline__`, `__restrict__`, and `__asm__` labels.
5. **CodeRabbit nit from pull request 30.** The GNU-mode pointer-sign relaxation no longer rests on integer size alone. It accepts only pointees that differ in signedness (`int *` and `unsigned *`). Types that just share a size stay errors, as they do in GCC 14: `long *` and `long long *`, `long long *` and `unsigned long *`, and `_Bool *` and `unsigned char *`. Sema unit tests cover each case.

GCC torture (`-std=gnu11`): **1,314 passed, 432 known failures, 0 unexpected** (was 919 passed, 827 known failures). 397 tests now pass and were removed from the list. Two tests that used to pass, `20060420-1.c` (`vector_size`) and `align-1.c` (an `aligned(16)` typedef of `int`), passed only because those attributes were silently ignored. Both are now diagnosed and listed (`vector`, `alignas`). Remaining failures in the old `compile-error` and `builtin` groups, by first error: inline assembly about 55, implicit `int` or implicit function declarations about 90, `vector_size` 36, bit-fields in a packed struct 9, range designators 6, and a long tail (`__builtin_longjmp`, `__complex__`, `__builtin_classify_type`, missing `__FLT_*__` / `__DBL_*__` macros, `alias`).

Deferred: `__builtin_object_size`, `__builtin_classify_type`, `__builtin_*_overflow_p`, `__builtin_setjmp` / `longjmp`, `__builtin_apply`, vector types, typedef alignment that differs from the natural alignment, bit-fields in records whose layout `packed` / `aligned` changes, and `T a[] = {}`, which gets one element instead of zero.

Csmith smoke (`tools/csmith_campaign.py`, default `gnu11` mode and options, seeds 1–200, compiler `9254d4e`): `-O0` **181 agreement, 19 reference-crash**; `-O2` **182 agreement, 18 reference-crash**. No candidate wrong output, crash, reject, bad IR, or timeout. The `reference-crash` seeds are programs whose reference binaries do not finish within the CPU limit, the same inconclusive class as in the earlier campaign.

Local results (rustc 1.85.1, Clang 19.1.7, GCC 14.2.0): `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **133 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **165/165 passed** (8 `tests/gnu/` and 5 `tests/gnu/both/` programs), `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**, `python3 harness/run_csmith.py` **34 passed, 6 known failures, 0 unexpected** (40 seeds), and `python3 harness/run_gcc_torture.py` **1314 passed, 432 known failures, 0 unexpected**.

## DOOM (doomgeneric)

PLAN.md step 5, first program. Clean room: the fix below was written from N1570 (6.6p9 address constants, 6.7.9 initialization) and the LLVM Language Reference (packed literal struct constants, `ptrtoint` constant expressions). Clang 19.1.7 was used only as a black-box reference. DOOM's source was read only to reduce the failure; none of it is in the compiler.

**Inputs, fetched and not vendored.** `scripts/fetch_doomgeneric.sh` clones https://github.com/ozkl/doomgeneric at `e257a09527062bfb986b6e5e0780839b6cbd5b71` into `tests/external/doomgeneric/` (GPL-2.0, a test input only). `scripts/fetch_freedoom.sh` downloads the Freedoom 0.13.0 release (BSD licence), checks the zip, `freedoom1.wad`, and `freedoom2.wad` against pinned SHA-256 sums, and keeps the two WADs and `COPYING.txt` in `tests/external/freedoom/`. A cached doomgeneric tree is reused only when its `HEAD` is the pinned commit and no tracked file has changed; otherwise it is fetched again. Both directories are gitignored and cached in CI.

**Harness.** `harness/doom/dg_headless.c` is this repository's own doomgeneric backend (MIT): no window, no input, a fake clock (one millisecond per `DG_GetTicksMs` call, plus whatever `DG_SleepMs` asks for), and a 64-bit FNV-1a style hash of every frame, written to a log with a final hash of all hashes. It plays one recorded demo with `-timedemo`, so each frame is one game tic. When the demo ends, `-timedemo` reports `timed N gametics` and exits with status 255; an `atexit` handler then writes the final hash line, so the demo is compared to its natural end. `--frames` is only a safety cap (default 100,000). `harness/doom/dg_alloc.c` replaces `malloc` and its relatives with a bump allocator at a fixed address: DOOM draws a few frames from memory it never initialised (the opening screen wipe starts from an uncleared video buffer, and a few later frames have stray pixels), that memory holds old heap addresses, and with the C library's allocator those frames changed with address-space randomisation and with the size of the executable. `harness/run_doom.py` builds the 80 doomgeneric files from the upstream Makefile's object list (minus its X11 backend) plus those two files, in three graded steps per level: compile each file with this compiler (`-std=gnu11`) and record ok, reject, ICE, bad IR, or timeout; if any failed, a mixed build with Clang objects for those files; otherwise a full build with this compiler only. By default it plays every demo in both WADs (`DEMO1` to `DEMO4` of `freedoom1.wad` and of `freedoom2.wad`, read from each WAD's directory) at each level; `--opt O0:freedoom1/demo1` picks demos for one level. For each demo it compares every frame hash, the number of frames, how the demo ended, and the game-tic count with a Clang `-std=gnu11` build at the same level, reports compile and run times, and can write the scorecard (`--scorecard`, `--json`, `--from-json`). `--play-jobs N` runs demos side by side. A demo that stops at the `--frames` cap compared only a prefix: the run then ends `DOOM: INCOMPLETE` and fails, unless `--allow-prefix` is given, and the scorecard labels such demos "prefix only" (`tools/test_run_doom.py`). A relative `DOOM_WAD` is resolved before the game changes directory, and a compiler killed by a signal is recorded as an ICE, not a reject. `make doom` fetches and runs it.

**Defects.**

| Id | Category | Where | Repro test | State |
| --- | --- | --- | --- | --- |
| DOOM-1 | frontend reject | `info.c`: `states[]` puts function addresses in the `actionf_t` union inside each struct element (`initializer of 'states' uses an address in a union`) | `tests/138_union_address_init.c` | fixed |
| DOOM-2 | wrong output (code review of the DOOM-1 fix) | a later designator naming another member of a union (`{ .a.p = &x, .b.q = &y }`) kept the earlier member's bytes or address | `tests/139_union_member_switch.c` | fixed |
| DOOM-3 | wrong output (code review of the DOOM-1 fix) | a static union whose first member holds a bit-field record, set through a longer member with an address, came out too large and moved the next field | `tests/140_union_bitfield_layout.c` | fixed |
| DOOM-H1 | harness (not a compiler defect) | frames 1–27 and a few later frames changed from run to run and between builds: DOOM reads uninitialised memory that holds addresses | none; see `dg_alloc.c` | fixed in the harness |
| DOOM-H2 | harness (not a compiler defect) | the doomgeneric cache trusted its `.pin` file alone, a relative `DOOM_WAD` was not found, and a compiler killed by a signal was recorded as a reject | none | fixed in the harness |

The demos themselves found no ICE, bad IR, wrong output, crash, or performance defect. DOOM-2 and DOOM-3 were found by CodeRabbit's review of the DOOM-1 fix; DOOM does not reach them. Nothing needed inline assembly, vector types, computed `goto`, or any other deferred feature, so nothing is deferred.

**DOOM-1 fix.** A union, or a bit-field record, in a static initializer is folded into an object image. An address cannot be written as bytes, so the image now keeps it beside the bytes as a relocation at its byte offset (`ConstInit::RelocBytes`); a later store that overlaps it replaces it. The IR generator spells such an image as a packed literal struct of byte runs and `ptr` (or `ptrtoint`) pieces. A struct or array around one becomes a packed literal struct laid out from the C offsets, or an ordinary array when every element still has the same type. The global then has that literal type instead of the named one; every access goes through an opaque `ptr`, so only the size and the alignment (given explicitly) must agree. This covers a top-level union, a designated member that is not the first, a union in a union, a later designator that replaces an earlier member, a struct inside a union's first member, and an address in a plain member of a bit-field struct. The sema unit test that expected the old diagnostic now checks the IR.

**DOOM-2 fix (C11 6.7.9p17, p19).** While an initializer is shaped, each union now remembers which member its stores belong to (keyed by the union's byte offset and type). When a designator, or the positional walk after one, names a different member, every store already made inside that union is dropped, along with the member choices of unions nested in it, and only the new member's stores remain; its other subobjects are zero. This works on the store list, so it applies to static and automatic objects alike. Naming the same member again keeps what was written. A nested union is always chosen after the union around it, so when a nested union switches member only the choices recorded after it, inside it, are cleared; an enclosing union at the same offset and of the same size keeps its choice (`tests/139_union_member_switch.c`, `union K`). The dropped stores are not thrown away unchecked: their expressions are checked like any other initializer (an undeclared identifier or an incompatible type is still a diagnostic), and in a static initializer they must still be constant expressions (6.7.9p4), as for any overridden initializer here; unit test `checks_overridden_union_initializers`. A compound literal inside the initializer keeps its own record, and the scratch walk that works out an unsized array's length leaves no choices or replaced stores behind for the real walk (`tests/gnu/09_union_array_length_label.c`).

**DOOM-3 fix (C11 6.7.2.1p16, 6.7.9).** A union that holds a bit-field record anywhere in its members, including inside a struct or array member, is now one complete byte image of the union's full size, rather than only the bit-field record of its first member being sliced out. An address stored through a longer member stays inside that image, so the union keeps its size and the fields after it keep their C offsets. A slice of an image now refuses a relocation that would cross its edge (a diagnostic, never a silently split address).

**Scorecard.** `reports/doom/baseline.md` (and `.json`) is the first run, frozen: on `be590a8`, `info.c` was rejected, so both levels were mixed builds, and frames 1402 (`-O0`) and 1–27 (`-O2`) differed for the DOOM-H1 reason. `reports/doom/scorecard.md` (and `.json`) is the final run: all eight Freedoom demos, each to its natural end, at `-O0` and `-O2`, one demo run at a time:

| WAD | Demo | Game tics | Frames | `-O0` | `-O2` |
| --- | --- | --- | --- | --- | --- |
| `freedoom1.wad` | DEMO1 | 7,117 | 7,156 | match | match |
| `freedoom1.wad` | DEMO2 | 7,089 | 7,128 | match | match |
| `freedoom1.wad` | DEMO3 | 6,042 | 6,081 | match | match |
| `freedoom1.wad` | DEMO4 | 3,136 | 3,175 | match | match |
| `freedoom2.wad` | DEMO1 | 2,664 | 2,703 | match | match |
| `freedoom2.wad` | DEMO2 | 5,444 | 5,483 | match | match |
| `freedoom2.wad` | DEMO3 | 2,743 | 2,782 | match | match |
| `freedoom2.wad` | DEMO4 | 4,662 | 4,701 | match | match |

That is 39,209 frames per level and 78,418 in all, every one identical to Clang's.

| | `-O0` | `-O2` |
| --- | --- | --- |
| Builds with this compiler alone | yes (82/82 files: 80 doomgeneric files plus the two harness files) | yes (82/82) |
| Frames compared / matching Clang | 39,209 / 39,209 | 39,209 / 39,209 |
| Compile time, ours vs Clang (sum over files) | 9.26 s vs 2.52 s (3.7x) | 11.84 s vs 5.73 s (2.1x) |
| Run time, ours vs Clang (all eight demos, hashing included) | 42.98 s vs 44.80 s (0.96x) | 23.48 s vs 24.12 s (0.97x) |

About 7.1 s of our compile time is this compiler's preprocessor and frontend (about 90 ms a file); the rest is Clang turning the IR into objects. That is an item for the further performance tuning step in `PLAN.md`, not a defect.

**CI.** A new `doom frame hashes` job fetches doomgeneric and Freedoom (both cached), builds the compiler, and runs `python3 harness/run_doom.py --opt O0 --opt O2 --play-jobs "$(nproc)"`: all eight demos to their end at both levels. Locally, with 8 cores and one demo run at a time, that takes about 2 minutes 20 seconds; with the demos side by side it is well under that.

**CodeRabbit nits from pull request 31.** `tests/gnu/03_typeof.c`, `tests/gnu/04_statement_expr.c`, and `tests/gnu/both/04_builtins_overflow.c` no longer use `#define`; each macro use is written out at its call site. Clang's output for each file is unchanged.

Local results (rustc 1.85.1, Clang 19.1.7, GCC 14.2.0): `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **134 passed**, `python3 -m unittest tools/test_csmith_campaign.py tools/test_run_doom.py` OK, `make` produced `build/compiler`, `python3 harness/run_tests.py` **169/169 passed** (with the new `139_union_member_switch.c` and `140_union_bitfield_layout.c`, which also match Clang at `-std=gnu11`, and `gnu/09_union_array_length_label.c`), `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**, `python3 harness/run_gcc_torture.py` **1314 passed, 432 known failures, 0 unexpected** (unchanged), `python3 harness/run_csmith.py` **34 passed, 6 known failures, 0 unexpected** (40 seeds), Csmith smoke (`tools/csmith_campaign.py`, `gnu11`, seeds 1–100, fresh output directory) **88 agreement, 12 reference-crash** at both `-O0` and `-O2`, with no candidate wrong output, crash, reject, bad IR, or timeout, `python3 harness/run_onetrueawk.py` passed (`bugs-fixed` 31 programs agree), and `python3 harness/run_doom.py` **PASS** at `-O0` and `-O2`, all eight demos to their end (78,418 frames).

## Lua 5.4.7

PLAN.md step 5, second program. Clean room: the fix below was written from the GCC manual ("Labels as Values"), N1570 (6.6p9 address constants, 6.8.6.1 `goto`), and the LLVM Language Reference (`blockaddress`, `indirectbr`). Clang 19.1.7 was used only as a black-box reference. Lua's source was read only to find what the failing file needed; none of it is in the compiler.

**Inputs, fetched and not vendored.** `scripts/fetch_lua.sh` downloads `lua-5.4.7.tar.gz` and `lua-5.4.7-tests.tar.gz` from lua.org, checks them against the SHA-256 sums lua.org publishes (`9fbf5e28…1e30` and `8a4898ff…811e`), and unpacks them into `tests/external/lua/` (gitignored, cached in CI). lua.org drops some TLS connections, so each download is retried on both host names. Lua is MIT licensed; it is a test input only and is not patched.

**Harness.** `harness/run_lua.py` builds two configurations at each level: `release` (the Makefile's `linux` target: `-DLUA_COMPAT_5_3 -DLUA_USE_LINUX`, linked with `-lm -ldl -Wl,-E`, no readline; programs `lua` and `luac`, 34 files) and `ltests` (the same interpreter with `-DLUA_USER_H="ltests.h"` and the suite's `ltests.c`, which turns on Lua's internal assertions and adds the `T` library for white-box tests; 34 files). Each file goes through the same graded steps as DOOM: per-file compile (ok, reject, ICE, bad IR, timeout), a mixed build with Clang objects if anything failed, otherwise a full build with this compiler only. The reference is Clang with the same `-std`, flags, and level. Then it runs, for both builds:

- the official suite three ways, each in a fresh copy of the test directory: `user` (`lua -e "_U=true" all.lua`, the suite's documented mode for user builds), `portable` (`-e "_port=true"`: every portable test, including the long ones) and `internal` (the ltests build with `-e "_port=true"`). The suite prints random seeds, dates and timings, so the outcome is compared: exit status, "final OK !!!", and the list of test files reached;
- nine workload scripts of our own in `harness/lua/` (strings and patterns, tables and sorting, closures and errors, coroutines, garbage-collector stress with weak tables and finalizers, integer and float maths, `string.format` and parsing of floats, metatables and objects, and five classic benchmark kernels), compared byte for byte (stdout, stderr, exit status) and timed;
- `luac -s` of all 42 Lua files (workloads and suite), whose output must be identical.

`--std`, `--opt`, `--suite`, `--run-jobs`, `--json`, `--scorecard`, `--notes` and `--from-json` work as in the DOOM harness. `tools/test_run_lua.py` tests the outcome parsing, timeouts, `luac` failures and stale `luac` outputs, the verdict (a mixed build fails even when every output matches), the scorecard, and the Makefile-driven file lists. `make lua` fetches and runs it.

**Defects.**

| Id | Category | Where | Repro test | State |
| --- | --- | --- | --- | --- |
| LUA-1 | frontend reject (missing GNU feature) | `lvm.c`: Lua uses a jump table of label addresses (`ljumptab.h`) when `__GNUC__` is defined, which it is here | `tests/gnu/10_labels_as_values.c`; unit test `labels_as_values_and_computed_goto` | fixed, in `-std=gnu11` |
| LUA-H1 | harness | lua.org drops some downloads | none | retries in the fetch script |
| LUA-H2 | harness | the suite's output has random seeds and timings | none | outcome compared, not bytes |

No ICE, bad IR, wrong output, crash, or performance defect was found. The first run (strict `-std=c11`, `reports/lua/baseline.md`, frozen) already matched Clang everywhere it could compare: the only problem was `lvm.c`, so the release programs were mixed builds there, while the ltests configuration, whose `ltests.h` turns the jump table off, built completely.

**LUA-1 fix: labels as values and computed `goto` (`-std=gnu11` only).** `&&label` is a new expression (`Expr::LabelAddr`) of type `void *`; the label must be defined in the current function, and outside a function it is an error. It is lowered to `blockaddress(@function, %label)`, and in a static initializer it is an address constant (`PtrConst::Label`), so `static const void *const disptab[] = { &&L_OP_MOVE, ... };` inside the function becomes a global of `blockaddress` constants; it also works inside unions and as a 64-bit integer, like other address constants, and an offset from it or a truncation is "not constant". `goto *expr;` (`Stmt::GotoPtr`) needs a pointer operand and is lowered to `indirectbr`, listing every label of the function whose address is taken, which semantic analysis records on the function (`FuncDef::addr_labels`). Strict C11 rejects both with an error that names `-std=gnu11`. Operands that are not evaluated (a `sizeof` operand, a `_Generic` controlling expression, an unselected `_Generic` association) are checked and then dropped, and everything they recorded for IR lowering is rolled back in one place (`Sema::begin_unevaluated` / `end_unevaluated`): block-scope `static` objects and file-scope compound literals with their numbering, string literals, library calls, label addresses, and labels. A label defined there is not a destination of a computed `goto`, and a `goto` or `&&` naming it is rejected ("cannot jump to label ... not evaluated"), as Clang rejects it; the name still cannot be redefined. A `static` function used only there needs no definition (C11 6.9p3), as in Clang and GCC (`tests/141_unevaluated_static_use.c`, `tests/gnu/12_generic_unevaluated_state.c`). Declarations there (a block-scope `extern`) stay declared, as C requires. Limitation: the LLVM Language Reference makes an equality test between two label addresses undefined (only a comparison with null and the use in `indirectbr` are defined), so `p == &&label` compiles but its result is not promised; the regression tests check label tables only by jumping through them, which is all Lua does. The same change made six GCC torture tests pass (`20040302-1.c`, `20041214-1.c`, `20071210-1.c`, `920501-4.c`, `920501-5.c`, `990208-1.c`), which were removed from the known-failure list. Deferred: differences of label addresses (`&&a - &&b`) and `void *` arithmetic, which Lua does not use (torture `comp-goto-1.c`, `pr70460.c`); `__label__` and nested functions stay out of scope.

**Scorecard** (`reports/lua/scorecard.md` and `.json`; `-std=gnu11`, one suite run at a time):

| | `-O0` | `-O2` |
| --- | --- | --- |
| Builds with this compiler alone | yes (release 34/34, ltests 34/34) | yes (34/34, 34/34) |
| Official suite: user / portable / internal | OK / OK / OK (Clang OK) | OK / OK / OK (Clang OK) |
| Workloads matching Clang byte for byte | 9 / 9 | 9 / 9 |
| `luac -s` output identical | 42 / 42 | 42 / 42 |
| Compile time, ours vs Clang (both configurations) | 16.3 s vs 3.3 s (4.92x) | 23.0 s vs 10.4 s (2.21x) |
| Suite run time, ours vs Clang | 20.4 s vs 19.2 s (1.06x) | 12.3 s vs 12.4 s (1.00x) |
| Workload run time, ours vs Clang | 11.5 s vs 9.6 s (1.21x) | 4.7 s vs 4.9 s (0.97x) |

Run times are single runs and vary by about 10% between runs here. The two builds take turns to run first (Clang first for the user and internal suites and for every other workload, this compiler first for the rest). A timeout on either side is never a match, and a `luac -s` file matches only when both compilers' `luac` exit 0 and write identical bytes; old outputs are deleted before each run. The first version of this scorecard (compiler `ab7bcaa`, taken while a Csmith smoke run shared the machine, Clang always run first) had compile ratios of 2.95x and 2.29x; the `-O0` Clang compile time there was inflated by that load.

**CI.** A new `lua test suite and workloads` job fetches Lua (cached by version and both SHA-256 sums), builds the compiler, and runs `python3 harness/run_lua.py --opt O0 --opt O2 --run-jobs "$(nproc)"`. Locally, with 8 cores and one suite run at a time, the whole run takes about two minutes.

Local results (rustc 1.85.1, Clang 19.1.7, GCC 14.2.0): `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **136 passed**, `python3 -m unittest tools/test_csmith_campaign.py tools/test_run_doom.py tools/test_run_lua.py` OK, `python3 harness/run_tests.py` **173/173 passed** (new `gnu/10_labels_as_values.c`, `gnu/11_label_address_constants.c` and `gnu/12_generic_unevaluated_state.c`, each also rejected in strict C11), `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**, `python3 harness/run_gcc_torture.py` **1320 passed, 426 known failures, 0 unexpected** (was 1314 / 432), `python3 harness/run_csmith.py` **34 passed, 6 known failures, 0 unexpected** (40 seeds), `python3 harness/run_doom.py` all eight demos match Clang at `-O0` and `-O2` (39,209 frames per level), `python3 harness/run_lua.py` **LUA: PASS**, and a Csmith smoke (`tools/csmith_campaign.py`, `gnu11`, seeds 1–100, fresh output directory) **88 agreement, 12 reference-crash** at both `-O0` and `-O2`, with no candidate wrong output, crash, reject, bad IR, or timeout.

## SQLite 3.54.1

PLAN.md step 5, third program. Clean room: the two fixes below were written from N1570 (6.7 declarations), the public GCC manual (128-bit integers), the LLVM Language Reference (`i128`), and the System V AMD64 ABI. Clang 19.1.7 was used only as a black-box reference. SQLite's source was read only to find what the failing files needed; none of it is in the compiler, and the amalgamation is not patched.

**Inputs, fetched and not vendored.** `scripts/fetch_sqlite.sh` downloads `sqlite-amalgamation-3540100.zip` (SQLite 3.54.1, 2026-10-16) from sqlite.org, checks it against a recorded SHA-256 (`0a9c8e34…ad1f9`) and the SHA3-256 sqlite.org publishes (`92281c86…8e30`), and unpacks it into `tests/external/sqlite/` (gitignored, cached in CI). Each download is retried on both host names. The amalgamation is public domain (the blessing is at the top of `sqlite3.h`); it is a test input only.

**Harness.** `harness/run_sqlite.py` builds the command-line shell from `sqlite3.c` (271,131 lines) and `shell.c` (39,939 lines) at each level, with `-std=gnu11 -D_DEFAULT_SOURCE -DSQLITE_THREADSAFE=0 -DSQLITE_ENABLE_FTS5 -DSQLITE_ENABLE_RTREE -DSQLITE_ENABLE_MATH_FUNCTIONS`, linked with `-lm -ldl -lpthread` and no readline. Each file goes through the same graded steps as DOOM and Lua: per-file compile (ok, reject, ICE, bad IR, timeout), a mixed build with Clang objects if anything failed, otherwise a full build with this compiler only. The reference is Clang with the same flags and level. Peak RSS is measured with `wait4` in a single-threaded helper process (`--wait4`), because forking from a thread pool is unsafe. This compiler's frontend does not take `-O`, so its IR is produced once and reused at every level; the measured frontend time is included at each level.

It then runs ten deterministic SQL workloads of our own in `harness/sqlite/` through `sqlite3 -batch -init /dev/null -bail` (DDL and inserts, joins and indexes, triggers with transactions and rollback, recursive CTEs, window functions, JSON, FTS5, `printf` and floating formats, 8,000 rows from a fixed-seed generator, VACUUM, RTree, and `integrity_check`). For each workload it compares stdout and stderr, and the SHA-256 of the database file. A timeout, a missing database, or a non-zero exit on either side is never a match. Stale database, journal, and WAL files are deleted before each run. The two builds take turns to run first. `--std`, `--opt`, `--json`, `--scorecard`, `--notes` and `--from-json` work as in the Lua harness. `tools/test_run_sqlite.py` tests timeouts, stale-database deletion, the verdict, the scorecard, and the `wait4` helper. `make sqlite` fetches and runs it.

The official TCL test suite is not used: it needs TCL and is far larger than a lean job can host. sqllogictest was considered as a portable public corpus, but its published runner is also TCL, so it is not included.

**Defects.**

| Id | Category | Where | Repro test | State |
| --- | --- | --- | --- | --- |
| SQLITE-1 | frontend reject | `shell.c`: `SQLITE_EXTENSION_INIT1;` expands to a lone `;` at file scope (`expected a type, found ';'`). C11 6.7 does not allow an empty declaration there; Clang 19 accepts it with `-Wextra-semi`, including under `-std=c11 -pedantic` | `tests/142_empty_declaration.c` | fixed |
| SQLITE-2 | frontend reject (missing GNU type) | `sqlite3.c`: this compiler defines `__GNUC__` and `__x86_64__`, so SQLite turns on `SQLITE_USE_UINT128` and writes `__uint128_t r = (__uint128_t)a * b;` (`expected ';', found 'r'`). Clang accepts `__int128` / `__uint128_t` even with `-std=c11` | `tests/143_uint128.c` | fixed |

By category: frontend reject 2 (fixed); ICE 0; bad IR 0; wrong output 0; crash 0; performance 0. The first run (`reports/sqlite/baseline.md`, frozen, same flags) rejected both files, so the baseline programs were mixed builds with all Clang objects; the ten workloads already matched there because this compiler contributed no object files.

**SQLITE-1 fix: empty file-scope declarations (both modes).** A lone `;` at file scope is accepted and ignored, so a macro that expands to nothing and is followed by a semicolon is not a diagnostic. This matches Clang and GCC.

**SQLITE-2 fix: 128-bit integers (both modes).** `__int128`, `signed __int128`, `unsigned __int128`, `__int128_t`, and `__uint128_t` are type specifiers. They have rank above `long long`, size and alignment 16, and lower to LLVM `i128`. Arithmetic, shifts, and conversions to and from the ordinary integer types work. A 128-bit parameter or return is an `i128` value, which LLVM lowers to the System V two-register convention. C integer literals still fit in 64 bits; constant evaluation carries the full 128-bit value after a conversion or an operation, including `_Static_assert`, static initializers, union images, and folding in a floating-constant context (`tests/144_uint128_constants.c`). A signed left shift or unary minus that does not fit is rejected, as is a bit-field of `__int128` (the backend still uses a 64-bit container). `__builtin_add_overflow` and friends, `va_arg`, and struct passing and return use the System V two-register / 16-byte-alignment convention (`tests/145_uint128_more.c`, `tests/gnu/13_int128_case_range.c`). SQLite also uses the type for run-time widening multiplies (`tests/143_uint128.c`). In a variadic call, a 128-bit argument that does not fit in the remaining GP registers goes on the stack but gives back only its own registers, so a later mixed INTEGER/SSE struct still gets the leftover one (System V AMD64 3.2.3; GCC agrees, while LLVM 19's backend places that struct's INTEGER half on the stack anyway, so only its SSE half is compared). A 128-bit `va_arg` load claims 8-byte alignment, because the register save area's GP slots are only 8-byte aligned, and a register-class record with 16-byte alignment is read from the overflow area at its own alignment (`tests/146_uint128_va_regs.c`). The minimum toolchain is now Clang/LLVM 18 (README, DESIGN, CI, `scripts/ci-apt-install.sh`, and a check in `harness/run_tests.py`), because LLVM 15–17 x86-64 backends split a stack-bound `i128` argument between `%r9` and the stack, which this `va_arg` (16 bytes from the overflow area) would misread. These fixes also make GCC torture `pr92904.c` pass. The same two changes made five GCC torture tests pass (`20050106-1.c`, `builtin-bitops-1.c`, `pr34154.c`, and `pr37573.c` for a trailing `};` after a function, and `pr84748.c` for `__int128`), which were removed from the known-failure list.

**Language mode.** SQLite 3.54.1 is written in C that strict C11 can accept, but the command-line shell uses POSIX functions (`strdup`, `lstat`, `symlink`, and others). glibc hides those declarations from a strict ISO C translation unit, so Clang itself refuses `shell.c` with `-std=c11` and no feature-test macro. The scorecard therefore builds both compilers with `-std=gnu11 -D_DEFAULT_SOURCE`. `_DEFAULT_SOURCE` is also needed with this compiler in GNU mode, because `__STRICT_ANSI__` is still predefined in both modes.

**Scorecard** (`reports/sqlite/scorecard.md` and `.json`; `-std=gnu11`):

| | `-O0` | `-O2` |
| --- | --- | --- |
| Builds with this compiler alone | yes (2/2) | yes (2/2) |
| Workloads matching Clang byte for byte | 10 / 10 | 10 / 10 |
| Database files identical | 10 / 10 | 10 / 10 |
| Compile time, ours vs Clang | 119.32 s vs 1.70 s (70.39x) | 141.92 s vs 24.38 s (5.82x) |
| Compile peak memory | ours 698.0 MiB (frontend), Clang 211.3 MiB | ours 698.0 MiB (frontend), Clang 273.6 MiB |
| Workload run time, ours vs Clang | 0.06 s vs 0.06 s (1.05x) | 0.05 s vs 0.06 s (0.89x) |

Per file: `sqlite3.c` frontend 112.22 s, peak 698.0 MiB, backend 1.07 s at `-O0` and 20.77 s at `-O2` (Clang compiling the C: 1.35 s / 21.06 s); `shell.c` frontend 5.74 s, peak 240.6 MiB, backend 0.28 s / 3.19 s (Clang: 0.34 s / 3.33 s). Workload run times are single runs of very short scripts and move a little from one run to the next. No inline assembly or other deferred GNU features were needed: hardware cycle counters in `hwtime.h` (inline `rdtsc`) are compiled only under flags that are off here. Interactive readline, zlib-backed recovery, and loading extensions from shared objects are not exercised.

**CI.** A new `sqlite shell workloads` job fetches the amalgamation (cached by version and both hashes), builds the compiler, and runs `python3 harness/run_sqlite.py --opt O0 --opt O2`. Locally the whole official run takes about two and a half minutes. The job has a 15-minute timeout.

Local results (rustc 1.85.1, Clang 19.1.7, GCC 14.2.0): `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **141 passed**, `python3 -m unittest tools/test_csmith_campaign.py tools/test_run_doom.py tools/test_run_lua.py tools/test_run_sqlite.py` OK, `python3 harness/run_tests.py` **179/179 passed** (new `tests/142_empty_declaration.c`, `tests/143_uint128.c`, `tests/144_uint128_constants.c`, `tests/145_uint128_more.c`, and `tests/146_uint128_va_regs.c`, each also matching Clang under `-std=c11`, plus `tests/gnu/13_int128_case_range.c`), `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**, `python3 harness/run_gcc_torture.py` **1326 passed, 420 known failures, 0 unexpected** (was 1320 / 426), `python3 harness/run_csmith.py` **34 passed, 6 known failures, 0 unexpected** (40 seeds), `python3 harness/run_doom.py` **DOOM: PASS** (all eight demos match Clang at `-O0` and `-O2`, 39,209 frames per level), `python3 harness/run_lua.py` **LUA: PASS**, and `python3 harness/run_sqlite.py` **SQLITE: PASS**.

## Compile-time performance

`tools/bench_compile.py` measures each frontend phase (`--time-phases`: lex, preprocess, parse, sema, irgen, write), the LLVM backend on our IR, and Clang compiling the same C at `-O0` and `-O2`, with peak memory, over `sqlite3.c`, `shell.c`, the Lua and DOOM files, and five torture files (median of three runs). The baseline before any change is frozen in `reports/perf/baseline.md`; the result is `reports/perf/scorecard.md`.

| | Before | After |
|---|---|---|
| `sqlite3.c` frontend | 111.35 s, 698 MiB | 1.34 s, 320 MiB |
| `sqlite3.c` total vs Clang, `-O0` / `-O2` | 90.4x / 6.46x | 1.77x / 1.03x |
| Lua (34 files) total vs Clang, `-O0` / `-O2` | 4.36x / 2.15x | 1.86x / 1.36x |
| DOOM (82 files) total vs Clang, `-O0` / `-O2` | 3.87x / 2.24x | 1.79x / 1.31x |

The cause was a quadratic preprocessor splice (each macro expansion moved the rest of the file's tokens), plus copying of tokens, file names, macro bodies, and struct member lists. The IR is byte-identical to the baseline compiler's on all 275 benchmark and `tests/` files after every fix. No optimiser was added. The remaining time is spread thinly over per-token allocation and type copying; see the scorecard notes. CI runs `tools/perf_guard.py` at the end of the SQLite job: it fails above 6x `clang -O0` (or 30 s) on `sqlite3.c` or `shell.c`. The DOOM, Lua, and SQLite scorecards were not re-measured and keep their recorded compile times.

## macOS on Apple Silicon, round 1 (branch `macos-arm64`)

Step 7 of `PLAN.md` has started, without a Mac. The compiler now takes `-target` (`--target`, `--target=`): `x86_64-linux-gnu` (the default on an x86_64 host), `arm64-apple-macos[VERSION]` (default macOS 11.0), and `aarch64-linux-gnu`. The x86_64 output is unchanged: on 2,172 inputs (`tests/`, GCC torture, Lua, SQLite, DOOM) the IR and exit status are byte-identical to `main` at `2f12890`.

What the arm64 targets do. Written from AAPCS64, Apple's "Writing ARM64 code for Apple platforms", the LLVM LangRef, and C11; Clang was used only as a black box (`-dM -E`, `-S -emit-llvm`).

- Triple and data layout: `arm64-apple-macosx11.0.0` with `e-m:o-i64:64-i128:128-n32:64-S128`, and `aarch64-unknown-linux-gnu`. Apple definitions carry `"frame-pointer"="non-leaf"`, as Clang's do.
- Predefined macros: `__APPLE__`, `__MACH__`, `__aarch64__`, `__arm64__`, the `__ARM_*` basics, `__USER_LABEL_PREFIX__` `_`, and `__ENVIRONMENT_OS_VERSION_MIN_REQUIRED__`; Linux gets `__linux__`, `__ELF__`, `__CHAR_UNSIGNED__`. `float.h` and `limits.h` follow `__LDBL_MANT_DIG__` and `__CHAR_UNSIGNED__`. Not defined: `__BLOCKS__`, NEON, Objective-C, `TARGET_OS_*`, the nullability keywords.
- Types: LP64 on all three. `long double` is `double` on Apple and IEEE binary128 (`fp128`) on AArch64 Linux. Plain `char` and `wchar_t` are unsigned on AArch64 Linux (character constants and `L"..."` arrays follow). `__builtin_bswap64` returns `unsigned long long` on Apple (`uint64_t`). On AArch64 Linux an unnamed bit-field raises the record's alignment to its declared type's (AAPCS64), so `struct { char a : 3; int : 0; char b; }` is 8 bytes there and 5 elsewhere.
- Calls: an HFA (one to four of the same floating type) is `[N x float|double|fp128]`; other composites up to 16 bytes are `i64`, `[2 x i64]`, or `i128` when 16-byte aligned, and a small result is the integer of its exact size (`i24`); larger composites are passed as a pointer to a caller copy and returned through `sret` (x8). Apple: `signext`/`zeroext` on narrow integers, every variadic argument on the stack, `va_list` is `char *`. AArch64 Linux: no extension, the 32-byte `va_list` with its general and FP save areas (implemented by hand, not with LLVM's `va_arg`), `alignstack` on HFAs, and a record's own `aligned` attribute does not choose a register pair. A call through a function without a prototype is a fixed-argument call on arm64 (a variadic call would put the arguments on the stack on Apple).
- Mach-O: an asm label is emitted as `\01label` so LLVM does not add a second `_`, and an ELF-style section name (no `segment,section` comma) is an error, as in Clang.

How it is tested (no Mac):

| Check | Result |
| --- | --- |
| `harness/run_arm64_ir.py`: lowered signatures of every function in the calling-convention battery and in every test program both compilers accept, against Clang, per target | Apple 434, AArch64 Linux 446 signatures, **0 differ** |
| same script: `llc -mtriple=... -filetype=obj` on this compiler's IR (Mach-O for Apple) | Apple **159 built**, 1 rejected by both compilers (ELF section name); Linux **158 built**, 2 rejected by both (`typedef int wchar_t`) |
| `harness/run_cross.py --target aarch64-linux-gnu`: `tests/*.c`, hosted, GNU, cross-ABI pairs (this compiler's object linked with Clang's both ways), `tests/arm64/`, static AArch64 glibc under `qemu-aarch64` | **178/181**, 3 known (two tests declare `wchar_t` as `int`; `0.1L` folded in double precision) |
| `harness/run_cross.py --target arm64-apple-macos`: the same programs through the Apple proxy below | **179/181**, 2 known (a test that dumps 10 bytes of `long double`; an ELF section name) |
| `harness/run_cross.py --torture` (report only, not in CI) | AArch64 Linux 1,328/1,746 pass, no failure outside the x86_64 known list; Apple proxy 1,320/1,746, 9 outside it, all proxy limits (glibc's `long double` functions and asm-label redirects) or `pr92904.c` (a variadic struct aligned to 32, which Clang's own Apple code also fails) |
| unit tests (`src/target_tests.rs`, `src/target.rs`, `src/aapcs64.rs`, driver) | target parsing, headers, macros, type model, aggregate lowering, prototype-less calls, `va_list`, bit-field layout, Mach-O names |

Review fixes on pull request 36 (CodeRabbit): a union whose alternatives overlap counts distinct positions toward the HFA limit (`union { float a[3]; float b[3]; }` is `[3 x float]`); the address of a function with an Apple asm label is quoted in the IR; `__builtin_isnormal` on binary128 uses its own minimum normal; a coerced load or store of a packed member uses alignment 1; `-target arm64-apple-macos13.2.1` (a non-zero patch level) is rejected instead of dropped; x86_64 cross headers are found on a non-x86_64 host; the harnesses fail on a compiler panic, a qemu start failure, or a time-out instead of comparing them; and the Apple stdio shim handles negative `*` precision, `%ls`/`%lc`, `%hn`-style counts, conversions longer than 4 KiB, and zero bytes. Objects with an asm label (`extern int v __asm__("x")`) still use their C name, as before on x86_64.

The Apple proxy: both compilers emit IR for `arm64-apple-macosx`; LLVM then builds it for `arm64-apple-darwin-elf`, which keeps Apple's calling convention but writes ELF, and the objects are linked with the AArch64 glibc and run under qemu. glibc's variadic `printf` family cannot be called with Apple's convention, so calls to it are renamed to a small Apple-convention shim (`harness/arm64/`).

Not covered without a Mac (at the end of round 1): the macOS SDK headers, Mach-O linking and the macOS C library, `long double` functions from Apple's libm, Lua/SQLite/DOOM on macOS, and the macOS CI job. The SDK headers, Mach-O linking, the C library, and the CI job are now covered by the macOS job below. AArch64 Linux `long double` constants are folded in double precision and then widened, so `0.1L` and `LDBL_MAX` are not exact there.

## macOS CI job (branch `macos-ci`)

The owner approved a short native macOS job. `.github/workflows/macos.yml` runs on GitHub's Apple Silicon runner (`macos-26`: macOS 26.7.2, Xcode 26.6, Apple Clang 21.0.0, the macOS 26.5 SDK) only on a push to `main` and on `workflow_dispatch`, never on pull requests or other branches, with a 15-minute timeout. It builds the compiler (no cargo cache: no dependencies, and the release build takes 7 s), runs `cargo test --lib`, builds and runs the guide's hello world, runs `harness/run_tests.py` with Apple's Clang as the oracle and `harness/macos_known_failures.txt`, and prints a report of which SDK headers compile (`harness/macos/sdk_headers.sh`, never fails the job). A run takes about 40 seconds.

Before review it ran four times on this branch. `workflow_dispatch` cannot start a workflow that is not yet on the default branch, so a temporary `push` trigger for `macos-ci` was used and removed in the last commit.

| Run | Result | Job time |
| --- | --- | --- |
| 1 | build and unit tests pass; hello world failed to link (`ld: library 'System' not found`: the toolchain Clang from `xcrun -f clang` has no SDK unless `SDKROOT` is set) | 28 s |
| 2 | hello world passes; `run_tests.py` 169/179 | 41 s |
| 3 | 172/179, 4 known | 38 s |
| 4 | **pass**: `run_tests.py` 175/179 with 4 known failures, 0 unexpected; unit tests 142 passed, 21 ignored; 64 of 68 header checks compile | 41 s |

Compiler fixes the real Mac found, each with a test that runs on Linux:

- `__has_include`, `__has_include_next` (searching like `#include`), and `__has_feature`, `__has_extension`, `__has_attribute`, `__has_c_attribute`, `__has_cpp_attribute`, `__has_declspec_attribute`, `__has_builtin`, `__has_warning`, `__building_module` (all 0, so headers take their plain-C path) in `#if`, and `defined` for each. A program's own macro of the same name wins. `__has_include(MACRO)` takes the file name from the macro, as `#include MACRO` does.
- GNU's named variadic macro parameter (`#define F(args...)`) is accepted in system headers (a header found in a system directory, or found next to a system header that includes it with `#include "..."`). With `__has_include` working, glibc's `<sys/stat.h>` now includes Linux's `<linux/stat.h>`, whose `<linux/stddef.h>` uses it; pull request CI caught this in the SQLite job. In a program's own code it is still an error (C11 6.10.3 has only `...`).
- `defined` produced by a macro in an `#if` line is evaluated (the SDK's `__is_modern_darwin` and pthread's nullability check use it). C11 leaves this undefined; GCC's manual documents the evaluation.
- The `__FLT_*__`, `__DBL_*__` and `__LDBL_*__` characteristics from GCC's list of predefined macros (`<math.h>` uses `__FLT_MIN__` and friends). Seven GCC torture tests that needed them now pass: **1,333 passed, 413 known failures** (was 1,326 / 420).
- `_Float16` is accepted in declarations as an incomplete type, because the SDK's `<math.h>` declares half-precision functions unconditionally. A `_Float16` object or value is an error.
- `<iso646.h>` was missing from the compiler's own headers.
- Unit tests no longer depend on the host: tests without a named target use x86_64 Linux (`Target::build_default()`), and the 21 that build and run x86_64 Linux code with the host Clang, or read glibc's `<math.h>`, are ignored on other hosts.
- `run_tests.py` accepts Apple Clang 16 or later (Apple numbers its Clang separately from LLVM) and an optional `KNOWN_FAILURES` file.

macOS known failures (Linux-specific tests): `112_longdouble.c` (dumps 10 bytes of an 8-byte `long double`), `116_uchar.c` (the SDK has no `<uchar.h>`), `121_signal.c` (glibc's `__sysv_signal`), `gnu/both/01_attributes.c` (an ELF section name).

Still not covered on macOS: `<complex.h>` (no complex types), `<uchar.h>` (not in the SDK), `_Float16` values, DOOM/Lua/SQLite and GCC torture on the Mac (a later manual-only option), and a person following `docs/USER_GUIDE_MACOS.md` on their own Mac.

## DOOM on a Mac (branch `doom-mac`)

Goal, approved by the owner: DOOM playable in a window on his Apple Silicon MacBook Air, built with this compiler, with a guide a non-programmer can follow ([docs/DOOM_ON_MAC.md](docs/DOOM_ON_MAC.md)).

- **`harness/doom/dg_sdl.c`** (this repository): a doomgeneric display backend for SDL2, following the shape of doomgeneric's own SDL backend (a test input). A resizable 4:3 window, DOOM's default keys plus W/A/S/D and E, the real clock; no sound (doomgeneric's sound needs SDL_mixer). Test settings: `DOOM_EXIT_AFTER_MS`, `DOOM_SCREENSHOT` (the last frame as a PPM), `DOOM_TEST_KEYS` (scripted presses through SDL's event queue: menu, New Game, episode, skill, then walk forward firing).
- **`tools/mac/play_doom.sh`**: checks for the Xcode Command Line Tools, Homebrew, Rust, and SDL2 (`sdl2-config`, also looked up under `/opt/homebrew` and `~/.cargo/bin` when a new Terminal has not got them on `PATH`) and prints the exact install line for each missing one; then builds the compiler, fetches doomgeneric (pinned) and Freedoom (SHA-256 checked), compiles DOOM's 80 C files and `dg_sdl.c` with `build/hcc`, turns the IR into objects and links with Apple's Clang and SDL2, and starts `freedoom1.wad` (or `--wad FILE`). Rebuilds only what changed.
- **Built with this compiler:** every C file, including SDL2's headers in `dg_sdl.c`. Apple's Clang only assembles the IR and links, as everywhere in this project. SDL2 is Homebrew's library.
- **Compiler change:** `__has_builtin(name)` answers 1 for the builtins this compiler implements (the inline builtins, the ones that forward to a library function, and the ones the parser handles by name), and 0 otherwise. SDL's `<SDL_endian.h>` otherwise falls back to x86 inline assembly for byte swaps. This change alone left the x86_64 IR byte-identical to `main` on 2,172 inputs.
- **Compiler change found by the first Mac run:** `inline` was ignored, so every function the macOS SDK's headers define as `extern __inline __attribute__((__gnu_inline__))` (`isupper`, `toupper`, `__sputc`, `__inline_isnanf`, `__sincos`, ...) became a strong definition in every file that included them, and DOOM failed to link with 50 duplicate symbols. Now C11 inline definitions (6.7.4p7) and `gnu_inline` `extern inline` definitions are emitted `available_externally`. `-fgnu89-inline` selects GNU89 semantics (and `__GNUC_GNU_INLINE__`); `run_gcc_torture.py` passes it to the 12 tests whose `dg-options` ask for it, so torture stays at 1333 passed / 413 known. The only other x86_64 IR change on the 2,172 inputs is `stdarg-4.c`, whose `inline` `always_inline` helpers are now inline definitions, as C11 says.
- **Portability of the headless harness:** `dg_alloc.c` uses `MAP_FIXED_NOREPLACE` where it exists and otherwise passes the arena address as a hint and checks it (macOS); `run_doom.py` hashes WADs with `hashlib`; `fetch_freedoom.sh` falls back to `shasum -a 256`.
- **macOS workflow:** a `doom` input on `workflow_dispatch` adds a job (never on push to `main`): `run_doom.py --opt O0 --opt O2` against Apple's Clang, `brew install sdl2`, `./tools/mac/play_doom.sh` with `SDL_VIDEODRIVER=dummy` and the test settings, and a non-failing launch in a real window. It uploads the scorecard, logs, and screenshots.

Native Mac results (GitHub's `macos-26` Apple Silicon runner, Apple Clang 21.0.0, macOS 26.5 SDK, Homebrew `sdl2` = `sdl2-compat` 2.32.74), dispatched by hand on this branch:

| Run | Head | Result | Wall time (native job, DOOM job) |
| --- | --- | --- | --- |
| 1 | `7836346` | native job passes. DOOM: every file compiles, but the link fails with 50 duplicate symbols from the SDK's inline header functions (fixed above); Apple Clang's own build plays all demos | 78 s (52 s, 75 s) |
| 2 | `57dd90d` | **pass**: frame hashes 39,209/39,209 match Apple Clang at `-O0` and at `-O2` (all eight Freedoom demos to their end); `./tools/mac/play_doom.sh` builds and runs with the dummy driver (516 frames in 8 s, the scripted keys start E1M1 and fire); a real Cocoa window opened on the runner's desktop and drew 349 frames in 6 s (screenshot in the run's `doom-macos` artifact) | 111 s (42 s, 108 s) |

| 3 | `e230fd5` | **pass** on the PR's first head: 39,209/39,209 at `-O0` and `-O2`; dummy driver 516 frames; the real window (now raised with `SDL_RaiseWindow`, so it is the front app and gets the keyboard) drew 345 frames | 114 s (50 s, 110 s) |

The billing API reports 0 billable ms for these runs, as for the earlier macOS runs; rounded up per job they are 3 macOS minutes each (about 30 Linux-equivalent minutes each).

Review fixes after CodeRabbit's review of `e230fd5`:

- A function definition without an `__asm__` label now keeps the label of an earlier file-scope declaration (GCC manual, "Asm Labels"); before, `int f(void) __asm__("renamed"); int f(void) { ... }` defined `@f` while calls went to `@renamed`. The inline bookkeeping is per C name, and only the linker name the definition is emitted under is marked, so another C name sharing that name through an asm label is not affected. x86_64 IR on the 2,172 inputs is unchanged by this.
- `__has_builtin` also answers 1 for `__builtin_inf`, `__builtin_huge_val`, `__builtin_nan` and their variants.
- `dg_sdl.c`: a full key queue no longer looks empty; when full, a press is dropped and a release replaces the oldest event, so no key stays held.
- `play_doom.sh`: a relative `DOOM_BUILD` is made absolute before the game starts in its saves folder, and cached objects are rebuilt when the optimisation level, Clang, or SDL flags change.

Local results on Linux x86_64 (Clang 19): DOOM headless 39,209/39,209 frames match at `-O0` and `-O2`; the SDL2 build with SDL 2.32.4's headers and the dummy video driver starts, takes the scripted keys, starts E1M1, and the player fires (ammo 50 to 49 in the saved frame); the full build takes about 10 s.

**Accepted on the owner's Mac.** On 2026-11-16 the owner downloaded and built DOOM with this compiler on his Apple Silicon MacBook Air, the setup [docs/DOOM_ON_MAC.md](docs/DOOM_ON_MAC.md) describes, and played it by hand. That accepts the macOS Apple Silicon port (step 7).

## Quality phase (step 8)

Measure first, then improve. Part A (branch `quality-baseline`) only measures; it changes no compiler code. The full baseline, with the raw data and how to rerun it, is in [reports/quality/baseline-2026-11/README.md](reports/quality/baseline-2026-11/README.md); the scripts are in `tools/quality/`. None of these tools run in CI.

Baseline on `f11131e` (non-test code in `src/`, 22,466 instrumented lines):

- **Coverage** (`cargo llvm-cov`, line and function; branch coverage needs a nightly toolchain): unit tests alone 85.1% of lines and 93.7% of functions; the main differential suite, WACC, and GCC torture alone 80.9% and 88.1%; together 89.6% and 95.5%. `sema.rs` 89.2%, `parser.rs` 91.3%, `irgen.rs` 92.5%.
- **Size and complexity** (`tools/quality/complexity.py`; rust-code-analysis-cli would not build on the box): 1,152 functions; 26 over 100 lines and 2 over 200 (`sema.rs` `check_stmt` 246, `irgen.rs` `gen_expr` 210); 17 with cyclomatic complexity over 25.
- **Crash points** (`unwrap`, `expect`, `panic!`, `unreachable!`; no `todo!` or `unimplemented!`): 333. (a) true invariant 293 (104 checked a few lines earlier, 189 relying on an earlier phase), (b) reachable from bad input 0, (c) unclear 40 (37 of them in `irgen.rs`, trusting `sema.rs`). 225,740 fuzz runs (217,632 damaged C programs from the test corpora plus the 8,108 unmodified seeds) ended with no panic, crash or hang. 18 crash points are on lines no test runs.
- **Mutation testing** (`cargo-mutants`, unit tests only, a bounded sample of 794 mutants in about 1.5 hours over `irgen.rs`, `sema.rs`, `parser.rs` and `pp.rs`): 513 caught, 230 missed, 13 timeouts, 38 unviable: 69% caught (513 of the 743 that completed), or 70% if timeouts count as caught. The main differential suite caught only 4 of 20 sampled survivors; 6 of the other 16 are ARM64 or Apple-only paths.

**Part B: internal errors instead of crashes, tighter unit tests** (branch `quality-b-diagnostics`; details and before/after numbers in [reports/quality/phase-b/README.md](reports/quality/phase-b/README.md)):

- A later phase that finds a case an earlier phase should have rejected now prints `file:line:column: internal compiler error: <message> (in irgen)` and exits with status 3 (`diag::EXIT_INTERNAL_ERROR`) instead of panicking. `irgen::render` returns `Result`; inside the large `Gen` functions the first error is recorded with its source position and lowering finishes with a placeholder, so their signatures are unchanged until part C.
- All 40 unclear crash points are gone: 33 became internal errors, 6 became `if let` / `let ... else` on a check made just before, and `sema::integer_ty` was folded into `require_integer`. Crash points 333 → 284; `assert!`s 7 → 1.
- One was reachable: with `-std=gnu11`, a `case` or `default` label of a switch inside a statement expression crashed the compiler. It is now the error `'case' label jumps into a statement expression from its switch` (GCC manual, "Statements and Declarations in Expressions"; GCC and Clang reject it too). `break` and `continue` out of a statement expression stay valid (`tests/gnu/14_switch_in_statement_expr.c`).
- 29 new unit tests (167 → 196): internal errors from deliberately damaged syntax trees, and exact IR or run output for code the part A mutation sample showed untested. On the 17 targeted functions the unit tests catch 72% of planted bugs, up from 46%; 50 of the 54 sample misses there are now caught.
- No output change for valid programs: the old and new compilers agree byte for byte on 13,148 compilations of every `.c` file in the test suites, in both language modes.

**Part C: split big files and functions** (branch `quality-c-split`; details and before/after numbers in [reports/quality/phase-c/README.md](reports/quality/phase-c/README.md)):

- `sema.rs`, `parser.rs`, `irgen.rs`, `pp.rs` and `ast.rs` (8,766, 7,786, 6,528, 3,819 and 3,559 lines) are now the module directories `src/sema/`, `src/parser/`, `src/irgen/`, `src/pp/` and `src/ast/`, each split in its own commit that only moves code. The largest file in `src/` is now `lexer.rs` (1,535 lines); the largest from the split is `sema/init.rs` (1,385). `src/README.md` lists the new files.
- The 14 functions over 120 lines (the longest were `check_stmt` at 246 and `gen_expr` at 216) were broken into named helpers. Now no function is over 120 lines, 16 are over 100 (was 27) and none is over 200 (was 2). 13 have cyclomatic complexity over 25 (was 17) and none is over 50 (was 2).
- Statement lowering in `irgen` returns `Result<(), Ice>`, so an internal error in a statement stops lowering of its function. Expression lowering still records the first error and returns a placeholder. Two new unit tests (196 → 198) check which error is reported.
- No output change: 0 differences in 13,148 compilations, old and new compiler. Crash points are the same 285 sites, and mutation testing on part B's 17 targeted functions misses exactly the same 87 mutants as before (221 caught, 0 timeouts). Code review found three wrong-code defects that were already on `main` in the moved code: a pointer-to-`__int128` static initializer, floating compound assignment to a bit-field, and re-initializing a whole subobject with a designator. They and the smaller findings were fixed in the follow-up below.

**Phase C follow-up: fix the bugs found during review** (branch `fix-review-bugs`; details in [reports/quality/phase-c-followup/README.md](reports/quality/phase-c-followup/README.md)):

- **The three review bugs are fixed:**
  - `int g; __int128 a = (__int128)&g;` is now a diagnostic. An address converted to a pointer-sized integer is still accepted (6.6p10).
  - Compound assignment to a bit-field writes only that bit-field, and converts the result to the bit-field's declared type, so `_Bool` and `long long` bit-fields are right too (6.5.16.2p3).
  - A designated brace list for a whole subobject replaces earlier initializers inside it (6.7.9p19).
  - Tests: `tests/147`, `tests/148`, and sema unit tests.
- **All 17 smaller CodeRabbit findings on pull request 45 were real and are fixed, each with a test.** Phase C's report said 15; one of the 18 was already fixed there. They cover:
  - preprocessor: `#line` digit sequences, unsigned character constants in `#if`, and a `##` from a macro argument
  - static initializers: `&1[a]`, and an address converted to `_Bool`
  - `void *` with function pointers in assignment and `?:`: an error in strict mode, a warning with `-std=gnu11`
  - the composite type of a block-scope function redeclaration
  - an enum specifier as the type specifier, and `_Noreturn` / `inline` only on functions
  - three typing rules for unevaluated operands
  - GNU `?:` with an omitted middle operand in floating constant expressions
  - `va_arg` of a 16-byte-aligned one-SSE-eightbyte record on SysV
  - checked bit-field offsets in record layout
- **Output changes.** Old and new compilers have the same exit status and IR on all 13,148 compilations. Messages differ in 4 of them, all from the `void *` / function pointer rule: two torture files fail earlier in strict mode and get one more warning in GNU mode. SQLite, Lua and DOOM give identical IR. `sqlite3.c` and two DOOM files get new GNU-mode warnings.
- **Review of the follow-up** found and fixed five more: a `signed char` bit-field read as unsigned on AArch64 Linux (already on `main`), a saturating static bit position past 512 MiB, `(_Bool)&g` in a static initializer, `?:` with `(void *)0` not taking the other pointer's type (already on `main`), and the parser's `?:` typing of `void` and record operands.
- **Unit tests:** 198 → 209. **Harness:** 187/187.
- **Seen, not fixed** (none is wrong code):
  - a pointer is rejected for a `_Bool` bit-field
  - `# ##` in a function-like macro is accepted
  - `(int *)(__int128)&g` at file scope is accepted (as GCC does)
  - a bit-field past bit 2^32 is an implementation limit

**Part D: stage dumps and a test-case reducer** (branch `quality-d-tooling`; details in [reports/quality/phase-d/README.md](reports/quality/phase-d/README.md); how to use them in [docs/DEBUGGING.md](docs/DEBUGGING.md)):

- **Stage dumps.** `--dump-tokens`, `--dump-ast`, `--dump-sema` and `--dump-ir` stop after that stage and print it (standard output, or `-o FILE`); `--dump-pp` is another spelling of `-E`. The sema dump shows each expression's type, implicit conversions, resolved names, member offsets and bit-field positions, folded `case` values and checked initializers, then record layouts, globals with their constant values, string literals and external declarations. The formats are deterministic and documented.
- **Golden tests.** Four programs in `harness/dumps/` with the expected dump of every stage, checked by `harness/run_dump_tests.py` (24 checks, including `--dump-ir` = `-o` output and repeatability). Unit tests 209 → 213.
- **Reducer.** `tools/reduce/reduce.py`: ddmin (Zeller and Hildebrandt 2002, own implementation) over lines, top-level declarations, block statements and comma lists, then tokens, to a fixpoint, with timeouts (killing the whole process group), output and file-size limits, and a cache. Modes `ice`, `crash`, `miscompile` (guarded against undefined behaviour by a sanitizer build and UB warnings), `reject`, `accept`, `bad-ir` and `cmd`. 37 unit tests.
- **Demo on real bugs** from before pull request 46 (`0cab101` compiler): the bit-field `+=` miscompile went from 118 lines to 14, and the `__int128` address initializer (invalid IR) from 39 lines to 2.
- **CI:** the dump golden tests and reducer tests run in the Linux `check` job; no macOS minutes.
- **No output change:** old and new compilers agree on exit status, messages and IR for all 13,172 compilations (6,586 files: the earlier 6,574, the 8 test files from pull request 46 and the 4 dump programs), and SQLite, Lua and DOOM give identical IR. Harness 187/187.

**Step 8 is complete.** Before and after, over parts A to D: unclear crash points 40 → 0 (333 → 285 sites), functions over 200 lines 2 → 0 and over 120 lines 14 → 0 (one, `fold_one`, is at 123 after pull request 46), largest file 8,766 → 1,535 lines, planted bugs caught in the 17 targeted functions 46% → 72%, unit tests 167 → 213, and stage dumps and a reducer for diagnosis; output byte for byte unchanged except the deliberate fixes of the follow-up.

## Linux on AArch64 (step 9, branch `linux-arm64`)

Target `aarch64-linux-gnu` (AAPCS64, binary128 `long double`, unsigned
`char`). It is the default target on an AArch64 Linux host. Every suite
that runs on x86-64 now runs for AArch64 against Clang for the same target.
Every row was checked natively on GitHub's arm64 runner (pull request 49)
and locally under qemu. Details, known-failure reasons and the CI cost
estimate are in
`reports/platforms/linux-arm64/README.md`.

| Suite | AArch64 Linux |
| --- | --- |
| Unit tests | 200 passed, 23 ignored (x86-64-host only), on the arm64 runner |
| Differential harness (with `tests/arm64/`) | 190/190 |
| `run_cross.py --target aarch64-linux-gnu` (qemu, x86-64 runner) | 190/190, 0 known failures |
| Random calling-convention pairs (`tools/abi_fuzz.py`) | 1,000/1,000 seeds |
| Writing a C Compiler | 1,376 passed, 0 failures (Clang oracle; 8 x86-64 assembly tests skipped) |
| Csmith, 40 fixed seeds | 34 pass, 6 known (same as x86-64), 0 unexpected |
| GCC torture execute | 1,335 passed, 411 known, 0 unexpected |
| One True AWK | agrees with the Clang build; bugs-fixed 31/31 |
| AWK Builder suites | all 528 cases agree between the builds (516/528 pass natively); volume test PASS |
| Lua 5.4.7 | matches Clang at `-O0` and `-O2` |
| SQLite 3.54.1 | 10/10 workloads match at `-O0` and `-O2` |
| DOOM | 39,209/39,209 frames match at `-O0` and `-O2` |
| Performance numbers | `harness/perf.py` on the arm64 runner |

Bugs fixed: exact binary128 constant folding (`src/fp128.rs`, `src/bigint.rs`;
`0.1L` was folded as `double`), 16-byte alignment of large arrays applied
on AArch64 (an x86-64-only rule), and, on x86-64, a struct with a bit-field
in a nested struct that starts mid-eightbyte was passed in memory (found by
the new `tools/abi_fuzz.py`), and a zero-width bit-field there claimed an
INTEGER eightbyte (found in review). The x86-64 output is otherwise unchanged: on
13,342 compilations (tests, WACC, torture, One True AWK, Csmith runtime and
the 2,000-seed campaign, the v0.6 challenge, Lua, SQLite, DOOM; C11 and
GNU modes) the only IR difference is that fix, in one Csmith program
(`O0-part1/90` and `O2-part1/90`, the same source).

CI: `.github/workflows/linux-arm64.yml` on GitHub's `ubuntu-24.04-arm`
runner ($0.005 a minute for private repositories, against $0.006 for x64).
A quick job (unit tests, harness, dumps, reducer tests, 100 ABI pairs) runs
on pull requests and pushes to `main`; the suites run in two jobs nightly (02:30 UTC, only when `main` changed in the last 25 hours) and by hand.

## macOS on Intel (step 10, branch `macos-x86_64`)

`-target x86_64-apple-macos[VERSION]` (default 10.15; the host target on an Intel Mac). It uses System V AMD64 with Mach-O names, and the Darwin differences were checked against Clang. Done the low-cost way the owner approved: the checking runs on Linux, and a short Intel Mac job (`macos-26-intel`) runs only when started by hand. The owner's Mac is Apple Silicon, so he cannot hand-test on Intel hardware. Full report: `reports/platforms/macos-x86_64/README.md`.

Intel Mac scorecard (manual run [44317168313](https://github.com/spacefilm900/c-compiler/actions/runs/44317168313) at `a1497fc`, Apple clang 21.0.0 as the oracle):

| Suite | Intel Mac result |
| --- | --- |
| Unit tests | 209 passed, 23 ignored (need an x86-64 Linux Clang) |
| Differential harness | 189/192, 3 known (`116_uchar.c`, `121_signal.c`, `gnu/both/01_attributes.c`), 0 unexpected |
| Lua 5.4.7 | builds alone; test suite (three modes), 9/9 workloads, `luac -s` 42/42 match at `-O0` and `-O2` |
| SQLite 3.54.1 | builds alone; 10/10 workloads and database files match at `-O0` and `-O2` |
| DOOM | builds alone; 39,209/39,209 frame hashes match at `-O0` and `-O2` |
| GCC torture execute | 1,334 passed, 412 known, 0 unexpected (x86-64 list plus the macOS overlay) |

A fifth run ([44325320398](https://github.com/spacefilm900/c-compiler/actions/runs/44325320398), at the review-fix head `044672b`) was cancelled during DOOM to stay within the budget. Before that it passed the unit tests (214), the harness (190/193, 3 known, 0 unexpected, including the new `xabi/09` against Apple clang), Lua, SQLite and the header report (100 of 108). DOOM and torture were not run natively at that head.

Linux checks for the target: 461 signatures match Clang, and llc builds Mach-O objects for 164 programs. The Linux stand-in passes 192/193 harness tests (1 known), torture has 0 failures outside the x86-64 list, and random ABI pairs pass (1,000/1,000 plus 300/300 at the review fixes, and the same with `--int128`).

Mac minutes: five manual runs, about 59 to 60 billed minutes (about $3.70), at the approved ceiling; no more are planned. Bugs fixed:

- Two x86-64 `__int128` ABI bugs, which affect Linux too: a record holding `__int128` went in memory, and a scalar `__int128` was counted as one register.
- `__llvm__` and `__LITTLE_ENDIAN__` for the SDK headers.
- Apple `<limits.h>` and `#include_next`.
- In GNU mode, enumerators outside `int` and fixed underlying enum types, with the review follow-ups (the `enum E : 3` bit-field, implicit constant types, values over 64 bits, an incomplete enum inside its own list).
- Intel macOS argument rules found in review: a lone X87UP eightbyte is an SSE `double`, and a union holding a flexible-array struct goes in memory.
- `darwinN` triples map to their macOS version, and only one compiler header tree is searched.

Linux x86-64 old-vs-new: 13,362 compilations, 18 differences, all from these fixes or the new tests.

## 64-bit Windows (step 11, branch `windows-x64`)

`-target x86_64-w64-windows-gnu` (MinGW-w64) and `-target x86_64-pc-windows-msvc`: LLP64 types, the Microsoft x64 calling convention (`src/win64.rs`), Microsoft bit-field layout, COFF names, the Windows predefined macros, Microsoft's C keywords and `__declspec` on MSVC, and MSVC's COMDAT rules for `inline` and `selectany`. Done the low-cost way of step 10: the checking runs on Linux, and a `windows-latest` job (`.github/workflows/windows.yml`) runs only by hand (`gh workflow run windows.yml --ref BRANCH [-f target=msvc|mingw]`, or "Run workflow" in the Actions tab). Full report: `reports/platforms/windows-x64/README.md`.

| Check | Result |
| --- | --- |
| Signatures against Clang (Linux) | MinGW 473, MSVC 449, 0 differ |
| llc COFF objects (Linux) | MinGW 160, MSVC 154, 0 failed |
| Harness under Wine, MinGW-w64 | 182/193, 11 known (LP64 tests), 0 unexpected |
| Harness under Wine, MSVC stand-in | 180/193, 13 known, 0 unexpected |
| Random ABI pairs under Wine | 100/100 each target, plain and `--int128` |
| GCC torture under Wine (MinGW, local) | 1,335/1,746; 4 beyond the x86-64 list (2 need `<sys/mman.h>`, 2 fail with Clang too) |
| Windows runner, MinGW-w64 (run 44537682786, `a125bfc`) | unit tests; harness 181/193, 12 known, 0 unexpected; ABI pairs 50/50 |
| Windows runner, MSVC + UCRT + `link.exe` (same run) | hello world; harness 178/193, 15 known, 0 unexpected; ABI pairs 50/50 |

Windows minutes: seven manual runs, about 40 runner minutes, about 80 billed (private repository, 2x), under the 120 ceiling. Not run natively: torture, Lua, SQLite, DOOM, WACC, Csmith, AWK. Changes that reach every target: `__typeof__` in strict C11, the `_Pragma` operator, function-like `-D`, and an aligned typedef of a record defined in the same declaration (GCC torture `20050215-1.c` now passes). Old-vs-new IR on the four existing targets: 1,883 files each, the only difference is that torture file.

## Required C11 features (step 12a, branch `c11-required-features`)

The four features C11 requires that were still missing after step 11, written from N1570 and checked against `clang -std=c11` as a black box.

- **`_Alignas` (6.7.5) and `<stdalign.h>` (7.15).** `_Alignas(type-name)` and `_Alignas(constant-expression)` apply to objects and to struct or union members. A specifier of 0 has no effect; any other value must be a power of two, not larger than 2^28, and not less than the type's `_Alignof`. It is a diagnostic on a typedef, a function, a bit-field, a `register` object, or a parameter. The requirement is lowered to LLVM `align N` on globals and `alloca`, and it is combined into record layout, padding, `sizeof`, and `_Alignof`. Several alignment specifiers in one declaration take the strictest value (6.7.5p6). If the definition has an alignment specifier, any other declaration specifies equivalent alignment or none; if the definition has none, no other declaration may have one (6.7.5p7). `<stdalign.h>` defines `alignas`, `alignof`, `__alignas_is_defined`, and `__alignof_is_defined`. A record that contains a member whose LLVM type does not carry the C alignment (for example a nested struct with `_Alignas(32)`, which is flattened to `[N x i8]`) is itself lowered as a byte array of the C size, so member offsets and array stride stay correct (6.2.8, 6.7.5, 6.7.2.1). `tests/152_alignas.c`, `tests/hosted/06_stdalign.c`, `tests/157_alignas_nested.c`, `tests/158_alignas_bitfield.c`. A bit-field may also sit next to an over-aligned ordinary member: extra alignment inserts padding after the bit-field's storage unit, which does not move, and the record is accepted (6.7.2.1, 6.7.5). A GNU packed record that would move a bit-field is still rejected.
- **`_Thread_local` (6.7.1, 6.2.4p4).** It may stand alone at file scope, or be combined with `static` or `extern` at file or block scope. It is a diagnostic on a function, a typedef, `auto`, `register`, a member, or a block-scope variable without `static` or `extern`. Every declaration of the object must include it (6.7.1p3). It is lowered to LLVM `thread_local` (and `external thread_local` for `extern _Thread_local`). `<threads.h>` is provided (see "Threads" below). `tests/153_thread_local.c` is a single-thread check; `tests/posix/01_tls_threads.c` (Linux) and `tests/posix/darwin/01_tls_threads.c` (macOS) use POSIX threads and are run only by the native harness (`-pthread`), not by the Windows Wine stand-in. They declare the thread functions themselves, with each fixture using that platform's `pthread_t`, because glibc's `<pthread.h>` uses a GNU `aligned` typedef whose size is not a multiple of its alignment, which this compiler does not accept.
- **Old-style K&R function definitions (6.9.1p6).** `ret fn(a, b) int a; double b; { ... }` is accepted, including `register` on a parameter declaration. Every identifier in the list must be declared (no implicit int, matching C11 and Clang/GCC in `-std=c11`); a name that is not in the list, a duplicate name, a non-`register` storage class, or an initializer is a diagnostic. A non-empty identifier list on a non-definition is still rejected (6.7.6.3p3). Compatibility with a prior prototype follows 6.7.6.3p15: the parameter count agrees, and each prototype parameter is compatible with the default-promoted identifier type. The definition's LLVM signature uses the promoted types; the body converts each parameter to its declared type, so a `float` parameter is passed as `double` and narrowed, and `char`/`short` are passed as `int` and narrowed (6.5.2.2). `tests/154_kr_defs.c`. Thirty-two GCC torture `old-style-decl` tests and `pr68532.c` (`alignas`) now pass; the known-failures list is 379.
- **`<tgmath.h>` (7.25).** A compiler-provided header selects the `float`, `double`, or `long double` function with `_Generic`. An integer argument is treated as `double`; a multi-argument macro takes `long double` over `double` over `float` (7.25p3). The header includes only `<math.h>`, not `<complex.h>`, because `__STDC_NO_COMPLEX__` stays defined. All of the real-floating type-generic macros listed in 7.25 are provided. `tests/hosted/05_tgmath.c`. The Intel Mac sysroot stand-in lists that test as an oracle-only known failure: Linux Clang cannot compile the SDK `<tgmath.h>`; a failure of this compiler on that target is still unexpected.

## Variable-length arrays (step 12b, branch `vla`)

C11 VLAs and variably modified types (6.7.6, 6.7.6.2, 6.7.6.3, 6.5.3.4p2, 6.2.4, 6.8.6.1p1, 6.8.4.2p2), written from N1570 and checked against `clang -std=c11` as a black box.

Block-scope VLA objects are allocated with a dynamic LLVM `alloca`. `llvm.stacksave` is taken at each VLA object declaration (not at the start of the enclosing block) and `llvm.stackrestore` runs when execution leaves that declaration's scope, so a loop of many iterations reuses the stack, and `goto` back to a label before the declaration, `break`, `continue`, and `return` free the storage they leave. Multidimensional VLAs and pointers to VLA types use runtime element sizes for indexing and pointer arithmetic. `sizeof` of a VLA type is evaluated at run time and evaluates its operand (6.5.3.4p2); `_Alignof` of an array is still the element alignment and does not evaluate a size expression. A VLA parameter (`int n, int a[n][n]`) is adjusted to a pointer to a VLA, but the unadjusted dimensions are still evaluated on entry, including side effects such as `int array[i++]`. `[*]` is accepted only in a prototype. A typedef of a VM type evaluates the size expression at the typedef. Size-expression side effects occur once when control reaches the introducing declarator, typedef, or `sizeof` type name; a later `sizeof` of that object or typedef uses the recorded size.

Diagnostics: no VM type at file scope, no static or `_Thread_local` VLA object, no VM identifier with linkage, no VM struct or union member, no initializer on a VLA, no jump into the scope of a VM identifier (goto or switch), and no VM type in a `_Generic` association (6.5.1.1p2). `__STDC_NO_VLA__` is no longer predefined. `tests/155_vla.c`.

Nine GCC torture execute tests that were listed under `vla` now pass (known failures 379 after step 12a → 370; 1376 passed, 1746 executed, 0 unexpected): `20040411-1.c`, `20040811-1.c`, `20221006-1.c`, `920929-1.c`, `970217-1.c`, `pr22061-2.c`, `pr43220.c`, `pr77767.c`, and `vla-dealloc-1.c`. Twelve VLA-tagged tests remain: they use a GNU nested function, an undeclared `alloca`, or a variably modified struct or union member, which C11 forbids.

Local results on rustc 1.85.1 and Clang 19.1.7: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **266 passed**, `make` produced `build/hcc`, `python3 harness/run_tests.py` **200/200 passed**, `python3 harness/run_gcc_torture.py` **1376 passed, 370 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped**.


## Atomics (step 12c, branch `atomic`)

C11 `_Atomic` and `<stdatomic.h>` (6.7.2.4, 6.7.3, 6.2.6.1p9, 6.5.16.2, 6.5.2.4, 7.17), written from N1570, the LLVM Language Reference, the published GCC/libatomic memory_order ABI, and the public GCC manual. Clang is used only as a black-box oracle.

`_Atomic` is a type qualifier, including after `*`, and `_Atomic(type-name)` is a type specifier. The specifier rejects an array, function, atomic, or qualified type name. The qualifier cannot modify a function type or appear on a bit-field. Qualifiers on an array still apply to the element (6.7.3p9). Plain access to an atomic object is a `memory_order_seq_cst` load or store. Compound assignment and `++`/`--` are a single read-modify-write: integer add, sub, and, or, and xor use LLVM `atomicrmw`; other operations, pointers, and floating types use a compare-exchange loop. Floating and 16-byte `long double` objects, and records, are accessed as an integer of the same size, because LLVM does not accept `cmpxchg` of `float`/`double` on x86 or `load atomic` of a struct or `x86_fp80`.

Lock-free sizes are 1, 2, 4, 8, and 16 bytes on every supported target, including the Windows Wine stand-in. An `_Atomic` type is aligned to at least its size, so LLVM can lower it without a lock. A packed atomic member that would be under-aligned, and a larger or incomplete atomic type, is a diagnostic. There is no locking runtime, and libatomic is not used. On x86-64, a function that uses a 16-byte atomic carries `"target-features"="+cx16"` in the IR, so `clang -w output.ll -o program` selects `cmpxchg16b` instead of a `__atomic_*_16` libcall. `__STDC_NO_ATOMICS__` is no longer predefined.

`<stdatomic.h>` provides the `memory_order` enumeration (values 0 through 5, matching the published GCC/libatomic ABI), the `ATOMIC_*_LOCK_FREE` macros (all 2), `ATOMIC_VAR_INIT`, `atomic_init`, `kill_dependency`, the fences, `atomic_is_lock_free`, the generic load/store/exchange/compare-exchange/fetch_* functions and their `_explicit` forms, `atomic_flag` with `ATOMIC_FLAG_INIT`, and the `atomic_int` typedefs (including the `least`/`fast`/`intptr`/`size`/`ptrdiff`/`intmax` names). `<stdint.h>` is included when `__has_include` finds it, so the typedefs match the C library; otherwise they use fundamental types of the same width, with `atomic_intptr_t` and `atomic_uintptr_t` following `ptrdiff_t` and `size_t`. The generics expand to `__hcc_atomic_*` builtins of this compiler's own design. `memory_order_consume` is lowered as acquire (LLVM has no consume), except `atomic_flag_clear` lowers a constant consume as relaxed (C11 7.17.8.2 allows consume, and the operation has no load). A `memory_order` argument that is not a constant is still evaluated, then lowered as `seq_cst`. `atomic_signal_fence` uses LLVM `fence syncscope("singlethread")`.

`tests/156_atomic.c`, `tests/hosted/07_stdatomic.c`, and `tests/posix/02_atomic.c` (Darwin fixture `tests/posix/darwin/02_atomic.c`).

Local results on rustc 1.85.1 and Clang 19.1.7: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **298 passed**, `make` produced `build/hcc`, `python3 harness/run_tests.py` **203/203 passed**, `python3 harness/run_gcc_torture.py` **1376 passed, 370 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped** (same as after step 12b; no listed torture test started passing).


## Threads (step 12d, branch `threads`)

C11 `<threads.h>` (7.26), written from N1570. Clang is used only as a black-box oracle. Clang, GCC, musl, glibc, and winpthreads source were not consulted.

On a Linux target, if the C library already provides `<threads.h>` (glibc 2.28 and later), `include/threads.h` includes it with `#include_next`, so typedefs and enumeration values come from the platform. Link with `-pthread`. The compiler does not link a second runtime, and `lib/threads.c` is a no-op if it is compiled on such a target. `#include_next` is gated on `__linux__`, so an Apple or Windows compile that happens to see a Linux sysroot does not pick up glibc typedefs.

macOS has no C library `<threads.h>`. MinGW-w64 on the Windows stand-in also has none. For those targets, and for a Linux C library without the header, this compiler supplies typedefs, macros, and prototypes in `include/threads.h`, and `lib/threads.c` is a small original runtime over pthreads or the Win32 thread API (SRW locks, condition variables, and `CreateThread`). Compile and link `lib/threads.c` with the program, and on POSIX also pass `-pthread`. The runtime does not include `<pthread.h>`, because this compiler rejects glibc's aligned typedef of pthread typedefs; it declares the needed POSIX functions itself and stores mutexes and condition variables in oversized, aligned buffers so the same object type works on Linux and macOS. Recursive and timed mutexes are implemented on top of a non-recursive platform lock. `call_once` uses C11 atomics. Thread-specific storage destructors run at thread exit for up to `TSS_DTOR_ITERATIONS` (4) rounds. Enumeration constants are used by name; their numeric values are not specified by the standard and may differ from glibc's.

`__STDC_NO_THREADS__` is no longer predefined. The coverage is the C11 API: `thrd_*`, `mtx_*`, `cnd_*`, `tss_*`, `call_once` / `ONCE_FLAG_INIT`, the `thread_local` macro, `TSS_DTOR_ITERATIONS`, and the result enumerations. `tests/hosted/08_threads.c` is a multithreaded program with deterministic output, compared with Clang on Linux (the system header) and compiled against the shim on macOS and Windows, including the Wine stand-in and the arm64-apple-macos Linux stand-in. That proxy keeps Apple's calling convention but cannot use Darwin TLS relocations against glibc, so `harness/run_cross.py` compiles the IR with `-femulated-tls` (stand-in only; a real Mac uses native Darwin TLS). The crash was not a pthread typedef-size mismatch.

Local results on rustc 1.85.1 and Clang 19.1.7: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **298 passed**, `make` produced `build/hcc`, `python3 harness/run_tests.py` **204/204 passed**, `python3 harness/run_gcc_torture.py` **1376 passed, 370 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped** (same as after step 12c; no listed torture test started passing).

## Adversarial challenge v0.7 (branch `adversarial-v07`)

A second adversarial round (HCC_Challenge_v0.7, 65 gated cases plus one non-gating probe) was run against `main` `4e69525`. The first run at both `-O0` and `-O2` was **64/65**. The only failure was HCC07-001, over-aligned nested record arrays: `_Alignas(32)` on a member makes the inner struct an LLVM `[64 x i8]` (alignment 1), so a regular enclosing LLVM struct placed that member at offset 1 and used an element stride of 66, while `sizeof` and `_Alignof` were already the correct 128 and 32 (C11 6.2.8, 6.7.5, 6.7.2.1). Clang 19.1.7 on this host passed all 65 cases at both levels, as did the release's GCC 14 and Clang 17 references. The three two-TU cases passed with mixed hcc/Clang in both directions. All seven strict-mode GNU rejections passed.

The enclosing record is now also flattened to a correctly sized byte array, and members are addressed by byte offset. After the fix the same suite is **65/65** at `-O0` and `-O2`. The non-gating probe `p += 2` on `_Atomic(int *)` is accepted and executed by this compiler and by GCC 14, and rejected by Clang 19 (the same divergence as Clang 17 in the release); it is not in the 65-case denominator (6.5.16.2, 6.5.6).

Local results on rustc 1.85.1 and Clang 19.1.7: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **299 passed**, `make` produced `build/hcc`, `python3 harness/run_tests.py` **205/205 passed**. The same layout fix makes GCC torture `20010904-1.c` and `20010904-2.c` pass (they check the stride of an array of structs that contain a 32-aligned member type), so the execute list is **1378 passed, 368 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped** (was 1376 / 370).

## Adversarial challenge v0.8 (branch `adversarial-v08`)

A third adversarial round (HCC_Challenge_v0.8, 80 gated native cases, plus mixed-ABI, thread stress, a cross-target object matrix, and non-gating probes) was run against `main` `e2cf13f`. The first run at both `-O0` and `-O2` was **76/80**. The four failures, HCC08-002, HCC08-006, HCC08-010, and HCC08-014, are one family: a bit-field followed by an `_Alignas` member (alignments 8, 16, 32, and 64). The frontend rejected these valid C11 programs with "bit-fields in a packed or aligned struct are not supported" (6.7.2.1, 6.7.5). The same rejection failed all six `nested-16` cells of the cross-target matrix. Clang 19.1.7 and GCC 14.2 on this host passed all 80 cases at both levels. The 12 mixed-TU ABI cases passed with Clang and with GCC as the complementary compiler, in both directions, at both levels (and the GCC/Clang mixed oracle was 48/48). The eight thread cases passed, including 20-repetition stress at both levels (160/160 each).

The record-layout guard now rejects a bit-field in an irregular layout only when a bit-field storage unit actually moved or would extend past the record. Extra alignment that only inserts padding is accepted. After the fix the same suite is expected to be **80/80** at `-O0` and `-O2`; packed bit-field records remain unsupported. Regression: the unchanged v0.6 (724) and v0.7 (65) suites are rerun at both levels as separate evidence.

Local results on rustc 1.85.1 and Clang 19.1.7: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **300 passed**, `make` produced `build/hcc`, `python3 harness/run_tests.py` **206/206 passed**, GCC torture execute **1378 passed, 368 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped**.

## Next actions

The testing stage is in place, and unmodified One True AWK builds with this compiler. The two awk binaries agree on `bugs-fixed` and on the upstream `testdir`, including the golden-file lines the Clang build also fails. The small C11 gaps in step 1 of `PLAN.md` are implemented: flexible array members, `u`/`U`/`u8` literals, `long double` aggregate returns, and file-scope initializers of a nested bit-field struct. Eight more gaps followed: `sizeof` in integer constant expressions, compound literals, non-prototype `int f();`, `_Static_assert` / `_Generic` / `_Alignof`, one `declare` per asm label, brace elision after a designator, UTF-8 and UCNs in literals, and prefixed character constants in `#if`. The four required C11 features of step 12 (`_Alignas` and `<stdalign.h>`, `_Thread_local`, old-style K&R definitions, and `<tgmath.h>`) are implemented, variable-length arrays (step 12b) are implemented, and `_Atomic` and `<stdatomic.h>` (step 12c, branch `atomic`) are implemented, and `<threads.h>` (step 12d, branch `threads`) is implemented. `__STDC_NO_VLA__`, `__STDC_NO_ATOMICS__`, and `__STDC_NO_THREADS__` are no longer defined. `_Complex` remains optional. The adversarial C11 challenge phase is closed: v0.6 was accepted as the final planned adversarial release, with 724/724 at both `-O0` and `-O2`, and pull request 27 merged (`d728a94`). The Csmith coverage step is done: a 2,000-seed campaign produced 1,492 valid differential comparisons at `-O0` and 1,490 at `-O2`, and `tools/csmith_campaign.py` always links with `-lm`. Its two defects are fixed (a struct-return overrun at `-O0`, and GNU-mode pointer comparisons and assignments), and the rerun has 1,821 and 1,822 valid comparisons, all in agreement, with no rejects. The GNU-compatibility cheap tier is implemented (see above; torture known failures 827 → 432). DOOM (doomgeneric) builds with this compiler alone and matches Clang frame for frame at `-O0` and `-O2` over all eight Freedoom demos, 78,418 frames (see above). Lua 5.4.7 builds with this compiler alone (`-std=gnu11`) and matches Clang at `-O0` and `-O2`: the official test suite in three modes, nine workloads byte for byte, and `luac` bytecode (see above). It needed labels as values and computed `goto`, the first middle-tier GNU feature, which are now implemented. SQLite 3.54.1 builds with this compiler alone (`-std=gnu11 -D_DEFAULT_SOURCE`, with FTS5, RTree, and the math functions) and matches Clang at `-O0` and `-O2` on ten deterministic SQL workloads, including the database files (see above). It needed empty file-scope declarations and 128-bit integers, both now accepted in either language mode. The official TCL suite is not used. The compile-time performance phase is done: `sqlite3.c`'s frontend went from 111 s to 1.3 s with identical output (see above). macOS on Apple Silicon (step 7): round 1 (pull request 36) is merged, and a native macOS CI job (main and manual runs only) passes on a real Apple Silicon runner (see above); DOOM on the Mac is done, and on 2026-11-16 the owner built and played it by hand on his MacBook Air, which accepts the port. The quality and maintainability phase (step 8) is under way: part A measured a baseline, and part B replaced the unclear crash points with internal compiler errors and tightened the unit tests (see "Quality phase" above). Part C split the largest files and functions with no change in output (pull request 45, merged). The follow-up fixed the three wrong-code bugs and 17 smaller findings that review found (pull request 46, merged). Part D added stage dumps, a test-case reducer and `docs/DEBUGGING.md`, which completes step 8. Step 9, Linux on ARM, runs every suite for `aarch64-linux-gnu` (see "Linux on AArch64" above). Step 10, macOS on Intel, passed every suite it runs on a real Intel Mac runner, started by hand (see "macOS on Intel" above). Step 11, 64-bit Windows, is merged (pull request 52, `8ed66ea`): MinGW-w64 and MSVC pass the harness and random ABI pairs on a real Windows runner, started by hand (see "64-bit Windows" above). Step 12a's four required C11 features are implemented (see "Required C11 features" above). Step 12b, variable-length arrays, is implemented (see above). Step 12c, `_Atomic` and `<stdatomic.h>`, is implemented (see above). Step 12d, `<threads.h>`, is implemented (see above). The remaining middle-tier GNU features (inline assembly, vectors) stay on demand and nested functions out of scope.

## Eight C11 gaps record

Eight standard C11 gaps found by the GCC torture triage, each written from N1570 and checked against `clang -std=c11` as a black box. No torture test shaped the code; the torture list only shrank.

1. **`sizeof` in integer constant expressions** (6.6p6). Array bounds, enumeration values, and bit-field widths are folded by the parser, which now keeps each ordinary identifier's declared type per scope, so `int g[sizeof a / sizeof a[0]]`, `sizeof p->d`, `sizeof f(x)`, and `sizeof (1 ? a : 0)` fold. An object is in scope after its declarator (6.2.1p7), so `int x[sizeof x];` is an undeclared identifier, and a length inferred from an initializer (`int t[] = {...}`) is visible to a later `sizeof t`. `sizeof` of a function type, an incomplete type, or a bit-field is diagnosed (6.5.3.4p1). Case labels and static initializers already used semantic analysis. `tests/119_sizeof_ice.c`.
2. **Compound literals** (6.5.2.5). `(T){...}` is an unnamed lvalue whose address may be taken. In a function body it is an automatic object in the entry block, initialized each time the literal is evaluated. Outside a function it has static storage and a constant initializer, so `int *p = (int[]){1, 2};` is an address constant. An unknown-size array takes its length from the list. Designators, structs, unions, `sizeof (T){...}`, and postfix operators work. A function type or an incomplete type is a diagnostic. `tests/123_compound_literal.c`.
3. **Non-prototype declarations** (6.7.6.3p14, p15, 6.5.2.2p6). See Limitations. `tests/120_nonproto.c`.
4. **`_Static_assert`, `_Generic`, `_Alignof`.** `_Static_assert` (6.7.10) works at file scope, in blocks, and in member lists, and requires the message. `_Generic` (6.5.1.1) converts the controlling expression as an lvalue (qualifiers dropped, arrays and functions to pointers), selects the compatible association or `default`, and diagnoses a duplicate `default`, two compatible association types, an incomplete or function association type, and no match. The selected expression keeps its lvalue-ness. Every association expression is analyzed for constraints and name resolution; only the selected one is evaluated and lowered to IR. `tests/128_generic_unselected.c`. `_Alignof(type-name)` is a `size_t` constant; an array type has its element's alignment. `_Alignof expr` is not C11 and is a diagnostic. `<stdalign.h>` and `max_align_t` are supplied. `__STDC_NO_COMPLEX__` is predefined as 1, because `_Complex` is not implemented. Variable-length arrays, `_Atomic`, and `<threads.h>` are implemented, so `__STDC_NO_VLA__`, `__STDC_NO_ATOMICS__`, and `__STDC_NO_THREADS__` are not defined. `_Thread_local` is implemented, and so is `<threads.h>` (see "Threads" above). `__STDC_UTF_16__` and `__STDC_UTF_32__` were already 1 and are now backed by UTF-16 and UTF-32 encoding. `tests/124_generic_assert.c`.
5. **One `declare` per linker symbol.** Several C names may share an asm label (glibc's `signal` is `__sysv_signal` in strict C mode). The module now emits one `declare` per LLVM symbol, and none for a symbol that is defined. `tests/121_signal.c`.
6. **Brace elision after a designator** (6.7.9p17, p20). `{ .m = 1, 2 }`, where `m` is a struct, union, or array, starts at the first scalar of `m`, and later positional items continue there, also when sizing `T a[] = { [1] = 3, 4 }`. A union nested in a static initializer is folded as its object representation, so a designated member other than the first (`{ .u.d = 2.5 }`) is no longer dropped. `tests/125_designator_elision.c`, `tests/126_designator_arrays.c`.
7. **UTF-8 and universal character names in literals** (5.2.1, 6.4.3, 6.4.4.4, 6.4.5). A non-ASCII source character or `\uXXXX` / `\UXXXXXXXX` is a code point. A UCN below U+00A0 other than `$`, `@`, and `` ` ``, a surrogate, or a value above U+10FFFF is a diagnostic. Strings are UTF-8 (unprefixed and `u8`), UTF-16 with surrogate pairs (`u`), or UTF-32 (`U` and `L`). A character constant takes one code unit: `'Ä'` and `u'😀'` are diagnostics, as in Clang. Numeric escapes are still single code units with the old range checks. `tests/122_utf8_ucn.c`.
8. **Prefixed character constants in `#if`** (6.10.1p4). The preprocessor evaluates `L'\400'`, `u'...'`, and `U'...'` as the phase-7 lexer does, with the prefix's type, so `L'\400' == 256` and `U'\xffffffff' > 0`.

Local results on rustc 1.85.1 and Clang 19.1.7: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **91 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **138/138 passed**, and `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**. `python3 harness/run_gcc_torture.py` was **900 passed, 846 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped**, against 828 passed and 918 known failures on `main`: 72 listed tests now pass and were removed (51 `old-style-decl`, 20 `compile-error`, 1 `long-double`). `python3 harness/run_onetrueawk.py` built unmodified awk; `bugs-fixed` is 31 programs and the two binaries agree, and `testdir` shows the same nine golden-file mismatches for both builds. Csmith could not be built on the agent VM (no C++ standard library), so its result is from CI.

Negative cases are unit tests: `sizeof` of a function, a bit-field, or an incomplete type in a bound; `int x[sizeof x]`; `int g(); int g(char);`, `g(float)`, `g(int, ...)`, and `int g() {} int g(int);`; a failed `_Static_assert` in each position and one without a message; `_Alignof` of `void`, an incomplete struct, a function type, or an expression; each `_Generic` constraint, including an undeclared identifier, a wrong-arity call, and a nested selection in an unselected association; a non-constant file-scope compound literal, one in a `static` local initializer, and an incomplete one; excess items after an elided designator and an address in a nested union; and each invalid UCN, an incomplete UCN, and a character too large for its constant.

Review fixes after CodeRabbit review `5857062872` on `e1c2526`. An automatic aggregate initializer now performs its stores after the zero fill in initializer order, so `{ .s = (struct S){1, 9}, .s.x = 2 }` keeps `.s.x` (`tests/127_init_order.c`). A record value in such an initializer is copied to a temporary when it is evaluated (review `5857267560` on `44c1dbf`), so `{ .a = (s = x), .b = (s = y) }` keeps `x` in `.a`. The identifier-list guard tests for any type specifier, so `void f(__builtin_va_list);` parses. `#if L'u'` is a signed `int`: only a number's spelling is scanned for a `u` suffix. Tests 119 to 126 declare their library functions in the file and use no headers or directives; `<stdalign.h>` and `max_align_t` are checked by a preprocessor unit test instead. The finding that `int h() {}` should not convert to `int (*)(int)` was not taken. C11 6.7.6.3p15 makes those types incompatible, but Clang accepts the conversion with no diagnostic, and this compiler has no warnings, so an error would reject programs Clang builds (`ieee/fp-cmp-1.c` to `-3.c` pass `leave()` to `signal`). A redeclaration `int h() {} int h(int);` is still a diagnostic.

## Unselected `_Generic` associations

C11 6.5.1.1p3 says unselected association expressions are not evaluated. They are still assignment-expressions, so 5.1.1.3 requires a diagnostic for a syntax or constraint violation in them (an undeclared identifier, 6.5.1p2 and footnote 91; a prototyped call whose argument count is wrong, 6.5.2.2p2). Semantic analysis now checks every association expression, then replaces the `_Generic` node with the selected expression so later passes emit no IR for the others. The parser also checks associations when it folds `_Generic` in an integer constant expression (an enumerator, array bound, bit-field width, or `_Static_assert`), which would otherwise discard the node before semantic analysis. Local results on rustc 1.85.1 and Clang 19.1.7: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **92 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **139/139 passed**.

## Unary `!` in integer constant expressions

The parse-time integer constant expression evaluator used by `_Static_assert`, enumerators, array bounds, and bit-field widths treated `!` as the operand's truth value instead of logical negation. `_Static_assert(!0, "ok")` was rejected, and `enum { E = !0 }` was 0. C11 6.5.3.3p5: the result of the `!` operator is 1 if the operand compares equal to 0, and 0 otherwise, and the result has type `int`. Short-circuit `&&` and `||` already skipped the unselected operand (6.5.13, 6.5.14) and were unchanged. File-scope initializers, `case` labels, and `#if` already evaluated `!` correctly and still agree. Regression tests were committed first (`2c50832`) so the before state is recorded; the new unit test and `tests/129_logical_not_ice.c` failed on that commit and pass after the fix (`15e1861`). They cover `_Static_assert`, enumerators, array bounds, bit-field widths, nested and multiple assertions, repeated inversion (`!!0`, `!!1`, `!!!0`, `!5`, `!!5`), and `!(0 && (1/0))` / `!(1 || (1/0))`. Preprocessor `#if !0` / `#if !1` are covered in `src/pp.rs`; the harness test does not use directives.

## Hidden extern type in a later file-scope initializer

C11 6.2.7p4 forms a composite type only when a prior declaration of the identifier is visible. After a block-scope `extern int a[1];` ends, a later file-scope `int a[] = { [0 && sizeof a] = 7 };` sees `a` with this declarator's type `int[]` (6.2.1p7), so `sizeof a` is a diagnostic (6.5.3.4p1). The previous code composed the hidden `int[1]` in before shaping, which wrongly accepted that program. Clang `-std=c11` rejects it, and also rejects the same designator on a plain `int a[]` with no prior declaration. The file entity is still reconciled after the initializer is shaped: a compatible hidden size is composed, and an initializer that completes `int a[]` to a different length keeps the later type (a hidden size is not a same-scope constraint under 6.7p4). `tests/130_hidden_extern_incomplete.c` checks compatible completions against Clang: a hidden `extern int b[3]` then `int b[] = {1, 2, 3}`, a hidden `extern int c[]` then `int c[] = {1, 2}`, and `{ [0 && sizeof a[0]] = 7 }` on a later `int a[]` (the identifier is in scope as incomplete `int[]`, and the element type is complete). Mismatched hidden lengths stay as unit tests, because incompatible types for the same object are undefined (6.2.7p2) and Clang is not a valid oracle for them.

Local results on rustc 1.85.1 and Clang 19.1.7: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **93 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **141/141 passed**, and `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**. The independent C11 challenge v0.2 is **206/206**, including reverse-direction cross-ABI on the 15 `cross-abi` cases. The v0.1 suite remains **87/87**. Review fixes after CodeRabbit on `53516a1` moved `#if !0` coverage to the preprocessor unit tests and restricted the hidden-extern differential test to compatible bounds, as described above.

## GCC floating-point format builtins for `<math.h>`

glibc's `<math.h>` expands `NAN` to `(__builtin_nanf (""))`, `INFINITY` to `(__builtin_inff ())`, and `HUGE_VAL` / `HUGE_VALF` / `HUGE_VALL` to `__builtin_huge_val` / `__builtin_huge_valf` / `__builtin_huge_vall` when `__GNUC_PREREQ (3, 3)` holds, which it does because this compiler predefines `__GNUC__` 4.2.1. Those names were not recognized, so a valid program that used `NAN` was rejected (independent C11 challenge C11-230). C11 7.12p4–p5: `INFINITY` and `NAN` are constant expressions of type `float`. 7.12p3: `HUGE_VAL` is a positive `double` constant, `HUGE_VALF` a `float` constant, and `HUGE_VALL` a `long double` constant. 6.3.1.2: a NaN converted to `_Bool` is 1. The compiler now recognizes the GCC floating-point format builtins documented in the GCC manual ("Floating-Point Format Builtins"): `__builtin_nan` / `nanf` / `nanl`, `__builtin_nans` / `nansf` / `nansl`, `__builtin_inf` / `inff` / `infl`, and `__builtin_huge_val` / `huge_valf` / `huge_vall`. `inf` and `huge_val` take no arguments and yield +infinity of the documented type. `nan` and `nans` take an ordinary string literal, parsed as by `strtol` with base 0 (empty, decimal, octal, or hexadecimal, consumed in full); the payload is placed in the least significant bits of the significand and forced quiet or signaling. A non-literal argument is a diagnostic. The result is a constant expression, usable in a static initializer, and is emitted as an LLVM hex floating constant (`0x` for `float`/`double`, `0xK` for 80-bit `long double`, LLVM Language Reference, "Simple Constants"). Regression tests were committed first (`aa3c431`) so the before state is recorded; the new unit tests and `tests/hosted/01_math_macros.c` failed on that commit (undeclared `__builtin_nanf`) and pass after the fix. The hosted test includes system `<math.h>` and checks types via `_Generic`, sizes, static-storage initializers, `_Bool` conversion of `NAN`, and runtime arithmetic, differentially against Clang. Harness tests from 119 onward in `tests/*.c` still do not use headers or directives.

Local results on rustc 1.85.1 and Clang 19.1.7: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **97 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **142/142 passed**, and `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**. The independent C11 challenge v0.3 is **336/336** at both `-O0` and `-O2`, including both ABI directions. The inherited v0.1 and v0.2 cases are included in that total.

An audit of common hosted headers, done after the fix and not itself a change, is recorded in the pull request. Under `__GNUC__` 4.2.1, `isnan` / `isinf` / `isfinite` / `fpclassify` / `isnormal` expand to libc helpers and compile; `signbit` already used `__builtin_signbit*`. The C11 comparison macros `isgreater` and friends already expand to `__builtin_isgreater` and are still unrecognized. Raising `__GNUC__` would also switch the classification macros to `__builtin_isnan` and similar names. Those findings are left for a later batch.

Review fixes after the first implementation. `check_uneval` now applies the same string-literal argument rule as semantic analysis, using one shared helper, so `_Static_assert(__builtin_nanf(0), "x")` and the same call in an enumerator or array bound (including under `sizeof` or `_Generic`) are diagnosed. The NaN tag is parsed only up to the first NUL, so `__builtin_nan("1\0ignored")` is a constant with payload 1. Widening a `float` signaling NaN to the LLVM 64-bit hex constant uses integer bit operations; host `f64::from(f32)` would quiet `__builtin_nansf`. Five GCC torture `ieee/` tests that now pass were removed from the known-failure list, as recorded under GCC torture execute.
Local results on rustc 1.85.1 and Clang 19.1.7 after those review fixes: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **99 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **142/142 passed**, and `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**. The independent C11 challenge v0.3 is **336/336** at both `-O0` and `-O2`, including both ABI directions. `python3 harness/run_gcc_torture.py` was **905 passed, 841 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped**, after removing the five `ieee/` tests listed under GCC torture execute.

A further review fix after CodeRabbit: a `__builtin_nan*` tag that is not consumed in full no longer becomes a payload-zero NaN in a runtime context. The same `nan_tag_from_expr` helper is used by constant evaluation and IR lowering. A valid tag still folds to a constant. A malformed quiet tag in a function is lowered to a call of the C library `nan` / `nanf` / `nanl` (C11 7.12.11.2), declaring the function if the unit does not already have that symbol. A malformed `__builtin_nans*` tag is a diagnostic, because there is no library equivalent. Static initializers and other constant-expression contexts still diagnose a malformed tag as not a constant. The whole-tag rule, matching Clang 19 as a black-box oracle and documented from the GCC manual ("Floating-Point Format Builtins"), is: the bytes up to the first NUL must be a (possibly empty) unsigned integer literal (`strtol` base 0: leading `0` or `0x`/`0X`), with no sign and no whitespace, consumed in full. Tags such as `""`, `"0"`, `"1"`, `"0x10"`, and `"010"` fold; `" 1"`, `"1xyz"`, and `"-1"` do not. `tests/hosted/02_runtime_nan_tag.c` checks that a runtime `__builtin_nan("1xyz")` (and the `nanf` / `nanl` forms) returns a NaN.

Local results on rustc 1.85.1 and Clang 19.1.7 after this review fix: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **100 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **143/143 passed**, and `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**. The independent C11 challenge v0.3 is **336/336** at both `-O0` and `-O2`, including both ABI directions. `python3 harness/run_gcc_torture.py` was **905 passed, 841 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped**.

## v0.4 repairs: `offsetof`, comparison builtins, and library-call symbols

The independent C11 challenge v0.4 first run (476 cases, at `e87dca9`) failed 20 cases at both `-O0` and `-O2`. The independent review accepted two root causes and asked for a third, deferred fix. Each fix has a failing-tests commit first, then the fix.

**`offsetof` as an integer constant expression (C11-457 to C11-466).** C11 7.19p3 makes `offsetof(type, member-designator)` an integer constant expression of type `size_t`. Our `<stddef.h>` used the null-pointer idiom, which neither constant evaluator folds, so `offsetof` failed in `_Static_assert`, enumerators, array bounds, `case` labels, and static initializers. `<stddef.h>` now expands it to `__builtin_offsetof(type, member)`. The parser reads the designator (an identifier, then `.identifier` and `[expression]`) and folds the offset from the record layout into an integer constant, so every constant-expression context accepts it (6.6p6). Nested, anonymous, and union members and constant subscripts are supported. A bit-field member, an incomplete or non-record type, an unknown member, and a subscript of a non-array are diagnostics. A subscript that is not constant (a GNU extension) is evaluated at run time as `size_t` integer arithmetic (the folded offset plus each such index times its element size) and is not a constant expression. Tests: `tests/131_offsetof_ice.c` (the builtin, no headers) and `tests/hosted/03_offsetof_constant.c` (the macro) compare each folded value with the run-time address difference.

**Floating-point comparison builtins (C11-467 to C11-476).** glibc `<math.h>` expands the C11 7.12.14 macros `isgreater`, `isgreaterequal`, `isless`, `islessequal`, `islessgreater`, and `isunordered` to the GCC builtins of the same names (GCC manual: "Other Builtins" in GCC 14, "Library Builtins" in the current online manual). Semantic analysis requires two arithmetic operands, at least one real floating, and the result is `int`. IR lowering converts both to their common real type (6.3.1.8) and emits one `fcmp` with `ogt`, `oge`, `olt`, `ole`, `one`, or `uno`, zero-extended to `int` 0 or 1. LLVM LangRef `fcmp` does not signal, the ordered predicates are false for a NaN, and `uno` is true exactly for a NaN, which is the quiet macro behaviour. Integer-only, pointer, and structure operands are diagnostics, as in Clang 19 and GCC 14 (black-box). A call is not a constant expression, as in Clang 19. Tests: `tests/132_fp_compare_builtins.c` (direct calls with NaN, both infinities, signed zeros, and mixed `float` / `double` / `long double` / integer operands) and `tests/hosted/04_fp_compare_macros.c` (the macros, with `FE_INVALID` checked clear after comparing quiet NaNs).

**Library-call symbol clashes (deferred CodeRabbit finding from PR 23).** LLVM IR has one namespace for globals and functions. IR lowering calls `nan`, `nanf`, or `nanl` for a malformed quiet `__builtin_nan*` tag, and LLVM may lower `llvm.memcpy`, `llvm.memmove`, and `llvm.memset` (which aggregate copies and the memory builtins use) to calls of `memcpy`, `memmove`, and `memset` (LLVM LangRef); its optimizer may introduce those calls too. Those six are the library calls the generated code makes without the source naming them. `static int nan;` plus a malformed tag wrote IR that could not be assembled, and `static int memmove;` plus a 4096-byte structure copy crashed at run time because the backend's call bound to the local object. An internal-linkage name cannot meet the library at link time (6.2.2p3), so IR lowering now renders the function bodies, learns which library calls they make, and gives each internal object with one of those names, and each internal object or defined function named `memcpy`, `memmove`, or `memset`, a private IR name such as `@nan.1`. Every reference follows. Nothing changes when there is no clash, and a user-defined `nan` function stays the call target. An object with external linkage and such a name is undefined behaviour (7.1.3p1-p2); when a runtime malformed tag would call `nan`, `nanf`, or `nanl` and the unit has an object of that name with external linkage (file-scope or block-scope `extern`, or a definition without `static`, before or after the call), semantic analysis reports an error at the builtin call naming the object and its declaration. `sizeof` operands, `_Generic` controlling expressions, and unselected `_Generic` associations make no call and are not reported. Tests are unit tests in `src/irgen.rs` (assembled and run) and `src/sema.rs`, plus `tests/133_internal_libcall_names.c`.

Local results at `f1cb3f6` (first push), on rustc 1.85.1 and Clang 19.1.7: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **107 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **148/148 passed**, `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**, `python3 harness/run_csmith.py` **25 passed, 15 known failures, 0 unexpected, 40 seeds**, `python3 harness/run_gcc_torture.py` **917 passed, 829 known failures, 0 unexpected** after removing twelve now-passing entries, `python3 harness/run_onetrueawk.py` `bugs-fixed` 31 programs agree and `testdir` shows the same nine golden-file mismatches for both builds, and `python3 harness/perf.py` reported 0 result mismatches and 0 timeouts on 139 harness programs and built and ran One True AWK. The independent C11 challenge v0.4 is **476/476** at both `-O0` and `-O2` (was 456/476), including 45/45 `cross-abi` cases in both directions. Its two GNU `nan` symbol probes now behave as agreed: `internal_nan.c` assembles and exits 0 at `-O0` and `-O2`, and `external_nan.c` is rejected with the clash diagnostic and writes no IR.

Review fixes after CodeRabbit review on `f1cb3f6` (five findings, all taken; failing tests committed before each behavioural fix):

1. **Runtime `offsetof` from a null base (Major).** The non-constant fallback built `&((type *)0)->designator` and emitted `getelementptr inbounds` on a null base with a nonzero offset, which the LLVM LangRef defines as poison. It is now `size_t` arithmetic: the folded offset of the constant steps plus each run-time index converted to `size_t` times its element size, lowered as `sext`/`zext`, `mul`, and `add` on `i64`, with no address formed from null. Tests: an IR unit test in `src/irgen.rs` (no `ptr null`, assembled and run) and `tests/134_offsetof_runtime_index.c` (signed, unsigned, narrow, and wide indices, nested and mixed constant/run-time subscripts, checked against real address differences).
2. **Unchecked subscript in a folded `offsetof`, and 3. comparison builtin operands in parser-folded constants.** Enumeration values, array bounds, bit-field widths, `_Static_assert`, and folded `offsetof` subscripts are evaluated by the parser and never reach semantic analysis, so a `sizeof` operand there was not checked against the comparison constraints. The parser's constraint pass now applies the relational and equality operand rules (C11 6.5.8p2, 6.5.9p2: two arithmetic operands, two object pointers for a relational operator, two pointers or a pointer and a null pointer constant for equality) and the comparison builtin rule (two arithmetic operands, at least one real floating), the same rules semantic analysis applies. `enum { N = sizeof(__builtin_isgreater(1, 2)) };` and `sizeof(1 < (int *)0)` in those contexts are now diagnostics. Parser unit tests.
4. **`nan` calls in a `_Generic` controlling expression.** The controlling expression is not evaluated (C11 6.5.1.1p3), so the library-call records it adds are discarded after it is checked, as for unselected associations. `_Alignof` takes only a type name here. Sema unit test.
5. **Test comment.** `tests/132_fp_compare_builtins.c` now says the builtins take two arithmetic operands, at least one real floating.

Local results on the final head (rustc 1.85.1, Clang 19.1.7): `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **111 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **149/149 passed**, `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**, `python3 harness/run_csmith.py` **25 passed, 15 known failures, 0 unexpected, 40 seeds**, `python3 harness/run_gcc_torture.py` **917 passed, 829 known failures, 0 unexpected** (no new now-passing entries), `python3 harness/run_onetrueawk.py` `bugs-fixed` 31 programs agree and `testdir` shows the same nine golden-file mismatches for both builds, and `python3 harness/perf.py` reported 0 result mismatches and 0 timeouts on 140 harness programs and built and ran One True AWK. The independent C11 challenge v0.4 is **476/476** at both `-O0` and `-O2` (was 456/476), including 45/45 `cross-abi` cases in both directions, and the two GNU `nan` symbol probes behave as before.

## v0.5 repairs: restrict return types and parser-folded constraints

The independent C11 challenge v0.5 first run (636 cases, at `0f22ba2`) failed one case, C11-614, at both `-O0` and `-O2`. The independent review approved that fix and the two CodeRabbit findings deferred from PR 24. Each fix has a failing-tests commit first, then the fix.

**`restrict` on a function return type (C11-614).** C11 6.7.3p2 allows `restrict` only on a pointer to an object type. The check already covered objects, typedefs, and parameters, but `int restrict f(void);` was accepted. The same check now runs on the return type of every function declarator where the declarator is derived, so file-scope and block-scope declarations, definitions, nested declarators such as `int restrict (*f(void))(void)`, and abstract declarators in type names are covered. `int *restrict f(void)` and restrict-qualified pointer parameters stay valid. Tests: parser unit test `restrict_function_return_requires_pointer` and `tests/135_restrict_pointer_return.c`.

**Pointer compatibility in parser-folded comparisons (CodeRabbit, PR 24, `src/parser.rs` near line 4259).** The parser's constraint pass for enumeration values, array bounds, bit-field widths, `_Static_assert`, and folded `offsetof` subscripts accepted any two pointers in an equality or relational operator, so `sizeof((int *)0 == (double *)0)` was accepted there. It now uses the pointer rules semantic analysis uses, moved to `src/ast.rs` and shared: pointers to compatible types ignoring pointee qualifiers, or one `void *` for an equality (C11 6.5.8p2, 6.5.9p2). Clang 19 and GCC 14 reject every case in the test (black-box). Test: parser unit test `constant_contexts_check_comparison_pointer_compatibility`.

**Builtin result types in parser-folded constants (CodeRabbit, PR 24, `src/parser.rs` near line 4285).** The parser did not know the result types of `__builtin_signbit*` (`int`) or `__builtin_memcpy`, `__builtin_memmove`, and `__builtin_memset` (`void *`). A nested call such as `__builtin_isgreater(__builtin_signbit(1.0), 2)` escaped the comparison builtin operand check, a valid `sizeof(__builtin_signbit(1.0))` in an enumeration value was rejected as an undeclared identifier, and the builtins' own operand errors carried that same message. The parser now gives every supported builtin call the result type semantic analysis gives it and checks the signbit operand (arithmetic) and the memory builtin operands (pointer destination, pointer source or integer memset value, integer size) by the rules semantic analysis applies. Clang 19 and GCC 14 agree on every case in the test except `sizeof(__builtin_signbit(1.0) == (int *)0)`, which Clang rejects and GCC accepts; a call is not an integer constant expression (6.6p6), so the result is no null pointer constant and this compiler rejects it, as semantic analysis already did in a function body. Tests: parser unit test `constant_contexts_type_nested_builtin_calls` and `tests/136_builtin_result_types.c`.

Local results on the final source (rustc 1.85.1, Clang 19.1.7): `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **114 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **151/151 passed**, `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**, `python3 harness/run_csmith.py` **25 passed, 15 known failures, 0 unexpected, 40 seeds**, `python3 harness/run_gcc_torture.py` **917 passed, 829 known failures, 0 unexpected** (no now-passing entries, so the known-failure list is unchanged), `python3 harness/run_onetrueawk.py` `bugs-fixed` 31 programs agree and `testdir` shows the same nine golden-file mismatches for both builds, and `python3 harness/perf.py` reported 0 result mismatches and 0 timeouts on 142 harness programs and built and ran One True AWK. The independent C11 challenge v0.5 is **636/636** at both `-O0` and `-O2` (was 635/636), including 65/65 `cross-abi` cases in both directions; C11-614 is the only case whose result changed.

Review fixes after CodeRabbit review on `3164855` (three Minor findings; one taken, two not taken):

1. **`restrict` on a pointer to a function (taken).** `int (*restrict pick(void))(void);` was accepted. C11 6.7.3p2 requires the referenced type to be an object type, so the restrict validation now rejects a restrict-qualified pointer to a function with "restrict requires a pointer to an object type". The check is shared, so objects, parameters, typedefs, and return types are all covered. Clang 19 and GCC 14 reject every case in the test (black-box). Failing test committed first. Parser unit test `restrict_requires_pointer_to_object_type`.
2. **`void *` relational comparisons (not taken).** The finding asked to reject `(void *)1 < (void *)2` because `void` is not an object type. In C11 `void` is an incomplete object type (6.2.5p19), so two `void *` operands satisfy 6.5.8p2. Clang 19 and GCC 14 accept the comparison with `-pedantic-errors`. No change.
3. **A `void *` compared for equality with a function pointer (not taken in this pull request).** 6.5.9p2 allows a `void *` only with a pointer to an object type, so `v == f` for `void *v` and a function `f` is a constraint violation. Semantic analysis and the parser both accept it today, through the shared `void_ptr_mix` rule. A fix that tightens that rule, and still accepts a null pointer constant such as `(void *)0`, was written and tested but held back: GCC torture `20041212-1.c` compares `void *` with a function pointer (a GNU extension that GCC and Clang accept without `-pedantic-errors`), so the fix turns that passing test into a known failure. That trade-off needs a decision first.

Local results on the final head (rustc 1.85.1, Clang 19.1.7): `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **115 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **151/151 passed**, `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**, `python3 harness/run_csmith.py` **25 passed, 15 known failures, 0 unexpected, 40 seeds**, `python3 harness/run_gcc_torture.py` **917 passed, 829 known failures, 0 unexpected** (no now-passing entries), `python3 harness/run_onetrueawk.py` `bugs-fixed` 31 programs agree and `testdir` shows the same nine golden-file mismatches for both builds, and `python3 harness/perf.py` reported 0 result mismatches and 0 timeouts on 142 harness programs and built and ran One True AWK. The independent C11 challenge v0.5 is **636/636** at both `-O0` and `-O2`**, including 65/65 `cross-abi` cases in both directions.

## v0.6 repairs: void pointers versus function pointers, and `-std=gnu11`

The independent C11 challenge v0.6 first run (724 cases, at `fa39ab2`) failed C11-703 and C11-704 at both `-O0` and `-O2`: `void *v; v == f` and `v != &f`, where `f` is a function, were accepted. C11 6.5.9p2 allows a `void *` operand of `==` or `!=` only with a pointer to an object type, so both are constraint violations. A null pointer constant, including `(void *)0`, stays comparable with any pointer. The independent review approved the fix together with an opt-in GNU mode, because GCC torture `20041212-1.c` relies on the GNU behaviour.

1. **Strict rule.** `void_ptr_mix` (shared by semantic analysis and the parser's folded constants) now requires both operands to be pointers to object types, one of them to `void`. The parser's null pointer constant check also accepts an integer constant zero cast to `void *`. Tests first (`764f425`), then the fix (`96850aa`); this is the change held back from PR 25.
2. **`-std=c11` and `-std=gnu11`.** `src/dialect.rs` adds a `Dialect`. `-std=c11` is strict ISO C11 and stays the default; `-std=gnu11` opts in to GNU extensions; other values are a usage error and the last `-std` wins. GNU mode is the hook for the PLAN.md stage-1 GNU-compatibility milestone and governs exactly one rule today: a `void *` compared for equality with a function pointer is accepted, and semantic analysis prints `file:line:column: warning: ...` (the parser's folded constants accept it without a warning). Relational comparisons, the predefined macros (including `__STRICT_ANSI__`), and every other rule are unchanged. `compile_with`, `parse` and `analyze_tokens` keep strict C11; `compile_dialect`, `parse_dialect` and `analyze_dialect` take a dialect and return warnings. Tests first (`a2c05e3`: sema, parser and driver tests covering `==` and `!=`, `f` and `&f`, null pointer constants, object pointers, relational comparisons, and the flag), then the switch (`5cb4d19`).
3. **Harness and docs.** `harness/run_gcc_torture.py` passes `-std=gnu11`; every other harness uses the strict default. README documents the flag (`371c748`).

Local results on the final source (rustc 1.85.1, Clang 19.1.7, GCC 14.2.0): `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **120 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **151/151 passed**, `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped**, `python3 harness/run_csmith.py` **25 passed, 15 known failures, 0 unexpected, 40 seeds**, `python3 harness/run_gcc_torture.py` **917 passed, 829 known failures, 0 unexpected** (with `-std=gnu11`; no now-passing entries), and `python3 harness/perf.py` completed. The independent C11 challenge v0.6 is **724/724** at both `-O0` and `-O2` (was 722/724), including the `cross-abi` cases in both directions; its metamorphic tool reports **60/60** and its IR verifier **150/150**.

Review fixes after CodeRabbit on `8e29121` (three Minor findings, all taken; tests first in `ce81be1`):

1. **Nested `void *` casts.** C11 6.3.2.3p3 makes a `void *` cast a null pointer constant only when its operand is an integer constant expression with the value 0. `(void *)(void *)0` is a null pointer but not a null pointer constant, so `f == (void *)(void *)0` is now a diagnostic in semantic analysis and in parser-folded constants (Clang 19 and GCC 14 with `-pedantic-errors` reject it too).
2. **`_Generic` in parser-folded constants.** The parser's null pointer constant check now classifies a generic selection by its selected expression, so `sizeof(f == _Generic(0, int: (void *)0))` is accepted again, as Clang 19 and GCC 14 with `-pedantic-errors` accept it.
3. **README.** GNU mode accepts the comparison in parser-folded constants without a warning; README now says so.

A 1,000-seed Csmith campaign with the challenge's tool (Csmith 2.3.0 default options, `-O0` and `-O2`) found **no wrong-code case**. Of 2,000 runs, 332 built with the candidate and all 332 match GCC and Clang (166 at `-O0` after relinking with `-lm`, which the tool omits); the other 1,668 are programs with incompatible-pointer comparisons or assignments that the candidate rejects as C11 constraint violations, confirmed by both `clang -std=c11 -pedantic-errors` and `gcc -std=c11 -pedantic-errors` (262 of them also time out in the reference binaries). Default Csmith output is therefore mostly not strict C11, which limits how much code generation that campaign exercises. The independent review accepted v0.6 as the final planned adversarial release. Pull request 27 merged as `d728a94`.

Close-out (15 November 2026): the unchanged v0.6 suite was rerun on `main` `0c43e32` after the GNU, DOOM, Lua, SQLite, performance, and macOS work, with **724/724** at both `-O0` and `-O2`, metamorphic **60/60**, and IR verification **150/150**. The independent reviewer formally closed the adversarial phase. The final report, the review, and the rerun evidence are in `docs/adversarial/v0.6/`.

## Small C11 gaps record

Flexible array members (C11 6.7.2.1), `u`/`U`/`u8` literals (C11 6.4.4.4 and 6.4.5), `long double` aggregate returns, and file-scope initializers that nest a bit-field struct. Local results on rustc 1.85.1 and Ubuntu Clang 18.1.3: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **84 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **129/129 passed**. `python3 harness/run_gcc_torture.py` was **828 passed, 918 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped**. No torture line was removed: the flexible-array entries still initialize the flexible member, which C11 does not allow, and `widechar-3.c` still needs `__builtin_strlen`. `python3 harness/run_csmith.py` was **25 passed, 15 known failures, 0 unexpected**, out of 40 seeds, after seeds 21 and 22 were removed from the known-failure list. `python3 harness/run_onetrueawk.py` built unmodified awk. `bugs-fixed` is 31 programs and the two binaries agree. `testdir` prints the same nine `BAD` lines for both builds.

`tests/115_fam.c` checks `sizeof`, `offsetof`, a local initializer that does not touch the flexible member, indexing through a larger object, and returning the fixed part. `tests/116_uchar.c` includes `<uchar.h>` and checks `u`/`U`/`u8` literals, concatenation with an unprefixed literal, and the widths of `char16_t` and `char32_t`. `tests/117_ldret.c` returns `struct { long double }`, `union { long double }`, and the larger memory-class aggregates. `tests/118_bitnest.c` initializes a nested bit-field struct, a plain member beside the bit-field, an array of them, and a sibling pointer, all at file scope. A designated bit-field in a union member other than the first is the same byte image, including next to a pointer. A `volatile` aggregate returned in `%st0` is `load volatile x86_fp80`. An `__asm__` label accepts an ordinary or `u8` string and rejects a wide prefix. `tests/xabi/04_ldagg_def.c` and `04_ldagg_use.c` are linked both ways with Clang. A pure `long double` aggregate is class `X87`: the argument is `byval` and the return is `x86_fp80` in `%st0`, which is what the psABI and Clang do.

### GitHub Actions

Observed on commit `d25dd9935b4a7de7e6811e0f99edc630f0c82f39`. Both runs completed with conclusion **success**. Each log contains `test result: ok. 83 passed; 0 failed` and `129/129 passed`.

| Event | Run | Jobs | Conclusion |
| --- | --- | --- | --- |
| `push` | https://github.com/spacefilm900/c-compiler/actions/runs/42912554967 | [fmt, clippy, tests, harness](https://github.com/spacefilm900/c-compiler/actions/runs/42912554967/job/130166671176), [writing-a-c-compiler suite](https://github.com/spacefilm900/c-compiler/actions/runs/42912554967/job/130166671667), [csmith differential](https://github.com/spacefilm900/c-compiler/actions/runs/42912554967/job/130166671626), [one true awk](https://github.com/spacefilm900/c-compiler/actions/runs/42912554967/job/130166671651), [gcc torture execute](https://github.com/spacefilm900/c-compiler/actions/runs/42912554967/job/130166671641), [performance numbers](https://github.com/spacefilm900/c-compiler/actions/runs/42912554967/job/130166671610) | success |
| `pull_request` | https://github.com/spacefilm900/c-compiler/actions/runs/42912568180 | [fmt, clippy, tests, harness](https://github.com/spacefilm900/c-compiler/actions/runs/42912568180/job/130166714196), [writing-a-c-compiler suite](https://github.com/spacefilm900/c-compiler/actions/runs/42912568180/job/130166714396), [csmith differential](https://github.com/spacefilm900/c-compiler/actions/runs/42912568180/job/130166714330), [one true awk](https://github.com/spacefilm900/c-compiler/actions/runs/42912568180/job/130166714233), [gcc torture execute](https://github.com/spacefilm900/c-compiler/actions/runs/42912568180/job/130166714201), [performance numbers](https://github.com/spacefilm900/c-compiler/actions/runs/42912568180/job/130166713890) | success |

### CodeRabbit

CodeRabbit reviewed pull request 19. Review `5850137303` on `dd2ed48`, submitted 2026-11-10, and review `5850301426` on `0737613`, submitted 2026-11-10. The volatile `x87` return spelling, the plain member of a nested bit-field struct, the `__asm__` wide label, the stale README sentences, and the test headers other than `<uchar.h>` were fixed on this branch. `<uchar.h>` stays in `tests/116_uchar.c` because that header is what this step makes usable. The docstring-coverage note (38.10% against an 80% threshold) was left as on pull requests 3, 4, 5, and 12. The sources document modules and the language rules, not every function.

## Stage 2 step 4 record

Aggregate calls and function pointers. Local results: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **74 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **107/107 passed**, and `python3 harness/run_wacc.py` **1194 passed, 91 known failures, 332 skipped, 0 unexpected**, on rustc 1.85.1 and Ubuntu Clang 18.1.3. `main` at `70e489f` was **1254 executed, 1176 passed, 78 known failures, 363 skipped**. GitHub Actions for this step is not in this file yet.

Classification follows the System V AMD64 psABI. Each eightbyte is INTEGER, SSE, x87, or MEMORY. INTEGER wins when it shares an eightbyte with SSE, so `struct { int i; float f; }` is one `i64` even though float lowering is not implemented. A pure SSE or x87 eightbyte is a diagnostic. More than 16 bytes, or an unaligned field, is memory. Register pieces use the field span in that eightbyte, rounded up to the strictest field alignment there: three bytes are `i24`, `struct { long a; char b; }` is `i64` plus `i8`, and a lone pointer eightbyte is `ptr`. `byval` is aligned to at least 8. `sret` uses the aggregate's alignment. The hidden `sret` pointer is a normal LLVM `sret` argument, so the x86-64 backend returns it in `%rax`. A variadic argument uses the same classification and the same register budget. A multi-eightbyte aggregate that does not fit in the registers still free is passed entirely in memory.

## Floating-point record

`float` is IEEE binary32, `double` is IEEE binary64, and `long double` is 80-bit extended precision. A decimal constant is rounded to nearest, ties to even: `float` and `double` by the host `f32`/`f64` conversion, and `long double` from the decimal digits. A hexadecimal constant is converted from its binary significand with the same rounding, including subnormals, overflow to infinity, and underflow to zero. An unsuffixed constant is `double`. `f`/`F` is `float`. `l`/`L` is `long double`. A floating constant in `#if` is a diagnostic, because a preprocessor expression is an integer constant expression.

The usual arithmetic conversions follow C11 6.3.1.8. `long double` wins over `double`, `double` wins over `float`, and any floating type wins over every integer. Integer-to-floating conversion uses `sitofp` or `uitofp`. Floating-to-integer conversion truncates toward zero (`fptosi` / `fptoui`). A constant that is not finite, or that does not fit in the destination integer, is a diagnostic. `_Bool` is 0 when the value compares equal to 0 and 1 otherwise, so a NaN converts to 1. `float` promotes to `double` in a variadic argument. `%`, shifts, and bitwise operators still require integers. Comparisons use ordered `fcmp` predicates, except `!=`, which is `une`, so a NaN compares unequal. `if (x)` is `fcmp une` against zero, so a NaN is true and `-0.0` is false. `!x` is that condition inverted, which is `0 == x`.

An SSE eightbyte is one XMM register. Two `float` fields in one eightbyte are `<2 x float>`. A `double` is `double`. A single `float` is `float`. INTEGER wins when an integer shares the eightbyte, and that integer is `i64` when several fields share it inside a larger aggregate, or the scalar width when the eightbyte holds one aligned scalar. A field that continues across an eightbyte boundary keeps the partial width, so `unsigned char[10]` is still `i64` plus `i16`. Eight XMM registers are tracked beside the six general registers. Scalar `float` and `double` consume XMM registers. LLVM places a scalar past `xmm7` on the stack. An aggregate that needs an XMM register and does not have one is `byval`.

`tests/xabi/02_fp_def.c` and `02_fp_use.c` are linked both ways with Clang. `tests/104_libm.c` calls glibc through `<math.h>`. The harness and the suite runner link with `-lm`.

Local results: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **80 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **110/110 passed**, and `python3 harness/run_wacc.py` **1354 passed, 0 known failures, 263 skipped, 0 unexpected**, on rustc 1.85.1 and Ubuntu Clang 18.1.3. GitHub Actions for this step is not in this file yet.

## Stage 2 step 6 record

Variadic definitions, `goto`, designated initializers, bit-fields, and `volatile`, rebased onto `main` at `0c30dc9` so `float` and `double` stay. Local results: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **81 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **116/116 passed**, and `python3 harness/run_wacc.py` **1384 passed, 0 known failures, 233 skipped, 0 unexpected**, on rustc 1.85.1 and Ubuntu Clang 18.1.3. Chapter 13 is 69/69. `main` before this change was **1354 executed, 1354 passed, 0 known failures, 263 skipped**.

A variadic definition keeps one named parameter before `...`. `llvm.va_start` fills `va_list` from that LLVM signature, so the signature has to match the ABI: `sret` consumes a general register, `byval` does not, and `float` or `double` uses an XMM register. `va_arg` reads `gp_offset` or `fp_offset` and, when the next argument does not fit, the overflow area. A multi-eightbyte argument that does not fit does not consume the leftover registers. An aggregate whose eightbytes are not one contiguous image in the save area (two XMM slots, or INTEGER plus SSE) is copied one eightbyte at a time. `tests/107_vararg.c` checks `int`, `double`, an integer struct, a memory struct, `vsnprintf`, a one-`double` struct, `struct { int; double; }`, and `struct { double; double; }`. `tests/xabi/02_vararg_def.c` and `02_vararg_use.c` are linked both ways with Clang.

`goto` names a label in the same function. The block is `C.<name>`, so it does not collide with a generated `L<n>`. A duplicate label and a `goto` with no label are diagnostics. `tests/103_goto.c` covers a forward jump and a loop.

A designated initializer may be `.field` or `[constant]`, including a chain. After a designator, the next positional item continues at the next subobject in depth-first order. A later designator is relative to the enclosing brace. A union's positional initializer is the first named member; a designator may select another. GNU ranges (`[0 ... 3]`) are rejected. `tests/104_desig.c` covers the continuation rule.

Bit-fields pack least-significant bit first. A field that does not fit in the current storage unit of its declared type starts at the next unit. A zero-width unnamed field aligns the bit cursor and does not raise the struct's alignment. A named field contributes its type's alignment. The expression type is the promoted type; a signed field sign-extends. `&` and `sizeof` of a bit-field are diagnostics. The LLVM type of a struct that contains a bit-field is a byte array. A file-scope initializer may initialize that struct directly. A file-scope initializer that nests it inside another struct or an array is a diagnostic. A local initializer of the nested form is accepted. An unnamed bit-field whose struct has no named member is an obscure ABI mismatch with Clang and is not a test target. `tests/105_bits.c` checks layout, signedness, and an unsigned field of width 3, which promotes to `int`.

`tests/106_volatile.c` checks that a volatile scalar and a volatile member are volatile loads and stores.

## Switch record

Local results for that step: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **68 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **103/103 passed**, and `python3 harness/run_wacc.py` **1176 passed, 78 known failures, 363 skipped, 0 unexpected**, on rustc 1.85.1 and Ubuntu Clang 18.1.3. `main` before that change was **1220 executed, 1143 passed, 77 known failures, 397 skipped**.

### GitHub Actions

Observed on commit `98b1b931256ef5fe4d1104b32b00f6523681fb85`. Both runs completed with conclusion **success**. Each log contains `test result: ok. 68 passed; 0 failed`, `103/103 passed`, and `1176 passed, 78 known failures, 0 now passing, 363 skipped`.

| Event | Run | Jobs | Conclusion |
| --- | --- | --- | --- |
| `push` | https://github.com/spacefilm900/c-compiler/actions/runs/42766873794 | [fmt, clippy, tests, harness](https://github.com/spacefilm900/c-compiler/actions/runs/42766873794/job/129665318455), [writing-a-c-compiler suite](https://github.com/spacefilm900/c-compiler/actions/runs/42766873794/job/129665318338) | success (2026-11-10 06:11:46Z–06:12:54Z) |
| `pull_request` | https://github.com/spacefilm900/c-compiler/actions/runs/42766891103 | [fmt, clippy, tests, harness](https://github.com/spacefilm900/c-compiler/actions/runs/42766891103/job/129665369570), [writing-a-c-compiler suite](https://github.com/spacefilm900/c-compiler/actions/runs/42766891103/job/129665369177) | success (2026-11-10 06:11:55Z–06:13:09Z) |

### CodeRabbit

CodeRabbit reviewed pull request 12 on `98b1b93`. Review `5837600509`, submitted 2026-11-10, state `COMMENTED`. No inline comments. Details URL: https://app.coderabbit.ai/change-stack/spacefilm900/c-compiler/pull/12.

The review summary repeated the docstring-coverage pre-merge note (17.07% against an 80% threshold). No function docstrings were added. The sources document modules and the language rules, not every function. The same note on pull requests 3, 4, and 5 was left that way. The CodeRabbit check run on `98b1b93` (https://github.com/spacefilm900/c-compiler/runs/129665449014) was still `in_progress` with no conclusion when this record was written.

A `case` label is a statement, so it can appear inside the statement that is the switch body, including inside `if` and loops. Semantic analysis keeps a stack of switches. The innermost switch owns the label. The case expression is folded with the same integer-constant rules as a file-scope initializer, then converted to the promoted type of the controlling expression. Duplicate detection uses that converted value. Code generation assigns one LLVM block to each label, emits `switch` to those blocks, and branches from the preceding block when control falls through. `break` and `continue` use separate stacks, so a `break` inside a loop that is inside a switch leaves the loop.

## Stage 2 step 2 record

`typedef` is an alias, not a new type. The parser keeps a scope of typedef names, enumeration constants, and ordinary identifiers, and decides the declaration-versus-expression ambiguity from that scope. A typedef name is a type specifier only when the declaration specifiers do not already contain a type, so `typedef T T` is a redeclaration of the alias. Compatible redeclarations are allowed. Function typedefs are function types; a declarator may add `*` to make a pointer, and a parameter of function type is adjusted to a pointer.

An enumeration constant has type `int`, and a completed enum type is compatible with `int`. That is the implementation-defined choice in C11 6.7.2.2: every value has to be representable as `int`, and a value that is not is a diagnostic. The constants are integer constant expressions. An enumerator list starts at 0. A trailing comma is allowed. `enum E` before `E` is defined is a diagnostic.

A union is laid out like a struct except every member starts at offset 0. The size is the largest member, rounded up to the strictest alignment. A brace initializer, and an unbraced initializer, initializes the first member only; further initializers belong to the enclosing object. The address of an object inside a union initializer was a diagnostic here, because the LLVM constant is raw bytes and cannot hold a relocation; since the DOOM step it is accepted (see "DOOM (doomgeneric)" above). A null pointer, and an integer converted to a pointer, are ordinary bytes. Assignment copies the whole object with `llvm.memmove`, as for a struct. The LLVM type is a one-element array whose element has the union's alignment, so a union member inside a struct is aligned the way the ABI requires. Member access uses a byte `getelementptr`. A union is passed and returned by value with the same classification as a struct. `<stdlib.h>`'s `div` and `ldiv` return `div_t` and `ldiv_t` in registers; `tests/102_div.c` calls the host libc.

`struct tag` and `union tag` share one tag namespace with enums. `struct tag; { union tag; }` hides the outer struct. `union tag *p` while `struct tag` is visible, with no inner `union tag;`, is a diagnostic. Untagged `struct { ... }` and `union { ... }` are ordinary types; a typedef may name them. Anonymous members (a member with no name) are still rejected. Bit-fields are still rejected.

`restrict` is a qualifier. It is part of type compatibility and of assignment, and it is not part of a parameter's function type, same as top-level `const`. It does not change the IR. It is rejected on a type that is not a pointer. `inline`, `__inline`, and `_Noreturn` are accepted and do not change linkage. `register` and `auto` are accepted as the default storage class.

`__attribute__((...))` and `__extension__` are parsed and ignored. `__asm__("name")` renames the linker symbol, which is what glibc's `__REDIRECT` expands to. Adjacent string literals in the asm operand are concatenated. `__func__` and `__PRETTY_FUNCTION__` are the name of the current function, as a string.

`_Bool` is an unsigned integer type of rank below `char`, stored in one byte, with values 0 and 1. Conversion to `_Bool` compares with zero, including a pointer (`icmp ne` against null, then 0 or 1). It is passed as `i8 zeroext`. `<stdbool.h>` defines `bool`, `true`, and `false`.

`float` is 4 bytes, `double` is 8, and `long double` is 16 (`x86_fp80`). They exist so a header can declare `strtod`. Arithmetic, assignment, and casts that produce one are diagnostics. Floating constants are still rejected by the lexer.

Function pointers are values. A function designator decays to a pointer except as the operand of `&` or `sizeof`. A call through a pointer is an indirect `call`. A direct call of a declared function is still a direct `call`.

Angle includes search `-I` directories, then `include/` (the compiler's freestanding headers), then `/usr/include/x86_64-linux-gnu`, then `/usr/include`. `#include_next` starts after the directory that provided the current file. The temporary `stdio.h`, `stdlib.h`, and `string.h` stand-ins are gone. `stddef.h` honors glibc's `__need_size_t` / `__need_NULL` protocol and defines `size_t`, `ptrdiff_t`, and `wchar_t` from `__SIZE_TYPE__`, `__PTRDIFF_TYPE__`, and `__WCHAR_TYPE__`. `stdarg.h` defines `__gnuc_va_list` as the x86_64 register-save descriptor (an array of one struct) when included with `__need___va_list`, and defines `va_list` otherwise. The `va_*` macros do not implement argument access. `limits.h` is the ISO C limits for this LP64 target, with `char` signed.

Predefined macros added for the headers: `__USER_LABEL_PREFIX__` (empty), `__WCHAR_MAX__`, and `__WCHAR_MIN__`. The macros that were already predefined (`__STDC_VERSION__`, `__x86_64__`, `__LP64__`, `__GNUC__` 4.2.1, the `__SIZEOF_*__` macros, `__SIZE_TYPE__`) select the LP64 path in glibc and keep the `_Float128` typedefs out of the headers.

Local results after merging `main` at `fdee7d5`: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **65 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **102/102 passed**, and `python3 harness/run_wacc.py` **1143 passed, 77 known failures, 397 skipped, 0 unexpected**, on rustc 1.85.1 and Ubuntu Clang 18.1.3. `main` was 1115 passed, 76 known failures, and 426 skipped. The 29 union extra-credit tests are no longer skipped. Fifteen tests that `main` still listed now pass and were removed from `harness/wacc_known_failures.txt`. Sixteen union tests remain known failures. GitHub Actions for this merge is not in this file yet.

## Stage 2 step 1 record

Local results are in the table above: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **60 passed**, `make` produced `build/compiler`, and `python3 harness/run_tests.py` **90/90 passed**, on rustc 1.85.1 and Ubuntu Clang 18.1.3. The 60 count is after the review fixes (a header without a final newline no longer runs into the next line, `#else` is not followed by another `#else` or `#elif`, an empty `#if` is reported on that directive, `#if` constants that fit in `intmax_t` stay signed, a shift's type follows its left operand, and `-E` inserts a space when two tokens would otherwise glue). GitHub Actions for this step is not in this file.

`include/` is a temporary bridge, not the host libc. `#include <stdio.h>` finds `include/stdio.h`, which declares `printf`, `puts`, and `putchar`. `string.h` declares `strlen`, `strcmp`, and `memcpy`. `stdlib.h` declares `malloc`, `free`, `exit`, and `abs`. `stddef.h` defines `NULL` as `((void *)0)`. There is no `typedef`, so those headers use `unsigned long` where a hosted header would use `size_t`. The compiler does not search `/usr/include`. Real glibc headers still need types this step does not parse. Quote includes are sought in the including file's directory, then each `-I` directory, then `include/`. Angle includes skip the including file's directory. The search stops at 200 nested includes. `#pragma once` remembers the canonical path.

`__FILE__` and `__LINE__` follow the invocation, including inside a macro. `__STDC__` is 1, `__STDC_VERSION__` is `201112L`, and `__STDC_HOSTED__` is 1. `__x86_64__`, `__linux__`, `__LP64__`, and the other target macros in `src/pp.rs` are predefined so a later header can take the LP64 Linux path. `__GNUC__` / `__GNUC_MINOR__` / `__GNUC_PATCHLEVEL__` are 4, 2, and 1, which is the baseline glibc already accepts from Clang. `#if` arithmetic uses `intmax_t` / `uintmax_t` (signed and unsigned 64-bit). `defined X` and `defined(X)` are evaluated before expansion. An undefined identifier in `#if` is 0. A skipped group is not expanded and its `#error` is not fired. An omitted `__VA_ARGS__` deletes a comma written as `, ## __VA_ARGS__`, matching Clang's `-std=c11` behavior; an empty-but-present argument (`F(1,)`) keeps the comma.

## Milestone 2 step 6 record

Local results for that step: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **46 passed**, `make` produced `build/compiler`, and `python3 harness/run_tests.py` **81/81 passed**, on rustc 1.85.1 and Ubuntu Clang 18.1.3. Those commands were run again after the review fixes (an unnamed `struct S` parameter is rejected, a nested redefinition of the struct being defined is rejected, assigning through a member of a struct rvalue is rejected, the module emits the System V AMD64 data layout, and a whole-struct assignment is `llvm.memmove`, which keeps padding). The later run had the same counts. GitHub Actions for that step is not in this file.

## Milestone 2 step 5 record

Local results are in the table above: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **41 passed**, `make` produced `build/compiler`, and `python3 harness/run_tests.py` **72/72 passed**, on rustc 1.85.1 and Ubuntu Clang 18.1.3. Those commands were run before the pull request, then again after the review fixes (`%ld` is given a `long`, the unsized-array string initializer no longer tests conditions that cannot fire, and the test-path note allows a hand-declared library prototype). The second run had the same counts. GitHub Actions is not recorded here.

`tests/62_hello.c` is the program above. The harness run compared its exit status, stdout, and stderr with `clang -std=c11`. Both print `hello, world` and a newline and exit 0.

## Milestone 2 step 4 record

Local results are in the table above: `cargo test --offline --lib` **37 passed**, `python3 harness/run_tests.py` **61/61 passed**, on rustc 1.85.1 and Ubuntu Clang 18.1.3. After the review fix, `cargo fmt --all -- --check` and `cargo clippy --offline --all-targets -- -D warnings` were clean and `cargo test --offline --lib` was **37 passed** again. The harness was not re-run locally after that fix. GitHub Actions on the fix commit did run the harness.

### GitHub Actions

Observed on commit `cf8cbf1c2f446ad90f2ef3f57e74d25861565318`. Both runs completed with conclusion **success**. Each job's log contains `test result: ok. 37 passed; 0 failed` and `61/61 passed`. The push job reported rustc 1.85.1 (4eb161250 2025-03-15), Ubuntu clang 18.1.3, Ubuntu LLVM 18.1.3 (`llvm-as --version`), GNU Make 4.3, and Python 3.12.3. The pull_request job's log also contains rustc 1.85.1 (4eb161250 2025-03-15), Ubuntu clang 18.1.3, Ubuntu LLVM 18.1.3, GNU Make 4.3, Python 3.12.3, `37 passed; 0 failed`, and `61/61 passed`.

| Event | Run | Job | Conclusion |
| --- | --- | --- | --- |
| `push` | https://github.com/spacefilm900/c-compiler/actions/runs/42616066665 | https://github.com/spacefilm900/c-compiler/actions/runs/42616066665/job/129156122794 | success (2026-11-09 12:19:39Z–12:20:26Z) |
| `pull_request` | https://github.com/spacefilm900/c-compiler/actions/runs/42616074831 | https://github.com/spacefilm900/c-compiler/actions/runs/42616074831/job/129156152654 | success (2026-11-09 12:19:42Z–12:20:33Z) |

The review-fix commit `cbab10c8aaaf4c3bb297befa262b469df518b766` had the same successful job: push https://github.com/spacefilm900/c-compiler/actions/runs/42615660546/job/129154765356 and pull_request https://github.com/spacefilm900/c-compiler/actions/runs/42615667217/job/129154787685. Both logs contain `37 passed; 0 failed` and `61/61 passed`. Commit `6269cc8`, before that fix, also had successful runs of the same job. Those earlier runs are not the head.

### CodeRabbit

CodeRabbit reviewed pull request 5 on `6269cc8`. Review `5825690316`, submitted 2026-11-09 12:14:56Z, state `COMMENTED`, two inline comments. Details URL: https://app.coderabbit.ai/change-stack/spacefilm900/c-compiler/pull/5.

Both inline comments were fixed in `cbab10c`:

- A global initializer that narrows a file-scope address, such as `int x = (int)&g` or `long x = (int)&g`, was emitted as `ptrtoint`. The link-time bits are unknown, and a narrower cast was ignored when the destination was wider. Those initializers are now a diagnostic. A 64-bit cast into a 64-bit object still lowers to `ptrtoint`.
- `int (*p)[]; p[0]` type-checked and lowered to a zero-byte `getelementptr` on `[0 x i32]`. A subscript of a pointer to an incomplete type is now a diagnostic. The `void *` subscript diagnostic is unchanged.

The follow-up review on `cf8cbf1` (review `5825799784`, submitted 2026-11-09 12:25:20Z) posted no new inline comments. Its summary repeated the docstring-coverage pre-merge note (15.15% against an 80% threshold). No function docstrings were added. The sources document modules and the language rules, not every function. The same note on pull requests 3 and 4 was left that way. The CodeRabbit check run on `cf8cbf1` (https://github.com/spacefilm900/c-compiler/runs/129156363281) was still `in_progress` with no conclusion when this record was written.

## Milestone 2 step 3 record

Local results are in the table above: `cargo test --offline --lib` **34 passed**, `python3 harness/run_tests.py` **47/47 passed**, on rustc 1.85.1 and Ubuntu Clang 18.1.3. Those commands were run again after the constant `?:` conversion fix, with the same results.

### GitHub Actions

Observed on commit `29d1635ec612101aaadf3995e471e71b7efb9c02`. Both runs completed with conclusion **success**. Each job's log contains `test result: ok. 34 passed; 0 failed` and `47/47 passed`. Both jobs reported rustc 1.85.1 (4eb161250 2025-03-15), Ubuntu clang 18.1.3, Ubuntu LLVM 18.1.3 (`llvm-as --version`), GNU Make 4.3, and Python 3.12.3. Both logs contain `34 passed; 0 failed` and `47/47 passed`.

| Event | Run | Job | Conclusion |
| --- | --- | --- | --- |
| `push` | https://github.com/spacefilm900/c-compiler/actions/runs/42608147836 | https://github.com/spacefilm900/c-compiler/actions/runs/42608147836/job/129129299371 | success (2026-11-09 11:19:46Z–11:20:39Z) |
| `pull_request` | https://github.com/spacefilm900/c-compiler/actions/runs/42608159630 | https://github.com/spacefilm900/c-compiler/actions/runs/42608159630/job/129129339364 | success (2026-11-09 11:19:52Z–11:20:38Z) |

Commit `ec14e21`, before the conditional conversion fix, is not the head.

### CodeRabbit

CodeRabbit reviewed pull request 4 on `ec14e21`. The review posted one inline comment, on `src/sema.rs`. A constant `?:` kept the chosen arm's type. `unsigned long g = 1 ? -1 : 1u` folded to all-ones instead of `4294967295`, and `(1 ? -1 : 1u) > 0` folded to 0. That was fixed in `29d1635`: the chosen arm is converted to the conditional's result type, and the unit test plus `tests/47_const_types.c` cover both cases.

The follow-up review on `29d1635` (review `5825162941`, submitted 2026-11-09 11:24:01Z) posted no new inline comments. Its summary repeated the docstring-coverage pre-merge note. No function docstrings were added. The sources document modules and the language rules, not every function. The same note on pull request 3 was left that way.

## Milestone 2 step 2 record

Local results are in the table above: `cargo test --offline --lib` **29 passed**, `python3 harness/run_tests.py` **37/37 passed**, on rustc 1.85.1 and Ubuntu Clang 18.1.3.

### GitHub Actions

Observed on commit `dc32fa16839ad5f2fd5a54f6334f6e40ea359ce9`. Both runs completed with conclusion **success**. Each job's log contains `test result: ok. 29 passed; 0 failed` and `37/37 passed`. The push job reported rustc 1.85.1 (4eb161250 2025-03-15), Ubuntu clang 18.1.3, Ubuntu LLVM 18.1.3 (`llvm-as --version`), GNU Make 4.3, and Python 3.12.3. The pull_request job's log also contains Ubuntu clang 18.1.3, GNU Make 4.3, Python 3.12.3, `29 passed; 0 failed`, and `37/37 passed`.

| Event | Run | Job | Conclusion |
| --- | --- | --- | --- |
| `push` | https://github.com/spacefilm900/c-compiler/actions/runs/42598953136 | https://github.com/spacefilm900/c-compiler/actions/runs/42598953136/job/129098387411 | success (2026-11-09 10:10:43Z–10:11:23Z) |
| `pull_request` | https://github.com/spacefilm900/c-compiler/actions/runs/42598959935 | https://github.com/spacefilm900/c-compiler/actions/runs/42598959935/job/129098409986 | success (2026-11-09 10:11:46Z–10:12:30Z) |

Commit `30f9dd47905de98939b48529381613e7b8481c7e`, before the review fix, also had successful push and pull_request runs of the same job. Those runs are not the head.

### CodeRabbit

CodeRabbit reviewed pull request 3 on `30f9dd4`. Check run `CodeRabbit`, app `coderabbitai`, conclusion **success**, completed 2026-11-09 10:14:36Z. GitHub URL: https://github.com/spacefilm900/c-compiler/runs/129095711382. Details URL: https://app.coderabbit.ai/change-stack/spacefilm900/c-compiler/pull/3.

The review posted two inline comments. Both were fixed in `dc32fa1`:

- `STATUS.md` had called `x += x += 1` defined. That expression modifies one scalar twice with no intervening sequence point, so the sentence now says it is undefined and states only this compiler's evaluation order.
- `tests/33_compound.c` used that expression. It now uses `x += extra += 1` on two different objects.

The review summary also listed a docstring-coverage pre-merge note. No function docstrings were added. The existing sources document modules and the operators, not every function. No follow-up review on `dc32fa1` had been posted when this record was written.

## Milestone 1 record

Milestone 1 (lexer, parser, checks, and LLVM IR for the integer subset, without prototypes or globals) was imported on `cursor/import-compiler-ci-08ff` and merged as pull request 1. Local results before that import: `cargo test --offline --lib` **21 passed**, then **23 passed** after the review fixes, and `python3 harness/run_tests.py` **17/17 passed**. The preparation environment used rustc 1.85.1 and Ubuntu Clang 18.1.3.

### GitHub Actions

Observed on commit `3b9d33e1cefd9a44f61c069011e7cbc460d033b8`. Both runs completed with conclusion **success**. Each job's log contains `test result: ok. 21 passed; 0 failed` and `17/17 passed`. The runner reported rustc 1.85.1 (4eb161250 2025-03-15), Ubuntu clang 18.1.3, Ubuntu LLVM 18.1.3 (`llvm-as --version`), GNU Make 4.3, and Python 3.12.3.

| Event | Run | Job | Conclusion |
| --- | --- | --- | --- |
| `push` | https://github.com/spacefilm900/c-compiler/actions/runs/42538152639 | https://github.com/spacefilm900/c-compiler/actions/runs/42538152639/job/128906962752 | success (2026-11-09 00:03:54Z–00:04:27Z) |
| `pull_request` | https://github.com/spacefilm900/c-compiler/actions/runs/42538162075 | https://github.com/spacefilm900/c-compiler/actions/runs/42538162075/job/128906993007 | success (2026-11-09 00:04:01Z–00:04:35Z) |

The push job also recorded a non-failing annotation: Node.js 20 is deprecated, and `actions/checkout@11d5960a326750d5838078e36cf38b85af677262` was forced onto Node.js 24. The job still succeeded. The pin was left as imported because the run did not fail.

The documentation commit `865c0f0ef1dc2dfe94186b89b089c31185d026da` was checked after the rows above. Both of its workflow runs completed with conclusion **success**, and both logs contain `21 passed; 0 failed` and `17/17 passed`:

| Event | Run | Conclusion |
| --- | --- | --- |
| `push` | https://github.com/spacefilm900/c-compiler/actions/runs/42539136514 | success |
| `pull_request` | https://github.com/spacefilm900/c-compiler/actions/runs/42539140762 | success |

The review-fix commit is `b930f0e` (escape LLVM strings, reject backslash-newline, reject integers that do not fit in signed 32-bit `int`). Its local result was **23 passed** and **17/17**. Runs after `865c0f0` were not copied into this record.

### CodeRabbit

After pull request 1 was opened, the App reviewed the import:

- Check run `CodeRabbit` on `3b9d33e`, app `coderabbitai`, conclusion **success**, completed 2026-11-09 00:13:11Z. GitHub URL: https://github.com/spacefilm900/c-compiler/runs/128907038494. Details URL: https://app.coderabbit.ai/change-stack/spacefilm900/c-compiler/pull/1.
- `coderabbitai[bot]` posted a review with 3 inline comments (profile `CHILL` from `.coderabbit.yaml`). Those three comments were fixed in `b930f0e`.

## Clean-room reminder

Frontend code was written from the C standard, the LLVM Language Reference, and the System V ABI. Clang and GCC source were not consulted. Clang is only the differential oracle and the linker/backend for emitted IR.
