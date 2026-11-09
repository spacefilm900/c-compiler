# STATUS

## Current milestone

**Milestone 2, step 4 of 6 — pointers and arrays.** Done.

The compiler is a Rust binary. `make` installs it at `build/compiler`. It accepts the Milestone 1 integer subset, prototypes, file-scope objects, the integer operators, the LP64 integer types, and pointers and arrays: declarators, `void *`, null pointer constants, unary `&` and `*`, pointer arithmetic, casts, array decay, subscripts, brace initializers, and address-constant initializers. It rejects other input with a diagnostic. It is not a full C compiler.

## Milestone 2 plan

1. **Done.** Function prototypes and file-scope `int` globals (tentative definitions, `extern`, `static`, integer constant initializers, internal linkage).
2. **Done.** Remaining integer operators: bitwise, shifts, `++`/`--`, compound assignment, comma, ternary, `sizeof`.
3. **Done.** `char` / `short` / `long` / unsigned, integer promotions, conversions, and casts.
4. **Done.** Pointers and arrays.
5. String literals and external calls such as `printf` / `puts` (hello world).
6. Simple structs.

## Language

Rust, edition 2021, standard library only. No crates.io dependencies (`Cargo.lock` has none). `rust-toolchain.toml` pins **1.85.1** plus `rustfmt` and `clippy`.

## Tests for this step

Run locally on the first commit of this step, then again after the review fix (reject a global initializer that truncates an address, and reject a subscript of a pointer to an incomplete array). The second run was `cargo fmt --all -- --check`, `cargo clippy --offline --all-targets -- -D warnings`, and `cargo test --offline --lib` (**37 passed, 0 failed**). The harness programs did not change, so `python3 harness/run_tests.py` was not run again. GitHub Actions for this step is recorded after that workflow finishes.

| Command | Toolchain | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | rustc **1.85.1** (4eb161250 2025-03-15) | clean |
| `cargo clippy --offline --all-targets -- -D warnings` | same | clean |
| `cargo test --offline --lib` | same | **37 passed, 0 failed** |
| `make` | GNU make **4.3** | `build/compiler` produced |
| `python3 harness/run_tests.py` | Ubuntu Clang **18.1.3** (`x86_64-pc-linux-gnu`), Python **3.12.3** | **61/61 passed** |

`cargo test` includes lexer regressions, parser AST dumps (including `int *a[3]` versus `int (*p)[3]`), negative diagnostics, a CLI check, IR checks (a constant return, globals, `declare` / `define internal`, the integer operators, pointer globals, `getelementptr`, assembled and executed), and a differential run of every `tests/*.c` file against `clang -std=c11`. The harness repeats that differential check through `build/compiler` (release binary) and compares exit status, stdout, and stderr.

Harness programs: `01_return_const` (42), `02_add` (5), `03_loop` (55), `04_call` (42), `05_if` (2), `06_logic` (9), `07_while` (6), `08_cmp` (3), `09_arith` (17), `10_shadow` (4), `11_break` (25), `12_recursion` (16), `13_void` (6), `14_fallthrough` (0), `15_comment` (5), `16_decls` (12), `17_empty_params` (7), `18_forward_call` (42), `19_mutual` (2), `20_global_counter` (5), `21_static_global` (7), `22_tentative` (9), `23_shadow_global` (18), `24_const_init` (0), `25_extern_global` (8), `26_unnamed_params` (6), `27_multi_decl` (7), `28_void_forward` (9), `29_extern_def` (11), `30_bitwise` (0), `31_shifts` (0), `32_incdec` (0), `33_compound` (0), `34_ternary` (0), `35_comma` (0), `36_sizeof` (0), `37_const_ops` (0), `38_promotions` (0), `39_unsigned` (0), `40_casts` (0), `41_suffixes` (0), `42_chars` (0), `43_sizeof_types` (0), `44_globals` (0), `45_calls` (0), `46_trunc` (0), `47_const_types` (0), `48_swap` (0), `49_arith` (0), `50_sum` (0), `51_multi` (0), `52_pp` (0), `53_param` (0), `54_global` (0), `55_sizeof` (0), `56_cmp` (0), `57_sort` (0), `58_cast` (0), `59_cond` (0), `60_init` (0), `61_decay` (0).

