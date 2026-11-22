# Independent reference validation

Host: native x86-64 Linux; GCC 14.2.0 (Debian 14.2.0-19), Clang 17.0.0 (Swift LLVM build). Host glibc supplies C11 threads. Actual command logs, versions, manifest hashes and individual outcomes are in reference-results/. The supplied compiler's documented backend is Clang >=18; local Clang 17 is a reference, not a candidate backend claim.

| Check | O0 | O2 |
|---|---:|---:|
| GCC native 100 cases | 100/100 | 100/100 |
| Clang native 100 cases | 100/100 | 100/100 |
| Mixed GCC/Clang, 9 cases, two directions | 18/18 | 18/18 |
| Clang six-target matrix, four fixtures each | 24/24 objects | 24/24 objects |
| Matching native Linux x86-64 target fixtures | 4/4 | 4/4 |
| Stress: 33 marked cases x 10, GCC | not run | 330/330 |
| Stress: 33 marked cases x 10, Clang | not run | 330/330 |
| Generated workload pairs, seeds 1–12, GCC | not run | 12/12 pairs (24/24 programs) |
| Generated workload pairs, seeds 1–12, Clang | not run | 12/12 pairs (24/24 programs) |

The stress optional selection filter was also separately smoke-tested on HCC09-036 using the GCC O2 executable (2/2). The new sources and final manifest were rerun after correcting two recursive mutex declarations to use mtx_plain|mtx_recursive and classifying over-aligned layout cases as target-contract. A draft oracle mistake (four variadic sums expected 43 instead of independently calculated 44) was corrected before freeze. The original draft was 96/100 on both references at O0; it was not a candidate failure. Final results above apply to the released sources. All prior releases have been kept unchanged; no cumulative candidate or reference rerun of the 869 inherited cases was performed here.

## Limits

No HCC candidate run, native non-x86 Linux, native macOS/Windows, weak-memory hardware run or sanitizer run has been performed independently. Object results are not ABI/runtime correctness. The generated pairs use the same workload pattern and do not cover arbitrary C programs. Repetitions are bounded schedule sampling, not proof of thread safety. Timed waits use absolute UTC time and predicate loops where applicable; resource exhaustion, interrupted sleeps, and clock/environment anomalies are inconclusive pending adjudication. C11 requires diagnostics, whereas this runner's rejection gate uses the project's stricter rejection policy; review warnings and rejected programs with the exact standard constraint. Extended alignments are supported target contracts, not required on every C implementation. No complex types, arbitrary-size atomics, unsupported VLA parameter qualifiers, or intentionally undefined data races included. The C11 thread library is delegated to libc on Linux; its successful operation alone is not evidence of a compiler-specific implementation. Native fallback-runtime verification on other platforms is particularly important.
