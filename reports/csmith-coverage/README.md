# Csmith coverage campaign (13 November 2026)

Local evidence lives in `/workspace/csmith-coverage-2026-11-13` (every seed, source, and classification). This directory keeps the totals and the minimised reproducers for the follow-up defects. The campaign was not run in CI.

The candidate was compiled with `-std=gnu11`. GCC 14.2.0 and Clang 19.1.7 ran in their default GNU mode, without `-pedantic-errors`. Every executable was linked with `-lm`, at both `-O0` and `-O2`. Csmith 2.3.0 used the size caps in `tools/csmith_campaign.py` (the same caps as `harness/run_csmith.py`), plus `--no-packed-struct` and `--no-argc`. Seeds 1–2000 were run at both optimisation levels.

## Totals

| Class | `-O0` | `-O2` |
| --- | ---: | ---: |
| Valid differential comparisons | 1,492 | 1,490 |
| `agreement` | 1,486 | 1,490 |
| `candidate-wrong-output` | 6 | 0 |
| `candidate-rejects-valid-gnu-c` | 508 | 505 |
| `reference-disagreement` | 0 | 5 |
| `candidate-crash-or-ice` | 0 | 0 |
| `candidate-bad-ir` | 0 | 0 |
| `candidate-timeout` | 0 | 0 |
| `reference-timeout` | 0 | 0 |
| `invalid-program` | 0 | 0 |

The five `-O2` reference disagreements (seeds 568, 1085, 1502, 1562, 1810) are cases where GCC and Clang did not agree with each other. They are not candidate defects.

## Defects, not fixed here

1. **`repro-struct-return-flag.c`** — after a discarded return of a bit-field struct, an `int` flag of 0 is treated as true when passed as the third argument of a function (integer, pointer, int). Found in seeds 121, 755, 819, 1468, 1841, and 1855 at `-O0`. The checksum line still matches; extra `transparent_crc` traces are printed. The same six seeds agree at `-O2`.
2. **`repro-pointer-compare.c`** and **`repro-pointer-assign.c`** — this compiler rejects incompatible object-pointer comparisons and assignments that GCC and Clang accept in GNU mode. 418 comparisons and 90 assignments at `-O0`. GNU mode today only allows `void *` compared with a function pointer.

Rerun: `python3 tools/csmith_campaign.py --compiler build/compiler --seed-start 1 --count 500 --opt O0 --opt O2 --out DIR`.
