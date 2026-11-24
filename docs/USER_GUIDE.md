# hcc user guide

hcc, the Haviland C Compiler, was developed as a test of fully agentic
development using a strong specification, readily available test suites and
test data, and embedded software engineering practices such as configuration
management, CI, agentic code review, structured testing including
adversarial testing using an independent AI platform. It is written in Rust
and was developed by a Grok Bot, that created the code, its own test suites
and the majority of documentation without human involvement. There was human
review at process level.

hcc, the Haviland C Compiler, compiles C11 to textual LLVM IR. LLVM, through
Clang, turns the IR into machine code. This guide covers installing hcc,
compiling and linking programs, the options, the language dialects, the
targets, the known limitations, and what to do when something goes wrong.
It describes version 1.0.0; `hcc --version` prints the version you have.

- [1. Install](#1-install)
- [2. First program](#2-first-program)
- [3. Compiling and linking](#3-compiling-and-linking)
- [4. Options](#4-options)
- [5. Dialects: -std=c11 and -std=gnu11](#5-dialects--stdc11-and--stdgnu11)
- [6. Targets](#6-targets)
- [7. Threads](#7-threads)
- [8. Diagnostics and exit status](#8-diagnostics-and-exit-status)
- [9. Known limitations](#9-known-limitations)
- [10. Troubleshooting](#10-troubleshooting)

**A few terms used in this guide:**

- **LLVM IR**: LLVM's intermediate representation, a portable,
  human-readable text form of a program (`.ll` files) that LLVM turns into
  machine code.
- **Translation unit**: one `.c` file together with everything it
  `#include`s; hcc compiles one at a time.
- **Target triple**: the name of the machine and system to compile for,
  such as `x86_64-linux-gnu` or `arm64-apple-macos`.
- **Data layout**: the sizes and alignments of types on a target, written
  at the top of each IR file so LLVM lays out memory the same way.
- **ABI** (application binary interface): a platform's rules for how
  functions pass arguments and return values and how data is laid out, so
  code from different compilers can call each other.

## 1. Install

hcc needs **Clang/LLVM 18 or newer** to turn its IR into programs. Emitting
IR alone needs nothing else.

| Platform | Clang |
| --- | --- |
| Linux | your distribution's `clang` package (Debian/Ubuntu: `sudo apt install clang`) |
| macOS | Xcode Command Line Tools: `xcode-select --install` (keep them up to date) |
| Windows | LLVM for Windows (<https://releases.llvm.org>) and Visual Studio's C++ tools (the linker and the Windows SDK), or MinGW-w64 |

### From a release archive

Each release has one archive per platform, with a `.sha256` file and a
`SHA256SUMS` list:

| Archive | Runs on |
| --- | --- |
| `hcc-1.0.0-linux-x86_64.tar.gz` | 64-bit Intel/AMD Linux (static binary, any distribution) |
| `hcc-1.0.0-linux-arm64.tar.gz` | 64-bit Arm Linux (static binary) |
| `hcc-1.0.0-macos-arm64.tar.gz` | Macs with Apple Silicon (M1 or later), macOS 11 or later |
| `hcc-1.0.0-windows-x86_64.zip` | 64-bit Windows 10 or later |

Check the download, then unpack it anywhere:

```sh
sha256sum -c hcc-1.0.0-linux-x86_64.tar.gz.sha256   # macOS: shasum -a 256 -c ...
tar -xzf hcc-1.0.0-linux-x86_64.tar.gz
export PATH="$PWD/hcc-1.0.0-linux-x86_64/bin:$PATH"
hcc --version
```

On Windows, unpack the `.zip` (Explorer's "Extract All", or
`Expand-Archive` in PowerShell) and add its `bin` folder to `PATH` for the
current PowerShell session:

```powershell
Expand-Archive hcc-1.0.0-windows-x86_64.zip -DestinationPath .
$env:Path = "$PWD\hcc-1.0.0-windows-x86_64\bin;$env:Path"
hcc --version
```

Keep the directory layout: hcc finds its own headers in `include/` and the
threads runtime in `lib/` next to its `bin/` directory.

On macOS, a browser download is quarantined. If macOS refuses to open
`hcc`, clear the flag once, from the directory you unpacked it in:
`xattr -d com.apple.quarantine hcc-1.0.0-macos-arm64/bin/hcc`.

### From source

hcc is Rust with no dependencies beyond the standard library. With Rust 1.85
or newer (<https://rustup.rs>):

```sh
git clone https://github.com/spacefilm900/c-compiler.git
cd c-compiler
make            # or: cargo build --release, then copy target/release/hcc
build/hcc --version
```

`make` writes `build/hcc`. Run it from there, or copy `build/hcc` together
with `include/` and `lib/` (as `bin/hcc`, `include/`, `lib/`) to install it.

## 2. First program

```c
/* hello.c */
#include <stdio.h>

int main(void) {
    printf("hello, world\n");
    return 0;
}
```

```sh
hcc --link hello.c -o hello      # compile and link in one step
./hello
```

or in two steps, keeping the IR:

```sh
hcc hello.c -o hello.ll          # C to LLVM IR
clang -w hello.ll -o hello       # IR to a program
```

On Windows the program is `hello.exe`.

## 3. Compiling and linking

**One file to IR.** `hcc FILE.c -o FILE.ll` compiles one translation unit.
The output is an LLVM IR module for the target, with its triple and data
layout, which `clang`, `llc` and `opt` accept. `-o -` writes the IR (or,
with `-E`, the preprocessed text) to standard output instead of a file.

**Several files.** Compile each to IR, then link the `.ll` files (and any
`.o` or library) with Clang:

```sh
hcc main.c -o main.ll
hcc util.c -o util.ll
clang -O2 -w main.ll util.ll -lm -o prog
```

or let hcc do it:

```sh
hcc --link -O2 main.c util.c -lm -o prog
```

With `--link`, every `.c` input, and every `.i` file written by `hcc -E`, is
compiled by hcc. (Output from another preprocessor starts with line
markers such as `# 1 "main.c"`, which hcc does not accept.) Every other
input (`.ll`, `.o`, `.a`, `.so`, `.obj`, `.lib`) goes to Clang as it is, so a
program can mix hcc's code with objects from another compiler. Sources in
other languages that Clang would compile itself (C++ such as `.cpp`,
Objective-C, headers) are refused, so nothing reaches the program
unchecked by hcc. hcc runs
`$HCC_CLANG` if it is set, else `clang` from `PATH`, with the target's LLVM
triple. Clang never sees the C sources.

**Optimisation.** hcc's IR is unoptimised; Clang optimises it. Pass `-O2`
(or `-O1`, `-O3`, `-Os`, `-Oz`) to `--link`, or to `clang` when you link the
`.ll` yourself.

**Preprocessing only.** `hcc -E FILE.c` writes the preprocessed text to
standard output (or `-o FILE.i`).

## 4. Options

`hcc --help` lists them all.

| Option | Meaning |
| --- | --- |
| `-o FILE` | Output file: IR, preprocessed text, a dump, or with `--link` the program |
| `--link` | Compile, then link a program with Clang (section 3) |
| `-E`, `--dump-pp` | Preprocess only |
| `--dump-tokens`, `--dump-ast`, `--dump-sema`, `--dump-ir` | Stop after that stage and print it (see [DEBUGGING.md](DEBUGGING.md)) |
| `-I DIR`, `-IDIR` | Add an include directory, searched before the system headers |
| `-D NAME`, `-D NAME=text`, `-D 'F(x)=text'` | Define a macro (`NAME` alone is `1`) |
| `-U NAME` | Undefine a macro; `-D` and `-U` apply in order |
| `-std=c11` | Strict ISO C11, the default |
| `-std=gnu11` | C11 with the GNU extensions hcc supports (section 5) |
| `-trigraphs` | Replace trigraphs under `-std=gnu11` too (`-std=c11` always does) |
| `-fgnu89-inline` | GNU89 `inline` semantics |
| `-target TRIPLE`, `--target=TRIPLE` | Compile for another target (section 6) |
| `-isysroot DIR` | Take the C library headers from `DIR/usr/include` (`DIR/include` for Windows); on macOS the SDK, which `--link` passes to Clang too |
| `-fno-caret-diagnostics` | Print messages without the source line and caret |
| `--time-phases` | Print the front end's phase times as JSON on standard error (not with `--link`) |
| `--version` | Version, commit, default target and supported targets |
| `-h`, `--help` | Help |

With `--link` only, these go to Clang: `-O0`, `-O1`, `-O2`, `-O3`, `-Os`,
`-Oz`, `-l NAME`, `-lNAME`, `-L DIR`, `-LDIR`, `-Wl,ARGS`, `-pthread` and
`-static`. Without `--link` they are an error, because there is nothing to
pass them to. Inputs and these options reach Clang in the order you wrote
them, so `-Wl,--whole-archive lib.a -Wl,--no-whole-archive` works.

hcc defines the usual predefined macros for the target: `__STDC__`,
`__STDC_VERSION__` (`201112L`), `__STDC_HOSTED__`, and the target's own,
such as `__linux__`, `__APPLE__`, `_WIN32`, `__x86_64__` and `__aarch64__`.
The C11 feature macros are described in section 9.

## 5. Dialects: -std=c11 and -std=gnu11

**`-std=c11`** (the default) is ISO C11 (N1570). Code that breaks a C11
constraint is an error.

C11 makes some features optional. hcc supports all of them except complex
numbers: variable-length arrays, `_Atomic` with `<stdatomic.h>`, and
`<threads.h>` all work, so `__STDC_NO_VLA__`, `__STDC_NO_ATOMICS__` and
`__STDC_NO_THREADS__` are not defined.

**`-std=gnu11`** adds the GNU extensions that real programs (DOOM, Lua,
SQLite, QuickJS, zlib, the Linux and macOS system headers) use, as GCC and
Clang accept them in their GNU modes. Among them:

- `typeof` / `__typeof__`, statement expressions `({ ... })`, `?:` with no
  middle operand, case ranges `case 1 ... 5:`, range designators
  `[0 ... 3] = x`, zero-length arrays, labels as values (`&&label`,
  `goto *p`), `__int128`, enum values outside `int`, forward-declared
  enums, and casting a struct to its own type;
- `__attribute__((...))`: `aligned`, `packed`, `noreturn`, `noinline`,
  `always_inline`, `section`, `weak`, `deprecated`, `constructor`,
  `destructor` and `mode` have their GCC meaning; hints such as `format`
  and `nonnull` are accepted and ignored; an unknown attribute is a
  warning;
- builtins such as `__builtin_expect`, `__builtin_unreachable`,
  `__builtin_clz` and friends, `__builtin_add_overflow` and friends,
  `__builtin_constant_p`, `__builtin_offsetof`, `__builtin_va_*`;
- a few C11 constraint violations become warnings, as in GCC and Clang
  (comparing `void *` with a function pointer, pointer conversions that
  differ only in signedness or drop a qualifier).

The full list, with the exact rules, is in
[LANGUAGE.md](LANGUAGE.md#gnu-extensions).

**Trigraphs** (`??=` for `#`, `??/` for `\`, and the other seven, C11
5.2.1.1) are replaced under `-std=c11`. Under `-std=gnu11` they are left
as written, as GCC does, unless you also pass `-trigraphs`.

**`__DATE__` and `__TIME__`** give the time of compilation in UTC. For
reproducible builds, set `SOURCE_DATE_EPOCH` to a number of seconds since
1970-01-01 00:00:00 UTC; hcc then uses that time instead. A value that is
not a plain decimal number (at most 253402300799) is an error.

## 6. Targets

The default target is the machine hcc runs on. `-target` picks another;
hcc then emits IR for that target, and Clang needs that target's C library
to link it.

| Target | `-target` | Notes |
| --- | --- | --- |
| Linux x86-64 | `x86_64-linux-gnu` | System V AMD64 ABI, glibc headers |
| Linux arm64 | `aarch64-linux-gnu` | AAPCS64, glibc headers; from x86-64, the Debian/Ubuntu cross headers |
| macOS arm64 | `arm64-apple-macos` (or `arm64-apple-macos14`) | Apple's arm64 ABI; minimum macOS 11 unless a version is given; headers from the SDK (`xcrun`, `$SDKROOT` or `-isysroot`) |
| macOS x86-64 | `x86_64-apple-macos` (or `x86_64-apple-macos13`) | minimum macOS 10.15 |
| Windows x64, MinGW-w64 | `x86_64-w64-windows-gnu` | Win64 ABI, MinGW-w64 headers (`/usr/x86_64-w64-mingw32` or `-isysroot`) |
| Windows x64, MSVC | `x86_64-pc-windows-msvc` | Win64 ABI, MSVC and Windows SDK headers from `INCLUDE` (a Visual Studio developer prompt) or `-isysroot` |

Type sizes in bytes, as hcc lays them out for each target (`char` is 1 and
`long long`, pointers and `size_t` are 8 everywhere):

| Target | `long` | `long double` | `wchar_t` | plain `char` |
| --- | --- | --- | --- | --- |
| `x86_64-linux-gnu` | 8 | 16 (x87 80-bit) | 4 | signed |
| `aarch64-linux-gnu` | 8 | 16 (128-bit IEEE) | 4 | unsigned |
| `arm64-apple-macos` | 8 | 8 (same as `double`) | 4 | signed |
| `x86_64-apple-macos` | 8 | 16 (x87 80-bit) | 4 | signed |
| `x86_64-w64-windows-gnu` | 4 | 16 (x87 80-bit) | 2 | signed |
| `x86_64-pc-windows-msvc` | 4 | 8 (same as `double`) | 2 | signed |

So `long` is 4 bytes on the Windows targets and 8 elsewhere; use
`long long` or `<stdint.h>` types for a 64-bit integer everywhere.

Other spellings of the same triples are accepted (`x86_64-pc-linux-gnu`,
`aarch64-apple-darwin`, `x86_64-w64-mingw32`, ...). On Windows, run hcc
from a "x64 Native Tools Command Prompt for VS" (or after `vcvars64.bat`)
so the MSVC headers and linker are found. A Mac-specific walkthrough is in
[USER_GUIDE_MACOS.md](USER_GUIDE_MACOS.md).

## 7. Threads

C11 `<threads.h>` (`thrd_create`, `mtx_lock`, `cnd_wait`, `tss_get`,
`call_once`, ...) works on every target:

- On Linux with glibc 2.28 or later, the C library provides it. Link with
  `-pthread`.
- On macOS, on Windows, and on a Linux whose C library has no
  `<threads.h>`, hcc's own header declares it and `lib/threads.c` (a small
  runtime over POSIX threads or the Win32 thread API) implements it. Compile
  `lib/threads.c` with hcc and link it with the program.

`hcc --link` does both automatically when the program uses a `<threads.h>`
function. By hand:

```sh
hcc prog.c -o prog.ll
hcc /path/to/hcc/lib/threads.c -o threads.ll     # macOS, Windows, old Linux
clang -w prog.ll threads.ll -pthread -o prog
```

`<stdatomic.h>` and `_Atomic` need nothing extra.

## 8. Diagnostics and exit status

hcc reports the first error and stops, in the usual form, with the line
and a caret:

```text
prog.c:4:12: error: use of undeclared identifier 'cuont'
    return cuont;
           ^
```

A message in an included header names that header. Warnings (only GNU
extensions produce them) do not stop compilation. On an error the output
file is not created.

| Exit status | Meaning |
| --- | --- |
| 0 | Success |
| 1 | An error in the program (or a file could not be read or written, or the link failed) |
| 2 | A command-line error |
| 3 | An internal compiler error: a bug in hcc, please report it |

## 9. Known limitations

- **Complex numbers.** `_Complex` and `<complex.h>` are not supported
  (C11 makes them optional; hcc defines `__STDC_NO_COMPLEX__`).
- **No object files or debug information.** hcc writes LLVM IR only; Clang
  writes objects and programs. There is no `-c`, `-S` or `-g`.
- **One translation unit per run**, except with `--link`.
- **Inline assembly.** Only `asm` statements with no operands are accepted
  (in GNU mode); `__asm__("name")` sets a symbol's linker name.
- **GNU attributes that change behaviour** are errors rather than ignored:
  `vector_size`, `cleanup`, `alias`, `ifunc`, `naked`, `weakref`. Nested
  functions are not supported.
- **Identifiers.** Universal character names (`\u00e9`) and other
  non-ASCII characters are not accepted in identifiers (C11 6.4.2.1 and
  6.4.3 allow UCNs there). Source files are UTF-8; non-ASCII characters
  are allowed in comments, character constants and string literals.
- **Label addresses.** `&&a - &&b` is not supported.
- **Platforms.** The targets in section 6 only. A pointer is 64 bits on all
  of them.
- **A GNU forward-declared enum completed inside a function body** counts as
  complete for that whole function (a use before the definition in the
  same function is not diagnosed).

## 10. Troubleshooting

| Problem | What to do |
| --- | --- |
| `'stdio.h' file not found` | hcc uses the target's C library headers. Linux: install `libc6-dev` (`build-essential`). macOS: `xcode-select --install`, or pass `-isysroot "$(xcrun --show-sdk-path)"`. Windows (MSVC): run from a Visual Studio developer prompt. Cross targets: install that target's headers or pass `-isysroot`. |
| `cannot run 'clang' to link` | Install Clang 18 or newer, or set `HCC_CLANG` to its full path. |
| Clang rejects the `.ll` file (`ptr` or data layout errors) | Clang is older than 18. Check `clang --version`. |
| `lib/threads.c was not found next to hcc` | Keep the archive's layout (`bin/`, `include/`, `lib/` side by side), or build from a full checkout. |
| A GNU extension is rejected in strict mode | Add `-std=gnu11`. |
| `'-O2' is a link option; it needs --link` | hcc's own output is unoptimised IR; optimise when linking (`--link -O2`, or `clang -O2 x.ll`). |
| Undefined `thrd_*` or `mtx_*` symbols when linking by hand | Compile and link `lib/threads.c` (section 7), or use `--link`. |
| `internal compiler error` (exit status 3) | A bug in hcc. Please report it (see [CONTRIBUTING.md](../CONTRIBUTING.md)) with the source, the command line and `hcc --version`. `tools/reduce/reduce.py` in the source tree can shrink the source first. |
| A program behaves differently from a Clang build | Build it with `clang -std=c11` (or `-std=gnu11`) too and compare; if hcc is wrong, that is a bug worth reporting with the smallest program that shows it. |

---

*This user guide was created by the Grok Bot with limited human review.*
