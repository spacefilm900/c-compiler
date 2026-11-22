# HCC independent extension challenge v0.7

**Phase 2 — round 1 of at most 3.** Release date: 2026-11-21.

This is a test-only, standalone addendum to the closed v0.6 C11 challenge; it does not modify the earlier release, hcc's source, or its in-tree tests. The builder writes any repairs independently from the applicable standards.

## Pinned inputs

- hcc baseline: `4e69525597d484ef0712ebad933a1333c0aba55a`, merge of PR 57.
- Supplied review pack: `c-compiler-review-4e69525.zip`, SHA-256 `e891de5389f959b516cb5bc776a721598c7e27300dffd1fc3851597808c14010`.
- Frozen v0.6 archive SHA-256: `07e26e6c29a48f1a605d4cc1da38d53db10f424689b95b9dc17030a63b7ff182`. It is a separate prerequisite and is not duplicated here.
- Scope and implementation choices: supplied `REQUEST.md`, `NEW-FEATURES.md`, `REVIEW-GUIDE.md` (especially §§11–15) and `STATUS.md` (GNU extensions and later feature records).
- Standard: ISO C11 committee draft N1570, https://www.open-std.org/jtc1/sc22/wg14/www/docs/n1570.pdf. Each case identifies the relevant section(s). GNU cases use the documented hcc contract and the published GCC manual, https://gcc.gnu.org/onlinedocs/gcc/C-Extensions.html, not compiler source. No Clang, GCC, musl, glibc or winpthreads source was inspected or adapted.

## What is in the release?

65 new gated cases: 45 accepted executable programs (including three two-TU cases), 20 required-diagnostic cases, and seven additional strict-mode rejection checks on GNU runtime cases. One exploratory oracle-divergence probe is outside the denominator. A case passes only if every required check passes. Runtime programs contain independent assertions and must exit 0, print exactly `ok\n`, and produce no runtime stderr. The target is native x86_64 Linux LP64 with the System V AMD64 ABI and glibc C11 threads. Per-case expectations, citations, contract classification and link flags are in `manifest.json`, with a readable `CASE_INDEX.md`.

| Area | Cases |
|---|---:|
| Alignment / stdalign | 11 |
| TLS | 6 |
| Old-style function definitions and nonprototype calls | 6 |
| Real type-generic math | 3 |
| VLA / VM types | 14 |
| Atomics | 10 |
| C11 threads and synchronization | 4 |
| GNU extensions | 11 |
| **Total** | **65** |

The classifications are C11 (46 cases), target-contract (8 cases), GNU-contract (11 cases). Target-contract cases use documented alignment, scalar promotion or atomic-size choices. Seven cases need pthread linking (including TLS cases). The runner checks the three two-TU cases with hcc on both units, and separately with hcc/Clang in each direction.

## How to run

From a clean unpacked copy of this directory:

```sh
sha256sum -c SHA256SUMS.txt
python3 runner.py --engine hcc --compiler /absolute/path/to/source/build/hcc \
  --linker clang --opt O0 --out /absolute/path/to/evidence/v07-O0
python3 runner.py --engine hcc --compiler /absolute/path/to/source/build/hcc \
  --linker clang --opt O2 --out /absolute/path/to/evidence/v07-O2
```

Use the project's supported Clang 18+ backend (the supplied pack uses Clang 19.1.7). On this Linux host, link only against the system C11 threads runtime (`-pthread` for the seven marked cases). Do not link `lib/threads.c` in addition. The candidate frontend emits IR; the runner applies optimisation to Clang's IR compilation. `-lm` is included in every link. GCC reference links additionally include `-latomic` as a reference-side accommodation; candidate links do not use it. For reference-only runs, use `--engine gcc` or `--engine clang`; `--compiler` is optional in either reference mode. The `--case` option is for investigation only, not a full release run. Output directories must be new. Use `--jobs 1` for timeout rechecks. `--timeout` defaults to 30 seconds per subprocess; record any change.

The standalone runner records command lines, exit codes, stdout/stderr, timeouts, manifest and binary hashes, compiler version/help output, and per-case verdicts. Per-case directories retain candidate IR, object files and executables where generated. Rejection checks require a nonzero diagnostic exit without an output artifact; crashes, Rust panic exit 101, or timeout are not a pass. Runtime reference compilations use the chosen language mode; diagnostic reference compilations use `-pedantic-errors` to avoid counting tolerated constraint violations as acceptance.

Read `HANDOVER_TO_GROK.md` before executing the first candidate run. See `VALIDATION.md` for the actual black-box reference evidence, `ROADMAP.md` for the maximum three-round plan, and `probes/` for the non-gating probe. This release has not been run against hcc. It is not a certification of the compiler or the entire C11 standard.
