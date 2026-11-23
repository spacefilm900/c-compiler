# Hardening notes (working state; not for merge)

Branch `hardening/fuzz`, worktree /workspace/cc-fuzz, off main 90e83d9.

## Box setup done
- rustup nightly (default toolchain kept at 1.85.1), cargo-fuzz 0.13.2, g++ (libFuzzer needs libstdc++ headers), cmake.
- PATH needs `$HOME/.cargo/bin` for rustup/cargo-fuzz.
- Csmith copied from /workspace/cc-v06/tests/external/csmith (pin 30dccd7).
- YARPGen built at /workspace/tools-ext/yarpgen/build/yarpgen (commit a65fa1d), black box only.
- Frozen compiler for campaigns: /workspace/cc-fuzz-logs/hcc-90e83d9.

## Running
- Fuzz logs: /workspace/cc-fuzz-logs/<target>-N.log; corpora fuzz/corpus/, artifacts fuzz/artifacts/.
- Csmith shards: seeds 200001-203000 into /workspace/csmith-hardening/default (resumable).
- YARPGen: /workspace/yarpgen-hardening/results.jsonl (resumable; failing seeds in fail/).
- Run YARPGen with --jobs 1 while fuzzers run: its programs can use 3+ GB and the box has 16 GB.

## Findings so far
- LeakSanitizer: record types leak (Rc cycles through self-referential structs). Not a bug for a compiler process; leak detection disabled.
- libFuzzer "timeouts" under heavy load (load 14) did not reproduce (all < 1.3 s under ASan); discarded.

## Status at 15:08 (Europe/London)
- Part A: PR 62 open (plan/roadmap-refresh, head 22ceef7), all CI green, CodeRabbit "Review completed", no actionable comments. Awaiting Keith's merge OK. Do not merge.
- Fuzz round 2 started 14:31, 4500 s per target (5 processes: lex, preprocess, parse, frontend, frontend+HCC_FUZZ_VERIFY). Round 1 was ~7 min x 5 (wasted on leak noise).
- At ~14:48 the fuzz targets switched to compiler-only headers (common::options); exec/s went from ~15 to ~200 and coverage from 22k to 36k.
- "timeout-*" and "slow-unit-*" artifacts all re-run in < 0.5 s: the VM pauses ("virtual machine fork" in dmesg). Discard after rechecking.
- Next: when round 2 ends, recheck artifacts, run another round, then write reports/hardening/README.md, open draft PR.
- Round 3 queued via /workspace/cc-fuzz-logs/round3.sh (parse 90m, preprocess 60m, frontend+verify 90m, frontend host headers 60m); roundtrip-3 running 90m.
- Csmith big-program shards (no CI caps, seeds 300001-300600) queued via /workspace/cc-fuzz-logs/csmith-big.sh into /workspace/csmith-hardening/big

## Status at 15:58 (Europe/London)
- PR 62: head 22ceef7, MERGEABLE, 12 checks pass + 2 expected skips, CodeRabbit check "completed success / Review completed", 0 review comments ("No actionable comments"). Waiting for Keith's OK; do not merge.
- Round 2 finished ~15:48: no crashes, no invalid IR. Round 3 running (started 15:48). roundtrip target added (IR of source == IR of its -E output); no false positives in trial.
- Differential: Csmith 1378+ seeds agree at O0 and O2 (only spurious timeouts from VM pauses; seed 202136 rechecked fine). YARPGen 604 agree; seed 585 timeout rechecked fine.
- Bugs found so far: none.
- constfold_fuzz.py (own generator) running seeds 1000-20999 -> /workspace/constfold-hardening, log constfold-1.log
- yarpgen seeds 1-1000 done (857 agree, 142 reference failed, 1 spurious timeout); 1001-2000 started
- 16:50 yarpgen restarted (resumable) with 1 GiB stack + 4 GiB address-space cap after reference programs hit the OOM killer at 9 GB
- layout_fuzz.py (own generator; struct/union/bit-field layout vs Clang) c11 and gnu, seeds 1000-5999, logs layout-*.log
