# STATUS

## Current milestone

**Milestone 2, step 2 of 6 — remaining integer operators.** Done.

The compiler is a Rust binary. `make` installs it at `build/compiler`. It accepts the Milestone 1 integer subset, function prototypes, file-scope `int` objects, and the remaining integer operators: bitwise operators, shifts, `++`/`--`, compound assignment, `?:`, the comma operator, and `sizeof`. Types are still only `int` and `void`. It rejects other input with a diagnostic. It is not a full C compiler.

## Milestone 2 plan

1. **Done.** Function prototypes and file-scope `int` globals (tentative definitions, `extern`, `static`, integer constant initializers, internal linkage).
2. **Done.** Remaining integer operators: bitwise, shifts, `++`/`--`, compound assignment, comma, ternary, `sizeof`.
3. `char` / `short` / `long` / unsigned, integer promotions, and casts.
4. Pointers and arrays.
5. String literals and external calls such as `printf` / `puts` (hello world).
6. Simple structs.

## Language

Rust, edition 2021, standard library only. No crates.io dependencies (`Cargo.lock` has none). `rust-toolchain.toml` pins **1.85.1** plus `rustfmt` and `clippy`.

## Tests for this step

Run on this tree after the step 2 changes, before the pull request:

| Command | Toolchain | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | rustc **1.85.1** (4eb161250 2025-03-15) | clean |
| `cargo clippy --offline --all-targets -- -D warnings` | same | clean |
| `cargo test --offline --lib` | same | **29 passed, 0 failed** |
| `make` | GNU make **4.3** | `build/compiler` produced |
| `python3 harness/run_tests.py` | Ubuntu Clang **18.1.3** (`x86_64-pc-linux-gnu`) | **37/37 passed** |

`cargo test` includes lexer regressions, parser AST dumps, negative diagnostics, a CLI check, IR checks (a constant return, globals, `declare` / `define internal`, the new operators, assembled and executed), and a differential run of every `tests/*.c` file against `clang -std=c11`. The harness repeats that differential check through `build/compiler` (release binary) and compares exit status, stdout, and stderr.

Harness programs: `01_return_const` (42), `02_add` (5), `03_loop` (55), `04_call` (42), `05_if` (2), `06_logic` (9), `07_while` (6), `08_cmp` (3), `09_arith` (17), `10_shadow` (4), `11_break` (25), `12_recursion` (16), `13_void` (6), `14_fallthrough` (0), `15_comment` (5), `16_decls` (12), `17_empty_params` (7), `18_forward_call` (42), `19_mutual` (2), `20_global_counter` (5), `21_static_global` (7), `22_tentative` (9), `23_shadow_global` (18), `24_const_init` (0), `25_extern_global` (8), `26_unnamed_params` (6), `27_multi_decl` (7), `28_void_forward` (9), `29_extern_def` (11), `30_bitwise` (0), `31_shifts` (0), `32_incdec` (0), `33_compound` (0), `34_ternary` (0), `35_comma` (0), `36_sizeof` (0), `37_const_ops` (0).

Negative cases covered inside `cargo test` (each must be a non-zero diagnostic, not a successful compile): missing semicolon, missing `)`, pointers, arrays, `char`, structs, address-of, pointer dereference, `sizeof(char)`, `sizeof(int *)`, a `?:` missing `:`, old-style `int f();` declarations, unnamed parameters on a definition, `static` locals, block-scope `extern`, function initializers, multiple storage-class specifiers, `void` variables, storage class on a parameter, nested functions, `switch`, `do`/`while`, stray `else`, casts, varargs, strings, character constants, floats, integer suffixes, out-of-range and invalid numeric literals, unterminated comments, preprocessor lines, non-ASCII, undeclared names, redefinitions, conflicting function types, a variable redeclared as a function, conflicting linkage, non-constant, divide-by-zero, and overflowing global initializers, undefined shifts in a constant initializer (`1 << 32`, `1 << -1`, `-1 << 1`), `1 << 31` as an overflowing constant, `++` / assignment / comma in an evaluated constant expression, `sizeof(void)`, a `?:` whose arms differ in type, `++` or compound assignment applied to a non-lvalue, bad call arity against a prototype, a static function that is used but not defined, `break`/`continue` outside a loop, `void` returns, non-assignable expressions, calls of non-functions, and a call to a function that has not been declared yet.

GitHub Actions for this step is recorded after the pull request's workflow runs finish.

## Implemented

