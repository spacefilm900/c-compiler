# Independent reference validation — frozen v0.8

Reference host: x86-64 Linux. GCC 14.2.0 and Clang 17.0.0. Locally executed candidate: **none**. GCC/Clang sources not inspected. Reference programs are the same frozen final case sources and expectations.

| Verification | O0 | O2 |
|---|---:|---:|
| GCC native | 80/80 | 80/80 |
| Clang native | 80/80 | 80/80 |
| GCC/Clang mixed ABI (12 cases x 2 directions) | 24/24 | 24/24 |
| Clang target/fixture object compilation (6 targets x 6 fixtures) | 36/36 | 36/36 |
| Matching Linux x86-64 platform fixture execution | 6/6 | 6/6 |

Both GCC and Clang native results include the six strict GNU diagnostic probes. At O2 the eight thread cases were also repeated ten times with each reference compiler: 80/80 with GCC and 80/80 with Clang. These checks supplement the native 80-case result rather than increasing it. The cross-target matrix's non-host object checks do **not** establish successful target execution, real platform thread/TLS behaviour or mixed ABI interoperability. Clang 17 was an independent reference, not a supported HCC backend. For the actual candidate use LLVM >=18, as documented in the project pack.

## Pre-freeze review and exclusions

The draft atomic-pointer generic `atomic_fetch_add`/`atomic_fetch_sub` test passed Clang 17 and failed GCC 14.2 at O0 (program exited at its first failing assertion). The test is retained separately, unchanged, together with the original raw reference outputs and explanation in `probes/atomic-pointer-fetch/`. Its native gating slot was replaced with an independently validated pointer exchange test before freezing. No compiler under test has run. This is not evidence of an HCC bug. The final reference native compilation uses `-pedantic-errors` for strict C11 sources; reference link flags allow libatomic only as reference infrastructure. The HCC path does not use libatomic. During draft validation, cross-targeting Apple's versionless triple with Clang 17 rejected TLS; explicitly setting supported deployment versions resolved the oracle toolchain configuration. Both were corrected before release, not modifications of any frozen candidate evidence.

All final native and cross-target JSON reports and stress/mixed outputs are under `reference-results/`. They retain command lines and raw output streams. No claims of independent HCC execution, full C11 compliance, comprehensive concurrence/memory-order coverage, or non-host execution are made. Previously reported v0.6/v0.7 results are not rerun by this release creation.
