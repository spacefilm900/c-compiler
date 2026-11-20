# Clean-room C compiler → LLVM IR

The program is `hcc`, the Haviland C Compiler. It is a small C compiler that emits textual LLVM IR. LLVM (`clang`, and optionally `opt` / `llc`) optimizes that IR and generates native code. This milestone is a C subset: enough to lex, preprocess, parse, check, and run functions and file-scope objects of the LP64 integer types, with prototypes, pointers, arrays, string literals, `const`, `volatile`, and `restrict`, calls to variadic library functions such as `printf`, simple structs and unions passed and returned by value, `float` and `double`, typedefs, enumerations, `_Bool`, function pointers, arithmetic, bitwise and shift operators, `++`/`--`, compound assignment, `?:`, the comma operator, `sizeof`, casts, branches, loops, `switch`, and calls. A built-in preprocessor runs before the parser. Hello world compiles from `#include <stdio.h>`, and that header is the host's. It is not a full C compiler.

**Play DOOM on a Mac:** [docs/DOOM_ON_MAC.md](docs/DOOM_ON_MAC.md) is a step-by-step guide for an Apple Silicon Mac. One command, `./tools/mac/play_doom.sh`, builds DOOM with this compiler and opens it in a window.

## Why Rust

The compiler is written in Rust. That is a deliberate break from the usual practice of implementing C in C or C++. Rust gives memory safety without a garbage collector, and Cargo builds a single static binary from the standard library alone. There are no crates.io dependencies, so a checkout builds without fetching packages.

The frontend was written from the C standard, the LLVM Language Reference, and the System V AMD64 ABI. It is not derived from Clang or GCC. Clang is used only as a black-box oracle in the test harness and as the LLVM backend that links the emitted `.ll` file. The compiler does not call into Clang's or GCC's frontend.

## Design

The decisions behind this compiler are in [DESIGN.md](DESIGN.md).

## Build

Emitting IR does not need Clang. Running the result does. The IR uses opaque pointers, the triple `x86_64-pc-linux-gnu`, and that triple's System V data layout. Clang/LLVM must be **18 or newer**: opaque pointers need 15, and LLVM 15–17 x86-64 backends split an `i128` argument between the last free GP register and the stack instead of passing it wholly on the stack as System V requires (fixed in LLVM 18). This compiler passes `__int128` as a plain `i128` and its `va_arg` reads a stack-passed one as 16 bytes from the overflow area, so on LLVM 15–17 a variadic `__int128` after five GP arguments would read the wrong value. `harness/run_tests.py` and CI refuse an older Clang.

`rust-toolchain.toml` pins Rust **1.85.1** with the `rustfmt` and `clippy` components. That matches the system `cargo` used for the local unit tests. On that machine, plain `cargo clippy` is not installed; clippy is the rustup command below.

```sh
make
# equivalent:
# cargo build --release
# cp -f target/release/hcc build/hcc
```

`make` (GNU make 4.4.1 locally) writes `build/hcc`.

Local execution used Debian Clang/LLVM **19.1.7**. Put that install's `bin` directory on `PATH` (set this to your path). Any Clang/LLVM 18 or newer on `PATH` is enough for the harness (for example a recent Homebrew `llvm`).

## Contract

```sh
build/hcc [options] INPUT.c -o OUTPUT.ll
build/hcc -E [options] INPUT.c [-o OUTPUT.i]
build/hcc --dump-STAGE [options] INPUT.c [-o OUTPUT]
```

Options are `-I DIR` (also `-IDIR`), `-D NAME[=text]` (also `-DNAME=text`; a bare `-DNAME` defines `NAME` as `1`), `-U NAME`, and `-E`. `-D` and `-U` apply in order, after the predefined macros.

`-target TRIPLE` (also `--target TRIPLE` and `--target=TRIPLE`) picks the target; the default is the host. `x86_64-linux-gnu` is the original target and its output is unchanged. `arm64-apple-macos` (optionally with a version, `arm64-apple-macos14`; the default minimum is macOS 11) emits IR for Apple Silicon Macs, and `aarch64-linux-gnu` for 64-bit Arm Linux. `-isysroot DIR` names the SDK (macOS) or sysroot whose `usr/include` holds the C library headers; on a Mac the SDK is otherwise found with `xcrun`. On a Mac the compiler reads the SDK's headers (`#if` supports `__has_include`, `__has_builtin`, which answers 1 for the builtins this compiler implements, and the other Clang feature queries, which answer 0); see [docs/USER_GUIDE_MACOS.md](docs/USER_GUIDE_MACOS.md).

