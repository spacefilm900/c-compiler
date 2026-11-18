# macOS on Intel (`x86_64-apple-macos`), PLAN.md step 10

On 2026-11-18 the owner approved doing this step the low-cost way. Most of
the checking runs on Linux: the cross target, llc Mach-O checks, a black-box
comparison with Clang, and a Linux stand-in that runs the code. A short job
on GitHub's Intel Mac runner (`macos-26-intel`) runs **only when started by
hand**. It never runs on a pull request, a push or a schedule. The owner's
own Mac is Apple Silicon, so he cannot hand-test on Intel hardware. The
runner is the only real Intel Mac this step uses.

The work is clean-room. It uses only C11 (N1570), the LLVM LangRef, the
System V AMD64 psABI, Apple's published x86-64 and Mach-O conventions, the
public GCC manual, and the macOS SDK's public headers (the code being
compiled). Clang and GCC were black-box oracles only.

## Type model

`-target x86_64-apple-macos[VERSION]`, also spelled `x86_64-apple-macosx…`
and `x86_64-apple-darwin…`.

- **Version:** the default is macOS 10.15. On an Intel Mac,
  `Target::host()` now picks this target. Before this change it fell
  through to x86-64 Linux.
- **Triple and data layout:** IR triple `x86_64-apple-macosx10.15.0`. The
  data layout is
  `e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128`,
  the same as Clang's.
- **ABI:** System V AMD64, as on Linux, with the Darwin differences
  listed below. Shared with Linux: `long double` (x87, 16 bytes), signed
  `char`, `int` `wchar_t`, `va_list` and the classification rules, apart
  from the two argument rules below.
- **Darwin differences (each checked against Clang):**
  - `wint_t` is `int`, and symbol names carry the `_` prefix (written as
    `\01` asm labels for asm-label declarations).
  - A Mach-O section attribute must be `segment,section`.
  - Every function has `"frame-pointer"="all"`.
  - An unnamed bit-field does not align the record.
  - A struct with a flexible array member, or a union holding one, is
    passed in memory, where Linux uses registers.
  - An eightbyte that is the upper half of a `long double` alone (X87UP
    not after X87, as in `union { long double d; void *p; }`) is an SSE
    `double`. The psABI, and Linux, pass that union in memory.
- **Predefined macros:** `__APPLE__`, `__MACH__`, `__APPLE_CC__`,
  `__DYNAMIC__`, `__USER_LABEL_PREFIX__` `_`, `__LITTLE_ENDIAN__`,
  `__ENVIRONMENT_MAC_OS_X_VERSION_MIN_REQUIRED__` (1090 for 10.9, 101500
  for 10.15) and `__llvm__`. There is no `__ELF__` and no `__linux__`.
  `__arm64__` is now defined only on arm64.

## What Linux checks, and what it cannot

| Check | What it shows | Result at the PR head |
| --- | --- | --- |
| `run_arm64_ir.py` signature check (`x86_64-apple-macos`) | Every function signature in `tests/` (parameter and return types and attributes, `byval`/`sret`, sign extension) matches Clang `-target x86_64-apple-macos10.15` | 461 signatures, 0 differ (`byval` compared by size and alignment) |
| `run_arm64_ir.py` llc check | `llc -mtriple=x86_64-apple-macosx -filetype=obj` (a Mach-O object) accepts our IR | 164 programs, 0 failed (1 rejected by Clang too) |
| `run_cross.py --target x86_64-apple-macos` (stand-in) | Our code and Clang's for the Darwin target, linked together and run on Linux | 193 tests: 192 pass, 1 known (`gnu/both/01_attributes.c`, an ELF section name) |
| Stand-in GCC torture | The same for `gcc.c-torture/execute` | 1,746 files: 1,334 pass, 412 fail, all of them on the x86-64 known list (with the macOS overlay) |
| `tools/abi_fuzz.py --target x86_64-apple-macos` | Random two-unit calling-convention programs, each unit built by us or by Clang, every mix | 1,000/1,000 seeds; with `--int128`, 1,000/1,000 (seeds 5001–6000) |

**How the stand-in works** (`harness/intel_mac_proxy.py`):

1. Both compilers build for the Darwin target against a merged x86-64
   glibc sysroot. A small overlay sets `__HAVE_FLOAT128 0`, because glibc's
   `__float128` does not parse for Darwin.
2. The IR's triple becomes `x86_64-apple-darwin-elf` and `m:o` becomes
   `m:e`. Then comes `opt -passes=default<O0>`, which inlines glibc's
   `always_inline` wrappers, and `llc -relocation-model=pic` to an ELF
   object.
