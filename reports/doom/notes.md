## Defects

| Id | Category | Where | Repro test | State |
| --- | --- | --- | --- | --- |
| DOOM-1 | frontend reject | `info.c`: the `states[]` table puts function addresses in the `actionf_t` union inside each struct element (`initializer of 'states' uses an address in a union`) | [tests/138_union_address_init.c](../../tests/138_union_address_init.c) | fixed |
| DOOM-H1 | harness, not a compiler defect | frames 1–27 and a few later frames changed between runs and between builds, because DOOM draws them from uninitialised memory that holds heap addresses | none; see [harness/doom/dg_alloc.c](../../harness/doom/dg_alloc.c) | fixed in the harness |

By category: frontend reject 1 (fixed); ICE 0; bad IR 0; wrong output 0; crash 0; performance 0.

Deferred: nothing. DOOM needed no inline assembly, vector types, computed `goto`, or other deferred feature.

Compile time is about 2.2 to 3.6 times Clang's, mostly in this compiler's preprocessor and frontend (about 90 ms per file, against Clang's whole compile of about 30 ms per file at `-O0`). That is a tuning item for PLAN.md step 11, not a defect. Run time is within a few percent of Clang's at both levels.

The first run, before the fix and with the first harness, is [baseline.md](baseline.md) and is kept unchanged.
