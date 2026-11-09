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
