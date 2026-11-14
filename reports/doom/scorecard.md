# DOOM scorecard

- Date: 2026-11-14
- Compiler commit: `214d2c2`
- doomgeneric: `e257a09527062bfb986b6e5e0780839b6cbd5b71`
- WAD: `freedoom1.wad` (SHA-256 `7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d`)
- WAD: `freedoom2.wad` (SHA-256 `a8772e088847032510d97ba2312406a6998f21cbab44d4ff10696faa9c0ecd4b`)
- Demos played with `-timedemo` to their natural end
- Reference: Debian clang version 19.1.7 (3+b1), `-std=gnu11`, same optimisation level
- Frame cap per demo: 100000; translation units: 82; compile jobs: 8; demo runs at once: 1

| | O0 | O2 |
|---| --- | --- |
| Builds with this compiler alone | yes | yes |
| Files compiled by this compiler | 82 / 82 | 82 / 82 |
| Build compared | full | full |
| Frames compared | 39209 | 39209 |
| Frames matching Clang | 39209 | 39209 |
| First mismatching frame | none | none |
| Matches Clang | yes | yes |
| Compile time, ours vs Clang | 9.26 s vs 2.52 s (3.67x) | 11.84 s vs 5.73 s (2.07x) |
| of which our frontend | 7.54 s | 7.37 s |
| Run time, ours vs Clang | 42.98 s vs 44.80 s (0.96x) | 23.48 s vs 24.12 s (0.97x) |

Compile time is the sum over all files of this compiler plus Clang turning
its IR into an object, against Clang compiling the C directly. Run time is
all the demo runs at that level, including hashing every frame.

## Demos

| WAD | Demo | Ends by | Game tics | Frames | O0 matches Clang | O2 matches Clang |
|---|---|---|---|---| --- | --- |
| `freedoom1.wad` | demo1 | demo end | 7117 | 7156 | yes (7156 frames) | yes (7156 frames) |
| `freedoom1.wad` | demo2 | demo end | 7089 | 7128 | yes (7128 frames) | yes (7128 frames) |
| `freedoom1.wad` | demo3 | demo end | 6042 | 6081 | yes (6081 frames) | yes (6081 frames) |
| `freedoom1.wad` | demo4 | demo end | 3136 | 3175 | yes (3175 frames) | yes (3175 frames) |
| `freedoom2.wad` | demo1 | demo end | 2664 | 2703 | yes (2703 frames) | yes (2703 frames) |
| `freedoom2.wad` | demo2 | demo end | 5444 | 5483 | yes (5483 frames) | yes (5483 frames) |
| `freedoom2.wad` | demo3 | demo end | 2743 | 2782 | yes (2782 frames) | yes (2782 frames) |
| `freedoom2.wad` | demo4 | demo end | 4662 | 4701 | yes (4701 frames) | yes (4701 frames) |

Total frames compared, all demos and levels: 78418.

## Defects

| Id | Category | Where | Repro test | State |
| --- | --- | --- | --- | --- |
| DOOM-1 | frontend reject | `info.c`: the `states[]` table puts function addresses in the `actionf_t` union inside each struct element (`initializer of 'states' uses an address in a union`) | [tests/138_union_address_init.c](../../tests/138_union_address_init.c) | fixed |
| DOOM-2 | wrong output (found in code review of the DOOM-1 fix, not by a demo) | a later designator that names another member of a union (`{ .a.p = &x, .b.q = &y }`) kept the earlier member's bytes or address; C11 6.7.9p17 and p19 make the rest of the newly named member zero. Follow-up from a second review: an enclosing union of the same offset and size now keeps its choice when a nested union switches, and the replaced expressions are still checked (and must be constant in a static initializer). Static and automatic objects, with or without addresses; the plain-bytes and automatic cases predate DOOM-1 | [tests/139_union_member_switch.c](../../tests/139_union_member_switch.c) | fixed |
| DOOM-3 | wrong output (found in code review of the DOOM-1 fix, not by a demo) | a static union whose first member holds a bit-field record, initialized through a longer member with an address: the address was attached to the shorter record's bytes, so the union came out too large and the field after it moved (`struct O { union U u; int tail; }` was 20 bytes, `tail` read 0) | [tests/140_union_bitfield_layout.c](../../tests/140_union_bitfield_layout.c) | fixed |
| DOOM-H1 | harness, not a compiler defect | frames 1–27 and a few later frames changed between runs and between builds, because DOOM draws them from uninitialised memory that holds heap addresses | none; see [harness/doom/dg_alloc.c](../../harness/doom/dg_alloc.c) | fixed in the harness |
| DOOM-H2 | harness, not a compiler defect | a cached doomgeneric tree was trusted from its `.pin` file alone; a relative `DOOM_WAD` was not found once the game ran in its temporary home; a compiler killed by a signal was recorded as a reject; a demo stopped at the `--frames` cap could be reported as a full pass | none | fixed in the harness |

By category: frontend reject 1 (fixed); ICE 0; bad IR 0; wrong output 2 (fixed, found by review); crash 0; performance 0. The extra demos found no new defect.

Deferred: nothing. DOOM needed no inline assembly, vector types, computed `goto`, or other deferred feature.

Compile time is about 2 to 3.5 times Clang's, mostly in this compiler's preprocessor and frontend (about 90 ms per file, against Clang's whole compile of about 30 ms per file at `-O0`). That is a tuning item for PLAN.md step 11, not a defect. Run time is within a few percent of Clang's at both levels.

The first run, before the fix and with the first harness, is [baseline.md](baseline.md) and is kept unchanged. It played only the first 7,100 frames of `freedoom1.wad` DEMO1.
