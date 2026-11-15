# C11 Challenge v0.6: Final Report

**Project:** clean-room C11 compiler in Rust that emits textual LLVM IR ([spacefilm900/c-compiler](https://github.com/spacefilm900/c-compiler), private)
**Prepared:** Sunday 15 November 2026 (Europe/London, UTC+0)
**Status:** v0.6 was accepted as the final adversarial release, and this phase is closed. Every one of the 724 cases passes on the current `main`.

## 1. Summary

| Stage | Compiler | O0 | O2 | Metamorphic | IR verify |
|---|---|---|---|---|---|
| First run (frozen) | main `fa39ab2` | 722/724 | 722/724 | 60/60 | 150/150 |
| After repair (PR 27 head `e21b231`) | PR 27 | 724/724 | 724/724 | 60/60 | 150/150 |
| **Re-confirmation today** | **main `0c43e32`** | **724/724** | **724/724** | **60/60** | **150/150** |

- **Silent wrong code:** none at any stage. All 491 run cases and 73 cross-ABI cases pass.
- **Invalid LLVM IR:** none found.
- **Defects found in v0.6:** one family, made up of two cases (C11-703 and C11-704), now fixed.
- **Csmith (1,000 seeds at O0 and O2):** no compiler defects found. Only 166 programs per optimisation level were valid strict C11, so this tested code generation on 166 programs at each level (see section 4).

## 2. Challenge identity and protocol

- The archive `C11_Challenge_v0.6.zip` (SHA256 `07e26e6c29a48f1a605d4cc1da38d53db10f424689b95b9dc17030a63b7ff182`) was imported unchanged as commit `4ba1b59`. All 1,590 checksum entries verified, and all 724 cases are present: 636 inherited from earlier versions plus 88 new.
- The raw first-run results were committed (`6dbe48a`) before any analysis (`e7f7df1`), as the freeze protocol requires.
- No challenge case or expected output was modified at any point.
- **Runner command:** `python3 runner.py --compiler <compiler> --linker clang --reference gcc --abi-direction both --opt O0|O2`
- **Host:** x86_64 Linux (LP64, System V AMD64), rustc 1.85.1, Debian clang 19.1.7, GCC 14.2.0, Python 3.13.5.

## 3. The defect and its repair

| Case | Code | Classification | Clause |
|---|---|---|---|
| C11-703 | `void *v=0; return v==f;` | Required diagnostic omitted | 6.5.9p2 |
| C11-704 | `void *v=0; return v!=&f;` | Required diagnostic omitted | 6.5.9p2 |

Under C11 6.5.9p2, a `void *` may be compared for equality only with an object pointer or a null pointer constant. Here `v` is an object, not a null pointer constant, so comparing it with a function pointer is a constraint violation that requires a diagnostic. GCC 14 and Clang 19 accept this comparison as an extension by default and reject it only with `-pedantic-errors`.

**Why the fix was held back earlier:** applying the diagnostic unconditionally breaks GCC torture test `20041212-1.c`, which depends on the GNU extension.

**Resolution (PR 27, merged 13 November 2026 as `d728a94`):**
- Strict ISO C11 is now the default mode, and it rejects the comparison.
- A new opt-in `-std=gnu11` accepts the comparison with a warning. The GCC torture harness runs in this mode.
- Null pointer constants follow 6.3.2.3p3 exactly. A `_Generic` that selects `(void *)0` counts as one, while `(void *)(void *)0` does not. Both cases were found in code review, and Clang 19 with `-pedantic-errors` gives the same answer for each.

**Effect:** C11-703 and C11-704 changed from fail to pass. No other case changed, at either optimisation level.

## 4. Csmith differential campaign (seeds 1–1000, O0 and O2)

The campaign used the challenge's own `tools/csmith_campaign.py`, unchanged, with Csmith 2.3.0 (`30dccd73`) at its default options. For each seed the tool built the program three ways: `gcc -std=c11`, `clang -std=c11`, and our compiler followed by `clang` on the resulting IR. It then compared each build's exit code and checksum output.

| Outcome | O0 | O2 |
|---|---:|---:|
| Agreement with GCC and Clang | 166 (after relink with `-lm`, see below) | 166 |
| Rejected as a C11 constraint violation (both `gcc` and `clang -pedantic-errors` agree) | 699 | 703 |
| Reference binary timed out after 15 s; our compiler had already rejected the program as a constraint violation | 132 | 130 |
| Transient build timeout under load; on rerun our compiler rejected the program as a constraint violation | 3 | 1 |
| **Miscompilations** | **0** | **0** |

- **Rejections:** default Csmith output mixes incompatible pointer types in comparisons and assignments (6.5.9p2 and 6.5.16.1p1). GCC and Clang only warn about this by default. Under `-pedantic-errors`, both rejected every program that our compiler rejected.
- **The `-lm` relink was a harness issue, not a compiler issue.** At O0 the tool links our IR without `-lm`, so the calls to `fabs` and `fabsf` stay unresolved. After relinking the unchanged IR with `-lm`, all 166 programs matched exactly. At O2, LLVM replaces those calls with instructions, so no relink was needed.
- **Limitation:** only 166 of the 1,000 programs exercised code generation. A broader campaign would need Csmith options or a filter that avoids incompatible pointer types.

## 5. Re-confirmation on the current compiler (15 November 2026)

Since PR 27, the compiler has had several more changes merged:
- GNU extensions (PR 31)
- DOOM (PR 32), Lua (PR 33) and SQLite (PR 34)
- Performance work (PR 35)
- The macOS Apple Silicon port, its CI and DOOM on Mac (PRs 36–38)

To make sure none of this caused a regression, the unchanged v0.6 suite was rerun today on `main` at `0c43e32` (binary SHA256 `b68fcd985207576b1846203162608eaa521c5d0062936255c8b289fa471a9ee4`), using the same host and tools:

- O0: **724/724**, 0 gating failures
- O2: **724/724**, 0 gating failures
- Metamorphic: 60/60 (O2). LLVM IR verification: 150/150.

## 6. Open items (none block closing this phase)

1. **The missing `-lm` in the Csmith tool has been fixed.** The repository's copy of `tools/csmith_campaign.py` now always links with `-lm` (Csmith coverage step, PR 29). The challenge archive itself was not changed.
2. **Csmith coverage has been widened.** In a follow-up campaign (PRs 29 and 30), the compiler ran in `-std=gnu11` mode with Csmith size caps, `--no-packed-struct` and `--no-argc`, over seeds 1–2000. It found two real defects. The first was wrong output at O0: after a discarded bit-field struct return, an `int` flag of 0 was treated as true. That changed only trace output in 6 seeds; the checksum still matched. The second was GNU-mode pointer comparisons and assignments being rejected. After both were fixed, there were 1,821 valid comparisons at O0 and 1,822 at O2, all in agreement. See `reports/csmith-coverage/README.md`.
3. **The supplied `metamorphic_smoke.py`** assumes a GCC- or Clang-style driver. The candidate-specific `tools/metamorphic_candidate.py` was used instead and remains the runner.

## 7. Evidence

The full evidence pack, `post-fix-report-v0.6.zip`, was sent on 13 November 2026 and is about 56 MB. It is also available in three parts. It contains the frozen first run, the post-fix and final-head runs, per-case comparisons, the Csmith sources, results and triage, a 30-seed sanity recheck, and checksums. The files from today's re-confirmation are in `docs/adversarial/v0.6/reconfirm-2026-11-15/` in the repository.