- [x] Lexer for identifiers, C11 keywords, comments, and integer constants
- [x] Recursive-descent parser and AST (C precedence, dangling `else`)
- [x] Semantic checks: scopes, types, calls, lvalues, loops
- [x] Textual LLVM IR (`alloca` / `load` / `store`, signed arithmetic, branches, calls)
- [x] Function prototypes, compatible redeclarations, and `declare` for functions with no body
- [x] File-scope `int` objects: tentative definitions, `extern`, `static`, constant initializers
- [x] Bitwise operators, shifts, `++`/`--`, compound assignment, `?:`, comma, and `sizeof` on `int`
- [x] CLI `build/compiler INPUT.c -o OUTPUT.ll`
- [x] Differential tests against Clang

## Supported syntax

`int` and `void`; function definitions and prototypes; parameters, including unnamed parameters on a prototype (`int f(int, int);`); file-scope `int` objects; `static` and `extern` on file-scope functions and `int` objects; locals with optional initializers; blocks; `if` / `else`; `while`; `for` (including `for (int i = ...)`); `break`; `continue`; `return`; empty statements; unary `+` `-` `!` `~`; `+` `-` `*` `/` `%` `&` `|` `^` `<<` `>>`; comparisons; short-circuit `&&` and `||`; `?:`; assignment and compound assignment; prefix and postfix `++`/`--`; the comma operator; `sizeof` on an expression or `sizeof(int)`; direct calls; decimal, octal, and hex constants that fit in signed 32-bit `int`; `//` and `/* */` comments.

`int` is i32. The module triple is `x86_64-pc-linux-gnu`. IR uses opaque pointers (LLVM 15+). Locals are allocated in the entry block. File-scope objects are LLVM globals (`global`, `internal global`, or `external global`). A defined function is `define`; a prototype with no body and external linkage is `declare`. `static` is internal linkage. `&&` / `||` and comparisons lower to 0 or 1. Signed division uses `sdiv` / `srem` (toward zero). Bitwise `&` `|` `^` lower to `and` / `or` / `xor`. `~` is `xor` with `-1`. `<<` is `shl`. `>>` is `ashr` (arithmetic). `?:` is a branch and a `phi`, and only the chosen arm is evaluated.

A prototype, or a definition, makes the function visible from that point to the end of the file. Calls check the argument count against that declaration. Several compatible prototypes may name the same function, and a definition may appear before or after them. Parameter names do not have to match. `int f(void);` and `int f() { ... }` are compatible: both have zero parameters.

File-scope `int name;` and `static int name;` are tentative definitions. If the translation unit never gives that object an initializer, it is emitted as zero. A later initializer replaces the tentative value. `extern int name;` does not define the object; `extern int name = expr;` does. The initializer must be an integer constant expression (C11 6.6). The operators above are allowed, including `?:`, `sizeof`, `~`, bitwise operators, and shifts. Assignment, `++`/`--`, function calls, and the comma operator are rejected when they are evaluated. An unevaluated operand may contain them: the operand of `sizeof`, the skipped operand of `&&` or `||`, and the arm of `?:` that is not selected. `&&` and `||` do not evaluate the right operand when the left decides the result. `?:` does not evaluate the arm that is not selected. An evaluated operation whose mathematical result does not fit in signed 32-bit `int`, including `INT_MIN / -1`, `INT_MIN % -1`, and `1 << 31`, is a diagnostic (C11 6.6p4). An evaluated division or remainder by zero is a diagnostic. `-2147483647 - 1` is the in-range spelling of the minimum `int`.

`sizeof` yields the `int` value 4 for `sizeof(int)` and for every expression of type `int`. It does not yield `size_t`. `sizeof(sizeof(int))` is therefore 4 in this compiler. Clang's `size_t` is 8 bytes on this target, so that expression is not in the differential tests. The operand is not evaluated: `sizeof(h++)` does not increment `h`, and `sizeof(tick())` does not call `tick`. `sizeof(void)`, and `sizeof` applied to a `void` expression, are diagnostics because `void` is incomplete.

Signed `>>` is arithmetic, including in a constant initializer (`-8 >> 1` is `-4`, and `(-2147483647 - 1) >> 1` is the arithmetic result). A shift count that is negative or greater than or equal to 32 is undefined. A left shift of a negative `int` is undefined. A left shift whose mathematical result does not fit in signed 32-bit `int`, including `1 << 31`, is undefined. Constant initializers reject a bad count or a negative left operand as an undefined shift, and reject a result that does not fit as overflow. Runtime shifts are not diagnosed. The IR uses `shl` and `ashr` without masking the count, so a count outside `0..31` is LLVM poison. An in-range left shift of a negative value is still emitted as `shl` of the two's complement bits and is not trapped. Tests stay in the defined range, except for an unevaluated shift such as `1 || (1 << 40)`, which folds to 1.

