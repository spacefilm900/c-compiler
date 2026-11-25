# Changelog

All notable changes to hcc are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and hcc uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html): for hcc the
public interface is the command line (`hcc --help`), the accepted C
language (`-std=c11` and `-std=gnu11`), and the ABI of the code it emits
for each target.

## [Unreleased]

## [1.0.1] - Unreleased

### Fixed

- In an old-style (K&R) definition, a later declaration in the
  parameter declaration list can no longer take the address of a
  `register` parameter (C11 6.5.3.2p1). For example,
  `void f(r, a) register int r; int a[sizeof &r]; {}` was accepted and is
  now an error.
- The user guide and SECURITY.md say to check a download against
  `SHA256SUMS`. They no longer mention per-archive `.sha256` files, which
  the release does not publish.
- The date of 1.0.0 is now 2026-11-25, the day it was released.

## [1.0.0] - 2026-11-25

The first release. hcc is a clean-room C11 compiler written in Rust that
emits textual LLVM IR, with builds for Linux x86-64, Linux arm64, macOS on
Apple Silicon and Windows x64. Earlier work was not versioned; the
milestones below summarise it, with the pull requests that made them.

### Added

- Release builds for Linux x86-64 and arm64 (static), macOS arm64 and
  Windows x64, each an archive with the compiler, its headers,
  `lib/threads.c`, the user guide and a SHA-256 checksum
  (`.github/workflows/release.yml`).
- `hcc --version`: the version, the Git commit, the default target, every
  supported target and the LLVM requirement.
- `hcc --link`: compile each C source to IR and link it with the other
  inputs (`.ll`, `.o`, `.a`) into a program with Clang, passing `-O`, `-l`,
  `-L`, `-Wl,` and `-pthread` in command-line order. A program that uses `<threads.h>` gets
  `-pthread`, and `lib/threads.c` where the C library has no `<threads.h>`
  (macOS, Windows, Linux before glibc 2.28), automatically.
- Diagnostics show the source line and a caret under the column;
  `-fno-caret-diagnostics` turns that off.
