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

> hcc is the result of an experiment in agentic software engineering. It
> is not intended to replace established supported compilers such as GCC
> or Clang, and it is provided strictly "as is" under the MIT licence.

**Version 1.0.1.** [User guide](docs/USER_GUIDE.md) ·
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

- **C11** (N1570), except the optional complex number types (`_Complex`).
  A clause-by-clause audit of clauses 5 to 7 has 288 probes: 54 programs
  and 234 constraint or syntax violations, compared against
  `clang -std=c11 -pedantic-errors`. hcc passes 286 of them. The 2 that
  fail are one known gap: the inline-definition constraints of 6.7.4p3 are
  not diagnosed ([docs/C11_CONFORMANCE.md](docs/C11_CONFORMANCE.md)).
  This is not a formal certification. The compiler supports the harder
  parts: variable-length
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
  scorecards. [Test evidence](#test-evidence) gives the numbers, with what
  each one counts.

**Play DOOM on a Mac:** [docs/DOOM_ON_MAC.md](docs/DOOM_ON_MAC.md) is a
step-by-step guide for an Apple Silicon Mac. One command,
`./tools/mac/play_doom.sh`, builds DOOM with hcc and opens it in a window.

## Why Rust

Rust is a deliberate break from the usual practice of implementing C in C
or C++. It gives memory safety without a garbage collector, and Cargo
builds a single binary from the standard library alone. There are no
crates.io dependencies, so a checkout builds without fetching packages.

## Design

The decisions behind this compiler are in [DESIGN.md](DESIGN.md). That
design document was created wholly by the Grok Bot.

## Clean-room provenance

hcc's compiler code was written only from public specifications:

- the C11 standard (the N1570 draft),
- the LLVM Language Reference,
- the platform ABI documents: the System V AMD64 ABI, the Arm AAPCS64
  with Apple's arm64 rules, and the Microsoft x64 calling convention,
- the public GCC manual, for what GNU extensions and attributes mean.

Clang and GCC were used only as black boxes, to see what a program
compiles to and what it prints. No compiler's or C library's source code was
read, copied or adapted while hcc was written, and none is in this
repository.
Clang is also the back end that turns hcc's LLVM IR into programs; hcc
does not call Clang's or GCC's front end. The test programs and suites
(Csmith, YARPGen, the GCC torture tests, the Writing a C Compiler suite,
the adversarial kits, and the big programs) are test inputs, and nothing
from them is in the compiler.

This records how the work was done. It is not a legal opinion or a
certification. The code was written by an AI agent, and the model behind
it was trained on public data that may include compiler source. The rules
above govern what was consulted during development; they cannot show what
the model learnt in training. The rules are in `PLAN.md` and
[CONTRIBUTING.md](CONTRIBUTING.md), and [DESIGN.md](DESIGN.md#clean-room)
gives the history. Where the standard and the ABI documents leave a
choice, DESIGN.md's [tie-break rule](DESIGN.md#tie-break) says how it was
made.

## Platforms

Six targets. The pull-request CI runs on x86-64 Linux and native arm64
Linux. The other targets are checked there through stand-ins, and on
their real platforms by the weekly and manual workflows
([docs/WEEKLY_PLATFORMS.md](docs/WEEKLY_PLATFORMS.md)).

| Target | Every pull request | Real platform | `<threads.h>` |
| --- | --- | --- | --- |
| Linux x86-64 | native (`ubuntu-24.04`): every suite | the same | glibc |
| Linux arm64 | native (`ubuntu-24.04-arm`) quick job, and the harness under qemu | native nightly (full suites) and weekly | glibc |
| macOS arm64 | emulated: IR and Mach-O checks with `llc`, and the harness under qemu with Apple's calling convention | native `macos-26` after merges; weekly `macos-15` | fallback runtime (`lib/threads.c`) |
| macOS x86-64 | emulated: IR and Mach-O checks, and a Linux stand-in for the harness and random ABI pairs | native `macos-26-intel`, by hand only | fallback runtime |
| Windows x64, MinGW-w64 | emulated: the harness and random ABI pairs under Wine | native `windows-latest` (MSYS2), weekly and by hand | fallback runtime |
| Windows x64, MSVC | emulated: the MSVC type model under Wine, and IR and COFF checks | native `windows-latest` (MSVC and UCRT), weekly and by hand | fallback runtime |

The fallback runtime is hcc's own small `<threads.h>` implementation over
POSIX threads or the Win32 API. The adversarial kits' thread cases on
those platforms compare hcc's build of it with Clang's build of the same
runtime, not with an independent C11 threads library.

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

### Test evidence

The first seven rows come from the release-candidate rerun on commit
`2e80ec0` ([reports/release-1.0/README.md](reports/release-1.0/README.md)).
Mutation testing and coverage come from the hardening and quality work on
earlier commits ([reports/hardening/](reports/hardening/README.md)). Each
row says what it counts.

| Check | Scope | Result |
| --- | --- | --- |
| C11 conformance probes | N1570 clauses 5 to 7: 54 programs and 234 constraint or syntax violations, x86-64 Linux; 20 more cases the standard leaves undefined are counted apart | 286 of 288 pass, 2 known gaps; all 20 others diagnosed |
| Differential harness | the programs in `tests/`, x86-64 Linux | 230 of 230 |
| Writing a C Compiler suite | chapters 1 to 18 (19 and 20 inspect the book's own assembly) | 1,384 of 1,384 run, 233 skipped |
| GCC torture tests (`execute`) | 1,746 executed | 1,412 passed, 334 listed known failures, 0 unexpected |
| Csmith | 3,000 generated programs (seeds 200001 to 203000); a program counts once GCC and Clang agree on it | 2,775 compared at `-O0` and 2,774 at `-O2`, 0 mismatches |
| YARPGen 2 | 2,000 generated programs (seeds 1 to 2000), about 8,000 lines each; a program counts once Clang's reference run completes | 1,651 compared, 0 mismatches (2 timeouts under load matched when re-run) |
| Adversarial kits | v0.6 (724 cases), v0.7 (65), v0.8 (80), v0.9 (100), each at `-O0` and `-O2` | all 969 pass at both levels |
| Mutation testing | 7 selected files (constant folding, static images, record layout, IR for expressions) | 92.9% of 1,429 viable mutants caught: 96 missed, 5 timeouts |
| Coverage | hcc's Rust code, over the unit tests and every in-repo suite | 92.7% of lines, 83.4% of branches |

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

Every action is pinned to a commit: `actions/checkout` v7.0.1,
`dtolnay/rust-toolchain` v1 (Rust 1.85.1), `actions/cache` v6.1.0,
`actions/upload-artifact` v7.0.2 and `actions/download-artifact` v8.0.2.
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
