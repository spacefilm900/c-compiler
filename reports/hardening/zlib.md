# zlib under hcc (hardening step 14)

zlib 1.3.1 is a test subject: hcc builds it and the result is compared
with a Clang build of the same sources. Nothing from zlib goes into hcc.

## Input

| Input | Source | SHA-256 |
| --- | --- | --- |
| `zlib-1.3.1.tar.gz` | GitHub release `v1.3.1` (fallback `zlib.net/fossils/`; zlib.net keeps only the newest release at its top level) | `9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23` (the published sum) |

`scripts/fetch_zlib.sh` puts it in `tests/external/zlib/` (gitignored) and
checks the sum. The tree is not patched.

## What the harness does

`harness/run_zlib.py` (unit tests: `tools/test_run_zlib.py`) builds the
library (15 files), `test/example.c` and `test/minigzip.c` with
`-std=gnu11 -D_LARGEFILE64_SOURCE=1 -DHAVE_HIDDEN`, as the Makefile's Linux
configuration does, once with hcc (hcc to IR, Clang IR to object) and once
with Clang, at each level. Then:

1. zlib's own tests as `make test` runs them: `example` (exit 0 and the same
   output as Clang's build) and `echo hello world | minigzip | minigzip -d`.
2. A round trip over a deterministic corpus (empty, one byte, 1 MiB of
   zeros, word-like text, random bytes, runs, a skewed distribution, a
   mixture, hcc's own Rust sources, `zlib.h`), compressed with `-1`, `-6`,
   `-9`, `-9 -f` (filtered), `-1 -h` (Huffman only) and `-r` (run length):
   both builds must write identical gzip streams, and each build must
   inflate the other's stream back to the original.

## Results

Local run on x86-64 Linux (Clang 19.1.7): `-O0` and `-O2` both full
builds (17/17 files), `example` matching, the `minigzip` pipe OK, and 60/60
round-trip cases identical and inverse at both levels. The harness takes
about 11 s.

## hcc bug zlib found

| Found as | Fix | Test |
| --- | --- | --- |
| `minigzip.c` failed to compile under `-std=gnu11`: `fileno` undeclared. hcc defined `__STRICT_ANSI__` in every mode, so glibc hid its POSIX declarations from GNU-mode programs. The GCC manual defines `__STRICT_ANSI__` only for `-ansi` or a strict `-std`. | defined only for `-std=c11` | `tests/gnu/22_no_strict_ansi.c` (an `#error` under `__STRICT_ANSI__`, so strict C11 rejects it), `pp::tests::strict_ansi_is_defined_only_for_strict_iso_c` |

Regression check of that change: harness 209/209, stage dumps 24/24, IR
checks on all targets, AArch64 209/209, Apple arm64 207 + 2 known, Intel
macOS 205 + 2 known, Lua and SQLite PASS at both levels, WACC 1,384, GCC
torture 1,378 passed with no unexpected result, the challenge kits 490/490.

## CI

`ci.yml` has a Linux-only `zlib` job (10-minute cap) that caches the
tarball keyed on its sum and runs `run_zlib.py --opt O0 --opt O2`.
