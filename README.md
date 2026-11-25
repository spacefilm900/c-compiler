# hcc: the Haviland C Compiler

**hcc** is a clean-room C11 compiler written in Rust. It compiles C to
textual LLVM IR, and Clang/LLVM turns that IR into programs for Linux,
macOS and Windows. It builds and runs DOOM, Lua, SQLite, QuickJS and zlib
with the same results as Clang.

It was developed agentically by a Grok Bot as an experiment in building
software of scale using agents, without human intervention at code level.
The work embedded software engineering practices throughout: configuration
management, CI, automated code review, and structured test approaches
including adversarial testing by another AI model, with regular human
input at process level.

**Version 1.0.0.** [User guide](docs/USER_GUIDE.md) ·
[Language reference](docs/LANGUAGE.md) · [Changelog](CHANGELOG.md) ·
[Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) ·
[Design](DESIGN.md) · [Licence: MIT](LICENSE)

## Quick start

```sh
make                                   # builds build/hcc (Rust 1.85+, no dependencies)
build/hcc --link hello.c -o hello      # compile and link (needs Clang 18+)
./hello

build/hcc hello.c -o hello.ll          # or just emit LLVM IR
clang -w -O2 hello.ll -o hello
```

Release archives for Linux x86-64, Linux arm64, macOS on Apple Silicon and
Windows x64 hold a ready-built `bin/hcc` with its headers; see the
[user guide](docs/USER_GUIDE.md#1-install).

## What it does

- Full **C11** (N1570) support, except the optional complex number types
  (`_Complex`). The compiler does support the harder parts: variable-length
  arrays, `_Atomic` and `<stdatomic.h>`, `<threads.h>`, `_Generic`,
  `_Alignas`, `_Thread_local`, `_Static_assert`, Unicode literals,
  universal character names in identifiers, old-style K&R function
  definitions and `<tgmath.h>`. By default hcc follows the standard
  strictly (`-std=c11`). Its other limitations are listed under
  [known limitations](docs/USER_GUIDE.md#9-known-limitations).
- **GNU C** with `-std=gnu11`: statement expressions, `typeof`,
  attributes, labels as values, `__int128`, case ranges and the builtins
  that real code and system headers use.
- **Targets**: x86-64 and arm64 Linux, arm64 and x86-64 macOS, and x64
  Windows (MSVC and MinGW-w64), each with its platform calling convention
  (`-target TRIPLE`).
- **Tested against Clang** as a black-box oracle: hand-written
  differential tests, the Writing a C Compiler suite, the GCC torture
  tests, Csmith and YARPGen random programs, independent adversarial test
  kits, fuzzing, coverage and mutation testing, and big programs with
  scorecards.

**Play DOOM on a Mac:** [docs/DOOM_ON_MAC.md](docs/DOOM_ON_MAC.md) is a
step-by-step guide for an Apple Silicon Mac. One command,
`./tools/mac/play_doom.sh`, builds DOOM with hcc and opens it in a window.

## Why Rust

Rust is a deliberate break from the usual practice of implementing C in C
or C++. It gives memory safety without a garbage collector, and Cargo
builds a single binary from the standard library alone. There are no
crates.io dependencies, so a checkout builds without fetching packages.

hcc was written from the C standard, the LLVM Language Reference and the
platform ABIs: the System V AMD64 ABI, the Arm AAPCS64 with Apple's arm64
rules, and the Windows x64 calling convention. It is not derived from
Clang or GCC. Clang is used only as a black-box oracle in the tests and as
the LLVM backend that turns hcc's IR into programs.

## Design

The decisions behind this compiler are in [DESIGN.md](DESIGN.md). That
design document was created wholly by the Grok Bot.

## Build and use

`make` writes `build/hcc`: it runs `cargo build --release` and copies
`target/release/hcc` there. With Cargo alone, use `target/release/hcc`
in the commands below.
`rust-toolchain.toml` pins Rust 1.85.1 with `rustfmt` and `clippy`.
Emitting IR does not need Clang; running the result needs Clang/LLVM **18
or newer** (opaque pointers, and LLVM 18's fix for passing `__int128` on
the stack). The harness and CI refuse an older Clang.

```sh
build/hcc [options] INPUT.c -o OUTPUT.ll
build/hcc --link [options] INPUT... -o PROGRAM
build/hcc -E [options] INPUT.c [-o OUTPUT.i]
build/hcc --dump-STAGE [options] INPUT.c [-o OUTPUT]
build/hcc --version
```

`hcc --help` lists every option and the [user guide](docs/USER_GUIDE.md)
explains them: `--link`, `-I`/`-D`/`-U`, `-std=c11`/`-std=gnu11`,
`-target`, `-isysroot`, the debugging dumps
([docs/DEBUGGING.md](docs/DEBUGGING.md)) and the exit statuses. Exactly
what hcc accepts and rejects is in [docs/LANGUAGE.md](docs/LANGUAGE.md).

## Test

```sh
make test          # cargo test, then harness/run_tests.py against Clang
```

`harness/run_tests.py` compiles each program in `tests/` with hcc and with
Clang, runs both, and compares exit status, standard output and standard
error. The external suites are fetched at pinned versions and run with
`make wacc`, `make csmith`, `make csmith-campaign`, `make otawk`,
`make awk-extra`, `make torture`, `make doom`, `make lua`, `make sqlite`
and `make perf`; QuickJS and zlib with `harness/run_quickjs.py` and
`harness/run_zlib.py` (see [tests/external/README.md](tests/external/README.md)).
`harness/run_cross.py` and `harness/windows_x64.py` run the differential
tests for arm64 under qemu and for Windows under Wine.

### Big programs

Each is built twice, with hcc and with Clang, at `-O0` and `-O2`, and the
two builds must behave identically:

- **DOOM** (doomgeneric and Freedoom): all eight demos, 39,209 frames,
  match frame for frame, also natively on macOS ([reports/doom/](reports/doom/)).
- **Lua 5.4.7**: the official test suite in three modes, nine workloads
  and `luac` bytecode match ([reports/lua/](reports/lua/)).
- **SQLite 3.54.1**: ten SQL workloads match byte for byte, database
  files included ([reports/sqlite/](reports/sqlite/)).
- **QuickJS 2026-06-04**: `qjsc` output, the Makefile tests and test262
  (83,558 runs) match ([reports/hardening/quickjs.md](reports/hardening/quickjs.md)).
- **zlib 1.3.1**: `example`, `minigzip` and a 60-case compression round
  trip; both builds write identical gzip streams
  ([reports/hardening/zlib.md](reports/hardening/zlib.md)).

## CI and CodeRabbit

Every action is pinned to a commit: `actions/checkout` v4.4.0,
`dtolnay/rust-toolchain` v1 (Rust 1.85.1), `actions/cache` v4.2.4,
`actions/upload-artifact` v4.6.2 and `actions/download-artifact` v4.3.0.
Job tokens have read-only access to the repository contents.

- `ci.yml`, on pull requests and pushes to `main` (not for documentation
  only): formatting, clippy, unit tests, the differential harness, dump
  tests and the arm64 and Intel macOS checks under qemu (25 minutes at
  most), plus one job each for the Windows stand-in under Wine, the
  Writing a C Compiler suite, Csmith, One True AWK, GCC torture,
  performance, DOOM, Lua, QuickJS, SQLite and zlib.
- `linux-arm64.yml`: a quick native arm64 job on pull requests, and the
  full suites nightly after `main` changes.
- `macos.yml` after merges to `main` and by hand; `windows.yml` by hand;
  `weekly-platforms.yml` every week ([docs/WEEKLY_PLATFORMS.md](docs/WEEKLY_PLATFORMS.md));
  `fuzz.yml` by hand ([docs/FUZZING.md](docs/FUZZING.md)).
- `release.yml` builds the release archives on a version tag
  ([docs/RELEASING.md](docs/RELEASING.md)).

CodeRabbit reviews every pull request, configured by `.coderabbit.yaml`.
Dependabot (`.github/dependabot.yml`) proposes weekly updates to the
pinned actions and the Cargo dependencies.

## Key files

- `src/`: hcc itself: `pp` (preprocessor), `lexer`, `parser`, `ast`,
  `sema`, `irgen`, `abi`, `target`, `aapcs64`, `win64`, `link` and `driver`
- `include/`: the headers hcc supplies (`stddef.h`, `stdarg.h`,
  `stdatomic.h`, `threads.h`, `tgmath.h`, ...); `lib/threads.c`: the C11
  threads runtime for C libraries without one
- `tests/`: the differential test programs; `harness/`: the test runners,
  including `harness/run_quickjs.py` and `harness/run_zlib.py`
- `reports/`: scorecards and campaign reports, including `reports/hardening/`
- `docs/`: the user guide, the language reference and the other guides
- `PLAN.md`: the staged plan; `STATUS.md`: detailed project status and
  test results; `DESIGN.md`: design decisions
- `CHANGELOG.md`, `docs/release-notes/`: release history and notes

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Outside contributions are welcome,
but are reviewed slowly.

## Licence

hcc is MIT (see [LICENSE](LICENSE)). The programs it is tested on are
third-party, downloaded at pinned versions and not stored in this
repository: the Writing a C Compiler test suite (MIT, Copyright (c) 2023
Nora Sandler; see [tests/external/README.md](tests/external/README.md)),
doomgeneric (GPL-2.0), the Freedoom game data (BSD), Lua 5.4.7 and its
test suite (MIT), the SQLite 3.54.1 amalgamation (public domain), QuickJS
2026-06-04 (MIT) and zlib 1.3.1 (zlib licence). They are test inputs
only.

---

*This README.md was created by the Grok Bot with limited human review.*
