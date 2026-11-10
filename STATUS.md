# STATUS

## Current milestone

**Flexible array members, `u`/`U`/`u8` literals, `long double` aggregate returns, and nested bit-field initializers, on top of variadic definitions, `goto`, designated initializers, bit-fields, and `volatile`.** A function may be defined with `...`. `<stdarg.h>` implements `va_list`, `va_start`, `va_arg`, `va_end`, and `va_copy` with `__builtin_va_list` and the matching builtins. `va_arg` follows the System V AMD64 register-save area: general registers at offsets 0..48, XMM registers at 48..176, then the overflow area. `float` promotes to `double`. An aggregate uses the same classification as a call. `llvm.va_start`, `llvm.va_end`, and `llvm.va_copy` are emitted; LLVM's `va_arg` instruction is not. `goto` and labels are function scope, including a forward jump. Designated initializers (`.field` and `[const]`) are accepted, and the next positional initializer continues at the next subobject. Bit-fields pack least-significant bit first in a storage unit of the declared type. `volatile` is `load volatile` and `store volatile`.

`float` and `double` stay as on `main`. Decimal and hexadecimal floating constants are lexed, including an `f`/`F` suffix. The usual arithmetic conversions, integer/float conversions, and the `float` to `double` default argument promotion are applied, so `printf("%f", x)` passes a `double`. Arithmetic, comparisons, unary `+`/`-`, `!`, compound assignment, `++`/`--`, `?:`, casts, and the conditions of `if` / `while` / `for` / `&&` / `||` accept floating operands. `switch` still requires an integer. File-scope and `static` objects may have constant floating initializers, and floating members are allowed in structs, unions, and arrays. Scalar `float` and `double` parameters and returns use XMM registers. An SSE eightbyte of an aggregate is `float`, `double`, or `<2 x float>`. INTEGER still wins over SSE in the same eightbyte. A lone integer eightbyte stays an integer when general registers run out. An SSE eightbyte, or a mixed aggregate that is short a register of either class, is `byval` and does not consume the registers that remain. `long double` is the 80-bit x87 format, stored in 16 bytes. A scalar parameter is passed in memory and a scalar return uses the x87 stack. `va_arg` of `long double` reads the overflow area, 16-byte aligned. An x87 aggregate is passed in memory. A return whose eightbytes are `X87` then `X87UP` (a struct or union that is one `long double`) is `x86_fp80` in `%st0`. Any other aggregate that contains `long double` is memory (`sret`), which is what the classification produces once the size exceeds 16 bytes or `X87UP` is not preceded by `X87`. Anonymous struct and union members (an untagged type specifier with no declarator) promote their members into the enclosing type. `L'...'` and `L"..."` are `wchar_t` literals, and `wchar_t` is `int`. `u'...'` / `u"..."` are `char16_t` (the unsigned 16-bit type). `U'...'` / `U"..."` are `char32_t` (the unsigned 32-bit type). `u8"..."` is an array of `char`. `u8'...'` is rejected; C11 has no such character constant. An unprefixed string adopts the other literal's prefix; two different non-ordinary prefixes are a diagnostic. A flexible array member is the last member of a struct that has another named member. `sizeof` excludes it. It is not initialized, and the struct is not an array element or a member of another struct. A union that contains one, recursively, has the same restriction. A file-scope initializer may nest a bit-field struct inside another struct or an array; the bit-field subobject is raw bytes and a sibling address stays a relocation. `__builtin_memcpy`, `__builtin_memmove`, `__builtin_memset`, and `__builtin_signbit` / `__builtin_signbitf` / `__builtin_signbitl` are recognized. Programs can `#include` the host glibc headers for `stdio.h`, `stdlib.h`, `string.h`, `stdint.h`, `ctype.h`, `math.h`, `assert.h`, and `uchar.h`. The compiler supplies `stddef.h`, `stdarg.h`, `stdbool.h`, and `limits.h`. The Nora Sandler suite is fetched at a pinned commit and run by `harness/run_wacc.py`. It is not vendored.

## External suite

