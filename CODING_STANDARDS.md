# Coding standards

This file records how hcc is written and reviewed. The first section is
already enforced. The guidelines that follow are aims, not CI gates: a
pull request is not rejected only because a function or a file is over
the sizes below.

## Rules already enforced

These are required for a change to merge. The owner merges only when CI
is green and CodeRabbit's review threads are resolved
([CONTRIBUTING.md](CONTRIBUTING.md)).

**Clean-room.** Compiler code is written only from the C11 standard
(N1570), the LLVM Language Reference, the platform ABI documents, and
the public GCC manual for GNU extensions. Do not read, copy or adapt the
source of Clang, GCC, any other C compiler, reducer or C library, and do
not paste compiler code produced by an AI assistant. Clang and GCC may
be run as black-box oracles. The rule is stated in
[CONTRIBUTING.md](CONTRIBUTING.md), and also in [README.md](README.md)
and [DESIGN.md](DESIGN.md).

**Toolchain.** Rust 1.85.1 is pinned by
[`rust-toolchain.toml`](rust-toolchain.toml), with `rustfmt` and
`clippy`.

**Formatting and lints.** Every pull request that runs the main CI
workflow ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) must
pass:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`

There is no `rustfmt.toml` or `clippy.toml`; both tools use their
defaults. Clippy warnings are treated as errors.
[CONTRIBUTING.md](CONTRIBUTING.md) lists the same checks for local use
(`cargo fmt --check` is the shorter form of the CI command).

**Write a failing test first.** Every bug fix starts with a test that
fails without the fix (a regression test): a unit test, a program under
`tests/` compared with Clang, or both
([CONTRIBUTING.md](CONTRIBUTING.md)).

**STATUS and changelog.** Keep [STATUS.md](STATUS.md) current when
behaviour changes, and add a line to the "Unreleased" section of
[CHANGELOG.md](CHANGELOG.md) for anything a user would notice
([CONTRIBUTING.md](CONTRIBUTING.md)).

**Diagnostic style.** Diagnostics are lower case, name the construct,
and quote source spellings with `'...'` (`use of undeclared identifier
'x'`), as in [CONTRIBUTING.md](CONTRIBUTING.md).

**Documentation claims.** [`tools/check_docs.py`](tools/check_docs.py)
compiles the language claims in the user-facing docs and checks that the
stated phrases are present. CI runs it in
[`.github/workflows/ci.yml`](.github/workflows/ci.yml), and in
[`.github/workflows/docs.yml`](.github/workflows/docs.yml) when a change
touches only documentation.

**CodeRabbit.** Pull requests are reviewed by CodeRabbit
([`.coderabbit.yaml`](.coderabbit.yaml)). Review threads must be
resolved before a merge ([CONTRIBUTING.md](CONTRIBUTING.md)).

## Soft guidelines

The following are not CI gates. They describe the shape we prefer, and
known exceptions we intend to split over time.

**Function and file size.** Prefer functions under about 150 lines, and
production files (excluding `#[cfg(test)]` modules) under about 1,200
lines. Length is measured as in
[`tools/quality/complexity.py`](tools/quality/complexity.py). As of
version 1.0.1, the exceptions are:

- `check_atomic_builtin` in `src/sema/atomic.rs` (252 lines), and a few
  other functions just over 150 lines (`finish` in `src/sema/decl.rs`,
  `render_function` in `src/irgen/module.rs`, `gen_stmt` in
  `src/irgen/stmt.rs`).
- These production files are over 1,200 lines: `src/sema/init.rs`,
  `src/sema/expr.rs`, `src/parser/expr.rs`, `src/sema/decl.rs`,
  `src/ast/tree.rs`, `src/parser/decl.rs`, and `src/lexer.rs`.

New work in those places should tend to shrink them, not grow them, when
that is practical.

**No `unsafe` without justification.** The compiler currently contains
no `unsafe` blocks. Do not add `unsafe` unless the need is explained in
a comment next to it and in the pull request.

**Module introductions.** Keep the `//!` comment at the top of each
module. It should say, in plain sentences, what the module does and the
main rules it follows.

**Rustdoc on the crate-root API.** Public items exported from
`src/lib.rs` (the compile entry points and the types callers use) should
have rustdoc. Helpers marked `pub` only so other modules in this crate
can use them do not need a comment when the name is enough.

**Crash points.** `.expect()`, `unreachable!()` and `.unwrap()` in
non-test code should only guard a state that an earlier stage has
already rejected, or that the same function has just established. Give a
short message that says why the case cannot happen (for example,
`expect("checked by parse_args")`). Do not use them for user errors:
those are diagnostics.
[`tools/quality/crash_points.py`](tools/quality/crash_points.py) can
list the current sites; it is a measurement tool, not a CI gate.

## Recurring review findings become rules

When CodeRabbit or an independent review flags the same kind of issue
more than once, add a rule to this file. If a cheap automatic check or a
regression test can catch it, add that too, so the same finding does not
have to be discussed on every pull request.

This is a living document. A new rule should describe existing practice,
or a change the owner has accepted.
