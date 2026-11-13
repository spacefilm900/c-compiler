# External test suites

These suites are not vendored. A script fetches a pinned commit into a gitignored directory, and CI caches that directory.

## Writing a C Compiler

Test programs for Nora Sandler's book [Writing a C Compiler](https://nostarch.com/writing-c-compiler).

- Source: https://github.com/nlsandler/writing-a-c-compiler-tests
- Pinned commit: `ae12014d2dec14488f3f80d14df4b6d8e4634d7d` (2026-02-17, "Return a unique value for the last test")
- Licence: MIT, Copyright (c) 2023 Nora Sandler

The MIT licence permits copying and redistribution. The suite is still fetched rather than committed: it is thousands of files, and the pin plus the cache is enough to reproduce a run. The fetched tree keeps this licence. It is not relicensed with this repository.

```
MIT License

Copyright (c) 2023 Nora Sandler

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Fetch and run (from the repository root, after `make`):

```sh
scripts/fetch_wacc_tests.sh
python3 harness/run_wacc.py
```

`scripts/fetch_wacc_tests.sh` clones the pinned commit into `tests/external/wacc/`. `harness/run_wacc.py` is this repository's runner. It does not call the suite's `test_compiler` script. Chapters and skips are recorded in `STATUS.md`. Failures that are not fixed yet are listed in `harness/wacc_known_failures.txt`.

## Csmith

Random C programs, used only as differential inputs. The generator is built from a pinned commit and is not vendored.

- Source: https://github.com/csmith-project/csmith
- Pin: `30dccd73b78652c4719f36572994778a5b233a4e` (tag `csmith-2.3.0`)
- Licence: the fetched `COPYING` (University of Utah; redistribution permitted, see that file)

```sh
scripts/fetch_csmith.sh
python3 harness/run_csmith.py
```

CI runs `harness/csmith_seeds.txt` through `harness/run_csmith.py`. That runner compiles the candidate with `-std=gnu11` and the Clang reference with `-std=gnu11 -O0`, and it links both binaries with `-lm`. A local batch is `python3 harness/run_csmith.py --count 500` and is not part of CI. Options and the checksum/timeout/ulimit wrapper are in `harness/run_csmith.py`. Failures that remain are listed in `harness/csmith_known_failures.txt`.

For a large coverage campaign, use `tools/csmith_campaign.py`. The default mode is `gnu11`: the candidate is given `-std=gnu11`, and GCC and Clang run in their default GNU mode, without `-pedantic-errors`. Every executable, at both `-O0` and `-O2`, is linked with `-lm`. Csmith is given the same size caps as the CI runner, plus `--no-packed-struct` and `--no-argc`, so programs usually finish in seconds and layouts stay comparable. Shards can share one `--out` directory with different `--seed-start` and `--count` values; a completed seed is skipped if the same range is run again. `--summarize DIR` rebuilds `SUMMARY.json` from the seed tree.

```sh
python3 tools/csmith_campaign.py \
  --compiler build/compiler \
  --seed-start 1 --count 500 \
  --opt O0 --opt O2 \
  --out /tmp/csmith-coverage
```

`--mode strict` compiles the candidate as default C11 and the references with `-std=c11`. Tests for the command lines, the `-lm` fix, and the classification are in `tools/test_csmith_campaign.py`.

## One True AWK

The flagship real program. The tree is fetched, not vendored. `bison -d` produces the parser the way the upstream makefile does. `maketab` is compiled with the host `cc` and writes `proctab.c`.

- Source: https://github.com/onetrueawk/awk
- Pin: `5739fd79bcfc75ba7526773d0cf634521f8aca3c` (tag `20260426`)
- Licence: the Lucent notice at the top of `makefile` in the fetched tree

```sh
scripts/fetch_onetrueawk.sh
python3 harness/run_onetrueawk.py
```

Both builds use `-O0`. Clang also gets `-std=c11`. Both pass `-D_DEFAULT_SOURCE`.

## AWK Builder suites

These repositories are private. They are not vendored. `cases/` is not modified. Result JSON is written under `tests/external/awk-results/`.

- Independent POSIX AWK Challenge: https://github.com/spacefilm900/awk-posix-challenge annotated tag `v0.6`. The tag object is `69b1f5895022956c5ebfc0d6d58fd45636b6dfaa`. The pin is the commit that tag points at, `b46ea063c6d25372b6a2010ac537ca19a8fd1b1e`, which is `HEAD` after checkout.
- Shakespeare volume test: https://github.com/spacefilm900/awk-volume-tests commit `46701e477fe48a7e0592b8022c77ac29c4114582`

```sh
scripts/fetch_awk_posix_challenge.sh
scripts/fetch_awk_volume.sh
scripts/build_onetrueawk.sh clang build/otawk-clang
scripts/build_onetrueawk.sh ccomp build/otawk-ccomp
AWK_CLANG=$PWD/build/otawk-clang/awk AWK_CCOMP=$PWD/build/otawk-ccomp/awk python3 harness/run_awk_extra.py
```

`AWK_EXTRA_HOOK` is an optional command run after those two suites. No other suite is invented here. If the private clones are missing, set `AWK_EXTRA_ALLOW_MISSING=1` to skip. CI does not set that. It skips the suites only when the `AWK_SUITE_READ_TOKEN` secret is unset, and fails the job when that secret is set and a fetch, build, or suite fails.

## GCC torture execute tests

Test programs only, from `gcc.c-torture/execute` at GCC 15.2.0. The fetch is a non-cone sparse checkout of that directory. Compiler sources are not checked out and are not read. The tests are not vendored.

- Source: https://github.com/gcc-mirror/gcc
- Pin: `5115c7e447fc07457443df874bf57840e8316d5f` (tag `releases/gcc-15.2.0`)
- Licence: the GCC testsuite's own licence, in the fetched tree. It is not relicensed here.

```sh
scripts/fetch_gcc_torture.sh
python3 harness/run_gcc_torture.py
```

Failures that remain are listed in `harness/gcc_torture_known_failures.txt`. `builtins/` is skipped: GCC's harness links each of those tests with a companion file and `lib/main.c`, and they target compiler builtins.

## DOOM (doomgeneric and Freedoom)

The first big program (PLAN.md step 5). Both inputs are fetched, not vendored, and are test inputs only: no DOOM code is copied into the compiler, and the tree is not patched.

- doomgeneric: https://github.com/ozkl/doomgeneric, pin `e257a09527062bfb986b6e5e0780839b6cbd5b71` (2026-10-11). Licence: GPL-2.0, in the fetched tree.
- Freedoom 0.13.0: https://github.com/freedoom/freedoom/releases/download/v0.13.0/freedoom-0.13.0.zip, zip SHA-256 `3f9b264f3e3ce503b4fb7f6bdcb1f419d93c7b546f4df3e874dd878db9688f59`, `freedoom1.wad` SHA-256 `7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d`. Licence: BSD-style, `COPYING.txt` kept beside the WAD. The WAD's `DEMO1` (vanilla 1.9 demo format) is the input.

```sh
scripts/fetch_doomgeneric.sh
scripts/fetch_freedoom.sh
python3 harness/run_doom.py                 # -O0 and -O2, first 2,100 frames
python3 harness/run_doom.py --frames 7100 --jobs 1 --json out.json --scorecard out.md
```

The headless backend (`harness/doom/dg_headless.c`) and the fixed-address allocator (`harness/doom/dg_alloc.c`) are this repository's own code. How the run is made deterministic, the graded per-file, mixed, and full builds, and the scorecard are described in `harness/run_doom.py` and `STATUS.md`. Scorecards are in `reports/doom/`.
