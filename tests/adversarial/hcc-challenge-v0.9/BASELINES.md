# Historical immutable challenge identity

- v0.6 (724), C11_Adversarial_Challenge_v0.6_FINAL.zip SHA256 `07e26e6c29a48f1a605d4cc1da38d53db10f424689b95b9dc17030a63b7ff182`.
- v0.7 (65), HCC_v07.zip SHA256 `752f7d098d004ef14239b2c657498b1eb6b5b7c07e4699e834c28ec9da26e248`. Merged PR #58 baseline `e2cf13f8af83795446608999d5b97385591d6056`.
- v0.8 (80), HCC_v08.zip SHA256 `f9856403c3d4e6f7ea2fbb6a6891900816f6b78b297e3ecf03cb43977665afca`. Reviewed PR #59 head `f17b8c368b22266d7247185595f6b7f2ad62b03b`. Confirm actual merge and clean baseline before v0.9.
- Scope based on Grok's c-compiler-review-4e69525.zip NEW-FEATURES.md sections 12c and 12d, declared target support, and hcc-challenge-v08-report.zip REPORT.md and ci metadata. Those sources document Linux libc delegation, macOS/Windows fallback threads runtime, documented atomic order strengthening and supported atomic sizes, and v0.8 bit-field/alignment repair.
