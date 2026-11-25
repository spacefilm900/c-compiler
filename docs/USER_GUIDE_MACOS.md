# Using hcc on a Mac (Apple Silicon and Intel)

hcc turns C11 source into LLVM IR (`.ll`), a text form of a program that
LLVM compiles to machine code. Clang then turns that IR into a program. On
an Apple Silicon Mac (M1 or later) hcc targets `arm64-apple-macos` by
default, and on an Intel Mac `x86_64-apple-macos`. The general
[user guide](USER_GUIDE.md) covers everything that is not Mac-specific.

**The quickest start on Apple Silicon** is the release archive
`hcc-1.0.0-macos-arm64.tar.gz` ([user guide section 1](USER_GUIDE.md#1-install)):
it holds a ready-built `bin/hcc`, so you need only the Xcode Command Line
Tools (step 1 below), not Rust. On an Intel Mac, build hcc from source
(step 2).

## 1. Install the tools

You need three things.

1. **Xcode Command Line Tools** — Apple's compiler, linker, and the macOS
   SDK (the C library headers). In Terminal:

   ```sh
   xcode-select --install
   ```

   Check it with `xcrun --sdk macosx --show-sdk-path`; it should print a
   path ending in `MacOSX.sdk`.

2. **Rust** — to build hcc. Install it from <https://rustup.rs>,
   then open a new Terminal window and check `cargo --version`. The
   project's CI uses Rust 1.85.1; any later stable release should work.

3. **Optional: Homebrew LLVM** — for `llc` and other LLVM tools. It is
   not needed to build programs: Apple's Clang 21 (Xcode 26) reads the IR
   hcc writes for the whole test suite. With an older Xcode, if
   Apple's Clang rejects a `.ll` file, install LLVM with Homebrew
   (<https://brew.sh>):

   ```sh
   brew install llvm
   export PATH="$(brew --prefix llvm)/bin:$PATH"
   ```

   Homebrew's Clang still uses Apple's SDK and linker.

## 2. Build hcc

```sh
git clone https://github.com/spacefilm900/c-compiler.git
cd c-compiler
make            # or: cargo build --release && mkdir -p build && cp target/release/hcc build/
```

The result is `build/hcc`.

## 3. Hello, world

Save this as `hello.c`:

```c
#include <stdio.h>

int main(void) {
    printf("Hello from the clean-room compiler on %s\n", "macOS");
    return 0;
}
```

Then (from the release archive, use `bin/hcc` in place of `build/hcc`
in both forms):

```sh
build/hcc --link hello.c -o hello     # compile and link in one step
./hello
```

or in two steps:

```sh
build/hcc hello.c -o hello.ll         # C to LLVM IR (target: arm64-apple-macos)
clang hello.ll -o hello               # IR to a Mac program
./hello
```

Either way, `./hello` prints `Hello from the clean-room compiler on macOS`.
Apple's Clang may warn `overriding the module target triple with
arm64-apple-macosx26.0.0`: the `.ll` file says macOS 11.0 (the default
minimum), and Clang uses the running system's version. The warning is
harmless; `-target arm64-apple-macos26` in the first command matches them.

`build/hcc` finds the SDK with `xcrun`. To name it yourself, pass
`-isysroot "$(xcrun --sdk macosx --show-sdk-path)"`, or set `SDKROOT`.
`-target arm64-apple-macos14` sets the minimum macOS version (default 11.0).

Link with `clang` (that is, `/usr/bin/clang`) or `xcrun clang`. Calling the
toolchain's Clang by its full path (what `xcrun -f clang` prints) without
`SDKROOT` set fails with `ld: library 'System' not found`, because only the
`/usr/bin` shim tells it where the SDK is.

`-std=gnu11` turns on the GNU extensions that the big programs need (see
[LANGUAGE.md](LANGUAGE.md#gnu-extensions)).

## 4. What works and what does not

On the CI Mac, `harness/macos/sdk_headers.sh` includes 26 C11 headers and
8 POSIX ones (`unistd.h`, `fcntl.h`, `dirent.h`, `sys/types.h`,
`sys/stat.h`, `sys/time.h`, `sys/wait.h`, `pthread.h`) one at a time, in
both `-std=c11` and `-std=gnu11`. All compile except `<tgmath.h>` and
`<uchar.h>`. `<complex.h>` is not
checked: hcc has no complex types (it defines
`__STDC_NO_COMPLEX__`). `<stdatomic.h>` and `<threads.h>` are provided
by hcc. macOS has no C library `<threads.h>`; `hcc --link` adds
`lib/threads.c` and `-pthread` automatically when a program uses it. To
link by hand, see [user guide section 7](USER_GUIDE.md#7-threads).

- **`<tgmath.h>` and `<complex.h>`** fail: hcc has no complex
  types (it defines `__STDC_NO_COMPLEX__`), and Apple's `<tgmath.h>`
  includes `<complex.h>`.
- **`<uchar.h>`** is not in the macOS SDK, so `char16_t`/`char32_t`
  programs that include it do not build with Apple's Clang either.
- **`_Float16`** appears in the SDK's `<math.h>` (half-precision
  functions). hcc accepts it in declarations only; using a
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

`run_tests.py` compiles each test program with hcc and with Apple's Clang
and compares the two programs' output. Every test passes on the CI Mac
except the few listed in `harness/macos_known_failures.txt`, which are
Linux-specific (a 10-byte `long double`, `<uchar.h>`, glibc's
`__sysv_signal`, an ELF section name). `cargo test` skips the unit tests
that build and run x86-64 Linux code.

## 6. DOOM

**To play DOOM in a window, follow [DOOM_ON_MAC.md](https://github.com/spacefilm900/c-compiler/blob/main/docs/DOOM_ON_MAC.md)**: one
command, `./tools/mac/play_doom.sh`, checks the tools, builds DOOM with hcc
and SDL2, and starts it with the free Freedoom game data (or your
own WAD).

The headless frame-hash check also runs natively on the Mac:

```sh
sh scripts/fetch_doomgeneric.sh && sh scripts/fetch_freedoom.sh
python3 harness/run_doom.py --opt O0 --opt O2
```

It builds DOOM with hcc and with Apple's Clang, plays every
Freedoom demo to its end with both, and compares a hash of every frame.

## What has been checked without a Mac

- For every function in the test programs, the arguments and results are
  passed the way Clang passes them for `arm64-apple-macos`.
- LLVM's `llc` builds hcc's output for every test program into Mach-O
  objects.
- The test programs run with Apple's calling convention under the
  `qemu-aarch64` emulator, linked against Linux's C library, and match
  Clang's results, apart from a few Linux-specific test details.

## What the CI Mac checks

The `macos` workflow (`.github/workflows/macos.yml`) runs after every merge
to `main` and by hand (`gh workflow run macos.yml`); it does not run on pull
requests because macOS minutes cost about ten times Linux minutes. It builds
hcc, runs the unit tests, builds and runs hello world, runs
`run_tests.py` against Apple's Clang with the SDK's headers, C library, and
Mach-O linker, and reports which SDK headers compile. A run takes under a
minute. Started by hand with the `doom` input
(`gh workflow run macos.yml -f doom=true`), it also runs the DOOM frame-hash
check against Apple's Clang and builds and launches the SDL2 game from
[DOOM_ON_MAC.md](https://github.com/spacefilm900/c-compiler/blob/main/docs/DOOM_ON_MAC.md) with no screen. Lua and SQLite are not
run on Apple Silicon yet. The Intel Mac job in the same workflow, started
by hand (`gh workflow run macos.yml -f intel=all -f arm64=false`), runs
the harness, Lua, SQLite, DOOM and the GCC torture tests on GitHub's
Intel runner.

This guide is validated by CI on GitHub's macOS runners. It has not yet
been walked through by hand on a person's own Mac; if a step fails, please
report the exact command and message.
