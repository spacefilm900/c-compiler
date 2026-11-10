# Clean-room C compiler → LLVM IR

A small C compiler that emits textual LLVM IR. LLVM (`clang`, and optionally `opt` / `llc`) optimizes that IR and generates native code. This milestone is a C subset: enough to lex, preprocess, parse, check, and run functions and file-scope objects of the LP64 integer types, with prototypes, pointers, arrays, string literals, `const`, `volatile`, and `restrict`, calls to variadic library functions such as `printf`, simple structs and unions passed and returned by value, `float` and `double`, typedefs, enumerations, `_Bool`, function pointers, arithmetic, bitwise and shift operators, `++`/`--`, compound assignment, `?:`, the comma operator, `sizeof`, casts, branches, loops, `switch`, and calls. A built-in preprocessor runs before the parser. Hello world compiles from `#include <stdio.h>`, and that header is the host's. It is not a full C compiler.

## Why Rust

The compiler is written in Rust. That is a deliberate break from the usual practice of implementing C in C or C++. Rust gives memory safety without a garbage collector, and Cargo builds a single static binary from the standard library alone. There are no crates.io dependencies, so a checkout builds without fetching packages.

The frontend was written from the C standard, the LLVM Language Reference, and the System V AMD64 ABI. It is not derived from Clang or GCC. Clang is used only as a black-box oracle in the test harness and as the LLVM backend that links the emitted `.ll` file. The compiler does not call into Clang's or GCC's frontend.

## Build

Emitting IR does not need Clang. Running the result does. The IR uses opaque pointers, the triple `x86_64-pc-linux-gnu`, and that triple's System V data layout, so Clang/LLVM must be 15 or newer.

`rust-toolchain.toml` pins Rust **1.85.1** with the `rustfmt` and `clippy` components. That matches the system `cargo` used for the local unit tests. On that machine, plain `cargo clippy` is not installed; clippy is the rustup command below.

```sh
make
# equivalent:
# cargo build --release
# cp -f target/release/compiler build/compiler
```

`make` (GNU make 4.4.1 locally) writes `build/compiler`.

Local execution used Debian Clang/LLVM **19.1.7** unpacked under `~/opt/llvm19-root` and exposed through wrappers on `~/.local/bin`. Any Clang/LLVM 15+ on `PATH` is enough for the harness.

## Contract

```sh
build/compiler [options] INPUT.c -o OUTPUT.ll
build/compiler -E [options] INPUT.c [-o OUTPUT.i]
```

Options are `-I DIR` (also `-IDIR`), `-D NAME[=text]` (also `-DNAME=text`; a bare `-DNAME` defines `NAME` as `1`), `-U NAME`, and `-E`. `-D` and `-U` apply in order, after the predefined macros.

On success the command exits 0 and writes a textual LLVM IR module, or the preprocessed text when `-E` is set. On failure it exits non-zero, prints one diagnostic to stderr (`file:line:column: error: ...`), and does not create the output file. A diagnostic in an included file names that file.

```sh
clang -w output.ll -o program
```

## Test

```sh
make test
# or, separately:
cargo test
python3 harness/run_tests.py
```

`cargo test` runs the lexer, parser, diagnostic, and IR unit tests, and compares every `tests/*.c` program with Clang. `harness/run_tests.py` is the project differential runner: it compiles each `tests/*.c` with this compiler and with Clang, links both, and compares exit status, stdout, and stderr.

On the machine where system `cargo` is Rust 1.85.1 and has no clippy component, the clippy check that passed was:

```sh
~/.cargo/bin/rustup run stable cargo clippy --offline --all-targets -- -D warnings
```

The numeric version of that rustup `stable` toolchain was not recorded.

## CI and CodeRabbit

`.github/workflows/ci.yml` runs on `push` and `pull_request`. The GitHub source of truth is `https://github.com/spacefilm900/c-compiler`. Observed Actions runs, including whether any run has completed, are recorded in `STATUS.md`.

The workflow checks out the repo, installs Rust 1.85.1 (`rustfmt`, `clippy`), and installs Ubuntu 24.04 packages `clang`, `llvm`, `make`, and `python3`. It exits if `clang --version` reports a major version below 15. Then it runs `cargo fmt --all -- --check`, `cargo clippy --offline --all-targets -- -D warnings`, `cargo test --offline --lib`, `make`, and `python3 harness/run_tests.py`. The job token can read contents only. The job times out after 20 minutes, and a newer push to the same ref cancels the older run.

