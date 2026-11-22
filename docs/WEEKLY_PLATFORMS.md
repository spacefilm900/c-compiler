# Weekly real-platform run

`.github/workflows/weekly-platforms.yml` runs the tests that the pull-request
CI covers only through stand-ins (qemu, Wine, IR retargeting) on the real
platforms, and runs hcc and the programs it builds under sanitizers. It is
separate from the pull-request CI: it never runs on a pull request or a
push, and it does not change `ci.yml`, `linux-arm64.yml`, `macos.yml` or
`windows.yml`.

## When it runs

- **Every Sunday at 03:00 UTC** (`schedule`), on `main`.
- **By hand**, on any branch:

  ```sh
  gh workflow run weekly-platforms.yml                        # every job, on main
  gh workflow run weekly-platforms.yml -f jobs=macos          # one job: arm64, macos, windows or sanitizers
  gh workflow run weekly-platforms.yml --ref BRANCH -f jobs=all
  ```

  or, on GitHub, Actions > weekly-platforms > Run workflow. GitHub offers
  "Run workflow" (and accepts `gh workflow run`) only once the workflow
  file is on the default branch; `--ref` then picks the branch to test.

Only one run per branch goes at a time. A run started while another is
going waits for it rather than cancelling it.

## The four jobs

| Job | Runner | Time cap | What it runs |
| --- | --- | ---: | --- |
| arm64 Linux native | `ubuntu-24.04-arm` | 60 min | Unit tests, the differential harness, stage dumps and 200 random calling-convention pairs. The v0.9 kit with its own runner at `-O0` and `-O2` (mixed objects with Clang and with GCC). Every case of v0.7, v0.8 and v0.9 on glibc's `<threads.h>` through `tools/platform/challenge_native.py`, against Clang at both levels with mixed hcc/Clang objects and the stress cases 100 times (the v0.7 and v0.8 runners accept only x86-64 Linux, so their GCC mixed-object checks do not run here). v0.9 thread stress: the 33 marked cases 1,000 times at each level, and the five weak-memory cases (publication, paired fences, release sequence, SC store buffering, SPSC ring) 10,000 times at `-O2`, ten times the kit protocol's minimum. The v0.9 generated campaign (100 seeds), the cross-target matrix with native execution, and the IR inventory. `lib/threads.c` forced on glibc for every kit case, with the stress cases 100 times. GCC and Clang on the v0.9 kit, for adjudication only. |
| macOS arm64 native | `macos-15` (Apple Silicon) | 25 min | The differential harness against Apple clang (`tests/hosted/08_threads.c` with `lib/threads.c`, `tests/posix/darwin` with native Darwin TLS, and the `tests/xabi` pairs both ways), 100 random calling-convention pairs, every case of the three kits against Apple clang with `lib/threads.c` (stress cases 20 times), and ThreadSanitizer on the v0.9 kit at `-O0`. |
| Windows x64 native | `windows-latest` | 35 min | For MSVC (real MSVC headers, linker and UCRT) and then MinGW-w64 (MSYS2): the differential harness (including `08_threads.c` with `lib/threads.c` over the Win32 API, and the `tests/xabi` pairs), 50 random calling-convention pairs with `__int128`, and every case of the three kits with `lib/threads.c` (stress cases 10 times). |
| Sanitizers | `ubuntu-24.04` | 45 min | hcc built with overflow checks and debug assertions, run over the harness, the three kits at both levels and GCC torture, and a crash fuzzer over mutated inputs (any panic fails). Programs built by hcc under AddressSanitizer (with LeakSanitizer) and ThreadSanitizer, with glibc's `<threads.h>` and again with `lib/threads.c`, at `-O0` and `-O2`. UBSan and ASan on the Clang-built reference programs. |

Each job runs every check even when an earlier one fails, and fails at the
end if any check failed. Each check is one row of a table in the run's
summary page (name, PASS, FAIL or INFO, seconds, last line of output),
followed by the per-kit tables. The same files, with each check's full log
and the JSON results, are uploaded as one artifact per job
(`weekly-arm64-linux`, `weekly-macos-arm64`, `weekly-windows-x64`,
`weekly-sanitizers`). INFO rows are another compiler's own results and
never fail the job.

## How the challenge kits run off Linux

The kits in `tests/adversarial/` (see the README there) ship runners for
native Linux only (v0.7 and v0.8: x86-64 Linux only; v0.9 also accepts
AArch64 Linux). `tools/platform/challenge_native.py` reads the same
`manifest.json` and `review-tests/` and builds every case twice on the host:
with hcc (IR, then the host Clang for the object) and with the host Clang
alone, the black-box oracle. Both are run and must agree on exit status,
standard output and standard error. A two-file case is also linked
hcc-first and Clang-first. If the Clang reference cannot be built or run,
hcc must match the kit's expected result and the case is counted as
"reference blocked". The kits' expected results were written on x86-64
Linux, so a reference that differs from them (a layout case on LLP64
Windows, say) is counted as "reference differs from kit", not as a
failure; hcc must still match the reference.

On macOS and Windows the C library has no `<threads.h>`, so both sides use
this project's `include/threads.h` and `lib/threads.c`: hcc builds the
runtime for the candidate and Clang builds it for the reference. That is a
"shared fallback runtime" comparison: it checks hcc's code and hcc's build
of the runtime against Clang's build of the same runtime, not against an
independent C11 threads library. On Linux `--force-threads-shim` does the
same in place of glibc's `<threads.h>`.

