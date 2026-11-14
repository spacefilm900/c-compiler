# SQLite scorecard

- Date: 2026-11-14
- Compiler commit: `22452f2`
- SQLite: 3.54.1 amalgamation from sqlite.org (`sqlite-amalgamation-3540100.zip` SHA-256 `0a9c8e34352dad46006aa82836d5383bff9dc6bc9ce5a60553c05aa30e6ad1f9`, SHA3-256 `92281c86e2957f9b9305890f9ac04204caa7b677cc868dc326d942689df28e30`)
- Language mode: `-std=gnu11` for this compiler and for the reference; `-D_DEFAULT_SOURCE -DSQLITE_THREADSAFE=0 -DSQLITE_ENABLE_FTS5 -DSQLITE_ENABLE_RTREE -DSQLITE_ENABLE_MATH_FUNCTIONS`, linked with `-lm -ldl -lpthread`, no readline
- Reference: Debian clang version 19.1.7 (3+b1), same flags and optimisation level
- Compile jobs: 8

| | O0 | O2 |
|---| --- | --- |
| Builds with this compiler alone | yes | yes |
| Files compiled (`sqlite3.c`, `shell.c`) | 2 / 2 | 2 / 2 |
| Build compared | full | full |
| Workloads matching Clang byte for byte | 10 / 10 | 10 / 10 |
| Database files identical | 10 / 10 | 10 / 10 |
| Matches Clang | yes | yes |
| Compile time, ours vs Clang | 119.32 s vs 1.70 s (70.39x) | 141.92 s vs 24.38 s (5.82x) |
| Compile peak memory | ours 698.0 MiB (frontend), Clang 211.3 MiB | ours 698.0 MiB (frontend), Clang 273.6 MiB |
| Workload run time, ours vs Clang | 0.06 s vs 0.06 s (1.05x) | 0.05 s vs 0.06 s (0.89x) |

Compile time is this compiler producing LLVM IR plus Clang turning that
IR into an object, against Clang compiling the C directly. The frontend
does not take `-O`, so its IR is produced once and reused at every
level; the measured frontend time is included at each level. Run times
are wall-clock times of all the workloads at that level. The two
builds take turns to run first: Clang first at the first level and on
every other workload in name order, this compiler first for the rest.

## Per-file compile time and peak memory

| File | O0 ours (front+back, peak) | O0 Clang | O2 ours (front+back, peak) | O2 Clang |
|---| --- | --- | --- | --- |
| `sqlite3.c` | 112.22 s + 1.07 s, 698.0 MiB | 1.35 s, 211.3 MiB | 112.22 s + 20.77 s, 698.0 MiB | 21.06 s, 273.6 MiB |
| `shell.c` | 5.74 s + 0.28 s, 240.6 MiB | 0.34 s, 125.4 MiB | 5.74 s + 3.19 s, 240.6 MiB | 3.33 s, 141.4 MiB |

## Workloads

| Script | Output bytes | O0 matches | O0 db | O0 ours vs Clang | O2 matches | O2 db | O2 ours vs Clang |
|---|---| --- | --- | --- | --- | --- | --- |
| `01_ddl_inserts.sql` | 164 | yes | yes | 0.01 s / 0.01 s | yes | yes | 0.01 s / 0.02 s |
| `02_joins_indexes.sql` | 187 | yes | yes | 0.01 s / 0.00 s | yes | yes | 0.00 s / 0.00 s |
| `03_triggers_tx.sql` | 143 | yes | yes | 0.00 s / 0.00 s | yes | yes | 0.00 s / 0.00 s |
| `04_cte_recursive.sql` | 229 | yes | yes | 0.00 s / 0.00 s | yes | yes | 0.00 s / 0.00 s |
| `05_windows.sql` | 277 | yes | yes | 0.00 s / 0.00 s | yes | yes | 0.00 s / 0.00 s |
| `06_json.sql` | 175 | yes | yes | 0.00 s / 0.00 s | yes | yes | 0.01 s / 0.00 s |
| `07_fts5.sql` | 184 | yes | yes | 0.00 s / 0.00 s | yes | yes | 0.00 s / 0.00 s |
| `08_printf_float.sql` | 662 | yes | yes | 0.00 s / 0.00 s | yes | yes | 0.00 s / 0.00 s |
| `09_random_seed.sql` | 418 | yes | yes | 0.02 s / 0.02 s | yes | yes | 0.01 s / 0.01 s |
| `10_vacuum_rtree.sql` | 89 | yes | yes | 0.01 s / 0.01 s | yes | yes | 0.00 s / 0.01 s |

