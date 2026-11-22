# HCC adversarial challenge v0.9 — final extension round

This is the third and final planned additional verification round after the frozen v0.6 closeout. Tests and tooling only; no HCC patches. Target baseline is the **merged PR #59 revision**, which must be identified by Grok before the first run. The latest reviewed PR head is f17b8c368b22266d7247185595f6b7f2ad62b03b; the supplied v0.8 report records it as open, not merged. If it has not merged, finish that decision first. Do not confuse PR head and merge commit.

## Contents

- 100 new curated cases: 27 thread API/TLS/TSS/once cases, 16 atomics/memory ordering, 12 concurrent application/interaction cases, 20 layout/ABI, 10 VLA/GNU, 15 additional atomics/diagnostics. 92 executable cases, 8 required-diagnostic cases; five additional GNU strict-mode checks. Nine two-TU cases (one external TLS, eight aggregate ABI). Some cases are parameter sweeps, not independent defect scenarios.
- `runner.py`: native Linux x86-64 or AArch64 (host HCC IR + Clang backend); outcomes for mixed objects are included in each two-TU case. This version was reference-executed on x86-64 only. No other native host claim.
- `mixed_reference.py`: reference cross-compiler combinations, same two Linux architectures.
- `stress_threads.py`: 33 marked cases, repeated without rebuilding or changing the first-run tree. Optional repeatable `--case` filter; no silent skips.
- `generated_campaign.py`: deterministic parameterized workload pairs (inline expression vs extracted helper), independent expected result, isolated generated corpus; seed records and outcomes preserved. This is a focused family, not Csmith or broad random fuzzing.
- `cross_target.py`: 4 fixtures x 6 target families; object compilation plus optional matching native Linux execution.
- `ir_inventory.py`: inventories candidate IR atomics, fences, TLS and target layout for manual auditing; not a correctness oracle.
- `CASE_INDEX.md`, `VALIDATION.md`, `PLATFORM_PROTOCOL.md`, `HANDOVER_TO_GROK.md`, `FINAL_FREEZE_PROTOCOL.md`, `BASELINES.md`, `reference-results/`, `SHA256SUMS.txt`.

All 869 historical curated tests remain in their own immutable releases; this adds 100, for 969 distinct case IDs in the cumulative inventory. None of the supplementary runs count toward that denominator. All candidate verification remains to be executed by Grok. Treat standards probes, runtime availability, resource limits and implementation-defined contracts separately.

Quick example (fresh output directories required):

```
sha256sum -c SHA256SUMS.txt
python3 runner.py --engine gcc --opt O0 --out /tmp/v09-gcc-O0
python3 runner.py --engine hcc --compiler /absolute/path/to/hcc --linker clang --opt O2 --out /tmp/v09-hcc-O2
python3 stress_threads.py --run-dir /tmp/v09-hcc-O2 --out /tmp/v09-stress-O2 --repetitions 100
python3 generated_campaign.py --engine hcc --compiler /absolute/path/to/hcc --opt O2 --seeds 100 --out /tmp/v09-generated-O2
python3 cross_target.py --engine hcc --compiler /absolute/path/to/hcc --opt O2 --out /tmp/v09-cross-O2 --run-native-linux
python3 ir_inventory.py --run-dir /tmp/v09-hcc-O2 --out /tmp/v09-ir-O2.json
```

See handover for complete protocol. Do not run generated workloads with untrusted inputs beyond this deterministic generator. The tools enforce per-command timeouts; thread creation/resource failures must be triaged rather than silently retried or scored as compiler defects.