`main` at `0c30dc9` (`float` and `double`) has **80** unit tests, **110/110** harness programs, and **1354 executed, 1354 passed, 0 known failures, 263 skipped**. Chapter 13 is 69/69. This step: **81** unit tests, **116/116** harness programs (three cross-ABI pairs), and **1384 executed, 1384 passed, 0 known failures, 233 skipped**, 0 unexpected, 0 panics. Chapter 13 is still 69/69, including the three NaN extra-credit tests. The 30 `goto` tests now run and pass, including `switch_goto_mid_case.c`.

Licence: **MIT**, Copyright (c) 2023 Nora Sandler. The licence allows redistribution. The tree is still fetched, not committed. Source: https://github.com/nlsandler/writing-a-c-compiler-tests. Pinned commit: `ae12014d2dec14488f3f80d14df4b6d8e4634d7d` (2026-02-17). Recorded in `tests/external/README.md`. `scripts/fetch_wacc_tests.sh` clones that commit into `tests/external/wacc/` (gitignored). CI caches that directory.

`harness/run_wacc.py` does not call the suite's `test_compiler`. A valid program is compiled with `build/compiler`, linked with `clang`, and compared with `expected_results.json` (return code and stdout). If that entry is missing, `clang -std=c11` is the oracle. A `libraries/` test compiles one translation unit here and the other with clang, in both directions. Assembly helpers named in `test_properties.json` are the Linux `.s` file. Each command has a 10 second timeout. An invalid program must exit non-zero and must not crash. A compiler panic fails the run even if the test is listed in `harness/wacc_known_failures.txt`. A listed test that passes is reported so the line can be removed. An unlisted failure fails the run.

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

Negative cases covered inside `cargo test` (each must be a non-zero diagnostic, not a successful compile): missing semicolon, missing `)`, function pointers, variable-length arrays, a non-positive array size, `sizeof(float)`, `sizeof(void)`, a `?:` missing `:`, old-style `int f();` declarations, unnamed parameters on a definition, a non-constant `static` local initializer, an incomplete `static` local, a `static` or `extern` declaration in a `for` header, an initializer on a block-scope `extern`, a block-scope `static` function, a nested function, same-scope linkage conflicts (`extern` after a no-linkage declaration, and the reverse), conflicting types on a block-scope redeclaration, a block-scope `extern` that would give one identifier both internal and external linkage, a `do` / `while` condition that names a variable declared only in the body, function initializers, multiple storage-class specifiers, `void` variables, an incomplete local array, storage class on a parameter, `switch`, stray `else`, casts to `float`, a definition of a variadic function, `int f(...);` with no named parameter, wide string and character literals, `restrict`, a duplicate `const`, floats, invalid integer suffixes (`1Ll`, `1uu`), integer constants that fit in no target type, out-of-range hex and octal escapes, empty and multi-character character constants, unknown escapes, unterminated string literals, unterminated comments, a stray `#` that is not a directive, non-ASCII, `#error`, an unknown directive, `#else` / `#endif` without `#if`, an unterminated `#if`, a macro redefined with a different body, the wrong number of macro arguments, an include that cannot be found, and `#error` inside an included file (the diagnostic names that file), invalid type-specifier combinations (`short long`, `unsigned signed`, `long long long`, `int int`, `char int`), conflicting integer types (`char` vs `signed char`, `int` vs `unsigned`, `long` vs `long long`), incompatible pointer assignment (`int *` from `char *`, `char *` from `const char *`, `const char **` from `char **`), assignment to a const object (`const int`, `*p` for `const char *`, `char *const`, and an element of `const char[]`), a string that is too long for a character array, a string used to initialize an `int` array, arithmetic on `void *`, a subscript of a pointer to an incomplete array, a pointer compared with `0` using `<`, an array assigned to, the address of a non-lvalue, a non-constant pointer initializer, a global initializer that truncates an address (`int x = (int)&g` and `long x = (int)&g`), excess array initializers, undeclared names, redefinitions, conflicting function types (including a variadic declaration followed by a non-variadic one, and `const int` versus `int` as a return type), a variable redeclared as a function, conflicting linkage, non-constant, divide-by-zero, and overflowing global initializers, undefined shifts in a constant initializer (`1 << 32`, `1 << -1`, `-1 << 1`, `1 << 40`), `1 << 31` and `1L << 63` as overflowing constants, `++` / assignment / comma in an evaluated constant expression, a `?:` mixing `void` with an integer, `++` or compound assignment applied to a non-lvalue, bad call arity against a prototype (including too few arguments to a variadic function), a static function that is used but not defined, `break`/`continue` outside a loop, `void` returns, non-assignable expressions, calls of non-functions, and a call to a function that has not been declared yet. There is no implicit function declaration. Struct diagnostics covered the same way: passing or returning a struct by value (including an unnamed parameter `int f(struct S);`), an incomplete struct object, `sizeof` of an incomplete struct, member access on an incomplete struct, an unknown member, redefining a struct (including `struct S { struct S { int x; } in; }`), a duplicate member, a flexible array member, a bit-field, an anonymous struct, an empty struct, a designated initializer, assignment of a struct that has a const member, assignment to a member of a struct rvalue (`(s = t).x = 1`), `++` on a struct, two prototypes that each introduce their own struct tag (they are different types), and too many elements in a struct initializer.

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
- [x] CLI `build/compiler INPUT.c -o OUTPUT.ll`, plus `-E`, `-I`, `-D`, and `-U`
- [x] Differential tests against Clang
- [x] Block-scope `static` objects, block-scope `extern`, block-scope function declarations, and `do` / `while`
- [x] Passing and returning structs and unions by value (System V AMD64 INTEGER and MEMORY classes)
- [x] Function pointers: decay, calls, arrays, members, typedefs, and `qsort`
- [x] `switch`, `case`, and `default`, including nested labels, fallthrough, and an LLVM `switch`
- [x] `float` and `double`: constants, conversions, operators, SSE parameters and returns, and mixed INTEGER/SSE aggregates

