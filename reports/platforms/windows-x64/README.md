# 64-bit Windows (`x86_64-w64-windows-gnu`, `x86_64-pc-windows-msvc`), PLAN.md step 11

This step was done the same low-cost way as step 10. Most of the checking
runs on Linux: the cross targets, llc COFF checks, a black-box comparison
with Clang, and real programs run under Wine with MinGW-w64. A job on
GitHub's `windows-latest` runner (`.github/workflows/windows.yml`) runs
**only when started by hand**. It covers what Linux cannot: the real MSVC
and Windows SDK headers, the MSVC linker and CRT, and native MinGW-w64.

The work is clean-room. It uses only C11 (N1570), the LLVM LangRef,
Microsoft's public x64 calling-convention, x64 software-convention and C
language documentation, and the public GCC manual (`-mms-bitfields`,
"Alternate Keywords", `-D 'name(args)=def'`). Clang 19 was a black-box
oracle only. Its `-E -dM` macro dumps, IR signatures and type sizes were
compared, never its source.

## Type model and ABI

`-target x86_64-w64-windows-gnu` (also spelled `x86_64-w64-mingw32` or
`x86_64-pc-windows-gnu`) and `-target x86_64-pc-windows-msvc` (also
`x86_64-windows-msvc`, `x86_64-pc-windows` or
`x86_64-unknown-windows-msvc19.33`). On a Windows host the default target
follows the host toolchain.

- **Data layout:** COFF (`e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128`), the same as Clang's.
- **LLP64:**
  - `long` is 32 bits.
  - `size_t`, `ptrdiff_t`, `intptr_t` and `intmax_t` are `long long`.
  - `wchar_t` and `wint_t` are 16-bit `unsigned short` (UTF-16).
  - `long double` is the 8-byte `double` on MSVC and the 16-byte x87 type on MinGW-w64.
  - Arrays of 16 bytes or more are aligned to 16.
  - The compiler's `<limits.h>` and `<float.h>` follow this, and they pull in the C library's own headers first.
- **Microsoft x64 convention (`src/win64.rs`):**
  - Each argument takes one 8-byte slot.
  - Records of 1, 2, 4 or 8 bytes are passed and returned as one integer, even when they hold floats. Every other size goes by reference to a caller copy and comes back through `sret`. So does a record with a flexible array member, or a union holding one, at any size (Clang, black box).
  - Only `_Bool` is extended.
  - `__int128` goes by reference and comes back as `<2 x i64>`.
  - MinGW's x87 `long double` goes by reference and comes back via `sret`.
  - `va_list` is `char *` with 8-byte slots. A by-reference type's slot holds a pointer.
- **Bit-fields:** Microsoft layout on both targets (GCC manual, `-mms-bitfields`):
  - A bit-field takes its declared type's alignment.
  - Adjacent bit-fields share a unit only when their types have the same size.
  - A zero-width bit-field after a bit-field closes the unit and aligns the next member.
