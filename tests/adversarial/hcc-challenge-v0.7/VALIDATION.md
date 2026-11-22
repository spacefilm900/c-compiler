# Independent reference validation — v0.7

Date: 21 November 2026. Native x86_64 Linux; actual compiler versions in raw JSON. No candidate build or execution was performed: the reviewing environment has GCC 14.2.0 and Clang 17.0.0, but no Rust toolchain. Grok's documented supported backend is Clang 18+ (reference pack uses 19.1.7); Clang 17 here is used only as a black-box reference, not to validate candidate IR.

| Reference | O0 | O2 |
|---|---:|---:|
| GCC 14.2.0 | 65/65 | 65/65 |
| Clang 17.0.0 | 65/65 | 65/65 |

All seven strict C11 rejection subchecks passed each reference at both optimisation levels. Each of the three paired-TU tests was additionally compiled with GCC for one unit and Clang for the other, in both directions and at both optimisation levels (12/12). Raw command/output results are in `reference-results/`. These reference checks compare against individually specified expected results, not only differential agreement. All runtime gates required zero exit, exact stdout `ok\n`, and empty runtime stderr.

A pre-release draft of HCC07-040 used `p += 2` on an atomic pointer. GCC accepted and executed the program with the asserted result; Clang 17 rejected that expression. This was a reference divergence before freeze, not a candidate failure. The gated case now uses atomic increment/decrement (all four reference runs passed), and the original draft is preserved under `probes/atomic-pointer-compound/` as a non-gating standards probe with its original reference logs. No expectations were changed in response to any hcc result.

The three multi-TU cases have mixed reference evidence, but actual hcc/Clang mixed ABI testing remains pending. No macOS, AArch64 or Windows execution was done. The v0.6 archive checksum agrees with the earlier final report; the 724-case baseline was not rerun here. No candidate result or universal conformance claim is implied.
