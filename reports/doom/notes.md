## Defects

| Id | Category | Where | Repro test | State |
| --- | --- | --- | --- | --- |
| DOOM-1 | frontend reject | `info.c`: the `states[]` table puts function addresses in the `actionf_t` union inside each struct element (`initializer of 'states' uses an address in a union`) | [tests/138_union_address_init.c](../../tests/138_union_address_init.c) | fixed |
| DOOM-2 | wrong output (found in code review of the DOOM-1 fix, not by a demo) | a later designator that names another member of a union (`{ .a.p = &x, .b.q = &y }`) kept the earlier member's bytes or address; C11 6.7.9p17 and p19 make the rest of the newly named member zero. Static and automatic objects, with or without addresses; the plain-bytes and automatic cases predate DOOM-1 | [tests/139_union_member_switch.c](../../tests/139_union_member_switch.c) | fixed |
| DOOM-3 | wrong output (found in code review of the DOOM-1 fix, not by a demo) | a static union whose first member holds a bit-field record, initialized through a longer member with an address: the address was attached to the shorter record's bytes, so the union came out too large and the field after it moved (`struct O { union U u; int tail; }` was 20 bytes, `tail` read 0) | [tests/140_union_bitfield_layout.c](../../tests/140_union_bitfield_layout.c) | fixed |
| DOOM-H1 | harness, not a compiler defect | frames 1–27 and a few later frames changed between runs and between builds, because DOOM draws them from uninitialised memory that holds heap addresses | none; see [harness/doom/dg_alloc.c](../../harness/doom/dg_alloc.c) | fixed in the harness |
| DOOM-H2 | harness, not a compiler defect | a cached doomgeneric tree was trusted from its `.pin` file alone; a relative `DOOM_WAD` was not found once the game ran in its temporary home; a compiler killed by a signal was recorded as a reject | none | fixed in the harness |

By category: frontend reject 1 (fixed); ICE 0; bad IR 0; wrong output 2 (fixed, found by review); crash 0; performance 0. The extra demos found no new defect.

Deferred: nothing. DOOM needed no inline assembly, vector types, computed `goto`, or other deferred feature.

Compile time is about 2 to 3.5 times Clang's, mostly in this compiler's preprocessor and frontend (about 90 ms per file, against Clang's whole compile of about 30 ms per file at `-O0`). That is a tuning item for PLAN.md step 11, not a defect. Run time is within a few percent of Clang's at both levels.

The first run, before the fix and with the first harness, is [baseline.md](baseline.md) and is kept unchanged. It played only the first 7,100 frames of `freedoom1.wad` DEMO1.