## Supported syntax

`void` and the LP64 integer types, in any legal specifier order: `char` (signed on this target), `signed char`, `unsigned char`, `short` / `signed short` / `short int` / `signed short int`, `unsigned short` / `unsigned short int`, `int` / `signed` / `signed int`, `unsigned` / `unsigned int`, `long` / `signed long` / `long int` / `signed long int`, `unsigned long` / `unsigned long int`, `long long` / `signed long long` / `long long int` / `signed long long int`, and `unsigned long long` / `unsigned long long int`. An invalid combination (`short long`, `unsigned signed`, `long long long`, `int int`, `char int`) is a diagnostic. `char`, `signed char`, and `unsigned char` are three distinct types. `long` and `long long` are both 64 bits and are still distinct types.

Those types may be used for locals, parameters, return types, prototypes, and file-scope objects, including `extern` and `static`. A later declaration must name the same type. Parameter names do not have to match. `int f(void);` and `int f() { ... }` are compatible: both have zero parameters. Calls check the argument count. An argument, a return value, an initializer, and the right-hand side of an assignment are converted to the destination type.

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
- An untagged unnamed struct or union member is anonymous. A tagged member with no declarator is still rejected. `long double` is computed as 80-bit extended precision. No variable-length arrays. `L`, `u`, `U`, and `u8` string literals are accepted, as are `L`, `u`, and `U` character constants. `u8` character constants are rejected. A wide or Unicode character constant in a preprocessor expression keeps the corresponding character value after the prefix is removed. An empty struct or union is rejected. A flexible array member that is not last, that is the only named member, that is initialized, or that makes the struct an array element or a struct member, is a diagnostic. A variadic definition with no named parameter (`int f(...)`) is rejected. `__builtin_alloca` and the other GCC builtins are not recognized, apart from the `va_*` builtins, the memory builtins, and `__builtin_signbit`.
- Multi-character character constants (`'ab'`) are rejected; Clang accepts them as an implementation-defined `int`. A hex or octal escape outside `unsigned char` is a diagnostic, matching Clang. Runtime shift counts outside `0 .. width`, signed left shifts of negative values, and signed left shifts that do not fit are undefined and are not diagnosed outside constant initializers. Signed overflow at runtime is undefined and is not diagnosed. Modifying a string literal is undefined and is not diagnosed. A cast may drop `const`.
- `goto` jumps to a label in the same function. Jumping into the scope of an automatic initializer is not diagnosed, the same as `switch`. A label followed by a declaration (`label: int x;`) is still a diagnostic. `?:` accepts two operands of the same struct or union type. `switch` is implemented, as described above. A `case` range (`case 1 ... 3`) is a GNU extension and is rejected. Computed `goto *expr` is rejected.
- An empty parameter list on a declaration that is not a definition (`int f();`) is rejected. C11 treats that as a non-prototype. This subset requires a prototype: `(void)` or a parameter type list. A definition may still use `()`.
- A `for` header cannot declare a function or use `static` or `extern`. A block-scope function cannot be `static`, and a nested function definition is rejected. An initializer is rejected on a block-scope declaration that has linkage.
- A `static` function that is called but never defined is a diagnostic. An external function or object that is declared and not defined is emitted as `declare` or `external global`, so another translation unit can provide it.
- A tentative definition is completed inside this translation unit as a zero initializer of the object's type, which is the C11 6.9.2 rule and matches Clang's default (`-fno-common`). It is not a common symbol.
- Uninitialized locals are not zeroed. Reading them is undefined in C; the IR loads the alloca anyway.
- The compiler reports the first error only and does not write the `.ll` file.
- Columns count every byte as one column, including tabs.
- No optimizations are performed here. Clang can optimize and codegen the IR.