`cargo test` also links two translation units both ways (this compiler's IR with a Clang object) for `char` / `unsigned char` / `short` / `unsigned short` parameters and returns. Both directions exit 0, matching Clang compiling the whole program.

Negative cases covered inside `cargo test` (each must be a non-zero diagnostic, not a successful compile): missing semicolon, missing `)`, function pointers, variable-length arrays, a non-positive array size, structs, `sizeof(float)`, `sizeof(void)`, a `?:` missing `:`, old-style `int f();` declarations, unnamed parameters on a definition, `static` locals, block-scope `extern`, function initializers, multiple storage-class specifiers, `void` variables, an incomplete local array, storage class on a parameter, nested functions, `switch`, `do`/`while`, stray `else`, casts to `float`, varargs, strings, floats, invalid integer suffixes (`1Ll`, `1uu`), integer constants that fit in no target type, out-of-range hex and octal escapes, empty and multi-character character constants, unknown escapes, unterminated comments, preprocessor lines, non-ASCII, invalid type-specifier combinations (`short long`, `unsigned signed`, `long long long`, `int int`, `char int`), conflicting integer types (`char` vs `signed char`, `int` vs `unsigned`, `long` vs `long long`), incompatible pointer assignment (`int *` from `char *`), arithmetic on `void *`, a subscript of a pointer to an incomplete array, a pointer compared with `0` using `<`, an array assigned to, the address of a non-lvalue, a non-constant pointer initializer, a global initializer that truncates an address (`int x = (int)&g` and `long x = (int)&g`), excess array initializers, undeclared names, redefinitions, conflicting function types, a variable redeclared as a function, conflicting linkage, non-constant, divide-by-zero, and overflowing global initializers, undefined shifts in a constant initializer (`1 << 32`, `1 << -1`, `-1 << 1`, `1 << 40`), `1 << 31` and `1L << 63` as overflowing constants, `++` / assignment / comma in an evaluated constant expression, a `?:` mixing `void` with an integer, `++` or compound assignment applied to a non-lvalue, bad call arity against a prototype, a static function that is used but not defined, `break`/`continue` outside a loop, `void` returns, non-assignable expressions, calls of non-functions, and a call to a function that has not been declared yet.

GitHub Actions for this step is in the step 4 record below.

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
- [x] CLI `build/compiler INPUT.c -o OUTPUT.ll`
- [x] Differential tests against Clang

## Supported syntax

`void` and the LP64 integer types, in any legal specifier order: `char` (signed on this target), `signed char`, `unsigned char`, `short` / `signed short` / `short int` / `signed short int`, `unsigned short` / `unsigned short int`, `int` / `signed` / `signed int`, `unsigned` / `unsigned int`, `long` / `signed long` / `long int` / `signed long int`, `unsigned long` / `unsigned long int`, `long long` / `signed long long` / `long long int` / `signed long long int`, and `unsigned long long` / `unsigned long long int`. An invalid combination (`short long`, `unsigned signed`, `long long long`, `int int`, `char int`) is a diagnostic. `char`, `signed char`, and `unsigned char` are three distinct types. `long` and `long long` are both 64 bits and are still distinct types.

Those types may be used for locals, parameters, return types, prototypes, and file-scope objects, including `extern` and `static`. A later declaration must name the same type. Parameter names do not have to match. `int f(void);` and `int f() { ... }` are compatible: both have zero parameters. Calls check the argument count. An argument, a return value, an initializer, and the right-hand side of an assignment are converted to the destination type.

Function definitions and prototypes; unnamed parameters on a prototype (`int f(int, int);`); file-scope objects; locals with optional initializers; blocks; `if` / `else`; `while`; `for` (including `for (unsigned i = ...)`); `break`; `continue`; `return`; empty statements; unary `+` `-` `!` `~`; `+` `-` `*` `/` `%` `&` `|` `^` `<<` `>>`; comparisons; short-circuit `&&` and `||`; `?:`; assignment and compound assignment; prefix and postfix `++`/`--`; the comma operator; `sizeof` on an expression or on a type name such as `sizeof(unsigned long)`; explicit casts `(type)expr` and `(void)expr`; direct calls; decimal, octal, and hex constants with the suffixes below; character constants; `//` and `/* */` comments.

Sizes on `x86_64-pc-linux-gnu`: `char` / `signed char` / `unsigned char` are 1, `short` / `unsigned short` are 2, `int` / `unsigned int` are 4, and `long` / `unsigned long` / `long long` / `unsigned long long` are 8. The module triple is `x86_64-pc-linux-gnu`. IR uses opaque pointers (LLVM 15+). Locals are allocated in the entry block. File-scope objects are LLVM globals (`global`, `internal global`, or `external global`) with natural alignment. A defined function is `define`; a prototype with no body and external linkage is `declare`. `static` is internal linkage.

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

File-scope `name;` and `static name;` are tentative definitions. If the translation unit never gives that object an initializer, it is emitted as zero of its type. A later initializer replaces the tentative value. `extern` without an initializer does not define the object; `extern name = expr;` does. The initializer must be an integer constant expression (C11 6.6) and is converted to the object's type, including truncation. The operators above are allowed, including casts, `?:`, `sizeof`, `~`, bitwise operators, and shifts. Assignment, `++`/`--`, function calls, and the comma operator are rejected when they are evaluated. An unevaluated operand may contain them. An evaluated division or remainder by zero is a diagnostic. `-2147483647 - 1` is the minimum `int`. `-9223372036854775807L - 1` is the minimum `long`.

A file-scope function with no storage class is treated as `extern`, so `static int f(void); int f(void) { ... }` keeps internal linkage. A file-scope object with no storage class has external linkage and does not inherit a prior `static`. The same identifier with both internal and external linkage is a diagnostic. Two bodies, or two initialized definitions, are a redefinition. A prototype parameter name is not visible outside that prototype. A local shadows a global of the same name. A global is in scope for its own initializer, but an identifier is not a constant, so `int g = g;` is rejected.

`int main(void)` and `int main()` are both no-parameter definitions. Reaching the closing brace of an integer function lowers to `ret` of 0 of that type. That is what C requires for `main`. A pointer-returning function that falls off the end lowers to `ret ptr null`. A `void` function that falls off the end lowers to `ret void`. The OS truncates `main`'s return value to 8 bits, so the tests keep exit codes in 0..255 and print nothing.

Pointers and arrays use the same declarator grammar as C. `*` binds less tightly than `[]` and `()`, so `int *a[3]` is an array of three `int *` and `int (*p)[3]` is a pointer to an array of three `int`. `void *` is an object pointer. `0` and `(void *)0` are null pointer constants; `(int *)0` is a null pointer of type `int *` but not a null pointer constant. Unary `&` requires an lvalue, including an array. Unary `*` requires a pointer. `&*p` and `*&x` are the address of the object; the IR does not load through the cancelled `*`. An array expression decays to a pointer to its first element except as the operand of `sizeof` or unary `&`. `a[i]` and `i[a]` are the same lvalue. A parameter declared as an array is adjusted to a pointer, so `sizeof` on `int a[10]` in a parameter list is 8. A function parameter of function type, and any function pointer, is rejected.

Pointer plus or minus an integer scales by the size of the pointed-to type (`getelementptr inbounds`). The integer is sign- or zero-extended to i64. Pointer minus pointer, for compatible complete object types, has type `long` (`ptrdiff_t`) and is `ptrtoint`, `sub`, and `sdiv exact` by the element size (`char` has no division). `++`/`--` and `+=`/`-=` on a pointer add or subtract one element. Other compound assignments on a pointer are rejected. Arithmetic on `void *` is rejected. Clang warns and treats the pointee size as 1 (`-Wpointer-arith`); this compiler does not accept it. Relational `<` `<=` `>` `>=` require compatible object pointers and lower to `icmp u*`. Equality also allows `void *` with an object pointer, and a pointer with a null pointer constant. `p < 0` is rejected. Clang warns for distinct pointer types and for a pointer compared with an integer; those are errors here. A pointer is true in `if`, `!`, `&&`, `||`, and the condition of `?:` when it is not null.

`?:` follows C11 6.5.15. Both arms arithmetic: the usual arithmetic conversions. Both `void`: `void`. Both pointers to compatible types: the composite type. One arm a pointer and the other a null pointer constant: the pointer type. One arm an object pointer and the other `void *`: `void *`. Other mixes are errors.

An integer converts to a pointer by extending it to i64 (`sext` if signed, `zext` if unsigned) and `inttoptr`. A constant 0 becomes `null`. A pointer converts to an integer by `ptrtoint` directly to that integer's width. Pointer-to-pointer is a no-op in the IR (opaque `ptr`). This matches Clang on `x86_64-pc-linux-gnu`: `(int *)(int)-1` is all-ones, and `(int *)(unsigned)-1` is the low 32 bits zero-extended. Assignment allows an arithmetic conversion, a compatible pointer, `void *` to or from an object pointer, or a null pointer constant. `int *p; char *c; p = c;` is an error. Clang only warns (`-Wincompatible-pointer-types`) and still compiles.

Arrays may be locals or globals, with any supported element type, including pointers and further arrays. `int a[] = {1, 2, 3}` and `int m[][3] = {{1}, {4, 5, 6}}` take their outer length from the initializer. A brace list may be partial; the rest is zero. A nested brace initializes one subaggregate. A flat list fills subaggregates in order (`int m[2][3] = {1, 2, 3, 4}`). One pair of braces around a scalar is allowed (`int x = {1}`). An empty list and an extra initializer are errors. A local aggregate is stored as `zeroinitializer` and then the explicit elements are stored. A global or `static` object with no initializer is zero, including a null pointer. An incomplete array that is defined and never completed (`int a[];` with no later size) is an error. `extern int a[];` is an external `[0 x i32]`, which is what Clang emits.

A global initializer may be an integer constant expression or an address constant: `&g`, `&arr[2]`, `arr + 1`, a null pointer constant, or a cast of those. The IR uses `getelementptr (i8, ptr @obj, i64 byte_offset)` and `ptrtoint` / `inttoptr` when the destination is an integer or the pointer was converted from an integer. A cast of an address through a 64-bit integer and back (`(int *)(unsigned long)&g`) is the same address. Truncating an address to 32 bits in a global initializer is rejected; the bits are not known until link time.

An array of 16 bytes or more has alignment at least 16, for locals and globals. That is the System V AMD64 rule. Smaller arrays use the element alignment. Pointers are 8-byte aligned.

## Limitations

- No preprocessor, trigraphs, or digraphs. A backslash-newline is rejected, including when it ends a `//` comment or appears inside a block comment. It is not deleted the way C translation phase 2 would delete it.
- An integer constant that fits in no type on the C11 6.4.4.1 list for its suffix and base is a diagnostic. A decimal constant that does not fit in signed `long long` is rejected even when it fits in `unsigned long long`.
- No structs, unions, enums, typedefs, qualifiers, or types other than `void`, the integer types above, pointers to those, and arrays of those. No `_Bool`, `float`, or `double`. No function pointers, no variable-length arrays, and no string literals.
- No string literals. Multi-character character constants (`'ab'`) are rejected; Clang accepts them as an implementation-defined `int`. A hex or octal escape outside `unsigned char` is a diagnostic, matching Clang. Runtime shift counts outside `0 .. width`, signed left shifts of negative values, and signed left shifts that do not fit are undefined and are not diagnosed outside constant initializers. Signed overflow at runtime is undefined and is not diagnosed.
- No `switch`, `do`/`while`, or `goto`.
- An empty parameter list on a declaration that is not a definition (`int f();`) is rejected. C11 treats that as a non-prototype. This subset requires a prototype: `(void)` or a parameter type list. A definition may still use `()`.
- `static` and `extern` are only accepted at file scope. A `static` local and a block-scope `extern` declaration are diagnostics.
- A `static` function that is called but never defined is a diagnostic. An external function or object that is declared and not defined is emitted as `declare` or `external global`, so another translation unit can provide it.
- A tentative definition is completed inside this translation unit as a zero initializer of the object's type, which is the C11 6.9.2 rule and matches Clang's default (`-fno-common`). It is not a common symbol.
- Uninitialized locals are not zeroed. Reading them is undefined in C; the IR loads the alloca anyway.
- The compiler reports the first error only and does not write the `.ll` file.
- Columns count every byte as one column, including tabs.
- No optimizations are performed here. Clang can optimize and codegen the IR.

## CI and CodeRabbit

- `.github/workflows/ci.yml` runs on `push` and `pull_request` with `contents: read`, a 20-minute job timeout, and cancel-in-progress for the same ref. The runner is `ubuntu-24.04`. It installs Rust 1.85.1 (`rustfmt`, `clippy`) and apt packages `clang`, `llvm`, `make`, and `python3`, and refuses Clang older than 15. Steps: `cargo fmt --all -- --check`, `cargo clippy --offline --all-targets -- -D warnings`, `cargo test --offline --lib`, `make`, `python3 harness/run_tests.py`.
- Pinned actions: `actions/checkout@11d5960a326750d5838078e36cf38b85af677262` (v4.4.0), `dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8` (v1) with toolchain `1.85.1`.
- `.coderabbit.yaml` follows CodeRabbit schema `https://coderabbit.ai/integrations/schema.v2.json`. The review profile is `CHILL`. The file does not install the GitHub App.
- Milestone 1's Actions runs and the CodeRabbit review on pull request 1 are in the record below. Step 2's Actions runs and the CodeRabbit review are in the step 2 record below. Step 3's Actions runs and the CodeRabbit review are in the step 3 record below. This step's Actions runs and the CodeRabbit review are in the step 4 record below.

## Next actions

1. Milestone 2 step 5: string literals and external calls such as `printf` / `puts`.

## Milestone 2 step 4 record

Local results are in the table above: `cargo test --offline --lib` **37 passed**, `python3 harness/run_tests.py` **61/61 passed**, on rustc 1.85.1 and Ubuntu Clang 18.1.3. After the review fix, `cargo fmt --all -- --check` and `cargo clippy --offline --all-targets -- -D warnings` were clean and `cargo test --offline --lib` was **37 passed** again. The harness was not re-run.

GitHub Actions and the CodeRabbit review are filled in after those runs finish. Nothing is claimed for them yet.

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
