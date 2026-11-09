# Clean-room C compiler → LLVM IR

A small C compiler that emits textual LLVM IR. LLVM (`clang`, and optionally `opt` / `llc`) optimizes that IR and generates native code. This milestone is a C subset: enough to lex, parse, check, and run functions and file-scope objects of the LP64 integer types, with prototypes, pointers, arrays, arithmetic, bitwise and shift operators, `++`/`--`, compound assignment, `?:`, the comma operator, `sizeof`, casts, branches, loops, and calls. It is not a full C compiler.

## Why Rust

The compiler is written in Rust. That is a deliberate break from the usual practice of implementing C in C or C++. Rust gives memory safety without a garbage collector, and Cargo builds a single static binary from the standard library alone. There are no crates.io dependencies, so a checkout builds without fetching packages.

The frontend was written from the C standard, the LLVM Language Reference, and the System V AMD64 ABI. It is not derived from Clang or GCC. Clang is used only as a black-box oracle in the test harness and as the LLVM backend that links the emitted `.ll` file. The compiler does not call into Clang's or GCC's frontend.

## Build

Emitting IR does not need Clang. Running the result does. The IR uses opaque pointers and the triple `x86_64-pc-linux-gnu`, so Clang/LLVM must be 15 or newer.

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
build/compiler INPUT.c -o OUTPUT.ll
```

On success the command exits 0 and writes a textual LLVM IR module. On failure it exits non-zero, prints one diagnostic to stderr (`file:line:column: error: ...`), and does not create the output file.

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
- Prototypes: `int f(void);`, `char id(char c);`, and unnamed parameters (`int f(int, char);`). A later definition may use different parameter names. The types must match, including the return type. A call checks the argument count and converts each argument to the parameter type. A function declared but not defined is an LLVM `declare`, so an external definition can be linked in. `static` on a file-scope function gives it internal linkage.
- On this target, `char`, `signed char`, and `short` parameters and return values are passed with LLVM `signext`. `unsigned char` and `unsigned short` use `zeroext`. That is the System V AMD64 rule that those values are extended to 32 bits. `int` and wider types have no extension attribute.
- File-scope objects of those integer types, of pointers, and of arrays: `char g;`, `unsigned long g = expr;`, `int *p = &g;`, `int a[4] = {1, 2};`, `int b[] = {1, 2, 3};`, `extern int g;`, and `static short g;`. Several declarators are allowed, and `*` belongs to each declarator (`int *a, b` is one pointer and one `int`). A declaration without an initializer and without `extern` is a tentative definition; if nothing in the file initializes it, it is zero (a null pointer, or `zeroinitializer` for an array). `extern` without an initializer does not define the object. An integer initializer is an integer constant expression. A pointer initializer may also be an address constant (`&g`, `&a[2]`, `a + 1`) or a null pointer constant. Locals and parameters shadow globals.
- Locals and `for (char c = ...)` declarations, with optional initializers. The initializer is converted to the variable's type. Arrays use a brace list. A partial list zeros the rest. `int a[] = {1, 2, 3}` gets its length from the list.
- Statements: blocks, expression statements, empty statements, `return`, `if` / `else` (an `else` binds to the nearest `if`), `while`, `for`, `break`, `continue`.
- Pointer declarators, including `int **pp`, `int *a[3]`, and `int (*p)[3]`, plus `void *`. Unary `&` and `*`. Subscripts `a[i]` and `i[a]`. Array-to-pointer decay except under `sizeof` and `&`. Pointer plus or minus an integer, scaled by the element size. Pointer minus pointer, with type `long`. Pointer equality, including null and `void *`, and relational comparisons of compatible pointers. `++`/`--` and `+=`/`-=` on pointers. Pointers in `if`, `!`, `&&`, `||`, and `?:`.
- Expressions, with C precedence and associativity: integer constants (decimal, octal, hexadecimal, with `u`/`U`, `l`/`L`, `ll`/`LL`, and the combined suffixes), character constants (`'a'`, `'\n'`, `'\0'`, `'\x41'`, `'\101'`), identifiers, parentheses, casts (integers, pointers, and between the two), unary `+` `-` `!` `~` `&` `*`, binary `+` `-` `*` `/` `%` `&` `|` `^` `<<` `>>`, comparisons, `&&` and `||`, `?:`, assignment and compound assignment, prefix and postfix `++`/`--`, the comma operator, `sizeof` on an expression or a type (including `sizeof(int *)` and `sizeof(int[3])`), and direct calls. Pointers and arrays may be parameters and return types. An array parameter is adjusted to a pointer.
- Integer promotions and the usual arithmetic conversions apply. Types narrower than `int` promote to `int`. Unsigned arithmetic wraps. Signed `/` and `%` round toward zero; unsigned `/` and `%` use `udiv` and `urem`. Comparisons and `&&` / `||` produce `int` 0 or 1. `-1 < 1u` is false. `&&`, `||`, and `?:` evaluate only the operands that C evaluates.
- `sizeof` is not evaluated. Its result type is `unsigned long`, and `sizeof(sizeof(int))` is 8. `sizeof(void)` and `sizeof` of an incomplete array are rejected. `sizeof(int *)` is 8. `sizeof` of an array is the element size times the length, not the size of a pointer.
- A signed right shift is arithmetic. An unsigned right shift is logical. A shift count outside `0 .. width` of the promoted left operand, a signed left shift of a negative value, and a signed left shift that does not fit are undefined. Constant initializers reject those shifts. A runtime shift is emitted without a count mask and without a diagnostic.
- `++`/`--` and compound assignment require an lvalue. On an integer, the operation uses the promoted type and the result is converted back. On a pointer, `++`/`--` and `+=`/`-=` move by one element. Compound assignment evaluates the right-hand side, then loads the lvalue once.
- Converting an out-of-range value to a signed type keeps the low bits and interprets them as two's complement, matching Clang on this target. `(char)1000` is `-24`. Character constants whose byte is above 127 are negative because `char` is signed: `'\xff'` is `-1`. Converting an integer to a pointer extends it to 64 bits and uses those bits as the address. Converting a pointer to an integer keeps the address bits, truncated if the integer is narrower than 64. `(int *)(int)-1` and `(int *)(long)-1` are the same all-ones address. `(int *)(unsigned)-1` is the low 32 bits only.
- Assigning a pointer requires a compatible pointer, `void *` in either direction, or a null pointer constant (`0` or `(void *)0`). `int *` and `char *` are not compatible. That assignment is an error here. Clang warns and still compiles.
- A function may call itself, and it may call a function that was declared earlier and defined later. Assignment targets are variables; parentheses do not change that (`(x) = 1` assigns to `x`).
- `//` line comments and non-nesting `/* */` block comments.