`-std=c11` selects strict ISO C11, which is also the default. `-std=gnu11` opts in to GNU extensions; any other `-std` value is a usage error, and the last `-std` wins. GNU mode is the GNU-compatibility layer of PLAN.md (stage 1); the extensions it adds are listed under [GNU extensions](#gnu-extensions) below. It also turns three C11 constraint violations into warnings (`file:line:column: warning: ...`), as GCC 14 and Clang 19 do in their default GNU modes: an equality comparison (`==`, `!=`) between a `void *` and a pointer to a function (C11 6.5.9p2); a comparison of two pointers to distinct types, for equality between any two pointers and relationally between two object pointers (C11 6.5.8p2, 6.5.9p2); and a pointer assignment, initialization, argument, or return whose pointed-to type differs only in the signedness of an integer type (`int *` from `unsigned *`, `char *` from `unsigned char *`) or drops a qualifier (`char *` from `const char *`) (C11 6.5.16.1p1). Deeper pointer mismatches such as `int **` from `unsigned **`, or `int *` from `long *`, stay errors in both modes, as GCC 14 rejects them. A comparison inside a constant expression that the parser folds before semantic analysis (for example in `sizeof` within an array bound, an enumerator, or `_Static_assert`) is accepted in GNU mode without a warning. Relational comparison of function pointers and the predefined macros (including `__STRICT_ANSI__`) are the same in both modes. A null pointer constant such as `(void *)0` may be compared with a function pointer in either mode. The GCC torture runner and the Csmith runner pass `-std=gnu11`; the other harnesses use the strict default.

For debugging, `--dump-tokens`, `--dump-ast`, `--dump-sema` and `--dump-ir` stop after that stage and print its dump (to standard output, or to `-o FILE`); `--dump-pp` is another spelling of `-E`. The formats and a triage guide are in [docs/DEBUGGING.md](docs/DEBUGGING.md), and `tools/reduce/reduce.py` shrinks a failing program.

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

`harness/run_arm64_ir.py` compares this compiler's arm64 function signatures with Clang's and checks that `llc` builds the IR for both arm64 targets (Mach-O objects for macOS). `harness/run_cross.py --target aarch64-linux-gnu` and `--target arm64-apple-macos` run the same differential programs under `qemu-aarch64`; the docstring explains how the Apple calling convention is run without a Mac.

On a real Mac, `.github/workflows/macos.yml` (Apple Silicon runner; pushes to `main` and manual runs only, because macOS minutes cost about ten times Linux minutes) builds the compiler, runs the unit tests and hello world, and runs `KNOWN_FAILURES=harness/macos_known_failures.txt python3 harness/run_tests.py` with Apple's Clang as the oracle. Start it by hand with `gh workflow run macos.yml`; `-f doom=true` adds a DOOM job (frame hashes against Apple's Clang at `-O0` and `-O2`, and the SDL2 game built and launched with no screen).

The external suites (Csmith, One True AWK, the AWK Builder suites, `gcc.c-torture/execute`, DOOM, Lua, and SQLite) and the performance script are described in [tests/external/README.md](tests/external/README.md) and [STATUS.md](STATUS.md).

```sh
make csmith
make otawk
make torture
make doom
make lua
make sqlite
make perf
make awk-extra
```

### Big programs