Actions are pinned to commits, not floating tags:

- `actions/checkout` `11d5960a326750d5838078e36cf38b85af677262` (v4.4.0)
- `dtolnay/rust-toolchain` `683b588c487a69d01f6c6ddf40f036ff682cc2c8` (v1), with `toolchain: 1.85.1`

`.coderabbit.yaml` is review configuration validated against CodeRabbit's published schema. The file does not install the GitHub App. Whether the App is active on this repository, and what it did on the import pull request, is recorded in `STATUS.md`. The optional CodeRabbit skills install was skipped.

## Supported subset

- One translation unit of function definitions, prototypes, and file-scope objects. The integer types are the LP64 sizes: `char` (signed), `signed char`, and `unsigned char` are 8 bits; `short` and `unsigned short` are 16; `int` and `unsigned int` are 32; `long`, `unsigned long`, `long long`, and `unsigned long long` are 64. `void` is a return type and the sole unnamed parameter (`int main(void)` and `int main()` are both no-parameter definitions). Specifiers may appear in any legal order (`unsigned long long int`, `int long unsigned`). `signed` alone is `int`. `unsigned` alone is `unsigned int`. Invalid combinations are diagnostics. `char` and `signed char` are not the same type.
- Prototypes: `int f(void);`, `char id(char c);`, unnamed parameters (`int f(int, char);`), and a trailing ellipsis on a declaration (`int printf(const char *, ...);`). A later definition may use different parameter names. The types must match, including the return type and whether the function is variadic. A call checks the argument count. Fixed arguments are converted as if assigned to the parameter. Arguments past the fixed parameters get the default argument promotions (`char` and `short` become `int`, and `float` becomes `double`). A function declared but not defined is an LLVM `declare`, so an external definition can be linked in. `static` on a file-scope function gives it internal linkage. A call to a function that has not been declared is an error. C11 has no implicit function declarations. Defining a variadic function in this file is rejected.
- On this target, `char`, `signed char`, and `short` parameters and return values are passed with LLVM `signext`. `unsigned char` and `unsigned short` use `zeroext`. That is the System V AMD64 rule that those values are extended to 32 bits. `int` and wider types have no extension attribute.
- `const` and `volatile` on declaration specifiers and after `*` (`const char *`, `char *const`, `const int`). The same qualifier twice in one list is a diagnostic. Assigning to a const-qualified object, including `*p` when `p` has type `const char *`, is a diagnostic. `char *` may be passed to a `const char *` parameter. `const char *` may not be passed to `char *`, and `char **` may not be passed to `const char **`. Top-level `const` on a parameter is not part of the function type. `volatile` is tracked on the type and does not change the IR. `restrict` is a qualifier on a pointer: it participates in compatibility and assignment, it is dropped from a parameter's function type the same way top-level `const` is, and it does not change the IR. `restrict` on a non-pointer is a diagnostic.
- File-scope objects of those integer types, of `float` and `double`, of pointers, and of arrays: `char g;`, `unsigned long g = expr;`, `float f = 1.5f;`, `double d = 1.5;`, `int *p = &g;`, `int a[4] = {1, 2};`, `int b[] = {1, 2, 3};`, `char s[] = "hi";`, `char *msg = "hi";`, `extern int g;`, and `static short g;`. Several declarators are allowed, and `*` belongs to each declarator (`int *a, b` is one pointer and one `int`). A declaration without an initializer and without `extern` is a tentative definition; if nothing in the file initializes it, it is zero (a null pointer, or `zeroinitializer` for an array). `extern` without an initializer does not define the object. An integer initializer is an integer constant expression. A `float` or `double` initializer is a floating constant expression. A pointer initializer may also be an address constant (`&g`, `&a[2]`, `a + 1`, or a string literal) or a null pointer constant. A const object is an LLVM `constant`. Locals and parameters shadow globals.
- Locals and `for (char c = ...)` declarations, with optional initializers. A block may also declare `static int x = 1` (a constant initializer, no linkage, the value persists across calls) and `extern int g` or `int f(int)` (linkage follows C11 6.2.2, and the name is not visible after the block). `static` and `extern` are not allowed on a `for` header, and a block-scope function cannot be `static`. The initializer is converted to the variable's type. Arrays use a brace list. A partial list zeros the rest. `int a[] = {1, 2, 3}` gets its length from the list. A character array may also be initialized from a string: `char s[] = "hi"` is three bytes including NUL, `char s[10] = "hi"` zero-fills, and `char s[2] = "hi"` has no NUL. A longer string is a diagnostic.
- Statements: blocks, expression statements, empty statements, `return`, `if` / `else` (an `else` binds to the nearest `if`), `while`, `do` / `while` (`continue` jumps to the condition), `for`, `break`, `continue`, `switch` / `case` / `default`. The controlling expression is promoted. Each case constant is converted to that type. Labels may be nested inside the body, and `break` leaves the switch. The IR is an LLVM `switch`.
- Pointer declarators, including `int **pp`, `int *a[3]`, and `int (*p)[3]`, plus `void *`. Unary `&` and `*`. Subscripts `a[i]` and `i[a]`. Array-to-pointer decay except under `sizeof` and `&`. Pointer plus or minus an integer, scaled by the element size. Pointer minus pointer, with type `long`. Pointer equality, including null and `void *`, and relational comparisons of compatible pointers. `++`/`--` and `+=`/`-=` on pointers. Pointers in `if`, `!`, `&&`, `||`, and `?:`.
- Expressions, with C precedence and associativity: integer constants (decimal, octal, hexadecimal, with `u`/`U`, `l`/`L`, `ll`/`LL`, and the combined suffixes), floating constants (decimal and hexadecimal, with an optional `f`/`F` suffix), character constants (`'a'`, `'\n'`, `'\0'`, `'\x41'`, `'\101'`), string literals (`"hi"`, `"\n"`, `"\x41"`, `"hel" "lo"`), identifiers, parentheses, casts (integers, `float` and `double`, pointers, and between an integer and a pointer), unary `+` `-` `!` `~` `&` `*`, binary `+` `-` `*` `/` `%` `&` `|` `^` `<<` `>>`, comparisons, `&&` and `||`, `?:`, assignment and compound assignment, prefix and postfix `++`/`--`, the comma operator, `sizeof` on an expression or a type (including `sizeof(int *)`, `sizeof(int[3])`, and `sizeof "abc"`, which is 4), and direct calls, including variadic calls. Pointers and arrays may be parameters and return types. An array parameter is adjusted to a pointer. A string literal has type `char[N]` including the terminating NUL and decays to `char *` except under `sizeof` and `&`. Its address is a private unnamed_addr constant. Modifying that object is undefined and is not diagnosed.
- Integer promotions and the usual arithmetic conversions apply. Types narrower than `int` promote to `int`. Unsigned arithmetic wraps. Signed `/` and `%` round toward zero; unsigned `/` and `%` use `udiv` and `urem`. Comparisons and `&&` / `||` produce `int` 0 or 1. `-1 < 1u` is false. `&&`, `||`, and `?:` evaluate only the operands that C evaluates.
- `sizeof` is not evaluated. Its result type is `unsigned long`, and `sizeof(sizeof(int))` is 8. `sizeof(void)` and `sizeof` of an incomplete array are rejected. `sizeof(int *)` is 8. `sizeof` of an array is the element size times the length, not the size of a pointer.
- A signed right shift is arithmetic. An unsigned right shift is logical. A shift count outside `0 .. width` of the promoted left operand, a signed left shift of a negative value, and a signed left shift that does not fit are undefined. Constant initializers reject those shifts. A runtime shift is emitted without a count mask and without a diagnostic.
- `++`/`--` and compound assignment require an lvalue. On an integer, the operation uses the promoted type and the result is converted back. On a pointer, `++`/`--` and `+=`/`-=` move by one element. Compound assignment evaluates the right-hand side, then loads the lvalue once.
- Converting an out-of-range value to a signed type keeps the low bits and interprets them as two's complement, matching Clang on this target. `(char)1000` is `-24`. Character constants whose byte is above 127 are negative because `char` is signed: `'\xff'` is `-1`. Converting an integer to a pointer extends it to 64 bits and uses those bits as the address. Converting a pointer to an integer keeps the address bits, truncated if the integer is narrower than 64. `(int *)(int)-1` and `(int *)(long)-1` are the same all-ones address. `(int *)(unsigned)-1` is the low 32 bits only.
- Assigning a pointer requires a compatible pointer, a qualification conversion (`char *` to `const char *`, and the same rule for `volatile`), `void *` in either direction when the destination does not drop a qualifier, or a null pointer constant (`0` or `(void *)0`). `int *` and `char *` are not compatible. Dropping `const` is an error here. Clang warns and still compiles. A cast may drop a qualifier; that is not diagnosed.
- A function may call itself, and it may call a function that was declared earlier and defined later. Assignment targets are variables; parentheses do not change that (`(x) = 1` assigns to `x`).
- Tagged structs at file scope and block scope. Members may be any supported type: integers, pointers, arrays, and nested structs, including a struct declared in the member list. `struct node;` is an incomplete type. A pointer to it is complete, so a struct may contain a pointer to itself. An object of incomplete struct type, and `sizeof` of that type, are diagnostics. Layout follows the System V AMD64 ABI: each member is aligned to its alignment, the struct alignment is the strictest member alignment, and the size is rounded up to that alignment. An array member uses the element alignment. The extra 16-byte alignment applies to a local or global array variable, not to a member. `.` and `->` name a member, including chains (`a.b.c`, `p->next->val`), and the result is an lvalue when the base is. `&s.x` and `s.arr[i]` work. A whole struct may be assigned, which copies every byte, including padding. Brace initializers work for structs and for arrays of structs, at local and file scope. A missing member is zero. A nested brace initializes one member. A flat list fills inner aggregates in order. An expression of the member's struct type initializes that member as a whole (`struct Line line = {p, q}`). Designated initializers (`.x = 1`, `[0] = 1`) are rejected. A struct with a const member cannot be assigned as a whole. A complete struct or union may be passed or returned by value. Aggregates of at most 16 bytes whose fields are integers, pointers, `float`, or `double` are coerced to register pieces; larger ones, and ones that need more registers than remain, use `byval` and `sret`. `f().x`, assignment from a call, and `?:` on a struct copy the result. An SSE eightbyte is passed in an XMM register. Returning an x87 aggregate is a diagnostic. A prototype may name an incomplete struct; a definition and a call need a complete type.
- `//` line comments and non-nesting `/* */` block comments. A backslash immediately before a new-line is deleted first (C translation phase 2), including inside a comment or a string, so the following physical line is part of the same logical line. `__LINE__` is the physical line of the token.
- A built-in preprocessor. `#define` / `#undef` cover object-like macros and function-like macros, including an empty parameter list and `__VA_ARGS__`. The `(` must follow the name with no space for the definition to be function-like. `#` stringifies an argument and `##` pastes two tokens. Expansion rescans, and a macro is not expanded again inside its own replacement. An identical redefinition is allowed; a different one is a diagnostic. `#if`, `#ifdef`, `#ifndef`, `#elif`, `#else`, and `#endif` nest. `#if` expressions are evaluated in `intmax_t` / `uintmax_t`. `defined X` and `defined(X)` are recognized, and any other identifier is 0. A group whose condition is false is skipped: macros there are not expanded and `#error` there is not fired. `#include "file"` searches the including file's directory, then each `-I` directory, then the compiler's `include/`, then the host directories. `#include <file>` searches `-I` directories, then `include/`, then `/usr/include/x86_64-linux-gnu` and `/usr/include`. `#include_next` resumes after the directory that provided the current file. The nested-include limit is 200. `#error` is a diagnostic. `#line` is accepted. `#pragma once` is honored; any other `#pragma` is ignored. A line that is only `#` is a null directive. Predefined names include `__FILE__`, `__LINE__`, `__STDC__` (1), `__STDC_VERSION__` (`201112L`), `__STDC_HOSTED__` (1), `__x86_64__`, `__linux__`, `__LP64__`, `__SIZE_TYPE__`, and `__USER_LABEL_PREFIX__`.
- `typedef` names an existing type, including a pointer, an array, a struct or union, or a function type. A typedef is an alias: `typedef int T` and `int` are the same type. The parser resolves the typedef-name versus identifier ambiguity with a scope stack.
- Enumerations. `enum E { A = 1, B }` defines `A` and `B` as `int` constants and gives `enum E` a type compatible with `int`. An anonymous `enum { RED, GREEN }` introduces the constants only.
- Unions. Members start at offset 0. The size is the largest member, rounded up to the strictest alignment. `.` and `->` name a member. A brace initializer initializes the first member. Assignment copies every byte. A forward `union U;` is an incomplete type, and a pointer to it is complete.
- `_Bool` (and `<stdbool.h>`'s `bool`, `true`, and `false`). A nonzero value converts to 1. `restrict` is a pointer qualifier and does not change the IR. Function pointers may be declared, assigned, compared with `==` and `!=`, stored in arrays and structs, and called as `fp(...)` or `(*fp)(...)`. A function designator decays to a pointer. `typedef` of a function type works. `qsort` takes a comparator. Relational `<` on function pointers is a diagnostic. `float` and `double` are computed, including constants, conversions, and SSE calling conventions. `long double` may appear in a declaration; an expression that produces one is a diagnostic.
- `__attribute__((...))` and `__extension__` are ignored. `__asm__("name")` is the linker name of a function.

`include/` holds headers a compiler supplies rather than the C library: `stddef.h` (`size_t`, `ptrdiff_t`, `wchar_t`, `NULL`, `offsetof`), `stdarg.h` (`va_list` and the `va_*` macros; the macros do not implement argument access), `stdbool.h`, and `limits.h` (the ISO limits for this LP64 target). `#include <stdio.h>` and the other library headers come from the host. Clang, when it compiles the same test, uses those same host headers.

Reaching the closing brace of an integer function lowers to a return of 0 of that type. That is what C requires for `main`. A `float` or `double` function that falls off the end lowers to a return of `0.0`. A `void` function that falls off the end lowers to `ret void`.

## Not accepted

Unsupported or invalid input is a diagnostic. It is not ignored and it is not lowered.

Trigraphs are not recognized. `_Pragma` is rejected or left as an ordinary token. `__DATE__` and `__TIME__` are not predefined. Wide string and character literals (`L"hi"`, `u8"hi"`, `L'a'`) are rejected. `long double` constants, binary literals, and non-ASCII bytes are errors. An integer suffix that is not in the C11 grammar (`1Ll`, `1uu`) is a diagnostic. An integer constant that fits in no type on the C11 6.4.4.1 list is a diagnostic, including a decimal value that does not fit in signed `long long`. Multi-character character constants and hex or octal escapes outside `unsigned char` are diagnostics.

The parser or checker rejects variable-length arrays, a `static` or `extern` declaration in a `for` header, a `static` function declaration at block scope, old-style declarations with an empty parameter list (`int f();`), nested functions, a variadic function with no named parameter (`int f(...);`), a definition of a variadic function, an incomplete struct or union passed or returned at a definition or a call, returning an x87 aggregate, bit-fields, flexible array members, anonymous struct or union members, empty structs and unions, designated initializers, `goto`, and a label followed by a declaration (`case 1: int x;`). Arithmetic on `void *` is rejected; Clang warns and treats the size as 1. A global initializer that is not an integer or floating constant expression, an address constant, or a constant brace list of those is a diagnostic. Assignment, `++`/`--`, calls, and the comma operator are not allowed in that constant expression when they are evaluated (C11 6.6). An evaluated division by zero is a diagnostic, and so is an evaluated signed constant whose result does not fit in its type (C11 6.6p4), including `INT_MIN / -1` and `1 << 31`. `long double` expressions are diagnostics. Other C11 keywords (`_Alignas`, `_Generic`, …) are recognized so they produce a specific "not supported" diagnostic.

## Layout

- `src/` — Rust frontend (`pp`, `lexer`, `parser`, `ast`, `sema`, `irgen`, `driver`)
- `include/` — headers the compiler supplies (`stddef.h`, `stdarg.h`, `stdbool.h`, `limits.h`)
- `build/compiler` — release binary produced by `make` (not committed)
- `tests/` — C programs for the differential harness; `tests/inc/` holds headers those programs include
- `harness/run_tests.py` — Clang oracle
- `PLAN.md` — staged plan (unchanged starter plan)
- `STATUS.md` — what this milestone actually does