Reaching the closing brace of an integer function lowers to a return of 0 of that type. That is what C requires for `main`. A `void` function that falls off the end lowers to `ret void`.

## Not accepted

Unsupported or invalid input is a diagnostic. It is not ignored and it is not lowered.

The lexer rejects the preprocessor (`#`). Trigraphs are not recognized. A backslash immediately before a new-line, including at the end of a `//` or `/*` comment, is a diagnostic rather than a line splice. String literals, floating constants, binary literals, and non-ASCII bytes are errors. An integer suffix that is not in the C11 grammar (`1Ll`, `1uu`) is a diagnostic. An integer constant that fits in no type on the C11 6.4.4.1 list is a diagnostic, including a decimal value that does not fit in signed `long long`. Multi-character character constants and hex or octal escapes outside `unsigned char` are diagnostics.

The parser or checker rejects function pointers, variable-length arrays, string literals, structs, unions, enums, typedefs, qualifiers, block-scope `static` and `extern`, old-style declarations with an empty parameter list (`int f();`), nested functions, variadic functions, member access, `switch`, `do`/`while`, and `goto`. Arithmetic on `void *` is rejected; Clang warns and treats the size as 1. A global initializer that is not an integer constant expression or an address constant is a diagnostic. Assignment, `++`/`--`, calls, and the comma operator are not allowed in that constant expression when they are evaluated (C11 6.6). An evaluated division by zero is a diagnostic, and so is an evaluated signed constant whose result does not fit in its type (C11 6.6p4), including `INT_MIN / -1` and `1 << 31`. Other C11 keywords (`float`, `_Bool`, …) are recognized so they produce a specific "not supported" diagnostic.

## Layout

- `src/` — Rust frontend (`lexer`, `parser`, `ast`, `sema`, `irgen`, `driver`)
- `build/compiler` — release binary produced by `make` (not committed)
- `tests/` — C programs for the differential harness
- `harness/run_tests.py` — Clang oracle
- `PLAN.md` — staged plan (unchanged starter plan)
- `STATUS.md` — what this milestone actually does
