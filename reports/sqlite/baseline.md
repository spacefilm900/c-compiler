# SQLite scorecard: first run (baseline, frozen)

- Date: 2026-11-14
- Compiler commit: `5a5abbd`
- SQLite: 3.54.1 amalgamation from sqlite.org (`sqlite-amalgamation-3540100.zip` SHA-256 `0a9c8e34352dad46006aa82836d5383bff9dc6bc9ce5a60553c05aa30e6ad1f9`, SHA3-256 `92281c86e2957f9b9305890f9ac04204caa7b677cc868dc326d942689df28e30`)
- Language mode: `-std=gnu11` for this compiler and for the reference; `-D_DEFAULT_SOURCE -DSQLITE_THREADSAFE=0 -DSQLITE_ENABLE_FTS5 -DSQLITE_ENABLE_RTREE -DSQLITE_ENABLE_MATH_FUNCTIONS`, linked with `-lm -ldl -lpthread`, no readline
- Reference: Debian clang version 19.1.7 (3+b1), same flags and optimisation level
- Compile jobs: 8

| | O0 | O2 |
|---| --- | --- |
| Builds with this compiler alone | no | no |
| Files compiled (`sqlite3.c`, `shell.c`) | 0 / 2 | 0 / 2 |
| Build compared | mixed | mixed |
| Workloads matching Clang byte for byte | 10 / 10 | 10 / 10 |
| Database files identical | 10 / 10 | 10 / 10 |
| Matches Clang | no | no |
| Compile time, ours vs Clang | 116.20 s vs 1.80 s (64.51x) | 117.39 s vs 25.01 s (4.69x) |
| Compile peak memory | ours 697.9 MiB (frontend), Clang 208.3 MiB | ours 698.1 MiB (frontend), Clang 271.1 MiB |
| Workload run time, ours vs Clang | 0.08 s vs 0.07 s (1.14x) | 0.05 s vs 0.06 s (0.92x) |

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
| `sqlite3.c` | reject (110.74 s, 697.9 MiB) | 1.42 s, 208.3 MiB | reject (111.95 s, 698.1 MiB) | 21.56 s, 271.1 MiB |
| `shell.c` | reject (5.46 s, 240.6 MiB) | 0.38 s, 122.7 MiB | reject (5.44 s, 240.6 MiB) | 3.45 s, 138.8 MiB |

## Workloads

| Script | Output bytes | O0 matches | O0 db | O0 ours vs Clang | O2 matches | O2 db | O2 ours vs Clang |
|---|---| --- | --- | --- | --- | --- | --- |
| `01_ddl_inserts.sql` | 164 | yes | yes | 0.01 s / 0.01 s | yes | yes | 0.01 s / 0.01 s |
| `02_joins_indexes.sql` | 187 | yes | yes | 0.01 s / 0.01 s | yes | yes | 0.01 s / 0.01 s |
| `03_triggers_tx.sql` | 143 | yes | yes | 0.01 s / 0.00 s | yes | yes | 0.00 s / 0.01 s |
| `04_cte_recursive.sql` | 229 | yes | yes | 0.00 s / 0.00 s | yes | yes | 0.00 s / 0.00 s |
| `05_windows.sql` | 277 | yes | yes | 0.00 s / 0.00 s | yes | yes | 0.00 s / 0.00 s |
| `06_json.sql` | 175 | yes | yes | 0.00 s / 0.00 s | yes | yes | 0.00 s / 0.00 s |
| `07_fts5.sql` | 184 | yes | yes | 0.00 s / 0.00 s | yes | yes | 0.00 s / 0.01 s |
| `08_printf_float.sql` | 662 | yes | yes | 0.01 s / 0.00 s | yes | yes | 0.00 s / 0.00 s |
| `09_random_seed.sql` | 418 | yes | yes | 0.02 s / 0.02 s | yes | yes | 0.01 s / 0.01 s |
| `10_vacuum_rtree.sql` | 89 | yes | yes | 0.02 s / 0.01 s | yes | yes | 0.00 s / 0.01 s |

## Files this compiler did not build

- O0 `sqlite3.c`: reject: `tests/external/sqlite/sqlite-amalgamation-3540100/sqlite3.c:37317:15: error: expected ';', found 'r'`
- O0 `shell.c`: reject: `tests/external/sqlite/sqlite-amalgamation-3540100/shell.c:7044:23: error: expected a type, found ';'`
- O2 `sqlite3.c`: reject: `tests/external/sqlite/sqlite-amalgamation-3540100/sqlite3.c:37317:15: error: expected ';', found 'r'`
- O2 `shell.c`: reject: `tests/external/sqlite/sqlite-amalgamation-3540100/shell.c:7044:23: error: expected a type, found ';'`

## First-run notes (frozen)

This is the first run, on `main` at `5a5abbd` (pull request 33) with the
first version of the harness, in `-std=gnu11` with `_DEFAULT_SOURCE` for
both compilers. The shell uses POSIX functions such as `strdup` and
`lstat`, which glibc hides from a strict ISO C translation unit; Clang
itself refuses `shell.c` with `-std=c11` and no feature-test macro. It
is kept unchanged as the baseline. The current scorecard is
`scorecard.md`.

- Neither file compiled. `shell.c` was rejected on a file-scope empty
  declaration: `SQLITE_EXTENSION_INIT1;` expands to a lone `;` at file
  scope (`expected a type, found ';'`). Clang accepts that with a
  `-Wextra-semi` warning, including under `-std=c11 -pedantic`.
- `sqlite3.c` was rejected on `__uint128_t`, a GNU 128-bit integer type.
  This compiler defines `__GNUC__` and `__x86_64__`, so SQLite selects
  its `SQLITE_USE_UINT128` path and writes
  `__uint128_t r = (__uint128_t)a * b;`. The parser does not know that
  type (`expected ';', found 'r'`).
- The programs were therefore mixed builds: Clang compiled both files.
  All ten workloads matched byte for byte, including the database files,
  at both levels. Those run times are not a fair comparison of this
  compiler.