3. The objects are linked with the host's glibc and run on Linux.
4. The label prefix is turned off (`-D__USER_LABEL_PREFIX__=`) so that
   glibc's asm redirects still match. For Clang the stack protector is
   off, and our compiler also gets `-D_GCC_LIMITS_H_`.

**What Linux cannot show:**

- The macOS SDK headers and Apple's libc.
- The Mach-O linker and dyld.
- `$INODE64`-style symbol variants.
- Apple clang's own behaviour (it is newer than the Linux Clang 19).

The Intel Mac job covers exactly these. It found four header problems the
stand-in could not see (below).

## Intel Mac results (manual runs)

All four runs used `gh workflow run macos.yml --ref macos-x86_64 -f
arm64=false -f intel=all` on `macos-26-intel` (Xcode 26.6, SDK 26.5, Apple
clang 21.0.0 as the oracle). Times are UTC, which is the same as London
time (GMT) on this date.

| Run | Head | Time (UTC) | Billed minutes | Outcome |
| --- | --- | --- | --- | --- |
| [44306712099](https://github.com/spacefilm900/c-compiler/actions/runs/44306712099) | `426f970` | 12:44–12:59 | 15 | Harness, SQLite, DOOM and torture failed on `<libkern/i386/_OSByteOrder.h>` (extended asm without `__llvm__`) and the `CHAR_MAX` redefinition |
| [44311262482](https://github.com/spacefilm900/c-compiler/actions/runs/44311262482) | `f4d1e76` | 13:12–13:22 | 10 | Harness, Lua, DOOM passed. SQLite failed on an enumerator outside `int` in `<mach_debug/ipc_info.h>`. Torture had 1 unexpected *pass* (`20050215-1.c`) |
| [44314319000](https://github.com/spacefilm900/c-compiler/actions/runs/44314319000) | `1ec15e4` | 13:30–13:41 | 11 | SQLite failed on `enum : uint64_t` in `<malloc/malloc.h>`. Apple clang 21 types enum constants as GCC does |
| [44317168313](https://github.com/spacefilm900/c-compiler/actions/runs/44317168313) | `a1497fc` | 13:47–13:57 | 11 | **Everything passed** |

A fifth run, [44325320398](https://github.com/spacefilm900/c-compiler/actions/runs/44325320398)
on `044672b` (the review fixes), 14:32–14:44 UTC, was cancelled during DOOM
to stay within the budget. Up to then it passed: unit tests (214 passed,
23 ignored), the harness (190/193, 3 known, 0 unexpected; the new
`xabi/09_x87up_union` passed against Apple clang), Lua and SQLite (both
match at `-O0` and `-O2`), and the header report (100 of 108; `<mach/mach.h>`
now compiles in `-std=gnu11`). DOOM and torture did not run on that head.

**Total: 47 billed minutes for runs 1 to 4, plus about 12 for run 5: about
59 to 60 minutes, about $3.70 at $0.062 a minute**, at the approved ceiling
of about 60 minutes (about $4). No more Mac runs are planned for this step.

**Scorecard at `a1497fc` (run 44317168313):**

| Suite | Intel Mac result |
| --- | --- |
| Unit tests (`cargo test --lib`) | 209 passed, 23 ignored (tests that need an x86-64 Linux Clang) |
| Hello, world (triple check) | `x86_64-apple-macosx10.15.0`, runs |
| Differential harness, Apple clang oracle | 189/192 passed, 3 known, 0 unexpected |
| Lua 5.4.7 | Builds alone at `-O0` and `-O2`. Test suite in three modes, 9/9 workloads, `luac -s` 42/42: all match Apple clang |
| SQLite 3.54.1 | Builds alone. 10/10 workloads and 10/10 database files match at `-O0` and `-O2` |
| DOOM (doomgeneric, Freedoom) | Builds alone (82/82 files). 39,209/39,209 frame hashes match at `-O0` and `-O2`, every demo to its end |
| GCC torture execute | 1,334 passed, 412 known, 0 unexpected |
| SDK header report (report only) | 99 of 108 compiled. Failures: `<tgmath.h>` (complex types), `<uchar.h>` (not in the SDK), and `<mach/mach.h>` (missing `__LITTLE_ENDIAN__`; it compiled after the fix in run 5). In strict C11 only: `<malloc/malloc.h>` and `<mach/mach_time.h>`, which need the GNU enum extensions, and `<sys/mount.h>` |

Run times on the Intel runner, ours against Apple clang:

- Lua workloads: 0.93x at `-O0`, 0.92x at `-O2`.
- DOOM: 0.97x at `-O0`, 0.92x at `-O2`.
- SQLite: 1.02x at `-O0`, 0.99x at `-O2`.

### Known-failure lists for Intel macOS

- **`harness/macos_x86_64_known_failures.txt`** (differential harness):
  - `116_uchar.c`: the SDK has no `<uchar.h>`.
  - `121_signal.c`: it uses glibc's `__sysv_signal`.
  - `gnu/both/01_attributes.c`: an ELF section name.
- **`harness/gcc_torture_known_failures_macos_x86_64.txt`**: drops
  `20050215-1.c`. That test asks for `aligned(8)` only `#ifdef __ELF__`,
  which is where the x86-64 failure comes from.
- **`harness/cross_known_failures.txt`**: adds
  `x86_64-apple-macos gnu/both/01_attributes.c`.

## Bugs fixed

1. **x86-64 (Linux too): a record holding `__int128` was passed in
   memory.** System V AMD64 3.2.3 makes `__int128` two INTEGER eightbytes,
   so `struct { __int128 q; }` goes in two registers. Clang and GCC agree.
   Found by the Intel signature check. Regression tests:
   `tests/xabi/07_int128_record` (the old compiler crashed on the pair) and
   the unit test `int128_record_is_two_integer_eightbytes`.
2. **x86-64 (Linux too): a scalar `__int128` parameter was counted as one
   general register.** A later two-eightbyte record could then be split
   between the last register and the stack. Found by `abi_fuzz --int128`:
   49 of 1,000 seeds, on Linux and the stand-in alike. Regression tests:
   `tests/xabi/08_int128_scalar_regs` and the unit test
   `scalar_int128_takes_two_general_registers`.
3. **Intel macOS: the SDK's `<libkern/i386/_OSByteOrder.h>` used GNU
   extended asm** because `__llvm__` was not defined. That header is
   reached from `<stdlib.h>`. `__llvm__` is now defined on
   `x86_64-apple-macos` only, because the compiler emits LLVM IR and
   provides `__builtin_bswap32/64`.
4. **Apple targets: `CHAR_MAX` was redefined.** The SDK's
   `<machine/limits.h>` spells it `127`. On `__APPLE__`, our `<limits.h>`
   now `#include_next`s the SDK's first and then puts in our values,
   keeping the SDK's `MB_LEN_MAX` of 6, as Apple clang does.
   `#include_next` also needed the compiler's header directory to be
   listed once. It had been listed twice, as `include` and as the absolute
   path. Regression test: `driver::apple_limits_h_reads_the_sdk_limits_first`.
5. **GNU mode: enumerators outside `int`** (`IPC_OTYPE_UNKNOWN = ~0u`).
   The enumerated type becomes `unsigned int`, `unsigned long` or `long`,
   and every constant of that list takes it. GCC and Apple clang 21 both do
   this; Clang 19 keeps the constants that fit in `int` as `int`. Strict
   C11 still rejects it (C11 6.7.2.2p2). Regression test:
   `tests/gnu/16_enum_outside_int.c`.
6. **GNU mode: fixed underlying types** (`enum : uint64_t { ... }`, C23
   6.7.2.2) in `<malloc/malloc.h>`. Regression test:
   `tests/gnu/17_enum_fixed_type.c`.
7. **Apple targets: `__LITTLE_ENDIAN__`**, which `<libkern/OSByteOrder.h>`
   needs. Clang defines it.

Found in the CodeRabbit review of PR 50, each checked against Clang (and
GCC for the enum rules) as a black box:

8. **Intel macOS: a lone X87UP eightbyte is SSE.** `union { long double d;
   void *p; }` is passed in `%rdi` and `%xmm0` and returned in `%rax` and
   `%xmm0` (`{ i64, double }`), not in memory. Regression tests:
   `tests/xabi/09_x87up_union` and the unit test
   `intel_mac_passes_a_lone_x87up_eightbyte_as_sse`.
9. **Intel macOS: a union holding a flexible-array struct goes in
   memory**, as the struct itself does. Unit test
   `intel_mac_passes_unions_holding_flexible_array_records_in_memory`.
10. **GNU enums, follow-ups to bugs 5 and 6:**
    - A colon after `enum E` starts an underlying type only when a type
      follows. `struct { enum E : 3; }` (an unnamed bit-field) and
      `_Generic(x, enum E: 1)` had stopped parsing in `-std=gnu11`.
    - An implicit constant after one outside `int` has, while the list is
      read, the previous constant's type if the value fits, else `long`
      after a signed one and `unsigned long` after an unsigned one (GCC and
      Clang agree). `enum { A = 0x7fffffff, B, C = sizeof(B) }` gives 8.
    - The underlying type is in force inside the list (`sizeof(enum E)`).
      Without one, the enum is incomplete until its `}` (C11 6.7.2.2p4),
      so `sizeof(enum E)` inside its own list is an error.
    - An enumeration spelled with `enum` is rejected as an underlying
      type.
    - Underlying types and values wider than 64 bits are rejected instead
      of being truncated.

    Regression tests: `tests/gnu/16` and `17`, and the parser tests
    `gnu_enum_values_stay_within_64_bits` and
    `gnu_enum_colon_needs_a_type_after_it`.
11. **`-target x86_64-apple-darwinN`** now maps the Darwin version to macOS
    as Clang does (`darwin25` is macOS 26, `darwin20` is macOS 11,
    `darwin19` is 10.15), and junk (`darwinjunk`) or versions above 99 are
    rejected. An arm64 Darwin version below 20 still means the arm64
    default, as before.
12. **Only one compiler header tree is searched** (the first candidate
    that has all of the compiler's headers), so `#include_next` in
    the compiler's `<limits.h>` cannot land in a second copy at another
    path.

Harness fixes:

- Lua and SQLite fetch and build on macOS (`shasum`, `LUA_USE_MACOSX`).
- The SQLite scorecard's `ru_maxrss` is in bytes on macOS.
- The torture runner skips `RLIMIT_AS` on macOS only, which does not
  support it.
- Torture overlays for macOS targets. The stand-in torture gate also fails
  on a compiler panic in a listed test and on a filter that selects
  nothing.
- The stand-in insists on LLVM 18 or later for `opt` and `llc`, and
  `run_arm64_ir.py` fails when its x86-64 headers are missing.
- `abi_fuzz.py` reports a failing Clang rerun as an oracle failure, a
  missing IR file as a build failure, and a sweep that compared nothing
  as inconclusive (exit 2).

## Linux x86-64 output is unchanged except for the fixes

Old (`main` at `7ca7beb`) against new, on every `.c` file under `tests/`,
the WACC, GCC torture, One True AWK and Csmith trees, Lua, SQLite and
DOOM, each compiled in strict C11 and in `-std=gnu11`:

- **13,362 compilations, 18 different** (rerun at the review fixes; the
  same 18). No exit status changed apart from
  the two new GNU tests, which the old compiler rejects.
- **16 of the 18 are the `__int128` ABI fixes** (bugs 1 and 2): eight
  files, each in both modes. They are `145_uint128_more.c`,
  `146_uint128_va_regs.c`, `arm64/xabi/01_calls_def.c`,
  `arm64/xabi/01_calls_use.c`, and the `xabi/07` and `xabi/08` pairs. The `arm64/xabi` files are compiled for
  x86-64 in this comparison.
- **The other 2 are the new GNU enum tests in `-std=gnu11`**, which the old
  compiler rejected.

## Open

- **Clang's caller and callee disagree for a variadic `__int128` that goes
  on the stack while one general register is left.** LLVM leaves the
  register unused, while `va_arg` for a later argument reads it. GCC uses
  the register on both sides, which is what psABI 3.2.3 says. This
  compiler emits what Clang does, so it shares the disagreement with GCC.
  It is not changed here because Clang is the reference.
  `abi_fuzz --int128` avoids the shape: in a variadic function a scalar
  `__int128` appears only as the last argument.
- **A named scalar `__int128` spilled to the stack with one register left
  depends on the LLVM version.** This compiler writes a plain `i128`
  parameter, as Clang does, so the backend decides. LLVM 19 (checked here)
  leaves `%r9` unused and reads the next argument from the stack; GCC puts
  it in `%r9`. The review of PR 50 reports that LLVM 20 uses `%r9` as GCC
  does (not checked here). Code from this compiler and from a Clang of the
  same LLVM version agree (`tests/xabi/08`, on Linux and on the Intel
  Mac). Lowering the parameter as a `byval` copy would match GCC but
  disagree with Clang 19, the Linux oracle.
- **Strict C11 rejects the SDK's GNU enum forms.** As a result,
  `<malloc/malloc.h>` and `<mach/mach_time.h>` need `-std=gnu11`. Apple
  clang accepts them with `-std=c11`.
- **DOOM and GCC torture have not run natively on the review-fix head.**
  Run 5 was cancelled before them. The fixes since run 4 that touch
  code generation for Intel macOS are the lone-X87UP union rule and the
  union-with-flexible-array rule. Neither shape appears in DOOM. Torture
  programs are built by this compiler alone, so both sides of every call
  use the same rule. The stand-in torture run on Linux at this head
  matches the list.
