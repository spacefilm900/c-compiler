# Release 1.0: rerun on the release-candidate commit

Every suite was rerun on one commit of the pre-tag pull request (#77),
with one build of hcc. The results below are recorded against that commit.

- **Commit tested:** `2e80ec02e3be1781546f35ba76e13c0cfc1fd1cd` (`2e80ec0`).
- **Run:** 25 November 2026, 10:47 to 12:08 UTC, from a clean worktree of
  that commit.
- **Host:** x86-64 Linux: Debian 13, glibc 2.41, 8 CPUs.
- **Toolchain:** Rust 1.85.1, Clang and LLVM 19.1.7 (the oracle and the
  linker), GCC 14.2.0 (the second reference for Csmith and the v0.6 kit).

The commit that adds this page changes only documentation: this report,
`README.md` and `STATUS.md`. It changes no source, harness or test file,
so these results hold for the pull request's final head. Run
`git diff --stat 2e80ec0 HEAD` to check.

## Results

| Suite | What it counts | Result |
| --- | --- | --- |
| Unit tests | `cargo test --lib` | 366 passed |
| Differential harness | the programs in `tests/`, hcc against Clang | 230 of 230 |
| Stage dumps | the golden dump tests | 24 of 24 |
| C11 conformance probes | 54 programs and 234 constraint or syntax violations (`docs/C11_CONFORMANCE.md`) | 286 of 288. The 2 failures are the known 6.7.4p3 gap. All 20 `[policy]` cases are diagnosed too. |
| Documentation claims | `tools/check_docs.py`: 11 claims compiled in both modes, and 6 stale phrases | 0 problems |
| Writing a C Compiler | chapters 1 to 18 | 1,384 passed, 0 known failures, 233 skipped (chapters 19 and 20 inspect the book's assembly) |
| GCC torture (`execute`) | 1,746 executed | 1,412 passed, 334 listed known failures, 0 unexpected |
| DOOM | 8 demos, frame hashes, `-O0` and `-O2` | 39,209 of 39,209 frames identical at both levels |
| Lua 5.4 | its test suite and workloads, `-O0` and `-O2` | match at both levels |
| SQLite | the shell, with 10 workloads, `-O0` and `-O2` | match at both levels (output and database) |
| QuickJS | its tests, `-O0` and `-O2` | match at both levels |
| zlib | tests and 60 round trips, `-O0` and `-O2` | match at both levels |
| one true awk | its test directory and 31 bug-fix programs | both builds agree. Both builds show the same 9 golden-file mismatches, which come from the host environment. |
| Adversarial kit v0.6 | 724 cases, ABI pairs in both directions, GCC as the other compiler | 724 of 724 at `-O0`, 724 of 724 at `-O2` |
| Adversarial kit v0.7 | 65 cases, kit's own runner | 65 of 65 at `-O0` and at `-O2` |
| Adversarial kit v0.8 | 80 cases | 80 of 80 at `-O0` and at `-O2` |
| Adversarial kit v0.9 | 100 cases | 100 of 100 at `-O0` and at `-O2` |
| Csmith | 3,000 programs generated (seeds 200001 to 203000, CI size caps, `gnu11`) | `-O0`: 2,775 compared, 0 mismatches. `-O2`: 2,774 compared, 0 mismatches, and 1 hcc-build timeout that matched when re-run alone. |
| YARPGen 2 | 2,000 programs generated (seeds 1 to 2000) | 1,651 compared, 0 mismatches, and 2 hcc-build timeouts that matched when re-run alone |

### Programs not compared, and timeouts

- **Csmith.** A program is compared only when GCC and Clang both build it,
  run it, and agree.
  - At `-O0`, 225 were not compared: 223 reference crashes (mostly the
    CPU limit on long-running programs) and 2 reference timeouts.
  - At `-O2`, 225 were not compared, all of them reference crashes.
  - The `-O2` candidate timeout was seed 201721. It agreed when re-run
    alone (`csmith-retry-201721`).
- **YARPGen.** 347 programs were not compared because the Clang `-O0`
  reference run failed. Some of these programs keep very large arrays on
  the stack, and each run is capped at 1 GiB of stack and 4 GiB of address
  space. The 2 candidate timeouts were seeds 741 and 749. Both agreed when
  re-run alone.
- **Load.** The generator campaigns ran six Csmith shards and two YARPGen
  jobs at once on 8 CPUs. All three timeouts came from that load. No
  result is a wrong-code finding.

The hardening campaign (`reports/hardening/`) ran the same Csmith seeds on
an earlier commit and compared the same counts, 2,775 at `-O0` and 2,774 at
`-O2`. Its YARPGen run compared 1,694. Different programs hit the
reference's limits in the two runs, which is why the YARPGen figure
differs.

### Superseded runs

- **`597889f`.** A rerun started on this commit. Every suite and kit
  passed, but CodeRabbit's second review then changed the source (the
  nested `_Generic` register check), so the rerun was repeated in full on
  `2e80ec0`. Two of its results came from load and passed when repeated:
  - QuickJS: the Clang reference build's `os.exec` test failed under load.
  - The v0.6 kit at `-O0`: two link timeouts.
- **`1620a32`.** A YARPGen run on this commit, the first head of the pull
  request, agreed on every program it compared.

## Not rerun here

- Real arm64 Linux, macOS and Windows machines. They run in the weekly
  real-platform workflow (`docs/WEEKLY_PLATFORMS.md`).
- The emulated targets: arm64 under qemu, and Windows under Wine. Pull
  request CI runs them on every head, including this one.
- Fuzzing, coverage and mutation testing. Their results are in
  `reports/hardening/` and `reports/quality/`, measured on earlier commits.
- QuickJS's test262 run. Pull request CI runs it.

## Reproducing

```sh
git worktree add rc 2e80ec0 && cd rc   # with tests/external fetched or linked
make
python3 harness/run_tests.py
python3 harness/run_conformance.py
python3 harness/run_wacc.py
python3 harness/run_gcc_torture.py     # and run_doom, run_lua, run_sqlite,
                                       # run_quickjs, run_zlib, run_onetrueawk
python3 tests/adversarial/hcc-challenge-v0.9/runner.py --engine hcc \
    --compiler build/hcc --linker clang --opt O2 --out /tmp/v09
python3 tools/csmith_campaign.py --compiler build/hcc --seed-start 200001 \
    --count 3000 --timeout 8 --out /tmp/csmith
python3 tools/yarpgen_campaign.py --compiler build/hcc \
    --yarpgen /path/to/yarpgen --seed-start 1 --count 2000 --out /tmp/yarpgen
```

The v0.6 kit (724 cases) is not in the tree; see
`tests/adversarial/README.md`.