DOOM is the first real program. `make doom` fetches [doomgeneric](https://github.com/ozkl/doomgeneric) at a pinned commit and the free [Freedoom](https://freedoom.github.io/) game data, builds DOOM twice (with this compiler and with Clang), plays the same recorded demos in both with no window, and checks that every frame is identical. It builds with this compiler alone, and all eight Freedoom demos (four in each of `freedoom1.wad` and `freedoom2.wad`, 39,209 frames) match Clang frame for frame at `-O0` and `-O2`. The same check runs natively on macOS with Apple's Clang, and `tools/mac/play_doom.sh` builds a playable SDL2 version in a window ([docs/DOOM_ON_MAC.md](docs/DOOM_ON_MAC.md)). The per-program scorecards are in [reports/doom/](reports/doom/): `baseline.md` is the first run, kept unchanged, and `scorecard.md` is the current one.

Lua is the second. `make lua` fetches the Lua 5.4.7 release and its official test suite from lua.org (both checked by SHA-256), builds Lua twice (with this compiler in `-std=gnu11` mode, because Lua's interpreter loop uses GNU computed `goto` when the compiler says it is GNU-compatible, and with Clang), and compares the two. Both the normal build and a build with Lua's internal self-checks turned on pass the official test suite in three modes, nine workload scripts of our own give byte-for-byte the same output, and `luac` produces identical bytecode, at `-O0` and `-O2`. Scorecards are in [reports/lua/](reports/lua/).

SQLite is the third. `make sqlite` fetches the SQLite 3.54.1 amalgamation from sqlite.org (checked by SHA-256 and SHA3-256), builds the command-line shell twice (with this compiler and with Clang, `-std=gnu11 -D_DEFAULT_SOURCE`, FTS5, RTree, and the math functions), and runs ten deterministic SQL workloads. Our compiler builds both `sqlite3.c` and `shell.c` alone, and every workload matches Clang byte for byte at `-O0` and `-O2`, including the database files. Scorecards are in [reports/sqlite/](reports/sqlite/).

On the machine where system `cargo` is Rust 1.85.1 and has no clippy component, the clippy check that passed was:

```sh
rustup run stable cargo clippy --offline --all-targets -- -D warnings
```

Point `rustup` at your install (set this to your path). The numeric version of that rustup `stable` toolchain was not recorded.

## CI and CodeRabbit

`.github/workflows/ci.yml` runs on `push` and `pull_request`. The GitHub source of truth is `https://github.com/spacefilm900/c-compiler`. Observed Actions runs, including whether any run has completed, are recorded in `STATUS.md`.

The workflow checks out the repo, installs Rust 1.85.1 (`rustfmt`, `clippy`), and installs Ubuntu 24.04 packages `clang`, `llvm`, `make`, and `python3`. It exits if `clang --version` reports a major version below 18. Then it runs `cargo fmt --all -- --check`, `cargo clippy --offline --all-targets -- -D warnings`, `cargo test --offline --lib`, `make`, and `python3 harness/run_tests.py`. The job token can read contents only. The job times out after 20 minutes, and a newer push to the same ref cancels the older run.

Actions are pinned to commits, not floating tags:

- `actions/checkout` `11d5960a326750d5838078e36cf38b85af677262` (v4.4.0)
- `dtolnay/rust-toolchain` `683b588c487a69d01f6c6ddf40f036ff682cc2c8` (v1), with `toolchain: 1.85.1`

`.coderabbit.yaml` is review configuration validated against CodeRabbit's published schema. The file does not install the GitHub App. Whether the App is active on this repository, and what it did on the import pull request, is recorded in `STATUS.md`. The optional CodeRabbit skills install was skipped.

## Supported subset

- One translation unit of function definitions, prototypes, and file-scope objects. The integer types are the LP64 sizes: `char` (signed), `signed char`, and `unsigned char` are 8 bits; `short` and `unsigned short` are 16; `int` and `unsigned int` are 32; `long`, `unsigned long`, `long long`, and `unsigned long long` are 64. `void` is a return type and the sole unnamed parameter (`int main(void)` and `int main()` are both no-parameter definitions). Specifiers may appear in any legal order (`unsigned long long int`, `int long unsigned`). `signed` alone is `int`. `unsigned` alone is `unsigned int`. Invalid combinations are diagnostics. `char` and `signed char` are not the same type.
- Prototypes: `int f(void);`, `char id(char c);`, unnamed parameters (`int f(int, char);`), and a trailing ellipsis on a declaration (`int printf(const char *, ...);`). A later definition may use different parameter names. The types must match, including the return type and whether the function is variadic. A call checks the argument count. Fixed arguments are converted as if assigned to the parameter. Arguments past the fixed parameters get the default argument promotions (`char` and `short` become `int`, and `float` becomes `double`). A function declared but not defined is an LLVM `declare`, so an external definition can be linked in. `static` on a file-scope function gives it internal linkage. A call to a function that has not been declared is an error. C11 has no implicit function declarations. A definition may end in `...` when it has a named parameter. `<stdarg.h>` implements `va_start`, `va_arg`, `va_end`, and `va_copy`.
- On this target, `char`, `signed char`, and `short` parameters and return values are passed with LLVM `signext`. `unsigned char` and `unsigned short` use `zeroext`. That is the System V AMD64 rule that those values are extended to 32 bits. `int` and wider types have no extension attribute.
- `const` and `volatile` on declaration specifiers and after `*` (`const char *`, `char *const`, `const int`). The same qualifier twice in one list is a diagnostic. Assigning to a const-qualified object, including `*p` when `p` has type `const char *`, is a diagnostic. `char *` may be passed to a `const char *` parameter. `const char *` may not be passed to `char *`, and `char **` may not be passed to `const char **`. Top-level `const` on a parameter is not part of the function type. `volatile` is a `load volatile` or `store volatile`, and a volatile aggregate copy tells `llvm.memmove` the copy is volatile. `restrict` is a qualifier on a pointer: it participates in compatibility and assignment, it is dropped from a parameter's function type the same way top-level `const` is, and it does not change the IR. `restrict` on a non-pointer is a diagnostic.
- File-scope objects of those integer types, of `float` and `double`, of pointers, and of arrays: `char g;`, `unsigned long g = expr;`, `float f = 1.5f;`, `double d = 1.5;`, `int *p = &g;`, `int a[4] = {1, 2};`, `int b[] = {1, 2, 3};`, `char s[] = "hi";`, `char *msg = "hi";`, `extern int g;`, and `static short g;`. Several declarators are allowed, and `*` belongs to each declarator (`int *a, b` is one pointer and one `int`). A declaration without an initializer and without `extern` is a tentative definition; if nothing in the file initializes it, it is zero (a null pointer, or `zeroinitializer` for an array). `extern` without an initializer does not define the object. An integer initializer is an integer constant expression. A `float` or `double` initializer is a floating constant expression. A pointer initializer may also be an address constant (`&g`, `&a[2]`, `a + 1`, or a string literal) or a null pointer constant. A const object is an LLVM `constant`. Locals and parameters shadow globals.
- Locals and `for (char c = ...)` declarations, with optional initializers. A block may also declare `static int x = 1` (a constant initializer, no linkage, the value persists across calls) and `extern int g` or `int f(int)` (linkage follows C11 6.2.2, and the name is not visible after the block). `static` and `extern` are not allowed on a `for` header, and a block-scope function cannot be `static`. The initializer is converted to the variable's type. Arrays use a brace list. A partial list zeros the rest. `int a[] = {1, 2, 3}` gets its length from the list. A character array may also be initialized from a string: `char s[] = "hi"` is three bytes including NUL, `char s[10] = "hi"` zero-fills, and `char s[2] = "hi"` has no NUL. A longer string is a diagnostic.
- Statements: blocks, expression statements, empty statements, `return`, `if` / `else` (an `else` binds to the nearest `if`), `while`, `do` / `while` (`continue` jumps to the condition), `for`, `break`, `continue`, `switch` / `case` / `default`, and `goto` with a label in the same function. The controlling expression is promoted. Each case constant is converted to that type. Labels may be nested inside the body, and `break` leaves the switch. The IR is an LLVM `switch`. A `goto` target is a block named `C.<label>`.
- Pointer declarators, including `int **pp`, `int *a[3]`, and `int (*p)[3]`, plus `void *`. Unary `&` and `*`. Subscripts `a[i]` and `i[a]`. Array-to-pointer decay except under `sizeof` and `&`. Pointer plus or minus an integer, scaled by the element size. Pointer minus pointer, with type `long`. Pointer equality, including null and `void *`, and relational comparisons of compatible pointers. `++`/`--` and `+=`/`-=` on pointers. Pointers in `if`, `!`, `&&`, `||`, and `?:`.
- Expressions, with C precedence and associativity: integer constants (decimal, octal, hexadecimal, with `u`/`U`, `l`/`L`, `ll`/`LL`, and the combined suffixes), floating constants (decimal and hexadecimal, with an optional `f`/`F` suffix), character constants (`'a'`, `'\n'`, `'\0'`, `'\x41'`, `'\101'`), string literals (`"hi"`, `"\n"`, `"\x41"`, `"hel" "lo"`), identifiers, parentheses, casts (integers, `float` and `double`, pointers, and between an integer and a pointer), unary `+` `-` `!` `~` `&` `*`, binary `+` `-` `*` `/` `%` `&` `|` `^` `<<` `>>`, comparisons, `&&` and `||`, `?:`, assignment and compound assignment, prefix and postfix `++`/`--`, the comma operator, `sizeof` on an expression or a type (including `sizeof(int *)`, `sizeof(int[3])`, and `sizeof "abc"`, which is 4), and direct calls, including variadic calls. Pointers and arrays may be parameters and return types. An array parameter is adjusted to a pointer. An ordinary or `u8` string literal has type `char[N]` including the terminating NUL and decays to `char *` except under `sizeof` and `&`. A `u` string is `char16_t[N]`, a `U` string is `char32_t[N]`, and an `L` string is `wchar_t[N]` (`wchar_t` is `int`). Each decays to a pointer to its element type except under `sizeof` and `&`. The address is a private unnamed_addr constant. Modifying that object is undefined and is not diagnosed.
- Integer promotions and the usual arithmetic conversions apply. Types narrower than `int` promote to `int`. Unsigned arithmetic wraps. Signed `/` and `%` round toward zero; unsigned `/` and `%` use `udiv` and `urem`. Comparisons and `&&` / `||` produce `int` 0 or 1. `-1 < 1u` is false. `&&`, `||`, and `?:` evaluate only the operands that C evaluates.
- `sizeof` is not evaluated. Its result type is `unsigned long`, and `sizeof(sizeof(int))` is 8. `sizeof(void)` and `sizeof` of an incomplete array are rejected. `sizeof(int *)` is 8. `sizeof` of an array is the element size times the length, not the size of a pointer.
- A signed right shift is arithmetic. An unsigned right shift is logical. A shift count outside `0 .. width` of the promoted left operand, a signed left shift of a negative value, and a signed left shift that does not fit are undefined. Constant initializers reject those shifts. A runtime shift is emitted without a count mask and without a diagnostic.
- `++`/`--` and compound assignment require an lvalue. On an integer, the operation uses the promoted type and the result is converted back. On a pointer, `++`/`--` and `+=`/`-=` move by one element. Compound assignment evaluates the right-hand side, then loads the lvalue once.
- Converting an out-of-range value to a signed type keeps the low bits and interprets them as two's complement, matching Clang on this target. `(char)1000` is `-24`. Character constants whose byte is above 127 are negative because `char` is signed: `'\xff'` is `-1`. Converting an integer to a pointer extends it to 64 bits and uses those bits as the address. Converting a pointer to an integer keeps the address bits, truncated if the integer is narrower than 64. `(int *)(int)-1` and `(int *)(long)-1` are the same all-ones address. `(int *)(unsigned)-1` is the low 32 bits only.
- Assigning a pointer requires a compatible pointer, a qualification conversion (`char *` to `const char *`, and the same rule for `volatile`), `void *` in either direction when the destination does not drop a qualifier, or a null pointer constant (`0` or `(void *)0`). `int *` and `char *` are not compatible. Dropping `const` is an error here. Clang warns and still compiles. A cast may drop a qualifier; that is not diagnosed.
- A function may call itself, and it may call a function that was declared earlier and defined later. Assignment targets are variables; parentheses do not change that (`(x) = 1` assigns to `x`).
- Tagged structs at file scope and block scope. Members may be any supported type: integers, pointers, arrays, and nested structs, including a struct declared in the member list. `struct node;` is an incomplete type. A pointer to it is complete, so a struct may contain a pointer to itself. An object of an incomplete struct type is a diagnostic, as is `sizeof` of that type. `extern struct S x;` is the exception: it may name an object completed in another translation unit. Layout follows the System V AMD64 ABI: each member is aligned to its alignment, the struct alignment is the strictest member alignment, and the size is rounded up to that alignment. An array member uses the element alignment. The extra 16-byte alignment applies to a local or global array variable, not to a member. `.` and `->` name a member, including chains (`a.b.c`, `p->next->val`), and the result is an lvalue when the base is. `&s.x` and `s.arr[i]` work. A whole struct may be assigned, which copies every byte, including padding. Brace initializers work for structs and for arrays of structs, at local and file scope. A missing member is zero. A nested brace initializes one member. A flat list fills inner aggregates in order. An expression of the member's struct type initializes that member as a whole (`struct Line line = {p, q}`). Designated initializers (`.x = 1`, `[0] = 1`) are accepted. The next positional initializer continues at the next subobject. Bit-fields (`int x : 3`) pack least-significant bit first. A zero-width unnamed bit-field aligns the next field and does not raise the struct's alignment, so `struct { char a; int : 0; char b; }` has alignment 1. A file-scope or local initializer may nest a bit-field struct inside another struct or an array. A plain member of that nested struct is part of the same byte image. A struct with a const member cannot be assigned as a whole. A complete struct or union may be passed or returned by value. Aggregates of at most 16 bytes whose fields are integers, pointers, `float`, or `double` are coerced to register pieces. An argument is `byval` when it is larger, unaligned, or the registers it needs are already taken. A return is `sret` only when its class is memory; a register-classified return still uses the return registers. `f().x`, assignment from a call, and `?:` on a struct copy the result. An SSE eightbyte is passed in an XMM register. A return whose eightbytes are `X87` then `X87UP` is `x86_fp80` in `%st0`. Other aggregates that contain `long double` are `sret`. The last member of a struct that already has another named member may be a flexible array. `sizeof` excludes it. Initializing it is a diagnostic, and that struct is not an array element or a member of another struct. A prototype may name an incomplete struct; a definition and a call need a complete type.
- `//` line comments and non-nesting `/* */` block comments. A backslash immediately before a new-line is deleted first (C translation phase 2), including inside a comment or a string, so the following physical line is part of the same logical line. `__LINE__` is the physical line of the token.
- A built-in preprocessor. `#define` / `#undef` cover object-like macros and function-like macros, including an empty parameter list and `__VA_ARGS__`. The `(` must follow the name with no space for the definition to be function-like. `#` stringifies an argument and `##` pastes two tokens. Expansion rescans, and a macro is not expanded again inside its own replacement. An identical redefinition is allowed; a different one is a diagnostic. `#if`, `#ifdef`, `#ifndef`, `#elif`, `#else`, and `#endif` nest. `#if` expressions are evaluated in `intmax_t` / `uintmax_t`. `defined X` and `defined(X)` are recognized, and any other identifier is 0. A group whose condition is false is skipped: macros there are not expanded and `#error` there is not fired. `#include "file"` searches the including file's directory, then each `-I` directory, then the compiler's `include/`, then the host directories. `#include <file>` searches `-I` directories, then `include/`, then `/usr/include/x86_64-linux-gnu` and `/usr/include`. `#include_next` resumes after the directory that provided the current file. The nested-include limit is 200. `#error` is a diagnostic. `#line` is accepted. `#pragma once` is honored; any other `#pragma` is ignored. A line that is only `#` is a null directive. Predefined names include `__FILE__`, `__LINE__`, `__STDC__` (1), `__STDC_VERSION__` (`201112L`), `__STDC_HOSTED__` (1), `__x86_64__`, `__linux__`, `__LP64__`, `__SIZE_TYPE__`, and `__USER_LABEL_PREFIX__`.
- `typedef` names an existing type, including a pointer, an array, a struct or union, or a function type. A typedef is an alias: `typedef int T` and `int` are the same type. The parser resolves the typedef-name versus identifier ambiguity with a scope stack.
- Enumerations. `enum E { A = 1, B }` defines `A` and `B` as `int` constants and gives `enum E` a type compatible with `int`. An anonymous `enum { RED, GREEN }` introduces the constants only.
- Unions. Members start at offset 0. The size is the largest member, rounded up to the strictest alignment. `.` and `->` name a member. A brace initializer initializes the first member. Assignment copies every byte. A forward `union U;` is an incomplete type, and a pointer to it is complete.
- `_Bool` (and `<stdbool.h>`'s `bool`, `true`, and `false`). A nonzero value converts to 1. `restrict` is a pointer qualifier and does not change the IR. Function pointers may be declared, assigned, compared with `==` and `!=`, stored in arrays and structs, and called as `fp(...)` or `(*fp)(...)`. A function designator decays to a pointer. `typedef` of a function type works. `qsort` takes a comparator. Relational `<` on function pointers is a diagnostic. `float`, `double`, and `long double` are computed, including constants, conversions, and the System V calling convention. `long double` is 80-bit extended precision. A scalar parameter is passed in memory and a scalar return uses the x87 stack.
- `__attribute__((...))` and `__extension__` are ignored. `__asm__("name")` is the linker name of a function.

`include/` holds headers a compiler supplies rather than the C library: `stddef.h` (`size_t`, `ptrdiff_t`, `wchar_t`, `NULL`, `offsetof`), `stdarg.h` (`va_list` and the `va_*` macros, lowered to the builtins), `stdbool.h`, `stdnoreturn.h` (`noreturn`), `float.h` (IEEE binary32, binary64, and the `LDBL_*` macros), and `limits.h` (the ISO limits for this LP64 target). `#include <stdio.h>` and the other library headers come from the host. Clang, when it compiles the same test, uses those same host headers. An `l`/`L` floating constant is an 80-bit extended value, so `float.h` can name `LDBL_MAX` and a program can use that value.

Reaching the closing brace of an integer function lowers to a return of 0 of that type. That is what C requires for `main`. A `float` or `double` function that falls off the end lowers to a return of `0.0`. A `void` function that falls off the end lowers to `ret void`.

## GNU extensions

These follow the public GCC manual, with GCC 14 and Clang 19 used only as black boxes. No compiler source was read.

**Only with `-std=gnu11`.** Strict C11 rejects each of these with an `error:` that names `-std=gnu11` (or, for `typeof`, treats the plain word as an identifier):

- `typeof` and `__typeof__` (`__typeof`), with a type name or an expression operand. The expression is not evaluated. A bit-field operand is an error.
- Statement expressions `({ ... })`. The value is the last expression statement's value (unqualified, after lvalue conversion), or `void`. They are allowed only inside a function. Labels and `goto` inside one work.
- Case ranges `case low ... high:`. An empty range warns ("empty case range specified"), and an overlap with another case is an error. Ranges of fewer than 256 values are expanded into the `switch`; wider ones become a range test before the default.
- Zero-length arrays `T a[0]`, including as the last member of a struct; the array adds no size.
- `x ?: y`, which evaluates `x` once and yields it when nonzero.
- Empty initializers `= {}`, treated as `= {0}`.
- Labels as values: `&&label` is the address of a label in the current function, of type `void *`, and `goto *expr;` jumps to it. A label address is a constant in a block-scope `static` initializer, so a dispatch table such as Lua's `static const void *const disptab[] = { &&L_OP_MOVE, ... };` works. A computed `goto` may reach any label of the function whose address is taken (an address taken only inside `sizeof` does not count). Differences of label addresses (`&&a - &&b`) are not supported. Use a label address only to jump to it or to compare it with a null pointer: LLVM, the backend, leaves an equality test between two label addresses undefined, so such a comparison compiles but its result is not promised.

**In both modes** (these spellings are reserved for the implementation, C11 7.1.3):

- `__attribute__((...))` on declarations, declarators, types, record members, functions, parameters, labels, and as a null statement. With meaning: `noreturn`, `unused`, `used` (`@llvm.used`), `aligned` / `aligned(N)` on objects, members, and struct or union types, `packed` on a struct or union or a member, `always_inline` / `noinline`, `gnu_inline`, `section("name")`, `weak` (definitions and `extern_weak` declarations), `deprecated` / `deprecated("msg")` (a call warns), `cold` / `hot`, `const` / `pure` (LLVM `memory(none)` / `memory(read)`), `constructor` / `destructor`, and `mode(QI|HI|SI|DI|word|pointer|byte)`. Accepted and ignored: `format`, `nonnull`, `noclone`, `nothrow`, `returns_twice`, `malloc`, `warn_unused_result`, `visibility`, `may_alias`, `transparent_union`, and similar optimisation hints. An unknown attribute is ignored with the warning `unknown attribute 'X' ignored`. These are an error because ignoring them would change behaviour: `vector_size`, `cleanup`, `alias`, `ifunc`, `naked`, and `weakref`. `aligned` on a typedef is allowed only when it does not change the alignment, and a bit-field in a struct whose layout `packed` or `aligned` changes is an error.
- `__int128`, `signed __int128`, `unsigned __int128`, `__int128_t`, and `__uint128_t`: 128-bit integers, size and alignment 16, lowered to LLVM `i128`; constant expressions keep all 128 bits. A parameter, return, or `va_arg` uses two general-purpose registers and 16-byte stack alignment. `__builtin_add_overflow` and friends accept 128-bit operands and destinations. Bit-fields of these types are not supported.
- An empty declaration at file scope (a lone `;`). C11 does not allow it; Clang and GCC accept it.
- `inline` follows C11 6.7.4p7: when every file-scope declaration of a function has `inline` and none has `extern`, its definition is an inline definition, emitted `available_externally` (calls may inline it, and otherwise call the external definition from another file or library). `__attribute__((gnu_inline))` and the `-fgnu89-inline` option give GCC's GNU89 rules instead: an `extern inline` definition is only for inlining and a plain `inline` one is external; `-fgnu89-inline` also predefines `__GNUC_GNU_INLINE__` in place of `__GNUC_STDC_INLINE__`. The macOS SDK's `<ctype.h>`, `<math.h>` and `<stdio.h>` define functions this way.
- `__extension__`, `__inline__` / `__inline`, `__signed__`, `__const__`, `__volatile__`, `__restrict__`, `__alignof__` (with a type or an expression), `__asm__` labels, and `__FUNCTION__`.
- Builtins folded to constants: `__builtin_constant_p` (1 for an integer constant expression, a floating constant or a string literal, otherwise 0), `__builtin_types_compatible_p` (top-level qualifiers ignored), `__builtin_choose_expr`, `__builtin_offsetof`, and the bit builtins below when their operand is constant.
- Builtins lowered to LLVM: `__builtin_expect`, `__builtin_unreachable`, `__builtin_trap`, `__builtin_clz` / `ctz` / `popcount` / `parity` / `ffs` / `clrsb` with their `l` and `ll` forms, `__builtin_bswap16` / `32` / `64`, `__builtin_alloca`, `__builtin_prefetch` (no code), `__builtin_assume_aligned`, `__builtin_return_address(0)`, `__builtin_frame_address(0)`, `__builtin_isnan` / `isinf` / `isfinite` / `isnormal`, the `va_*` builtins, and the overflow builtins: generic `__builtin_add_overflow` / `sub` / `mul` on any integer operands, and the typed `__builtin_sadd_overflow`, `__builtin_uaddl_overflow`, `__builtin_smulll_overflow`, and the rest of that family.
- Library forwarders: `__builtin_abort`, `__builtin_exit`, `__builtin_printf`, `__builtin_memcpy` and the other string and memory functions, `__builtin_malloc`, `__builtin_abs` / `labs`, the `<ctype.h>` functions, and the `<math.h>` functions with their `f` and `l` forms (for example `__builtin_fabs`, `__builtin_sqrtf`). Each is a call to the library function of the same name without the prefix, with that function's standard prototype. GCC accepts some forwarders that Clang does not, such as `__builtin_exit` and the `<ctype.h>` ones; for those, the output was checked against GCC.

Not yet: `__builtin_object_size`, `__builtin_classify_type`, `__builtin_*_overflow_p`, `__builtin_setjmp` / `longjmp`, `__builtin_apply`, vector types, inline assembly, nested functions, range designators, implicit `int` and implicit function declarations, and K&R definitions.

## Not accepted

Unsupported or invalid input is a diagnostic. It is not ignored and it is not lowered.

Trigraphs are not recognized. `_Pragma` is rejected or left as an ordinary token. `__DATE__` and `__TIME__` are not predefined. `L`, `u`, `U`, and `u8` string literals are accepted, as are `L`, `u`, and `U` character constants. `wchar_t` is `int`, `char16_t` is unsigned 16-bit, `char32_t` is unsigned 32-bit, and a `u8` string is an array of `char`. `u8'...'` is rejected. Inside a character constant or string literal, a non-ASCII source character (UTF-8) or a universal character name (`\u`, `\U`) is encoded for the prefix: UTF-8 for an unprefixed or `u8` string, UTF-16 for `u`, UTF-32 for `U` and `L`. A character constant whose character needs more than one code unit is a diagnostic. A prefixed character constant in `#if` has the value and type it has in program text, so `L'\400'` is 256. Binary literals and non-ASCII bytes outside literals and comments are errors. An integer suffix that is not in the C11 grammar (`1Ll`, `1uu`) is a diagnostic. An integer constant that fits in no type on the C11 6.4.4.1 list is a diagnostic, including a decimal value that does not fit in signed `long long`. Multi-character character constants and hex or octal escapes outside `unsigned char` are diagnostics. A wide hex escape must fit in `unsigned int`.

The parser or checker rejects variable-length arrays, a `static` or `extern` declaration in a `for` header, a `static` function declaration at block scope, old-style K&R definitions and identifier lists (`int f(a, b);`), nested functions, a variadic function with no named parameter (`int f(...);`), an incomplete struct or union passed or returned at a definition or a call, empty structs and unions, a GNU range designator (`[0 ... 3]`, in both modes), computed `goto` and `&&label` in strict C11 (they need `-std=gnu11`), and a label followed by a declaration (`case 1: int x;`). An untagged unnamed struct or union member is anonymous: its members are members of the enclosing type. A tagged unnamed member is still a diagnostic. Arithmetic on `void *` is rejected; Clang warns and treats the size as 1. A global initializer that is not an integer or floating constant expression, an address constant, or a constant brace list of those is a diagnostic. Assignment, `++`/`--`, calls, and the comma operator are not allowed in that constant expression when they are evaluated (C11 6.6). An evaluated division by zero is a diagnostic, and so is an evaluated signed constant whose result does not fit in its type (C11 6.6p4), including `INT_MIN / -1` and `1 << 31`. `int f();` declares a function without a prototype: a call applies the default argument promotions, and compatibility with a prototype follows C11 6.7.6.3p15. Compound literals, `_Generic`, `_Static_assert`, and `_Alignof(type-name)` are supported. Other C11 keywords (`_Alignas`, `_Atomic`, `_Complex`, `_Thread_local`) are recognized so they produce a specific "not supported" diagnostic, and `__STDC_NO_ATOMICS__`, `__STDC_NO_COMPLEX__`, `__STDC_NO_THREADS__`, and `__STDC_NO_VLA__` are predefined. The `__builtin_*` functions this compiler knows are listed under [GNU extensions](#gnu-extensions); any other `__builtin_*` name is an ordinary undeclared identifier.

## Layout

- `src/` — Rust frontend (`pp`, `lexer`, `parser`, `ast`, `sema`, `irgen`, `driver`)
- `include/` — headers the compiler supplies (`stddef.h`, `stdarg.h`, `stdbool.h`, `limits.h`)
- `build/hcc` — release binary produced by `make` (not committed)
- `tests/` — C programs for the differential harness; `tests/inc/` holds headers those programs include
- `harness/run_tests.py` — Clang oracle
- `src/target.rs`, `src/aapcs64.rs` — target selection and the Arm 64-bit calling convention
- `harness/run_arm64_ir.py`, `harness/run_cross.py`, `harness/arm64/` — arm64 checks without a Mac; `tests/arm64/` holds the arm64 calling-convention programs
- `docs/USER_GUIDE_MACOS.md` — building and using the compiler on a Mac (draft)
- `docs/DOOM_ON_MAC.md`, `tools/mac/play_doom.sh`, `harness/doom/dg_sdl.c` — playing DOOM, built with this compiler, in an SDL2 window on a Mac
- `harness/run_doom.py`, `harness/doom/` — DOOM harness and its headless backend
- `harness/run_lua.py`, `harness/lua/` — Lua harness and its workload scripts
- `harness/run_sqlite.py`, `harness/sqlite/` — SQLite harness and its SQL workloads
- `reports/` — scorecards and campaign reports (`reports/doom/` for DOOM, `reports/lua/` for Lua, `reports/sqlite/` for SQLite)
- `PLAN.md` — staged plan (unchanged starter plan)
- `STATUS.md` — what this milestone actually does
- `DESIGN.md` — design decisions
- `LICENSE` — MIT licence for this compiler

## Licence

The compiler is MIT (see [LICENSE](LICENSE)). The Writing a C Compiler test suite is third-party (MIT, Copyright (c) 2023 Nora Sandler), downloaded at a pinned commit and not stored in this repo (see [tests/external/README.md](tests/external/README.md)). So are doomgeneric (GPL-2.0), the Freedoom game data (BSD), Lua 5.4.7 with its test suite (MIT), and the SQLite 3.54.1 amalgamation (public domain), which are test inputs only.