## CI and CodeRabbit

- `.github/workflows/ci.yml` runs on `push` and `pull_request` with workflow `contents: read`, cancel-in-progress for the same ref, and the runner `ubuntu-24.04`. The original job is unchanged: 20-minute timeout, Rust 1.85.1 (`rustfmt`, `clippy`), apt packages `clang`, `llvm`, `make`, and `python3`, Clang 15 or newer, then `cargo fmt --all -- --check`, `cargo clippy --offline --all-targets -- -D warnings`, `cargo test --offline --lib`, `make`, and `python3 harness/run_tests.py`.
- A second job, `writing-a-c-compiler suite`, fetches the pinned suite and runs `python3 harness/run_wacc.py`. It has a 30-minute timeout and inherits the workflow token, which is `contents: read`. Saving the suite cache does not need `actions: write`. For `push` and `pull_request`, the runner's cache mode is `write`. The cache key is the pinned commit.
- Stage 3 adds four jobs: `csmith differential` (20 minutes), `one true awk` (75 minutes), `gcc torture execute` (40 minutes), and `performance numbers` (25 minutes). They use the same runner and the same toolchain pin. The performance job's token is `contents: read` and `actions: write` so it can upload `reports/perf.json`. The other jobs stay at the workflow's `contents: read`. Fetched trees are cached by pin. The One True AWK job skips the private AWK Builder suites, naming the missing `AWK_SUITE_READ_TOKEN` secret, when that secret is unset. When the secret is set, it is passed as `GH_TOKEN`, and a clone, build, or suite failure fails the job.
- Pinned actions: `actions/checkout@11d5960a326750d5838078e36cf38b85af677262` (v4.4.0), `dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8` (v1) with toolchain `1.85.1`, `actions/cache@0400d5f644dc74513175e3cd8d07132dd4860809` (v4.2.4), and `actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02` (v4.6.2) on the performance job.
- `.coderabbit.yaml` follows CodeRabbit schema `https://coderabbit.ai/integrations/schema.v2.json`. The review profile is `CHILL`. The file does not install the GitHub App.
- Milestone 1's Actions runs and the CodeRabbit review on pull request 1 are in the record below. Step 2's Actions runs and the CodeRabbit review are in the step 2 record below. Step 3's Actions runs and the CodeRabbit review are in the step 3 record below. Step 4's Actions runs and the CodeRabbit review are in the step 4 record below. Step 5's Actions runs were not copied into that step's record. This change's Actions runs and the CodeRabbit review are in the switch record below.

## Stage 3 testing

Csmith, One True AWK, the AWK Builder suites, `gcc.c-torture/execute`, and a performance script. Pins and commands are in `tests/external/README.md`. Nothing here is a new platform. Compiler sources of GCC and Clang were not read. The torture checkout is only `gcc.c-torture/execute`.

### Csmith

Pin `30dccd73b78652c4719f36572994778a5b233a4e` (tag `csmith-2.3.0`). Every seed uses the options in `harness/run_csmith.py`: no packed structs (`#pragma pack` is ignored), `main(void)`, and size caps so a program finishes in seconds. The program is compiled with this compiler and with `clang -std=c11 -O0`, then both binaries run under an 8 second timeout, `RLIMIT_CPU`, and a 2 GiB address-space limit. Exit status and stdout are compared. The checksum line is the one Csmith's `main` prints.