- **Predefined macros (from Clang's dumps):**
  - Both targets: `_WIN32`, `_WIN64`, an empty `__USER_LABEL_PREFIX__`, and the LLP64 `__SIZEOF_*__`/`__*_TYPE__` values.
  - MinGW-w64: `__MINGW32__`, `__MINGW64__`, `__MSVCRT__`, `__SEH__`, `__declspec(x)` and the calling-convention keywords as attributes, and `__GNUC__`. It also gets `__NO_INLINE__` (GCC and Clang define it at `-O0`), so the CRT headers declare their functions instead of defining x87-asm inline versions.
  - MSVC: `_MSC_VER` 1933, `_M_X64`, `_M_AMD64`, and no `__GNUC__`. Microsoft's C keywords (`__int64`, `__cdecl`, `__forceinline`, `__declspec(...)`, `__pragma(...)` and others) are macros over this compiler's spellings, and `__declspec(align(N))` means `aligned(N)`.
  - MSVC also puts every non-static definition written `inline` in a COMDAT, as Clang does for that target: `linkonce_odr`, or `weak_odr` for `extern inline`, even after a non-inline declaration (the UCRT's `<stdio.h>` defines `printf` and its relatives this way). `__declspec(selectany)` objects are `weak_odr` COMDATs too (`<fenv.h>` uses it).
  - Two intrinsics are added: `__va_start` (used by `<vadefs.h>`), and `__debugbreak`/`__fastfail` on both targets.
- **Also added for every target:** the `_Pragma` operator (C11 6.10.9; the pragma is ignored like an unknown `#pragma`), and `__typeof__` in strict C11 (plain `typeof` stays GNU-only). `typedef __attribute__((aligned(N))) struct {...} T;` now works when the record is defined there and its size is a multiple of N. Function-like `-D` is accepted.

## What Linux checks (normal CI job `windows-standin`)

| Check | What it shows | Result |
| --- | --- | --- |
| `run_arm64_ir.py` signatures (both targets) | Every function signature in `tests/` matches Clang (`sret`, compared by size and alignment) | MinGW 473, MSVC 449, 0 differ |
| `run_arm64_ir.py` llc | `llc -filetype=obj` writes a COFF object from our IR | MinGW 160, MSVC 154 programs, 0 failed (3 rejected by Clang too; 2 LP64-only tests) |
| `run_cross.py --target x86_64-w64-windows-gnu` | Our objects and Clang's, linked by `x86_64-w64-mingw32-gcc` and run under Wine | 193 tests: 182 pass, 11 known |
| `run_cross.py --target x86_64-pc-windows-msvc` (stand-in) | MSVC type model and ABI on MinGW-w64's headers, under Wine | 193 tests: 180 pass, 13 known |
| `abi_fuzz.py --target …` | Random two-unit calling-convention programs, every mix of us and Clang, under Wine | 100/100 seeds each target, plain and `--int128` |
| GCC torture under Wine (MinGW, report mode, run locally) | `gcc.c-torture/execute` built by us alone | 1,746 files: 1,335 pass; 4 failures beyond the x86_64 list (2 need POSIX `<sys/mman.h>`, 2 fail with Clang too) |

The 11 known failures on both targets are tests written for LP64:
- `1L << 40` and similar shifts of a 32-bit `long`;
- an address cast to `long`;
- `_Generic` arms naming `long`;
- glibc's `__sysv_signal`;
- `L'\U0001F600'` in a 16-bit `wchar_t`.

Clang rejects or miscompiles several of these too. The stand-in adds two more:
- `112_longdouble.c` dumps 10 bytes of an 8-byte `long double`;
- MinGW's `nanl` expects x87.

## What the Windows runner checks (manual, `windows.yml`)

The job builds `compiler.exe` with the MSVC Rust toolchain and runs the unit
tests, then, for MSVC: sets up the Visual Studio developer environment
(`vswhere`, `vcvars64`), compiles and runs hello world, and runs the
differential harness and 50 random ABI pairs (`--int128`) against Clang for
`x86_64-pc-windows-msvc`, with the real MSVC and Windows SDK headers, the
UCRT and `link.exe`. For MinGW it installs MSYS2's `mingw-w64-x86_64-gcc`
and runs the same against GCC's headers, CRT and linker. A commit message
with `[msvc-only]` runs only the MSVC half.

| Target | Run | Differential harness | Random ABI pairs |
| --- | --- | --- | --- |
| MinGW-w64 (MSYS2) | 44528247584 | 193 tests: 181 pass, 12 known, 0 unexpected | 50/50 |
| MSVC + UCRT | 44530767776 | 193 tests: 178 pass, 15 known, 0 unexpected | 50/50 |

The native-only known failures are:
- MinGW: `gnu/15_cond_short_floating_constant.c` (a hand-declared `printf` given `%Lg` with an x87 `long double`; msvcrt reads a `double`);
- MSVC: `131_offsetof_ice.c` and `134_offsetof_runtime_index.c` typedef `size_t` as `unsigned long`, which Clang for MSVC rejects because it predeclares `size_t`;
- MSVC: `145_uint128_more.c` converts a `double` to `__int128`, a call to the compiler-rt helper `__fixdfti` that the Microsoft libraries do not have (Clang's object needs it too).

The harness links MSVC programs with `legacy_stdio_definitions.lib`, which
Microsoft documents for code that calls the `printf` family without
`<stdio.h>` (`__builtin_sprintf` does).

Five runs were used, about 28 runner minutes, so about 56 billed minutes at
the private-repository rate (budget: 120).

## What remains

- Not run on the Windows runner: GCC torture, Lua, SQLite, DOOM, WACC, Csmith and the AWK suites. Torture was run under Wine with MinGW (above). Each could be added to `windows.yml` later as a manual option.
- The harness does not run `cl.exe` itself; Clang for MSVC is the oracle and linker driver. `link.exe` and the UCRT are real.
- `#pragma pack` is ignored on every target (an older gap). It matters more on Windows, where the SDK headers use it.
- `returns_twice` is not emitted for `setjmp` on any target. `setjmp`/`longjmp` work at `-O0` under Wine.
- `__declspec(align(N))` written *before* `struct` in a declaration with no declarator is ignored, as GCC ignores an attribute there. The SDK uses the form after `struct`.
- `typedef __attribute__((aligned(16))) struct _S {...} T;` (MinGW-w64's `<setjmp.h>`) also raises `_Alignof(struct _S)`, where GCC and Clang keep the record's own alignment. It is accepted only for an untagged record or a tag reserved to the implementation (`__x`, `_X`); with any other tag it is diagnosed, as before.
- `__ptr32` (a 4-byte pointer) is erased, not implemented: `<basetsd.h>` uses it in inline functions that every `<windows.h>` user sees. A record with a `__ptr32` member gets an 8-byte pointer, so its layout differs from MSVC's. `__vectorcall` is rejected with a diagnostic.
- A by-reference argument's caller copy is 16-byte aligned, as Microsoft's convention requires (Clang uses the record's own alignment; both are compatible).
- The temporary push trigger in `windows.yml` (commit message `[win-native]`) must be removed before merging. GitHub only dispatches workflows that are already on `main`.
