# Using the compiler on a Mac with Apple Silicon

> **Draft — untested on a real Mac yet.** Everything below was written and
> checked on Linux: the compiler's arm64 output has been compared with
> Clang's, built into Mach-O objects with `llc`, and run with Apple's calling
> convention under an emulator. No one has followed these steps on a Mac.
> If a step fails, please note the exact command and message.

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

3. **Optional: Homebrew LLVM** — a newer Clang and `llc`. The IR this
   compiler writes is checked against LLVM 18 and 19. Apple's Clang from the
   Command Line Tools is usually a little older; if it rejects a `.ll` file,
   install LLVM with Homebrew (<https://brew.sh>):

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

`build/compiler` finds the SDK with `xcrun`. To name it yourself, pass
`-isysroot "$(xcrun --sdk macosx --show-sdk-path)"`, or set `SDKROOT`.
`-target arm64-apple-macos14` sets the minimum macOS version (default 11.0).

`-std=gnu11` turns on the GNU extensions that the big programs need (see
the README).

## 4. Things that may go wrong (expected, since this is untested)

- **Header errors from the SDK.** Apple's headers use some Clang features
  this compiler's preprocessor does not have yet (for example
  `__has_include`). An error inside a file under `MacOSX.sdk` is most
  likely one of these.
- **`long double`** is the same as `double` on Apple Silicon, so the
  `long double` math functions are the `double` ones under another name.
  This is how macOS works, not a bug.
- **A linker error about a symbol starting with `_`.** Mach-O adds `_` to
  C names. Report the symbol name.

## 5. DOOM (later)

DOOM, Lua, and SQLite already build with this compiler on Linux and match
Clang's programs exactly. On the Mac the plan is the same harness
(`make doom`), with the headless DOOM backend comparing frame hashes against
a Clang build. That has not been tried yet; this section will get exact
steps after the first run on a Mac.

## What has been checked without a Mac

- For every function in the test programs, the arguments and results are
  passed the way Clang passes them for `arm64-apple-macos` (432 functions).
- LLVM's `llc` builds the compiler's output into Mach-O objects (159
  programs).
- The test programs run with Apple's calling convention under the
  `qemu-aarch64` emulator, linked against Linux's C library, and match
  Clang's results (179 of 181; the 2 known differences are Linux-specific
  test details).

What still needs a Mac: Apple's own headers and C library, the Mach-O
linker, and running DOOM. A macOS CI job would cover these on every change;
it has not been added yet.
