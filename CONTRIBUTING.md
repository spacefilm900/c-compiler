# Contributing to hcc

Thank you for helping. hcc is a clean-room C11 compiler, so the most
important rule is about where code comes from.

## The clean-room rule

Compiler code in this repository is written only from public
specifications:

- the C11 standard (N1570),
- the LLVM Language Reference,
- the platform ABI documents (System V AMD64, AAPCS64 and Apple's arm64
  rules, the Microsoft x64 calling convention),
- the public GCC manual, for the behaviour of GNU extensions.

Do **not** read, copy or adapt the source of Clang, GCC, any other C
compiler, reducer or C library, and do not paste compiler code produced by
an AI assistant. Clang and GCC may be run as black boxes, to see what they
accept and what a program prints. Test programs may come from anywhere
whose licence allows it. The big programs the harness builds (DOOM, Lua,
SQLite, QuickJS, zlib, One True AWK) are test subjects only: nothing from
them goes into the compiler. Tools such as Csmith, YARPGen, cargo-fuzz,
cargo-mutants and C-Reduce are used only as black boxes.

## Building and testing

You need Rust 1.85.1 (pinned by `rust-toolchain.toml`, with `rustfmt` and
`clippy`), Clang/LLVM 18 or newer, Python 3 and GNU make.

```sh
make                      # build/hcc
cargo test                # unit tests
python3 harness/run_tests.py   # differential tests against Clang
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The README's "Test" section lists the larger suites (the Writing a C
Compiler tests, GCC torture, Csmith, DOOM, Lua, SQLite, QuickJS, zlib).
Pull-request CI runs most of them.

## Making a change

1. Open an issue first for anything bigger than a small fix.
2. Work on a branch and open a pull request against `main`.
3. Every bug fix starts with a test that fails without it: a unit test, a
   program under `tests/` compared with Clang, or both.
4. Keep `STATUS.md` current when behaviour changes, and add a line to the
   "Unreleased" section of `CHANGELOG.md` for anything a user would notice.
5. CI must be green and the CodeRabbit review threads resolved before a
   merge. The owner merges.

Commit messages say what changed and why, in plain sentences. Diagnostics
are lower case, name the construct, and quote source spellings with `'...'`
(`use of undeclared identifier 'x'`).

## Reporting a bug

Include the smallest C program that shows the problem, the exact command
line, `hcc --version`, what you expected (for example Clang's output for
the same program) and what hcc did. An internal compiler error (exit
status 3) is always a bug. `tools/reduce/reduce.py` can shrink a large
program while keeping the failure.

Security problems: see [SECURITY.md](SECURITY.md).

## Licence

By contributing you agree that your contribution is licensed under the
project's MIT licence ([LICENSE](LICENSE)).
