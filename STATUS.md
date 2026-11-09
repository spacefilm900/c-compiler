# STATUS

## Current milestone

**Milestone 1 — lexer, parser, checks, and LLVM IR for a C subset.**

The compiler is a Rust binary. `make` installs it at `build/compiler`. It accepts a small integer subset and rejects other input with a diagnostic. It is not a full C compiler.

## Language

Rust, edition 2021, standard library only. No crates.io dependencies (`Cargo.lock` has none). Rust was chosen because it is a clear departure from implementing a C compiler in C or C++. `rust-toolchain.toml` pins **1.85.1** plus `rustfmt` and `clippy`, which is the system Cargo that ran the local unit tests.

## Tests actually run

Commands below were run on the compiler tree before this import. None of those earlier commands failed. GitHub Actions had not run at that time. The import verification for `https://github.com/spacefilm900/c-compiler` is in the next section, and that section is the one to trust for this repository.

Earlier local tree `/workspace/c-compiler`, after a user-space LLVM install that did not change compiler sources:

| Command | Toolchain | Result |
| --- | --- | --- |
| `make` | GNU make **4.4.1** | `build/compiler` produced |
| `cargo test --offline --lib` | system Rust **1.85.1** | **21 passed, 0 failed** |
| `~/.cargo/bin/rustup run stable cargo clippy --offline --all-targets -- -D warnings` | rustup `stable` (numeric version not recorded) | clean |
| `python3 harness/run_tests.py` | Debian Clang/LLVM **19.1.7** from `~/opt/llvm19-root`, on `PATH` via `~/.local/bin` | **17/17 passed** |

Plain `cargo clippy` is not available on that system Cargo 1.85.1. Use the rustup command above there.

The same 21 unit tests were re-run in this preparation environment with `rustup run 1.85.1`, including `cargo fmt --all -- --check` and `cargo clippy --offline --all-targets -- -D warnings`. Both were clean, and the unit tests were **21 passed, 0 failed**. That environment's Clang was Ubuntu 18.1.3; the harness result cited above is the LLVM 19.1.7 run.

## Import verification (2026-11-09)

Imported into the private repository `https://github.com/spacefilm900/c-compiler` from the tested archive. The default branch `main` already had one commit, `2582a22d92d35b05097ecaaab77f71aaa75cb16b` (`Initial commit`), containing only `README.md` with the text `# c-compiler`. That path conflicts with this tree. The placeholder was replaced by the compiler `README.md`. No other file was already in the repository. History was not rewritten and nothing was force-pushed. The compiler commit is `3b9d33e1cefd9a44f61c069011e7cbc460d033b8` on `cursor/import-compiler-ci-08ff`, branched from `main` at `2582a22`. GitHub accepted the pull request: https://github.com/spacefilm900/c-compiler/pull/1.

This environment, on that branch, before the branch was pushed:

| Command | Toolchain | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | rustc **1.85.1** (4eb161250 2025-03-15), selected by `rust-toolchain.toml` | clean |
| `cargo clippy --offline --all-targets -- -D warnings` | same | clean |
| `cargo test --offline --lib` | same | **21 passed, 0 failed** |
| `make` | GNU make **4.3** | `build/compiler` produced |
| `python3 harness/run_tests.py` | Ubuntu Clang **18.1.3** (`x86_64-pc-linux-gnu`) | **17/17 passed** |

Pinned actions were resolved on GitHub before the push. Both commits exist and match the tags named in the workflow:

- `actions/checkout` `11d5960a326750d5838078e36cf38b85af677262` is tag `v4.4.0` (2026-07-16, "backport fixes to releases-v4 (#2524)").
- `dtolnay/rust-toolchain` `683b588c487a69d01f6c6ddf40f036ff682cc2c8` is tag `v1` (2026-10-16, "Add 1.99.1 patch release").

`llvm-as` is not installed in this environment. The unit tests and the harness invoke `clang`, which is. The workflow installs the `llvm` package and checks `llvm-as --version` on the runner.

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

Any commit after `865c0f0` starts further runs. Those are not listed here.

