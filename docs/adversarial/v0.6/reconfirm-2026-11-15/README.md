# v0.6 re-confirmation, 15 November 2026

The unchanged C11 Challenge v0.6 (archive SHA256 `07e26e6c29a48f1a605d4cc1da38d53db10f424689b95b9dc17030a63b7ff182`) was rerun on `main` at `0c43e32` (the merge of PR 38).

- Compiler binary SHA256: `b68fcd985207576b1846203162608eaa521c5d0062936255c8b289fa471a9ee4` (built with `make` from a clean detached worktree)
- Host: x86_64 Linux, rustc 1.85.1, Debian clang 19.1.7, GCC 14.2.0, Python 3.13.5

| Run | Command (from the challenge directory) | Result |
|---|---|---|
| O0 | `python3 runner.py --compiler <compiler> --linker clang --reference gcc --abi-direction both --opt O0` | 724/724, 0 gating failures |
| O2 | same with `--opt O2` | 724/724, 0 gating failures |
| Metamorphic | `python3 tools/metamorphic_candidate.py --compiler <compiler> --opt O2 --limit 60` | 60/60 |
| IR verify | `python3 tools/ir_verify_candidate.py --compiler <compiler> --limit 150` | 150/150 |

Files: the runner logs (`O0.log`, `O2.log`), the full per-case results compressed with gzip (`results-O0.json.gz`, `results-O2.json.gz`), and the outputs of the metamorphic and IR checks (`metamorphic.json`, `irverify.json`).
