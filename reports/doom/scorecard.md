# DOOM scorecard

- Date: 2026-11-13
- Compiler commit: `8896153`
- doomgeneric: `e257a09527062bfb986b6e5e0780839b6cbd5b71`
- WAD: `freedoom1.wad` (SHA-256 `7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d`), demo `demo1` with `-timedemo`
- Reference: Debian clang version 19.1.7 (3+b1), `-std=gnu11`, same optimisation level
- Frames requested: 7100; translation units: 82; parallel jobs: 1

| | O0 | O2 |
|---| --- | --- |
| Builds with this compiler alone | yes | yes |
| Files compiled by this compiler | 82 / 82 | 82 / 82 |
| Build compared | full | full |
| Frames compared | 7100 | 7100 |
| Frames matching Clang | 7100 | 7100 |
| First mismatching frame | none | none |
| Matches Clang | yes | yes |
| Compile time, ours vs Clang | 9.13 s vs 2.54 s (3.59x) | 12.11 s vs 5.49 s (2.21x) |
| of which our frontend | 7.07 s | 7.13 s |
| Run time, ours vs Clang | 8.12 s vs 7.96 s (1.02x) | 4.12 s vs 4.32 s (0.95x) |

Compile time is the sum over all files of this compiler plus Clang turning
its IR into an object, against Clang compiling the C directly. Run time is
the whole demo run, including hashing every frame.

## Defects

| Id | Category | Where | Repro test | State |
| --- | --- | --- | --- | --- |
| DOOM-1 | frontend reject | `info.c`: the `states[]` table puts function addresses in the `actionf_t` union inside each struct element (`initializer of 'states' uses an address in a union`) | [tests/138_union_address_init.c](../../tests/138_union_address_init.c) | fixed |
| DOOM-H1 | harness, not a compiler defect | frames 1–27 and a few later frames changed between runs and between builds, because DOOM draws them from uninitialised memory that holds heap addresses | none; see [harness/doom/dg_alloc.c](../../harness/doom/dg_alloc.c) | fixed in the harness |

By category: frontend reject 1 (fixed); ICE 0; bad IR 0; wrong output 0; crash 0; performance 0.

Deferred: nothing. DOOM needed no inline assembly, vector types, computed `goto`, or other deferred feature.

Compile time is about 2.2 to 3.6 times Clang's, mostly in this compiler's preprocessor and frontend (about 90 ms per file, against Clang's whole compile of about 30 ms per file at `-O0`). That is a tuning item for PLAN.md step 11, not a defect. Run time is within a few percent of Clang's at both levels.

The first run, before the fix and with the first harness, is [baseline.md](baseline.md) and is kept unchanged.