### CodeRabbit

`.coderabbit.yaml` does not install the GitHub App. The optional skills install was skipped and was not performed here.

Before the pull request existed, the token used for this import could not see an installation: collaborators were only `spacefilm900` (admin); `main` had no check runs; issues, pull requests, and commit comments were empty; `GET /repos/spacefilm900/c-compiler/hooks` and `GET /user/installations` returned HTTP 403 `Resource not accessible by integration`; `GET /repos/spacefilm900/c-compiler/installation` returned HTTP 401 because that route expects a GitHub App JWT; `GET /orgs/spacefilm900/installations` returned HTTP 404 because `spacefilm900` is a user. Those negative checks do not list other Apps.

After pull request 1 was opened, the App was active on this repository:

- Check run `CodeRabbit` on `3b9d33e`, app `coderabbitai`, conclusion **success**, completed 2026-11-09 00:13:11Z. GitHub URL: https://github.com/spacefilm900/c-compiler/runs/128907038494. Details URL: https://app.coderabbit.ai/change-stack/spacefilm900/c-compiler/pull/1. The check summary says `Result: Success - Review completed`.
- `coderabbitai[bot]` commented on the pull request at 2026-11-09 00:04:08Z and posted a review state `COMMENTED` with 3 inline comments. The comment's run configuration names file `.coderabbit.yaml`, layers `Repository YAML`, review profile `CHILL`, and plan `Advanced`.
- The three comments on the import commit were fixed in a later commit. `"` in an LLVM string is emitted as `\22` (and `\` as `\\`); a backslash-newline, including inside a comment, is a diagnostic; an integer constant that does not fit in signed 32-bit `int` is a diagnostic instead of a panic. C11 6.4.4.1's wider types were not added. Locally, after that fix: `cargo fmt --all -- --check` clean, `cargo clippy --offline --all-targets -- -D warnings` clean, `cargo test --offline --lib` **23 passed, 0 failed**, `make` produced `build/compiler`, and `python3 harness/run_tests.py` **17/17 passed** (rustc 1.85.1, Ubuntu Clang 18.1.3).

No subscription was started and no account permissions were changed from this import. Nothing further is required to turn the App on for this repository; it already reviewed the import. The skills package remains uninstalled.

`cargo test` includes lexer regressions, parser AST dumps, negative diagnostics, a CLI check, an IR smoke check (`ret i32 42` assembled and executed), and a differential run of every `tests/*.c` file against `clang -std=c11`. The harness repeats that differential check through `build/compiler` (release binary) and compares exit status, stdout, and stderr.

Harness programs: `01_return_const` (42), `02_add` (5), `03_loop` (55), `04_call` (42), `05_if` (2), `06_logic` (9, including short-circuit stores), `07_while` (6), `08_cmp` (3), `09_arith` (17, including toward-zero division), `10_shadow` (4), `11_break` (25), `12_recursion` (16), `13_void` (6), `14_fallthrough` (0), `15_comment` (5), `16_decls` (12), `17_empty_params` (7).

Negative cases covered inside `cargo test` (each must be a non-zero diagnostic, not a successful compile): missing semicolon, missing `)`, globals, pointers, arrays, `char`, structs, bitwise AND, `++`, `sizeof`, `?:`, comma operator, prototype without a body, nested function, `switch`, `do`/`while`, stray `else`, casts, varargs, strings, character constants, floats, integer suffixes, out-of-range and invalid numeric literals, unterminated comments, preprocessor lines, non-ASCII, undeclared names, redefinitions, bad call arity, `break`/`continue` outside a loop, `void` returns, non-assignable expressions, calls of non-functions, and a call to a function that has not been defined yet.

## Implemented

- [x] Lexer for identifiers, C11 keywords, comments, and integer constants
- [x] Recursive-descent parser and AST (C precedence, dangling `else`)
- [x] Semantic checks: scopes, types, calls, lvalues, loops
- [x] Textual LLVM IR (`alloca` / `load` / `store`, signed arithmetic, branches, calls)
- [x] CLI `build/compiler INPUT.c -o OUTPUT.ll`
- [x] Differential tests against Clang

## Supported syntax

`int` and `void`; function definitions; parameters; locals with optional initializers; blocks; `if` / `else`; `while`; `for` (including `for (int i = ...)`); `break`; `continue`; `return`; empty statements; unary `+` `-` `!`; `+` `-` `*` `/` `%`; comparisons; short-circuit `&&` and `||`; assignment; direct calls; decimal, octal, and hex constants that fit in signed 32-bit `int`; `//` and `/* */` comments.

`int` is i32. The module triple is `x86_64-pc-linux-gnu`. IR uses opaque pointers (LLVM 15+). Locals are allocated in the entry block. `&&` / `||` and comparisons lower to 0 or 1. Signed division uses `sdiv` / `srem` (toward zero).

## Limitations

- No preprocessor, trigraphs, or digraphs. A backslash-newline is rejected, including when it ends a `//` comment or appears inside a block comment. It is not deleted the way C translation phase 2 would delete it.
- Integer constants must fit in signed 32-bit `int`. A value that overflows `i64` while parsing uses the same "does not fit in int" diagnostic. `long` and unsigned types from C11 6.4.4.1 are not supported.
- No pointers, arrays, structs, unions, enums, typedefs, globals, or other types.
- No string or character literals, floats, `sizeof`, casts, bitwise operators, shifts, `++`/`--`, compound assignment, `?:`, or the comma operator.
- No `switch`, `do`/`while`, or `goto`.
- No function prototypes. A callee other than the current function must already be defined, so two functions cannot call each other.
- Uninitialized locals are not zeroed. Reading them is undefined in C; the IR loads the alloca anyway.
- Falling off an `int` function emits `ret i32 0` (required for `main`, and also done for other `int` functions).
- The compiler reports the first error only and does not write the `.ll` file.
- Columns count every byte as one column, including tabs.
- No optimizations are performed here. Clang can optimize and codegen the IR.

## CI and CodeRabbit

The workflow file from the tested archive is unchanged. Its first comment still says the file had not been executed on GitHub; that comment describes the archive, and the Import verification section is where a real Actions result is recorded.

- `.github/workflows/ci.yml` runs on `push` and `pull_request` with `contents: read`, a 20-minute job timeout, and cancel-in-progress for the same ref. The runner is `ubuntu-24.04`. It installs Rust 1.85.1 (`rustfmt`, `clippy`) and apt packages `clang`, `llvm`, `make`, and `python3`, and refuses Clang older than 15. Steps: `cargo fmt --all -- --check`, `cargo clippy --offline --all-targets -- -D warnings`, `cargo test --offline --lib`, `make`, `python3 harness/run_tests.py`.
- Pinned actions: `actions/checkout@11d5960a326750d5838078e36cf38b85af677262` (v4.4.0), `dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8` (v1) with toolchain `1.85.1`. Both SHAs were confirmed to exist on GitHub on 2026-11-09.
- `.coderabbit.yaml` follows CodeRabbit schema `https://coderabbit.ai/integrations/schema.v2.json`. The file was checked against that schema before this import. It does not install anything.
- CodeRabbit skills were not installed; that install question was skipped, and they were not installed during this import.
- The CodeRabbit GitHub App reviewed pull request 1. See Import verification for the check URL, the `CHILL` profile loaded from `.coderabbit.yaml`, and the three comments that were left unapplied.
- GitHub Actions succeeded on the import commit `3b9d33e` and on the documentation commit `865c0f0`. See Import verification for the run URLs. A commit after `865c0f0` is a further run and is not one of those URLs.

## Next actions

1. Accept prototypes so mutually recursive functions can be declared before use.
2. Add file-scope variables, then pointers and arrays, still rejecting anything not yet implemented.
3. Record the Actions and CodeRabbit results for the review-fix commit once those checks finish.

## Clean-room reminder

Frontend code was written from the C standard, the LLVM Language Reference, and the System V ABI. Clang and GCC source were not consulted. Clang is only the differential oracle and the linker/backend for emitted IR.
