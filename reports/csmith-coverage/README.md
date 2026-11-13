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

## Defects found by the first run (fixed in the follow-up below)

1. **`repro-struct-return-flag.c`** — after a discarded return of a bit-field struct, an `int` flag of 0 is treated as true when passed as the third argument of a function (integer, pointer, int). Found in seeds 121, 755, 819, 1468, 1841, and 1855 at `-O0`. The checksum line still matches; extra `transparent_crc` traces are printed. The same six seeds agree at `-O2`.
2. **`repro-pointer-compare.c`** and **`repro-pointer-assign.c`** — this compiler rejects incompatible object-pointer comparisons and assignments that GCC and Clang accept in GNU mode. 418 comparisons and 90 assignments at `-O0`. GNU mode today only allows `void *` compared with a function pointer.

Rerun: `python3 tools/csmith_campaign.py --compiler build/compiler --seed-start 1 --count 500 --opt O0 --opt O2 --out DIR`.

## Follow-up run after the fixes (13 November 2026)

Both defects above are fixed on branch `csmith-fixes`. The binary used for this run was built from `c39698015d913f09a6a4fde738b4f582406a49b5`; later commits change only documents and known-failure lists. The same 2,000 seeds were rerun with the same options, GCC 14.2.0 and Clang 19.1.7 as black-box oracles. Local evidence: `/workspace/csmith-fixes-2026-11-13`. `SUMMARY-after-fixes.json` is the summary file from that run.

| Class | `-O0` before | `-O0` after | `-O2` before | `-O2` after |
| --- | ---: | ---: | ---: | ---: |
| Valid differential comparisons | 1,492 | **1,821** | 1,490 | **1,822** |
| `agreement` | 1,486 | **1,821** | 1,490 | **1,822** |
| `candidate-wrong-output` | 6 | **0** | 0 | **0** |
| `candidate-rejects-valid-gnu-c` | 508 | **0** | 505 | **0** |
| `reference-disagreement` | 0 | 0 | 5 | 0 |
| `reference-crash` | — | 179 | — | 178 |
| `candidate-crash-or-ice` / `candidate-bad-ir` / `candidate-timeout` / `reference-timeout` / `invalid-program` / `generation-failed` | 0 | 0 | 0 | 0 |

The six wrong-output seeds (121, 755, 819, 1468, 1841, 1855) now agree at both levels.

`reference-crash` is not a candidate defect. The first run's table predates the classification fixes merged at the end of pull request 29, and the candidate rejected these programs before they ran, so they were counted as rejects. Now that the candidate compiles them, they reach the run step, and the reference binaries do not finish: in 352 of the 357 records GCC, Clang, and the candidate binary are all killed by `SIGKILL` from the CPU limit (`RLIMIT_CPU`, 8 seconds). In the other five (seed 568 and the four former `-O2` reference disagreements 1085, 1502, 1562, 1810) GCC is killed by the CPU limit while the Clang binary exits with `SIGSEGV` or status 16, and the candidate is killed by the CPU limit. These are inconclusive programs (very long or undefined-behaviour-dependent runs), not comparisons.

No new reject category appeared: the candidate compiled every seed in both runs of the follow-up.

### What changed

1. **Struct returns whose size is not a multiple of eight.** A struct of 9–15 bytes returned or passed in two general registers used an `i64` for its second eightbyte whenever several fields (or one bit-field storage unit) shared it, so the caller stored eight bytes into the last four bytes of a 12-byte temporary and overwrote the neighbouring stack slot; the callee also loaded past the end of the object. In the reproducer the overwritten slot was the `int` flag. The second piece now covers only the bytes the object owns (`{ i64, i32 }` for 12 bytes, `{ i64, i24 }` for 11, `{ i64, i16 }` for 10), which is also what Clang produces for the same types (checked as a black box). Regression tests: `tests/137_struct_return_tail.c` and the `abi` unit test `last_eightbyte_never_extends_past_the_object`.
2. **GNU-mode pointer comparisons and assignments.** With `-std=gnu11`, comparing pointers to distinct types, and assigning a pointer whose pointee differs only in integer signedness or drops a qualifier, are accepted with a warning. Strict C11 still rejects them. Regression tests: `tests/gnu/01_distinct_pointer_compare.c`, `tests/gnu/02_pointer_sign_assign.c` (each is also checked to be rejected in strict mode), and the `sema` unit tests `gnu_dialect_accepts_distinct_pointer_comparisons` and `gnu_dialect_accepts_pointer_sign_and_qualifier_assignments`.