`tools/platform/known_failures_macos.txt`,
`tools/platform/known_failures_msvc.txt` and
`tools/platform/known_failures_mingw.txt` list cases that are expected to
fail on that platform, each with a reason; a listed case that passes is
reported so its line can be removed. After the first validation run they
hold:

- macOS, HCC09-054 at -O0: Darwin does not honour `_Alignas(32)` on a
  `_Thread_local` object. Apple Clang's own build fails the kit's
  alignment check as well, so this is a platform limit, not an hcc bug.
- MinGW-w64, HCC09-011, -012 and -015: MSYS2's `mingw64` environment uses
  `msvcrt.dll`, which has no `timespec_get`, so its `<time.h>` declares
  neither `timespec_get` nor `TIME_UTC`. Clang and hcc both reject the
  test. The MSVC (UCRT) job runs all three.

`harness/cross_known_failures.txt` gained one `oracle` entry for the MSVC
harness: on a real Windows runner Clang's own `<tgmath.h>` does not compile
against the UCRT `<complex.h>`, so `hosted/05_tgmath.c` has no oracle there.

## Sanitizers: what is and is not covered

- **hcc itself.** hcc has no `unsafe` code and no dependencies, so the
  memory errors AddressSanitizer or Miri look for cannot happen in safe
  Rust; Rust's own checks are the useful ones. The job builds hcc with
  integer overflow checks and debug assertions and feeds it every suite
  input plus a few tens of thousands of mutated files (a different seed
  each week). Miri is not run: it is very slow on a compiler this size and
  has no unsafe code to check.
- **AddressSanitizer and ThreadSanitizer on hcc's code.** Clang
  instruments a function only when it has the `sanitize_address` or
  `sanitize_thread` attribute (LLVM Language Reference, "Function
  Attributes"), which hcc does not emit. `challenge_native.py
  --sanitize` adds the attribute to every function definition in hcc's
  IR before Clang compiles it, and checks it on known-positive controls
  first: a stack overflow and a data race compiled by hcc must be
  reported, and hcc's objects must call the sanitizer runtime. The run
  stops if a control is not reported. A report from hcc's program fails
  the case unless the Clang reference reports it too (for example
  ThreadSanitizer on a stand-alone fence, which it does not model); those
  are listed as "the sanitizer also reports the reference".
- **UndefinedBehaviorSanitizer on hcc's code is not possible.** Its checks
  are inserted by the C frontend while it still knows the C types and
  rules; LLVM IR does not carry that information, so they cannot be added
  to hcc's IR. The job instead builds the Clang reference with UBSan and
  ASan to check that the test programs themselves are free of undefined
  behaviour.
- **MemorySanitizer is not run.** It needs every piece of code in the
  program, the C library included, to be instrumented, and the runners'
  glibc is not.
- ThreadSanitizer needs `vm.mmap_rnd_bits=28` on the Ubuntu 24.04 runner
  kernel; the job sets it.

### Found by the first sanitizer runs

AddressSanitizer at `-O0` moves the first store of each function parameter
into its stack slot to the top of the function. hcc stored a whole-slot
`zeroinitializer` into the slot of a struct parameter passed in registers
before storing the pieces, so under ASan the zero store ran last and erased
the parameter: `thrd_join` in `lib/threads.c`, built by hcc, received
thread id 0, and every thread case failed with "Joining already joined
thread". Programs without a sanitizer were not affected. hcc now stores the
pieces first and zeroes only the bytes no piece covers
(`store_param` in `src/irgen/call.rs`, test
`struct_parameter_pieces_are_not_followed_by_a_whole_slot_zero_store`).
`tools/platform/controls/param_spill_*.c` is a regression control the
ASan and TSan runs check first.

## Cost

GitHub bills private-repository Actions per minute, rounding each job up to
a whole minute. Published per-minute prices (November 2026): Linux x64
2-core $0.006, Linux arm64 2-core $0.005, Windows $0.010, macOS $0.062.
These are estimates from those prices and the measured run times below;
the real bill can differ a little.

| Job | Estimated minutes | Estimated cost |
| --- | ---: | ---: |
| arm64 Linux | 30 | $0.15 |
| macOS arm64 | 15 | $0.93 |
| Windows x64 | 30 | $0.30 |
| Sanitizers | 25 | $0.15 |
| **One full run** | **100** | **about $1.55** |
| **Weekly, per month (4.3 runs)** | | **about $6.70** |

The monthly CI budget is $30. A manual run of one job costs that job's
row. The macOS job is the dearest per minute, which is why its cap is 25
minutes.

## Not covered

- Intel macOS natively (manual Intel job in `macos.yml`), Windows on ARM,
  and 32-bit targets.
- An independent C11 threads library on macOS or Windows (see "shared
  fallback runtime" above).
- ThreadSanitizer on Windows (not available for these targets), and
  ThreadSanitizer on macOS beyond the v0.9 kit at `-O0`.
- The v0.1-v0.6 challenge kits (724 cases), which are not in the tree.
- Stress runs and the crash fuzzer are bounded samples, not proofs.
