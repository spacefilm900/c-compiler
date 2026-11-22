# Inputs and prior baselines

The new sources are informed by the owner's Grok information pack, v0.7 repair report and v0.6 regression confirmation. They use documented feature/target contracts and N1570; no external compiler implementation source was inspected or copied. GCC/Clang were black-box reference compilers. No source from the private HCC repository is redistributed or changed.

| Input | SHA-256 |
|---|---|
| c-compiler-review-4e69525.zip | e891de5389f959b516cb5bc776a721598c7e27300dffd1fc3851597808c14010 |
| hcc-challenge-v07-report.zip | a0ded04a925c958dfc5cedf37d3a62729a159e73f3edbb5b6556bb2ec39dc6c3 |
| hcc-v06-regression-e2cf13f.zip | 31330377f7a92b965f70f4e5c00f18f7766fa6a33e47f483afe254111a630bef |
| HCC_v07.zip | 752f7d098d004ef14239b2c657498b1eb6b5b7c07e4699e834c28ec9da26e248 |
| C11_v06.zip (same original frozen archive as v0.6) | 07e26e6c29a48f1a605d4cc1da38d53db10f424689b95b9dc17030a63b7ff182 |

The regression report records merged commit `e2cf13f8af83795446608999d5b97385591d6056` and `hcc` SHA-256 `64be05329e3f002a4474dbdb9ede0151ab1f53fa3f4c01ea9b641bbafaece90a` for Grok's build. A new clean build may have a different binary hash; record it, not silently substitute the old value. Its reported v0.6 regression result was 724/724 at both levels. v0.7 first-run 64/65, post-repair 65/65, subject to the preserved report. These are historical input results, not newly executed tests for v0.8.

Existing archives remain external to this package to maintain their historical identity and keep the new handover compact. Retain the known restrictions from NEW-FEATURES.md: optional non-support of complex numbers, limited supported atomic sizes, stronger memory-order lowering, unsupported VLA parameter qualifiers/static bounds, incomplete computed-goto-into-VM-scope diagnostic, documented nonprototype deviation, and platform TLS stand-in limitations. Do not report these as novel defects by default.
