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

Imported into the private repository `https://github.com/spacefilm900/c-compiler` from the tested archive. The default branch `main` already had one commit, `2582a22d92d35b05097ecaaab77f71aaa75cb16b` (`Initial commit`), containing only `README.md` with the text `# c-compiler`. That path conflicts with this tree. The placeholder was replaced by the compiler `README.md`. No other file was already in the repository. History was not rewritten and nothing was force-pushed. The compiler commit is on `cursor/import-compiler-ci-08ff`, branched from `main` at `2582a22`, so the import can be a pull request. The pull request URL is recorded below once GitHub accepts it.

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

No Actions run has been observed yet. This section will be updated with the run URL and conclusion after GitHub records one. Do not treat the local results above as a green GitHub check.

### CodeRabbit

`.coderabbit.yaml` is in the tree. It does not install the GitHub App, and the skipped skills install was not performed here either.

Checks made with the token available to this import, before the pull request existed:

- Collaborators: `spacefilm900` only, with admin, maintain, push, triage, and pull. No CodeRabbit account is a collaborator.
- Issues, pull requests, and commit comments: none.
- Check runs and commit statuses on `main`: none.
- Actions workflows before this import: none.
- Repository events list: empty.
- `GET /repos/spacefilm900/c-compiler/hooks`: HTTP 403, `Resource not accessible by integration`.
- `GET /user/installations`: HTTP 403, `Resource not accessible by integration`.
- `GET /repos/spacefilm900/c-compiler/installation`: HTTP 401, `A JSON web token could not be decoded` (that route expects a GitHub App JWT).
- `GET /orgs/spacefilm900/installations`: HTTP 404. `spacefilm900` is a user account, not an organization.
- Code scanning alerts: code scanning is not enabled.

There is no evidence that the CodeRabbit GitHub App is active on this repository. The token cannot list other GitHub Apps, so this is not an account-wide installation audit. Activation is a repository-owner step and was not done from here. No subscription was started and no account permissions were changed.

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

- No preprocessor, trigraphs, digraphs, or line splicing.
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
- The CodeRabbit GitHub App is not shown as active on this repository by the checks in Import verification. Reviews do not start until the account owner installs the App.
- No GitHub Actions run has been observed yet. Do not treat this document as a green CI check.

### Owner step to activate CodeRabbit

This was not performed during the import. The owner of `spacefilm900` does it in a browser, on the personal account (this repository is not in an organization):

1. Sign in at [the CodeRabbit login page](https://app.coderabbit.ai/login) with **Login with GitHub**, as `spacefilm900`.
2. When GitHub asks to authorize CodeRabbit, review the read of organizations, teams, and email addresses, and authorize only if that is acceptable.
3. Select the personal account `spacefilm900`, not an organization.
4. On the GitHub App install screen for [coderabbitai](https://github.com/apps/coderabbitai), choose **Only select repositories** and select `c-compiler`. Do not choose **All repositories**.
5. Review the App permissions CodeRabbit documents (read of actions, discussions, members, metadata, and merge queues; read-and-write of checks, commit statuses, workflows, and repository contents so it can post review comments). Confirm them only if that access is acceptable. Workflow write is what lets an agent edit `.github/workflows/` when asked; ordinary reviews do not need to change those files.
6. Click **Install & Authorize** (or **Save** if the App is already installed on the account and `c-compiler` only needs to be added to the selected-repository list: GitHub → Settings → Applications → CodeRabbit → Configure).
7. Open this pull request in the CodeRabbit UI, or wait for the App to review it. `.coderabbit.yaml` is configuration the App reads after it is installed. The file does not install the App.

CodeRabbit's current plan page says a new organization starts on a 14-day Advanced trial with no credit card, the Free plan includes private repositories but limits GitHub pull-request reviews to summarization, and full pull-request reviews after the trial require a paid plan the owner chooses. No trial was started and no payment was made here. Official steps: [GitHub integration](https://docs.coderabbit.ai/platforms/github-com), [plans](https://docs.coderabbit.ai/management/plans).

## Next actions

1. Record the GitHub Actions run URL and conclusion in this file once GitHub reports them.
2. Repository owner installs the CodeRabbit GitHub App on `c-compiler` only, if reviews are wanted.
3. Accept prototypes so mutually recursive functions can be declared before use.
4. Add file-scope variables, then pointers and arrays, still rejecting anything not yet implemented.

## Clean-room reminder

Frontend code was written from the C standard, the LLVM Language Reference, and the System V ABI. Clang and GCC source were not consulted. Clang is only the differential oracle and the linker/backend for emitted IR.