## Defects

| Id | Category | Where | Repro test | State |
| --- | --- | --- | --- | --- |
| SQLITE-1 | frontend reject | `shell.c`: `SQLITE_EXTENSION_INIT1;` expands to a lone `;` at file scope (`expected a type, found ';'`). C11 6.7 does not allow an empty declaration there; Clang 19 accepts it with `-Wextra-semi`, including under `-std=c11 -pedantic` | [tests/142_empty_declaration.c](../../tests/142_empty_declaration.c) | fixed |
| SQLITE-2 | frontend reject (missing GNU type) | `sqlite3.c`: this compiler defines `__GNUC__` and `__x86_64__`, so SQLite turns on `SQLITE_USE_UINT128` and writes `__uint128_t r = (__uint128_t)a * b;` (`expected ';', found 'r'`). Clang accepts `__int128` / `__uint128_t` even with `-std=c11` | [tests/143_uint128.c](../../tests/143_uint128.c) | fixed |

By category: frontend reject 2 (fixed); ICE 0; bad IR 0; wrong output 0; crash 0; performance 0.

**Language mode.** SQLite 3.54.1 is written in C that strict C11 can accept, but the command-line shell uses POSIX functions (`strdup`, `lstat`, `symlink`, and others). glibc hides those declarations from a strict ISO C translation unit, so Clang itself refuses `shell.c` with `-std=c11` and no feature-test macro. The scorecard therefore builds both compilers with `-std=gnu11 -D_DEFAULT_SOURCE`. `_DEFAULT_SOURCE` is also needed with this compiler in GNU mode, because `__STRICT_ANSI__` is still predefined in both modes. The frozen baseline used the same flags.

**What the fixes do.** An empty declaration at file scope (a lone `;`) is accepted and ignored, so a macro that expands to nothing and is followed by a semicolon is not a diagnostic. `__int128`, `signed __int128`, `unsigned __int128`, `__int128_t`, and `__uint128_t` are 128-bit integers, size and alignment 16, lowered to LLVM `i128`. Arithmetic, shifts, and conversions to and from the ordinary integer types work; a 128-bit parameter or return is an `i128` value, which LLVM lowers to the System V two-register convention. Integer constants in this compiler are stored in 64 bits, so a 128-bit expression is not folded to a constant; SQLite only uses the type for run-time widening multiplies, which is what the regression test checks. The same two changes made five GCC torture tests pass (`20050106-1.c`, `builtin-bitops-1.c`, `pr34154.c`, and `pr37573.c` for a trailing `};` after a function, and `pr84748.c` for `__int128`), which were removed from the known-failure list.

**Not covered by this run.** SQLite's official TCL test suite is not run: it needs TCL and is far larger than a lean job can host. sqllogictest was considered as a TCL-free corpus, but its published runner is also TCL, so it is not included; the ten workloads cover DDL, joins, indexes, triggers, transactions and rollback, recursive CTEs, window functions, JSON, FTS5, `printf` and floating formats, a fixed-seed generator of 8,000 rows, VACUUM, RTree, and `integrity_check`. Interactive readline, zlib-backed recovery, and extension loading via shared objects are not exercised. Hardware cycle counters in `hwtime.h` (inline `rdtsc`) are compiled only when `VDBE_PROFILE`, `SQLITE_PERFORMANCE_TRACE`, or `SQLITE_ENABLE_STMT_SCANSTATUS` is set; those are off here, so no inline assembly was needed.

**Deferred.** Differences of label addresses, GNU vector types, and nested functions stay out of scope, as PLAN.md says. A later compile-time performance phase will use this amalgamation as its benchmark: `sqlite3.c` is 271,131 lines, and this compiler's frontend is much slower and heavier than Clang compiling the C directly, especially at `-O0` where Clang itself does very little work.

**Times.** Run times are single runs and move a little from one run to the next on this machine. The two builds take turns to run first, so neither always gets a warm cache. Compile time is the item to improve in the performance phase that follows this milestone.

The first run, before the fixes, is [baseline.md](baseline.md) and is kept unchanged.