CI runs the 40 seeds in `harness/csmith_seeds.txt`. The wall-clock limit is twice the CPU limit, so a program that burns CPU dies from `SIGXCPU` on both sides instead of one side winning a race with the parent's timeout. A local batch is `python3 harness/run_csmith.py --count 500`. It is not part of CI.

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

| Category | Tests |
| --- | --- |
| compile-error | 419 |
| builtin | 360 |
| old-style-decl | 65 |
| vla | 23 |
| nested-function | 13 |
| runtime | 10 |
| asm | 8 |
| long-double | 6 |
| complex | 6 |
| flexible-array | 4 |
| wide-literal | 1 |
| anonymous-member | 2 |
| alignas | 1 |

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

## Next actions

The testing stage is in place, and unmodified One True AWK builds with this compiler. The two awk binaries agree on `bugs-fixed` and on the upstream `testdir`, including the golden-file lines the Clang build also fails. The small C11 gaps in step 1 of `PLAN.md` are implemented: flexible array members, `u`/`U`/`u8` literals, `long double` aggregate returns, and file-scope initializers of a nested bit-field struct. Variable-length arrays, old-style K&R definitions, and `_Alignas` remain. No new platform is started. The next step in `PLAN.md` is an adversarial review that does not change compiler code.

## Small C11 gaps record

Flexible array members (C11 6.7.2.1), `u`/`U`/`u8` literals (C11 6.4.4.4 and 6.4.5), `long double` aggregate returns, and file-scope initializers that nest a bit-field struct. Local results on rustc 1.85.1 and Ubuntu Clang 18.1.3: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **83 passed**, `make` produced `build/compiler`, `python3 harness/run_tests.py` **129/129 passed**. `python3 harness/run_gcc_torture.py` was **828 passed, 918 known failures, 0 unexpected, 1746 executed, 144 builtins files skipped**. No torture line was removed: the flexible-array entries still initialize the flexible member, which C11 does not allow, and `widechar-3.c` still needs `__builtin_strlen`. `python3 harness/run_csmith.py` was **25 passed, 15 known failures, 0 unexpected**, out of 40 seeds, after seeds 21 and 22 were removed from the known-failure list. `python3 harness/run_onetrueawk.py` built unmodified awk. `bugs-fixed` is 31 programs and the two binaries agree. `testdir` prints the same nine `BAD` lines for both builds.

`tests/115_fam.c` checks `sizeof`, `offsetof`, a local initializer that does not touch the flexible member, indexing through a larger object, and returning the fixed part. `tests/116_uchar.c` includes `<uchar.h>` and checks `u`/`U`/`u8` literals, concatenation with an unprefixed literal, and the widths of `char16_t` and `char32_t`. `tests/117_ldret.c` returns `struct { long double }`, `union { long double }`, and the larger memory-class aggregates. `tests/118_bitnest.c` initializes a nested bit-field struct, a plain member beside the bit-field, an array of them, and a sibling pointer, all at file scope. A `volatile` aggregate returned in `%st0` is `load volatile x86_fp80`. An `__asm__` label accepts an ordinary or `u8` string and rejects a wide prefix. `tests/xabi/04_ldagg_def.c` and `04_ldagg_use.c` are linked both ways with Clang. A pure `long double` aggregate is class `X87`: the argument is `byval` and the return is `x86_fp80` in `%st0`, which is what the psABI and Clang do.

GitHub Actions and CodeRabbit for this step are filled in after the pull request runs.

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

A union is laid out like a struct except every member starts at offset 0. The size is the largest member, rounded up to the strictest alignment. A brace initializer, and an unbraced initializer, initializes the first member only; further initializers belong to the enclosing object. The address of an object inside a union initializer is a diagnostic, because the LLVM constant is raw bytes and cannot hold a relocation. A null pointer, and an integer converted to a pointer, are ordinary bytes. Assignment copies the whole object with `llvm.memmove`, as for a struct. The LLVM type is a one-element array whose element has the union's alignment, so a union member inside a struct is aligned the way the ABI requires. Member access uses a byte `getelementptr`. A union is passed and returned by value with the same classification as a struct. `<stdlib.h>`'s `div` and `ldiv` return `div_t` and `ldiv_t` in registers; `tests/102_div.c` calls the host libc.

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
