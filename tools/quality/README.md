# Quality tools (step 8)

Measurement scripts for the quality phase. They are run by hand and not in CI. Results go under `reports/quality/`; the November 2026 baseline and its tool versions are in `reports/quality/baseline-2026-11/README.md`.

| Script | What it does |
|---|---|
| `coverage.sh` | Line and function coverage with cargo-llvm-cov: unit tests, then an instrumented compiler under the suites (`SUITES="tests wacc torture"`), merged. |
| `lcov_summary.py` | Per-file coverage of non-test code from one or more lcov files, and the largest uncovered regions. |
| `complexity.py` | Function length, nesting and approximate cyclomatic complexity (used because rust-code-analysis-cli does not build on the box). |
| `crash_points.py` | Every `unwrap`/`expect`/`panic!`/`unreachable!`/`todo!`/`unimplemented!`/`assert!` in non-test code, as a categorised CSV. |
| `panic_fuzz.py` | Feeds damaged C files to the compiler and records any panic, crash or hang, with a reproducer. |
| `check_missed.py` | Re-tests mutants the unit tests missed against `harness/run_tests.py`. |
| `rust_scan.py` | Shared helper: blanks comments and strings and finds `#[cfg(test)]` code. |

Tests: `python3 -m unittest tools/quality/test_quality_tools.py`. Exact commands are in the baseline README, section 6.
