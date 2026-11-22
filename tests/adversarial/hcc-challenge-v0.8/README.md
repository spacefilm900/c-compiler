# HCC Adversarial Challenge v0.8
**Release date:** 21 November 2026. **Round:** second of at most three extension rounds (v0.7–v0.9).
**Reference baseline:** merged PR #58, `e2cf13f8af83795446608999d5b97385591d6056`.

This is a standalone, independent, clean-room test release. It does not replace or alter v0.6 or v0.7. No HCC source, patches or implementation advice are included. Prior releases must be run separately as regressions. The original expectations are fixed before candidate execution.

## Inventory

| Group | Cases |
|---|---:|
| Nested layout/over-alignment | 16 |
| Mixed-TU aggregate ABI | 12 |
| Atomic operations | 12 |
| VLA / variably modified types | 12 |
| Threads and synchronisation | 8 |
| GNU interactions | 8 |
| Required diagnostics | 8 |
| K&R / real tgmath interactions | 4 |
| **Native total** | **80** |

72 runtime and 8 diagnostic cases; 40 portable C11, 32 documented target contract and 8 GNU-contract cases. There are six supplementary strict-mode rejections and twelve ABI cases with both mixed-TU directions. The alignment and ABI cases use parameterised patterns, so individual cases do not represent independent defect families. Separately: six fixtures across six target families = 36 target/fixture cells, an optional repeated concurrency run, and one non-gating oracle disagreement probe.

## Run on native x86-64 Linux

Use Python 3, GCC, and Clang. For HCC use the project's supported LLVM >=18 (the local independent reference checks used Clang 17 only as a C oracle). Build HCC exactly as instructed by the project. Absolute executable paths are recommended. The runner uses `hcc -std=... INPUT.c -o OUTPUT.ll`, then Clang assembles IR to objects at O0/O2. The native environment uses `-pthread` for C11 threads and `-lm`; the candidate is deliberately **not** linked with libatomic, consistent with the stated standalone lock-free lowering contract. Reference compilers may link libatomic for aggregate atomic operations. The strict C11 reference compilation uses `-pedantic-errors`.

```
python3 runner.py --engine gcc --opt O0 --out /tmp/v08-gcc-O0
python3 runner.py --engine clang --opt O0 --out /tmp/v08-clang-O0
python3 runner.py --engine hcc --compiler /absolute/path/to/build/hcc --opt O0 --out /tmp/v08-hcc-O0
python3 runner.py --engine hcc --compiler /absolute/path/to/build/hcc --opt O2 --out /tmp/v08-hcc-O2
```

All output destinations must be new directories. `--case ID` (repeatable) selects a partial run; never present a partial run as complete. `--abi-reference gcc` runs complementary C translation units through GCC while still using Clang for the emitted HCC IR and linking. Default mixed ABI reference is the linker Clang. Both directions are part of each selected mixed case's verdict. See `HANDOVER_TO_GROK.md` for the freeze protocol.

## Separate platform matrix and stress checks

```
python3 cross_target.py --engine clang --opt O0 --out /tmp/v08-cross-clang-O0 --run-native-linux
python3 cross_target.py --engine hcc --compiler /absolute/path/to/build/hcc --opt O0 --out /tmp/v08-cross-hcc-O0 --run-native-linux
python3 mixed_reference.py --out /tmp/v08-ref-mixed
python3 stress_threads.py --run-dir /tmp/v08-hcc-O2 --out /tmp/v08-hcc-stress --repetitions 20
```

Repeat cross-target commands for O2. The target runner compiles freestanding sources to real target object formats, records candidate emitted IR triples and data layouts, checks target family and assembles with Clang, without replacing the triple with a proxy. `--run-native-linux` additionally links and executes only the six matching x86-64 Linux cells. Other targets are **object-only** unless executed separately on appropriate environments; do not infer portability or ABI correctness from compile success. The aggregate-call fixture has separate TUs so native execution can detect a real calling convention mismatch. Mac deployment versions are explicit (Apple Silicon 11.0 and Intel 10.15). These fixtures require no host headers. If a target object run fails due to a missing backend or toolchain, classify it as an evidence gap pending reproduction. See `PLATFORM_PROTOCOL.md`.

The stress runner repeats existing binaries, uses the same expectations, records each executable hash and original run hash, and writes a new output directory. It does not alter the original first-run verdict or silently skip unavailable binaries. Failures must be retained. Neither stress counts nor platform cells are added to the curated 80-case denominator.

## Integrity and evidence

From the release directory, `sha256sum -c SHA256SUMS.txt`. The manifest excludes itself. Reference JSON captures actual commands, return codes, streams and relevant versions/hashes. Some command paths refer to the creator's temporary workspace; they are historical evidence, not portable command paths. No HCC candidate execution has occurred. See `VALIDATION.md`, `BASELINES.md`, and `CASE_INDEX.md`. No expected-output regeneration is permitted without a separately adjudicated successor version.