- The three C11 preprocessor features hcc lacked: trigraphs (replaced
  under `-std=c11`, and under `-std=gnu11` with the new `-trigraphs`
  option), `__DATE__` and `__TIME__` (UTC, or `SOURCE_DATE_EPOCH` for
  reproducible builds), and the `_Pragma` operator, now carried out as a
  `#pragma` line (`_Pragma("once")` takes effect) (#71).
- Universal character names in identifiers (C11 6.4.2.1, 6.4.3, Annex D),
  and the same characters written directly in UTF-8, as GCC and Clang
  accept them: `caf\u00e9` and `café` are one identifier, also as a macro
  name and through `##`. Invalid UCNs and characters outside Annex D are
  diagnostics. The LLVM symbol is the quoted UTF-8 name, so extended
  names link with Clang and GCC objects on every target.
- Multi-character character constants (`'ab'`, C11 6.4.4.4p10), with the
  value GCC and Clang give on every target (each byte shifted in from the
  right, the last four kept, as an `int`) and their warnings. A
  multi-character `L`, `u` or `U` constant is a diagnostic, as in Clang.
- [docs/C11_CONFORMANCE.md](docs/C11_CONFORMANCE.md): a clause-by-clause
  audit of N1570 clauses 5 to 7. Its 288 probes, 54 programs and 234
  constraint or syntax violations (`conformance/`, run by
  `harness/run_conformance.py`), are compared with
  `clang -std=c11 -pedantic-errors`. 286 pass, and the other 2 are a
  known gap (6.7.4p3). CI runs the probes, and `tools/check_docs.py`, which
  compiles the language claims in the documentation.
- A [user guide](docs/USER_GUIDE.md), a
  [language reference](docs/LANGUAGE.md) (moved out of the README), this
  changelog, [CONTRIBUTING.md](CONTRIBUTING.md),
  [SECURITY.md](SECURITY.md) and [docs/RELEASING.md](docs/RELEASING.md).
- Dependabot version updates, weekly, for the GitHub Actions and Cargo
  dependencies (`.github/dependabot.yml`). Vulnerabilities are reported
  through GitHub private vulnerability reporting (SECURITY.md).

### Changed

- `hcc --link` refuses C++, Objective-C and other non-C sources that Clang
  would compile itself, passes a file named `@...` as a file rather than
  a Clang response file, and recognises quoted `<threads.h>` names in
  `.ll` inputs.
- `hcc --help` is grouped by topic and lists every option, the exit
  statuses and the targets.
- Command-line errors say what is wrong (`hcc: error: unknown option
  '-x'`, `missing argument to '-o'`, `no input file`, `more than one input
  file`) instead of printing the whole usage text.
- `-o -` writes the IR, the preprocessed text or a dump to standard
  output, as other compilers do, instead of a file named `-`; `--link`
  rejects it.
- `&&a - &&b` (a difference of GNU label addresses) is reported as not
  supported, instead of as arithmetic on `void *`.
- The README's C11 statement now says what the conformance audit shows.
  The README also gives the scope and denominator of each test figure
  (mutation testing, Csmith, YARPGen), has a per-target table of what is
  tested natively, under emulation and with the fallback threads runtime,
  and has a clean-room provenance section. DESIGN.md states the tie-break
  rule as it is applied: the standard, then the ABI documents, then a
  written-down manual decision, with Clang as the CI oracle.
- CI actions updated by Dependabot (#72-#75): `actions/checkout` v7.0.1,
  `actions/cache` v6.1.0, `actions/upload-artifact` v7.0.2 and
  `actions/download-artifact` v8.0.2, each pinned to its commit.

### Fixed

- Rescanning (C11 6.10.3.4p2): a macro no longer stays "being replaced"
  after rescanning has read past the end of its replacement list, so
  6.10.3.5's `f(2)(9)` is `2*9*g`, not `2*f(9)`.
- A file-scope `int a[];` that is never completed is a one-element array,
  with a warning (6.9.2p2), instead of an error.
- These constraint violations, which were accepted, are now diagnostics:
  `__VA_ARGS__` in a macro that is not variadic (6.10.3p5); `auto` or
  `register` at file scope (6.9p2); `inline` or `_Noreturn` on `main`
  (6.7.4p4); a function definition whose function type comes from a
  typedef (6.9.1p2); `&` of a `register` object or of a member of one
  (6.5.3.2p1); and the `_Atomic` qualifier on an array or function
  typedef (6.7.3p3). `#undef` of the predefined macros of 6.10.8.1, and
  `#define` of `__LINE__` and `__FILE__`, are now errors too (undefined
  behaviour, 6.10.8p2).
- `#` no longer puts a `\` before a lone `\` token (only inside string
  literals and character constants, C11 6.10.3.2).
- GNU forward-declared enums (`typedef enum E E;` before `enum E { ... }`)
  are incomplete until their definition, as in GCC and Clang, and may then
  take any enumerated type (#68, #69).

### Milestones before 1.0

- **Hardening** (#63–#67): `cargo-fuzz` targets for the lexer, the
  preprocessor, the parser and the whole front end; large Csmith and
  YARPGen campaigns against Clang at `-O0` and `-O2`; coverage measurement
  and mutation testing with the fixes they found; QuickJS (its tests and
  test262 match a Clang build) and zlib (its tests and a round trip match)
  built by hcc and checked in CI.
- **Platform tests** (#61): a weekly run on real arm64 Linux, macOS and
  Windows machines, plus sanitizer builds.
- **Adversarial review** (#39, #58–#60): independent challenge kits v0.6 to
  v0.9; 969 of 969 cases pass at `-O0` and `-O2`.
- **C11 completed** (#53, #55–#57): `_Alignas`, `_Thread_local`, K&R
  definitions, `<tgmath.h>`, variable-length arrays, `_Atomic` with
  `<stdatomic.h>`, and `<threads.h>` with its own runtime.
- **Name** (#54): the program is `hcc`, the Haviland C Compiler.
- **Windows x64** (#52): MSVC and MinGW-w64 targets.
- **macOS on Intel** (#50) and **Linux on arm64** (#49), including
  binary128 `long double` folding.
- **Quality** (#42, #44–#48): internal compiler errors instead of crashes,
  smaller modules, stage dumps (`--dump-tokens`, `--dump-ast`,
  `--dump-sema`, `--dump-ir`) and a test-case reducer.
- **macOS on Apple Silicon** (#36–#38): the AAPCS64 and Apple calling
  conventions, the macOS SDK headers, a macOS CI job, and DOOM in a window
  on a Mac.
- **Compile-time performance** (#35): the SQLite amalgamation's front end
  from 111 s to 1.3 s with identical output.
- **Big programs** (#32–#34): DOOM (frame-for-frame identical to a Clang
  build), Lua 5.4.7 and SQLite 3.54.1, each with a scorecard.
- **GNU dialect** (#27, #29–#31): opt-in `-std=gnu11` with `typeof`,
  statement expressions, attributes, case ranges, zero-length arrays,
  `?:` without a middle operand, labels as values and many builtins.
- **Testing at scale** (#17, #18): Csmith differential testing, One True
  AWK with its test suites, and the GCC torture tests as inputs.
- **The C language** (#1–#16, #19–#25): integers, pointers, arrays,
  structs and unions with the System V layout, the preprocessor, floating
  types including x87 `long double`, aggregates by value, variadic
  functions, `switch`, `goto`, designated initializers, bit-fields,
  `_Generic`, `_Static_assert`, compound literals and Unicode literals;
  the Writing a C Compiler test suite passes.

[Unreleased]: https://github.com/spacefilm900/c-compiler/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/spacefilm900/c-compiler/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/spacefilm900/c-compiler/releases/tag/v1.0.0