Prefix and postfix `++`/`--` apply to lvalues: locals, parameters, and globals. The IR loads the scalar once, adds or subtracts 1, and stores the result immediately. Prefix yields the new value; postfix yields the old value. That store is sequenced before the value is used by the surrounding operator. A program that modifies the same scalar twice with no sequence point in between is still undefined, and the tests do not do that. Signed overflow of `++`/`--` is undefined, the same way `+` and `-` are.

Compound assignment (`+=` `-=` `*=` `/=` `%=` `&=` `|=` `^=` `<<=` `>>=`) evaluates the lvalue address once, evaluates the right-hand side, then loads, applies the operator, and stores. A variable's address has no side effect, so the load happens after the right-hand side. `x += x += 1` is defined: the inner update is sequenced before the outer one.

The comma operator yields the right operand and has a sequence point between the operands. It is not an integer constant expression when it is evaluated. Clang accepts `int g = (1, 2);` as an extension; this compiler rejects it. A comma in an unevaluated operand, such as `sizeof(1, x)`, is allowed. Commas that separate arguments or declarators are not this operator. `f(1, (2, 3))` is two arguments, and the second is a comma expression.

A file-scope function with no storage class is treated as `extern`, so `static int f(void); int f(void) { ... }` keeps internal linkage. A file-scope object with no storage class has external linkage and does not inherit a prior `static`. The same identifier with both internal and external linkage is a diagnostic. Two bodies, or two initialized definitions, are a redefinition. A prototype parameter name is not visible outside that prototype. A local shadows a global of the same name. A global is in scope for its own initializer, but an identifier is not a constant, so `int g = g;` is rejected.

`int main(void)` and `int main()` are both no-parameter definitions. Reaching the closing brace of an `int` function lowers to `ret i32 0`. That is what C requires for `main`. The same lowering is used for every other `int` function. A `void` function that falls off the end lowers to `ret void`.

## Limitations

- No preprocessor, trigraphs, or digraphs. A backslash-newline is rejected, including when it ends a `//` comment or appears inside a block comment. It is not deleted the way C translation phase 2 would delete it.
- Integer constants must fit in signed 32-bit `int`. A value that overflows `i64` while parsing uses the same "does not fit in int" diagnostic. `long` and unsigned types from C11 6.4.4.1 are not supported.
- No pointers, arrays, structs, unions, enums, typedefs, or types other than `int` and `void`.
- No string or character literals, floats, or casts. `sizeof` yields `int` 4, not `size_t`. Runtime shift counts outside `0..31`, left shifts of negative values, and left shifts that do not fit in `int` are undefined and are not diagnosed outside constant initializers.
- No `switch`, `do`/`while`, or `goto`.
- An empty parameter list on a declaration that is not a definition (`int f();`) is rejected. C11 treats that as a non-prototype. This subset requires a prototype: `(void)` or a parameter type list. A definition may still use `()`.
- `static` and `extern` are only accepted at file scope. A `static` local and a block-scope `extern` declaration are diagnostics.
- A `static` function that is called but never defined is a diagnostic. An external function or object that is declared and not defined is emitted as `declare` or `external global`, so another translation unit can provide it.
- A tentative definition is completed inside this translation unit as a zero initializer (`global i32 0`), which is the C11 6.9.2 rule and matches Clang's default (`-fno-common`). It is not a common symbol.
- Uninitialized locals are not zeroed. Reading them is undefined in C; the IR loads the alloca anyway.
- The compiler reports the first error only and does not write the `.ll` file.
- Columns count every byte as one column, including tabs.
- No optimizations are performed here. Clang can optimize and codegen the IR.

## CI and CodeRabbit

- `.github/workflows/ci.yml` runs on `push` and `pull_request` with `contents: read`, a 20-minute job timeout, and cancel-in-progress for the same ref. The runner is `ubuntu-24.04`. It installs Rust 1.85.1 (`rustfmt`, `clippy`) and apt packages `clang`, `llvm`, `make`, and `python3`, and refuses Clang older than 15. Steps: `cargo fmt --all -- --check`, `cargo clippy --offline --all-targets -- -D warnings`, `cargo test --offline --lib`, `make`, `python3 harness/run_tests.py`.
- Pinned actions: `actions/checkout@11d5960a326750d5838078e36cf38b85af677262` (v4.4.0), `dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8` (v1) with toolchain `1.85.1`.
- `.coderabbit.yaml` follows CodeRabbit schema `https://coderabbit.ai/integrations/schema.v2.json`. The review profile is `CHILL`. The file does not install the GitHub App.
- Milestone 1's Actions runs and the CodeRabbit review on pull request 1 are in the record below. This step's Actions run and CodeRabbit review are added when those checks finish.

## Next actions

1. Milestone 2 step 3: `char` / `short` / `long` / unsigned, integer promotions, and casts.
2. Record the Actions and CodeRabbit results for this step once those checks finish.

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
