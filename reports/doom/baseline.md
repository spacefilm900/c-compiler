# DOOM scorecard: first run (baseline, frozen)

- Date: 2026-11-13
- Compiler commit: `be590a8`
- doomgeneric: `e257a09527062bfb986b6e5e0780839b6cbd5b71`
- WAD: `freedoom1.wad` (SHA-256 `7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d`), demo `demo1` with `-timedemo`
- Reference: Debian clang version 19.1.7 (3+b1), `-std=gnu11`, same optimisation level
- Frames requested: 2100; translation units: 81; parallel jobs: 8

| | O0 | O2 |
|---| --- | --- |
| Builds with this compiler alone | no | no |
| Files compiled by this compiler | 80 / 81 | 80 / 81 |
| Build compared | mixed | mixed |
| Frames compared | 2100 | 2100 |
| Frames matching Clang | 2099 | 2073 |
| First mismatching frame | 1402 | 1 |
| Matches Clang | no | no |
| Compile time, ours vs Clang | 8.95 s vs 2.85 s (3.14x) | 11.56 s vs 5.15 s (2.25x) |
| of which our frontend | 7.30 s | 7.19 s |
| Run time, ours vs Clang | 2.48 s vs 2.36 s (1.05x) | 1.27 s vs 1.31 s (0.97x) |

Compile time is the sum over all files of this compiler plus Clang turning
its IR into an object, against Clang compiling the C directly. Run time is
the whole demo run, including hashing every frame.

## Files this compiler did not build

- O0 `info.c`: reject: `tests/external/doomgeneric/doomgeneric/info.c:129:20: error: initializer of 'states' uses an address in a union`
- O2 `info.c`: reject: `tests/external/doomgeneric/doomgeneric/info.c:129:20: error: initializer of 'states' uses an address in a union`

## First-run notes (frozen)

This is the first run, on `main` at `be590a8` (pull request 31) with the
first version of the harness. It is kept unchanged as the baseline. The
current scorecard is `scorecard.md`.

- One file was rejected: `info.c`, whose state table puts function
  addresses in a union member (`actionf_t`) inside each struct element.
  The program was therefore a mixed build: Clang compiled `info.c`, this
  compiler compiled the other 80 files.
- Frames that differed from Clang: frame 1402 at `-O0`, and frames 1 to 27
  at `-O2`. The same frames also differed between two runs of one Clang
  build when address-space randomisation was on, so they did not point at
  a compiler bug. DOOM draws them from memory it never initialised (the
  opening screen wipe starts from a video buffer that was never cleared,
  and a few later frames have stray pixels), and that memory holds old
  heap addresses. This first harness turned off address-space
  randomisation, which made each build repeatable but still left those
  addresses depending on the size of the executable.
- This run hashed each frame byte by byte; later runs hash one 32-bit pixel
  per step, so the hash values in later logs are not comparable with this
  one, only the match counts are.
- Compile and run times here were measured with 8 parallel jobs.
