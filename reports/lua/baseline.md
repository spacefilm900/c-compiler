# Lua scorecard: first run (baseline, frozen)

- Date: 2026-11-14
- Compiler commit: `52994ed`
- Lua: 5.4.7 from lua.org (`lua-5.4.7.tar.gz` SHA-256 `9fbf5e28ef86c69858f6d3d34eccc32e911c1a28b4120ff3e84aaa70cfbf1e30`)
- Test suite: `lua-5.4.7-tests.tar.gz` (SHA-256 `8a4898ffe4c7613c8009327a0722db7a41ef861d526c77c5b46114e59ebf811e`)
- Language mode: `-std=c11` for this compiler and for the reference; `-DLUA_COMPAT_5_3 -DLUA_USE_LINUX`, linked with `-lm -ldl -Wl,-E`, no readline
- Reference: Debian clang version 19.1.7 (3+b1), same flags and optimisation level
- Compile jobs: 8; test-suite runs at once: 1

| | O0 | O2 |
|---| --- | --- |
| Builds with this compiler alone (both configurations) | no | no |
| Files compiled, release (`lua`, `luac`) | 33 / 34 | 33 / 34 |
| Files compiled, ltests (internal checks on) | 34 / 34 | 34 / 34 |
| Build compared | mixed | mixed |
| Test suite, user mode (`_U=true`) | OK; Clang OK | OK; Clang OK |
| Test suite, portable mode (`_port=true`) | OK; Clang OK | OK; Clang OK |
| Test suite, internal (ltests build) | OK; Clang OK | OK; Clang OK |
| Workloads matching Clang byte for byte | 9 / 9 | 9 / 9 |
| `luac -s` output identical | 42 / 42 | 42 / 42 |
| Matches Clang | no | no |
| Compile time, ours vs Clang | 14.56 s vs 3.20 s (4.55x) | 20.25 s vs 9.91 s (2.04x) |
| Test-suite run time, ours vs Clang | 17.15 s vs 16.53 s (1.04x) | 10.30 s vs 10.45 s (0.99x) |
| Workload run time, ours vs Clang | 9.02 s vs 9.38 s (0.96x) | 4.52 s vs 4.72 s (0.96x) |

Compile time is the sum over all files of both configurations: this
compiler plus Clang turning its IR into an object, against Clang
compiling the C directly. Run times are wall-clock times of the
three test-suite runs and of all the workloads at that level.

## Workloads

| Script | Output bytes | O0 matches | O0 ours vs Clang | O2 matches | O2 ours vs Clang |
|---|---| --- | --- | --- | --- |
| `benchmarks.lua` | 253 | yes | 2.05 s / 2.44 s | yes | 1.15 s / 1.19 s |
| `closures.lua` | 375 | yes | 0.25 s / 0.30 s | yes | 0.15 s / 0.17 s |
| `coroutines.lua` | 247 | yes | 0.14 s / 0.15 s | yes | 0.08 s / 0.08 s |
| `format.lua` | 6021 | yes | 0.16 s / 0.14 s | yes | 0.10 s / 0.11 s |
| `gc.lua` | 290 | yes | 0.36 s / 0.36 s | yes | 0.22 s / 0.22 s |
| `math.lua` | 926 | yes | 1.80 s / 1.83 s | yes | 0.99 s / 1.01 s |
| `objects.lua` | 191 | yes | 0.42 s / 0.40 s | yes | 0.20 s / 0.21 s |
| `strings.lua` | 589 | yes | 0.09 s / 0.09 s | yes | 0.06 s / 0.05 s |
| `tables.lua` | 271 | yes | 3.76 s / 3.69 s | yes | 1.56 s / 1.69 s |

## Files this compiler did not build

- O0 release `lvm.c`: reject: `tests/external/lua/lua-5.4.7/src/ljumptab.h:28:1: error: expected an expression, found '&&'`
- O2 release `lvm.c`: reject: `tests/external/lua/lua-5.4.7/src/ljumptab.h:28:1: error: expected an expression, found '&&'`

## First-run notes (frozen)

This is the first run, on `main` at `060ca52` (pull request 32) with the
first version of the harness, in strict `-std=c11` mode for both compilers.
It is kept unchanged as the baseline. The current scorecard is
`scorecard.md`.

- One file was rejected in the release configuration: `lvm.c`. This
  compiler defines `__GNUC__`, so Lua's `lvm.c` turns on its jump table
  (`LUA_USE_JUMPTABLE`, in `ljumptab.h`), which uses the GNU extensions
  "labels as values" (`&&label`) and computed `goto` (`goto *p;`). This
  compiler did not support either, in any mode. Clang accepts them even
  with `-std=c11`. The release programs were therefore mixed builds: Clang
  compiled `lvm.c`, this compiler compiled the other 33 files.
- The ltests configuration was a full build (34 of 34 files): `ltests.h`
  turns the jump table off, so its `lvm.c` uses the plain `switch`
  dispatch, which this compiler already handled.
- Every comparison that could be made matched: all three test-suite modes
  ended with "final OK !!!" for both builds at both levels, all nine
  workloads matched Clang byte for byte, and `luac -s` gave identical
  bytecode for all 42 Lua files.
- Run times at `-O0` in the release configuration are not a fair
  comparison here: the interpreter loop, `lvm.c`, came from Clang.
