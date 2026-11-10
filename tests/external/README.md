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

CI runs `harness/csmith_seeds.txt`. `python3 harness/run_csmith.py --count 500` is a larger local batch and is not part of CI. Options and the checksum/timeout/ulimit wrapper are in `harness/run_csmith.py`. Failures that remain are listed in `harness/csmith_known_failures.txt`.

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

- Independent POSIX AWK Challenge: https://github.com/spacefilm900/awk-posix-challenge tag `v0.6`, commit `69b1f5895022956c5ebfc0d6d58fd45636b6dfaa`
- Shakespeare volume test: https://github.com/spacefilm900/awk-volume-tests commit `46701e477fe48a7e0592b8022c77ac29c4114582`

```sh
scripts/fetch_awk_posix_challenge.sh
scripts/fetch_awk_volume.sh
scripts/build_onetrueawk.sh clang build/otawk-clang
scripts/build_onetrueawk.sh ccomp build/otawk-ccomp
AWK_CLANG=$PWD/build/otawk-clang/awk AWK_CCOMP=$PWD/build/otawk-ccomp/awk python3 harness/run_awk_extra.py
```

`AWK_EXTRA_HOOK` is an optional command run after those two suites. No other suite is invented here. If the private clones are missing, set `AWK_EXTRA_ALLOW_MISSING=1` to skip. CI does that when `gh` cannot read the other repositories.

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
