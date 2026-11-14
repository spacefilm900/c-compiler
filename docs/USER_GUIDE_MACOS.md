# Using the compiler on a Mac with Apple Silicon

> **Checked on a CI Mac, not yet by a person.** Sections 2 and 3 (build,
> hello world) and the test suite run on GitHub's Apple Silicon runner
> (`macos-26`: macOS 26.7, Xcode 26.6, Apple Clang 21, the macOS 26.5 SDK)
> in the `macos` workflow. Nobody has followed this guide by hand on their
> own Mac yet; if a step fails, please note the exact command and message.

This compiler turns C11 source into LLVM IR (`.ll`). Clang then compiles
that IR into a program. On an Apple Silicon Mac (M1 or later) the compiler
targets `arm64-apple-macos` by default.

## 1. Install the tools

You need three things.

1. **Xcode Command Line Tools** — Apple's compiler, linker, and the macOS
   SDK (the C library headers). In Terminal:

   ```sh
   xcode-select --install
   ```

   Check it with `xcrun --sdk macosx --show-sdk-path`; it should print a
   path ending in `MacOSX.sdk`.

2. **Rust** — to build the compiler. Install it from <https://rustup.rs>,
   then open a new Terminal window and check `cargo --version`. The
   project's CI uses Rust 1.85.1; any later stable release should work.

3. **Optional: Homebrew LLVM** — for `llc` and other LLVM tools. It is
   not needed to build programs: Apple's Clang 21 (Xcode 26) reads the IR
   this compiler writes for the whole test suite. With an older Xcode, if
   Apple's Clang rejects a `.ll` file, install LLVM with Homebrew
   (<https://brew.sh>):

   ```sh
   brew install llvm
   export PATH="$(brew --prefix llvm)/bin:$PATH"
   ```

   Homebrew's Clang still uses Apple's SDK and linker.

## 2. Build the compiler

```sh
git clone https://github.com/spacefilm900/c-compiler.git
cd c-compiler
make            # or: cargo build --release && mkdir -p build && cp target/release/compiler build/
```

The result is `build/compiler`.

## 3. Hello, world

Save this as `hello.c`:

```c
#include <stdio.h>

int main(void) {
    printf("Hello from the clean-room compiler on %s\n", "macOS");
    return 0;
}
```

Then:

```sh
build/compiler hello.c -o hello.ll     # C to LLVM IR (target: arm64-apple-macos)
clang hello.ll -o hello                # IR to a Mac program
./hello
```

This prints `Hello from the clean-room compiler on macOS` on the CI Mac.
Apple's Clang may warn `overriding the module target triple with
arm64-apple-macosx26.0.0`: the `.ll` file says macOS 11.0 (the default
minimum), and Clang uses the running system's version. The warning is
harmless; `-target arm64-apple-macos26` in the first command matches them.

`build/compiler` finds the SDK with `xcrun`. To name it yourself, pass
`-isysroot "$(xcrun --sdk macosx --show-sdk-path)"`, or set `SDKROOT`.
`-target arm64-apple-macos14` sets the minimum macOS version (default 11.0).

Link with `clang` (that is, `/usr/bin/clang`) or `xcrun clang`. Calling the
toolchain's Clang by its full path (what `xcrun -f clang` prints) without
`SDKROOT` set fails with `ld: library 'System' not found`, because only the
`/usr/bin` shim tells it where the SDK is.

`-std=gnu11` turns on the GNU extensions that the big programs need (see
the README).

## 4. What works and what does not

On the CI Mac, `harness/macos/sdk_headers.sh` includes 26 C11 headers and
8 POSIX ones (`unistd.h`, `fcntl.h`, `dirent.h`, `sys/types.h`,
`sys/stat.h`, `sys/time.h`, `sys/wait.h`, `pthread.h`) one at a time, in
both `-std=c11` and `-std=gnu11`. All compile except `<tgmath.h>` and
`<uchar.h>`. `<complex.h>`, `<stdatomic.h>`, and `<threads.h>` are not
checked: this compiler has no complex types, atomics, or threads (it
defines `__STDC_NO_COMPLEX__`, `__STDC_NO_ATOMICS__`, and
`__STDC_NO_THREADS__`).

- **`<tgmath.h>` and `<complex.h>`** fail: this compiler has no complex
  types (it defines `__STDC_NO_COMPLEX__`), and Apple's `<tgmath.h>`
  includes `<complex.h>`.
- **`<uchar.h>`** is not in the macOS SDK, so `char16_t`/`char32_t`
  programs that include it do not build with Apple's Clang either.
- **`_Float16`** appears in the SDK's `<math.h>` (half-precision
  functions). This compiler accepts it in declarations only; using a
  `_Float16` value is an error.
- **`long double`** is the same as `double` on Apple Silicon, so the
  `long double` math functions are the `double` ones under another name.
  This is how macOS works, not a bug.
- **ELF section names** such as `__attribute__((section(".mydata")))` are
  rejected: Mach-O needs `"segment,section"`, for example
  `"__DATA,__mydata"`.
- **A linker error about a symbol starting with `_`.** Mach-O adds `_` to
  C names. Report the symbol name.

## 5. Running the tests

```sh
make
cargo test --lib
KNOWN_FAILURES=harness/macos_known_failures.txt python3 harness/run_tests.py
```

`run_tests.py` compiles each test program with this compiler and with
Apple's Clang and compares the two programs' output. On the CI Mac it
reports 175 of 179 passing; the four in `harness/macos_known_failures.txt`
are Linux-specific tests (a 10-byte `long double`, `<uchar.h>`, glibc's
`__sysv_signal`, an ELF section name). `cargo test` skips 21 unit tests
that build and run x86_64 Linux code.

## 6. DOOM (later)

DOOM, Lua, and SQLite already build with this compiler on Linux and match
Clang's programs exactly. On the Mac the plan is the same harness
(`make doom`), with the headless DOOM backend comparing frame hashes against
a Clang build. That has not been tried yet.

## What has been checked without a Mac

- For every function in the test programs, the arguments and results are
  passed the way Clang passes them for `arm64-apple-macos` (434 functions).
- LLVM's `llc` builds the compiler's output into Mach-O objects (159
  programs).
- The test programs run with Apple's calling convention under the
  `qemu-aarch64` emulator, linked against Linux's C library, and match
  Clang's results (179 of 181; the 2 known differences are Linux-specific
  test details).

## What the CI Mac checks

The `macos` workflow (`.github/workflows/macos.yml`) runs after every merge
to `main` and by hand (`gh workflow run macos.yml`); it does not run on pull
requests because macOS minutes cost about ten times Linux minutes. It builds
the compiler, runs the unit tests, builds and runs hello world, runs
`run_tests.py` against Apple's Clang with the SDK's headers, C library, and
Mach-O linker, and reports which SDK headers compile. A run takes under a
minute. DOOM, Lua, and SQLite on the Mac are not covered yet.
