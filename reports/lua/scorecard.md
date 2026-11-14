# Lua scorecard

- Date: 2026-11-14
- Compiler commit: `ab7bcaa`
- Lua: 5.4.7 from lua.org (`lua-5.4.7.tar.gz` SHA-256 `9fbf5e28ef86c69858f6d3d34eccc32e911c1a28b4120ff3e84aaa70cfbf1e30`)
- Test suite: `lua-5.4.7-tests.tar.gz` (SHA-256 `8a4898ffe4c7613c8009327a0722db7a41ef861d526c77c5b46114e59ebf811e`)
- Language mode: `-std=gnu11` for this compiler and for the reference; `-DLUA_COMPAT_5_3 -DLUA_USE_LINUX`, linked with `-lm -ldl -Wl,-E`, no readline
- Reference: Debian clang version 19.1.7 (3+b1), same flags and optimisation level
- Compile jobs: 8; test-suite runs at once: 1

| | O0 | O2 |
|---| --- | --- |
| Builds with this compiler alone (both configurations) | yes | yes |
| Files compiled, release (`lua`, `luac`) | 34 / 34 | 34 / 34 |
| Files compiled, ltests (internal checks on) | 34 / 34 | 34 / 34 |
| Build compared | full | full |
| Test suite, user mode (`_U=true`) | OK; Clang OK | OK; Clang OK |
| Test suite, portable mode (`_port=true`) | OK; Clang OK | OK; Clang OK |
| Test suite, internal (ltests build) | OK; Clang OK | OK; Clang OK |
| Workloads matching Clang byte for byte | 9 / 9 | 9 / 9 |
| `luac -s` output identical | 42 / 42 | 42 / 42 |
| Matches Clang | yes | yes |
| Compile time, ours vs Clang | 26.42 s vs 8.96 s (2.95x) | 25.86 s vs 11.29 s (2.29x) |
| Test-suite run time, ours vs Clang | 23.67 s vs 22.23 s (1.06x) | 12.70 s vs 12.69 s (1.00x) |
| Workload run time, ours vs Clang | 11.30 s vs 9.97 s (1.13x) | 5.03 s vs 5.34 s (0.94x) |

Compile time is the sum over all files of both configurations: this
compiler plus Clang turning its IR into an object, against Clang
compiling the C directly. Run times are wall-clock times of the
three test-suite runs and of all the workloads at that level.

## Workloads

| Script | Output bytes | O0 matches | O0 ours vs Clang | O2 matches | O2 ours vs Clang |
|---|---| --- | --- | --- | --- |
| `benchmarks.lua` | 253 | yes | 2.68 s / 2.26 s | yes | 1.37 s / 1.50 s |
| `closures.lua` | 375 | yes | 0.37 s / 0.43 s | yes | 0.22 s / 0.26 s |
| `coroutines.lua` | 247 | yes | 0.16 s / 0.17 s | yes | 0.09 s / 0.08 s |
| `format.lua` | 6021 | yes | 0.18 s / 0.15 s | yes | 0.11 s / 0.13 s |
| `gc.lua` | 290 | yes | 0.43 s / 0.45 s | yes | 0.32 s / 0.27 s |
| `math.lua` | 926 | yes | 2.36 s / 1.95 s | yes | 1.04 s / 1.09 s |
| `objects.lua` | 191 | yes | 0.50 s / 0.42 s | yes | 0.27 s / 0.28 s |
| `strings.lua` | 589 | yes | 0.10 s / 0.11 s | yes | 0.07 s / 0.05 s |
| `tables.lua` | 271 | yes | 4.51 s / 4.02 s | yes | 1.56 s / 1.66 s |

## Defects

| Id | Category | Where | Repro test | State |
| --- | --- | --- | --- | --- |
| LUA-1 | frontend reject (missing GNU feature) | `lvm.c`: this compiler defines `__GNUC__`, so Lua turns on its jump table (`LUA_USE_JUMPTABLE`, `ljumptab.h`): `static const void *const disptab[] = { &&L_OP_MOVE, ... };` and `goto *disptab[op];`. Labels as values and computed `goto` were not supported in any mode (`ljumptab.h:28:1: error: expected an expression, found '&&'`). Clang accepts them even with `-std=c11` | [tests/gnu/10_labels_as_values.c](../../tests/gnu/10_labels_as_values.c); unit test `labels_as_values_and_computed_goto` in `src/lib.rs` | fixed (in `-std=gnu11`) |
| LUA-H1 | harness, not a compiler defect | lua.org sometimes drops TLS connections, so a single download failed about half the time | none | fixed in the fetch script (retries on both host names; CI caches the tree) |
| LUA-H2 | harness, not a compiler defect | the test suite prints random seeds, dates and timings, so its output differs between any two runs of the same binary | none | the suite's outcome is compared (exit status, "final OK !!!", and the list of test files reached), not its bytes; the workloads are compared byte for byte |

By category: frontend reject 1 (fixed); ICE 0; bad IR 0; wrong output 0; crash 0; performance 0.

**Language mode.** Lua 5.4.7 is written in C99 that strict C11 accepts, except for the jump table, which Lua selects only because the compiler says it is GNU-compatible. Labels as values are a GNU extension, so, like the other GNU extensions here, they are accepted only with `-std=gnu11`; strict C11 still rejects them with an error that names `-std=gnu11`. The final scorecard therefore builds both compilers with `-std=gnu11`. The frozen baseline used `-std=c11` for both: there every file except `lvm.c` in the release configuration compiled, and the ltests configuration (whose `ltests.h` turns the jump table off) built completely.

**What the fix does** (GCC manual, "Labels as Values"; LLVM Language Reference, `blockaddress` and `indirectbr`). `&&label` is a `void *` naming a label of the current function; it is lowered to `blockaddress(@function, %label)`. It is an address constant in a block-scope `static` initializer, which is how Lua builds its table. `goto *expr;` is lowered to `indirectbr`, listing every label of the function whose address is taken. These were also the only reason six GCC torture tests failed (`20040302-1.c`, `20041214-1.c`, `20071210-1.c`, `920501-4.c`, `920501-5.c`, `990208-1.c`); they now pass and were removed from the known-failure list.

**Deferred.** Differences of label addresses (`&&a - &&b`, used to make position-independent tables) and arithmetic on `void *`, which they need: Lua does not use them; GCC torture `comp-goto-1.c` and `pr70460.c` do. Local label declarations (`__label__`) and nested functions stay out of scope, as PLAN.md says.

**Not covered by this run.** The interactive interpreter tests in `main.lua` and the C-module loading tests in `attrib.lua` are skipped in the portable modes (`_port=true`), because they need readline-style terminal echo and shared libraries built by the suite's own makefile. Everything else in `all.lua` runs, including the long tests in portable mode and the white-box `T` tests in the ltests build.

**Times.** Run times are single runs and move by about 10% from one run to the next on this machine (this one was taken while a single-threaded Csmith smoke run used one of the eight cores). At `-O2` this compiler's Lua is as fast as Clang's (test suite 1.00x, workloads 0.94x); at `-O0` it is a little slower (1.06x and 1.13x) (not investigated; probably this compiler's unoptimised IR doing a little more work than Clang's). Compile time is about 2.3 to 3 times Clang's, mostly in this compiler's preprocessor and frontend; that is a tuning item for PLAN.md step 11, not a defect.

The first run, before the fix, in strict `-std=c11` mode, is [baseline.md](baseline.md) and is kept unchanged.
