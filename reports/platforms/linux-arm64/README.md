# Linux on AArch64 (PLAN.md step 9)

Target `aarch64-linux-gnu`: AAPCS64 and the SysV/ELF AArch64 ABI. It is the
compiler's default target on an AArch64 Linux host. The oracle is Clang
for the same target (`clang --target=aarch64-linux-gnu`, or the native
Clang on an AArch64 host), used as a black box only.

## Type model

| | AArch64 Linux | x86-64 Linux |
| --- | --- | --- |
| `long double` | IEEE binary128 (`fp128`), 16 bytes, align 16 | x87 80-bit, 16 bytes |
| plain `char`, `wchar_t` | unsigned | signed |
| `va_list` | AAPCS64 32-byte struct (`__stack`, `__gr_top`, `__vr_top`, offsets) | SysV 24-byte array |
| arrays of 16+ bytes | natural alignment | aligned to 16 (SysV) |
| aggregates | HFA/HVA in vector registers, other composites over 16 bytes by reference | SysV eightbyte classes |

## Suites and results

Results were taken twice. Natively, on GitHub's `ubuntu-24.04-arm` runner
with its Clang 18 (pull request 49, run 44283979580, 2026-11-18); every
row below matched there. Locally, runs used `qemu-aarch64` (user mode,
glibc from Debian's `libc6-dev-arm64-cross`) with wrappers that add
`-target aarch64-linux-gnu` to this compiler and
`--target=aarch64-linux-gnu` to Clang 19, so each harness runs exactly as it
would on an AArch64 host. CI runs the same harnesses natively (below).

| Suite | Before step 9 | AArch64 result |
| --- | --- | --- |
| Unit tests (`cargo test --lib`) | x86-64 host only | 200 passed, 23 ignored (tests that need an x86-64 host Clang) on the arm64 runner |
| Differential harness (`run_tests.py`, now with `tests/arm64/`) | under qemu via `run_cross.py` | 190/190 |
| `run_cross.py --target aarch64-linux-gnu` | 189/189 | 190/190 (new cross-ABI pair) |
| Stage dumps, reducer tests | x86-64 only | 24/24 dump checks (they pin `-target x86_64-linux-gnu`), reducer tests OK |
| Random calling-convention pairs (`tools/abi_fuzz.py`, new) | none | 1,000/1,000 seeds locally, 100/100 per CI run (x86-64: 1,800/1,800 after the fix below) |
| Writing a C Compiler | not run | 1,376 passed, 0 failures; 241 skipped (the 233 skipped on x86-64, where 1,384 pass, plus 8 tests that link the book's x86-64 assembly helper) |
| Csmith, 40 fixed seeds | not run | 34 pass, 6 known (the same 6 inconclusive seeds as x86-64), 0 unexpected |
| GCC torture execute | report only, under qemu | 1,335 passed, 411 known, 0 unexpected (x86-64: 1,333 / 413) |
| One True AWK (`testdir` + `bugs-fixed`) | not run | agree with the Clang build; 31/31 bugs-fixed; the 9 golden-file mismatches are shared with the Clang build |
| AWK Builder suites | not run | POSIX challenge v0.6: all 528 cases agree between the two builds (516/528 pass natively, 515 under qemu, the same for both builds); Shakespeare volume: PASS for both |
| Lua 5.4.7 | not run | builds alone at `-O0` and `-O2`; test suite in three modes, 9/9 workloads, `luac -s` 42/42: all match Clang |
| SQLite 3.54.1 | not run | builds alone; 10/10 workloads and database files match at `-O0` and `-O2` |
| DOOM (doomgeneric, Freedoom) | not run | 39,209/39,209 frame hashes match at `-O0` and `-O2` (every demo to its end) |
| Performance numbers | x86-64 only | `harness/perf.py` runs; numbers under qemu are not meaningful, the native CI job uploads `perf-numbers-linux-arm64` |

### Known-failure lists for AArch64

- `harness/gcc_torture_known_failures_aarch64.txt` overlays the x86-64 list
  (`-name` removes an entry, `+name` adds one). Each entry has a reason:
  - `-20031003-1.c`: `(int)2147483648.0f` is undefined (C11 6.3.1.4p1);
    the test expects saturation, which AArch64's `fcvtzs` gives and
    x86-64's `cvttss2si` does not, so it passes on AArch64.
  - `-pr28982b.c`: the x86-64 entry is LLVM 19's x86 instruction selector
    failing on the IR; the AArch64 backend accepts it and it passes.
- `harness/cross_known_failures.txt`: the three AArch64 Linux entries
  (`112_longdouble.c`, `113_wide.c`, `122_utf8_ucn.c`) are gone, fixed as
  below. Only the two Apple-proxy entries remain.
- Writing a C Compiler: on a non-x86-64 target Clang is the oracle for
  every program (the suite's expected results assume signed `char` and
  x86-64), and the 8 programs that link the book's x86-64 assembly helper
  are skipped with that reason.

## Bugs fixed

1. **`long double` constants were folded as `double` on AArch64 Linux.**
   Constant folding of binary128 is now exact: `src/fp128.rs` does IEEE
   binary128 arithmetic, conversions and comparisons on exact big integers
   (`src/bigint.rs`, shared with the x87 code) with one round to nearest,
   ties to even. `0.1L` is `0xLA0000000000000003FFB999999999999`, as Clang
   writes it. Checked against Clang with a randomised differential of
   decimal and hex literals and folded `+ - * /`, comparisons and
   conversions, 48,000 items, 0 differences. Constants that were
   truncated to 80 bits in the lexer (a `u16` high part) now keep 128 bits.
   Hex-float exponent overflow and the classification builtins use the
   target's infinity and `LDBL_MIN`.
2. **Arrays of 16 bytes or more were aligned to 16 on AArch64.** That is a
   SysV x86-64 rule; AAPCS64 uses natural alignment. Found by WACC's
   `test_alignment.c`.
3. **x86-64: a struct whose bit-field sits in a nested struct starting
   mid-eightbyte was passed in memory** instead of INTEGER eightbytes
   (`struct { char a[2]; struct { int b:4; } s; long m; }`). Found by
   `tools/abi_fuzz.py` seed 71 on x86-64. Regression tests: unit test
   `nested_bit_field_uses_the_outer_eightbyte` and `tests/xabi/06_nested_bitfield`.

`tests/113_wide.c` and `tests/122_utf8_ucn.c` declared `wchar_t` as `int`;
they now use `__WCHAR_TYPE__` (unsigned on AArch64 Linux).

## CI and cost

`.github/workflows/linux-arm64.yml` runs on GitHub's native
`ubuntu-24.04-arm` runner. On it the compiler's default target is
AArch64 and the native Clang is the oracle, so the harnesses run as they do
on x86-64.

Prices used (GitHub Actions per-minute rates for private repositories,
read from GitHub's billing documentation on 2026-11-18): Linux 2-core x64
$0.006, **Linux 2-core arm64 $0.005**, macOS $0.062. The arm64 runner is
cheaper per minute than the x64 runner and far faster than the same suites
under qemu on an x64 runner (DOOM alone took about 8 minutes of qemu time
here). The qemu steps in `ci.yml` stay as they are.

| Job | When | Estimated billed minutes per run |
| --- | --- | --- |
| `arm64 quick`: unit tests, harness with `tests/arm64/`, stage dumps, reducer tests, 100 random ABI pairs | every PR run, push to `main`, manual | 2 to 3 (1 min 57 s measured) |
| `arm64 wacc, csmith, torture, awk` (with the AWK Builder suites) | push to `main`, manual | 7 (6 min 34 s measured) |
| `arm64 lua, sqlite, doom, perf` | push to `main`, manual | 7 (6 min 45 s measured) |

Run counts over the 30 days to 2026-11-18 (`ci.yml`): 140 pull-request
runs and 178 push-to-`main` runs (48 merged pull requests).

- Pull requests: 140 x 3 = 420 minutes, about $2.10.
- Main: 178 x 17 = 3,030 minutes, about $15.15 at the observed rate of
  main runs; at one run per merged pull request (48), 820 minutes, about
  $4.10.
- Total: about 1,240 to 3,450 minutes a month, **about $6 to $17**.

If that is too much of the $30 budget, the cheapest change is to move the
two suite jobs from every push to `main` to a nightly schedule plus manual
dispatch (`on: schedule`), about 30 x 14 = 420 minutes ($2.10) a month;
the quick job keeps gating pull requests.
