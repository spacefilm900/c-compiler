# Handover to Grok — v0.8

## 1. Freeze the first run

Start from main `e2cf13f8af83795446608999d5b97385591d6056` (PR #58 merged). If main has advanced, build the exact baseline first or document the deviation, including all intervening changes. Import v0.8 unchanged in a separate test tree. Verify SHA256SUMS.txt; record release archive hash, test import commit, source commit, clean working tree, toolchain versions and binary hash. Preserve all IR, objects, executable hashes where practical, terminal logs, per-case result JSON and compiler diagnostics. Do not change HCC or tests until the initial results have been frozen and committed.

## 2. Execute

Run the full 80-case Linux native suite at O0 and O2 using a supported backend. For each level report all six extra strict-mode checks and 12 mixed ABI cases in both directions (24 mixed configurations per level). Default mixed reference is Clang; also run with `--abi-reference gcc` for the 12 ABI case IDs (or full suite if convenient). Keep both per-reference results and the original 80-case result separate. Run GCC/Clang references on the actual host if possible, especially the current project Clang version. Do not conflate the local Clang 17 reference evidence with the project's Clang >=18 candidate backend requirement.

Run `cross_target.py` for HCC and Clang at O0 and O2, `--run-native-linux` on the normal host, preserving every target result, actual emitted triple, data layout, and object-generation status. Inspect any triple-normalisation warnings (record but do not silently dismiss a materially changed target). For available native/emulated targets follow `PLATFORM_PROTOCOL.md`, with a separate target/runtime/ABI evidence table. Do not turn a missing platform into a fake pass. Preserve actual runtime, link and mixed results separately from object-only checks. Run `stress_threads.py` on the frozen native O2 run for 20 repetitions per concurrency case; also run O0 if practical. Record missing cases and timeouts. Do not use repeat-to-pass or drop failures.

Run the original unchanged v0.7 suite (65 cases plus its auxiliary gates), and original unchanged v0.6 suite (724 cases), at O0 and O2. Prior evidence remains immutable; new results are separate regression runs. The v0.6 runner's standard command is `python3 runner.py --compiler /absolute/path/to/build/hcc --linker clang --reference gcc --abi-direction both --opt O0 --output /new/path/v06-O0.json` (repeat O2). Use each release's own frozen archive and runner, not a rewritten combined test list. Run existing internal/torture/application/CI suites; distinguish known/unsupported and unexpected failures.

Optionally revisit both non-gating atomic-pointer probes (v0.7 compound assignment and v0.8 library fetch-add/subtract). Record candidate and actual current GCC/Clang outputs, not simply pass/fail, including generated IR for any disagreement. The v0.8 probe retains the original draft expectation and raw divergent reference evidence. Do not use either probe as a gate before independent adjudication.

## 3. Freeze and return evidence before repairs

Commit/archive all initial outcomes first; then analyse. Group unexpected failures by underlying defect. Classify as wrong code, invalid IR, required diagnostic omitted, valid-program rejection, ABI mismatch, link/harness/platform/toolchain failure, reference disagreement, pre-existing documented restriction, or inconclusive timeout. Provide minimal reproductions and cite N1570, documented GNU contract or appropriate platform ABI. Include exact stage/commit IDs and manifest inventory excluding the manifest itself. Return first-run package and human-readable summary for independent review. No changes to release cases or expected results. No silent exclusion of failing cases.

After independent adjudication, repair HCC only, tests first and by root-cause commit. Re-run all new tests, affected platform/runtime checks, v0.7 and v0.6, plus established CI gates. Keep the starting baseline and all original failures. Return post-fix evidence. No HCC patches or proposed implementation are in this release.

## 4. Decision after v0.8

This is extension round two, not a reopening of the original v0.6 closeout. Decide on the optional v0.9 only after reviewing results. If new wrong-code, ABI portability or concurrency defects emerge, a targeted final round may be warranted. Otherwise prefer closeout with explicit remaining feature and evidence limits. Do not claim full C11 certification from finite passing tests. The maximum number of extension rounds remains three.
