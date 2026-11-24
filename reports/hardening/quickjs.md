# QuickJS under hcc (hardening step 14)

QuickJS 2026-06-04 (Fabrice Bellard's JavaScript engine, about 80,000 lines
of GNU C) is a test subject: hcc builds it and the result is run against a
Clang build of the same sources. Nothing from QuickJS goes into hcc. Its
Makefile was read for build flags only.

## Inputs

| Input | Source | SHA-256 |
| --- | --- | --- |
| `quickjs-2026-06-04.tar.xz` | https://bellard.org/quickjs/ | `b376e839b322978313d929fd20663b11ba58b75df5a46c126dd19ea2fa70ad2a` |
| test262 `5c8206929d81b2d3d727ca6aac56c18358c8d790` (the Makefile's `TEST262_COMMIT`) | `codeload.github.com/tc39/test262/tar.gz/<commit>` | `d2bb9b620c0697ea1052ffb67cc77ad6a3bdb9ff0c9cfba95ac7f4f64b98014a` |

`scripts/fetch_quickjs.sh` downloads both into `tests/external/quickjs/`
(gitignored), checks the sums, and applies QuickJS's own
`tests/test262.patch` to test262, as `make test2-bootstrap` does.

## What the harness does

`harness/run_quickjs.py` (unit tests: `tools/test_run_quickjs.py`) builds
the Linux configuration (`-std=gnu11 -D_GNU_SOURCE
-DCONFIG_VERSION="2026-06-04"`, `-lm -lpthread -ldl -rdynamic`) twice per
level, once with hcc (hcc to IR, Clang IR to object) and once with Clang:
`qjsc`, `qjs` (with `repl.c`, generated from `repl.js` by the Clang
`qjsc`), `run-test262`, and the native modules `tests/bjson.so` and
`examples/point.so`. Then:

1. `qjsc -c` of `repl.js` and every `tests/*.js` by both `qjsc` builds:
   the generated C (the bytecode) must be identical.
2. The Makefile's `test` list (nine `tests/test_*.js`, `test_bjson.js`,
   `examples/test_point.js`) by both `qjs` builds, each with its own native
   modules: exit status, stdout and stderr must match byte for byte.
3. With `--test262 LEVEL`: `run-test262 -a -c test262.conf` (strict and
   sloppy) by both builds; the sets of failing tests must be equal.

## Results

Local run (x86-64 Linux, Clang 19.1.7, 8 threads), `python3
harness/run_quickjs.py --test262 O2` on this branch:

| Level | hcc build | `qjsc -c` identical | Makefile tests matching | test262 failing (hcc / Clang) |
| --- | --- | ---: | ---: | --- |
| `-O0` | full, 12/12 files | 15/15 | 11/11 | 60 / 60, same set (separate run) |
| `-O2` | full, 12/12 files | 15/15 | 11/11 | 58 / 58, same set |

test262: 83,558 test runs (strict and sloppy), 3,356 excluded and 6,000
skipped by QuickJS's `test262.conf`. At `-O2` both builds fail exactly the
58 tests upstream lists in `test262_errors.txt`. At `-O0` both fail two more,
the same two (`staging/sm/class/superPropChains.js`, stack overflow in both
modes, because unoptimised frames are larger). A full test262 run takes
about 20 s per build at `-O2` on 8 threads.

hcc compiles all twelve translation units in about 3 s (front end);
`quickjs.c` alone is about 60,000 lines.

Regression check of the fixes on the rest of the test set: harness 215/215,
stage dumps 24/24, the IR checks on all targets, AArch64 215/215, Apple
arm64 213 + 2 known, Intel macOS 211 + 2 known, Lua and SQLite PASS at
`-O0` and `-O2`, WACC 1,384 passed, the three challenge kits 490/490, and
GCC torture 1,412 passed (34 listed failures now pass and were removed from
`harness/gcc_torture_known_failures.txt`: 25 compile errors, 7 that needed
`asm("")`, one builtin, and `20030714-1.c`, a runtime failure fixed by the
enum change); AArch64 torture 1,414 passed with no failure outside the list.

## hcc bugs and gaps QuickJS found

Every fix has a test that fails without it: a differential test (hcc's
binary against Clang's, and for `tests/gnu/` also rejected under
`-std=c11`) and unit tests in `src/sema/tests.rs`
(`quickjs_gnu_extensions_and_their_limits`).

| # | Found as | Fix | Test |
| --- | --- | --- | --- |
| 1 | `<stddef.h>` redefined `offsetof` when glibc had already defined it | hcc's header `#undef`s it first | build of `quickjs.c` |
| 2 | An aligned typedef of an untagged struct whose size is not a multiple of the alignment (glibc `__pthread_unwind_buf_t`) was rejected | accepted; layout checked against GCC and Clang | build of `quickjs-libc.c` |
| 3 | GNU cast of a struct or union to its own type, `(JSValue)v`, rejected | accepted in gnu11 as a copy of the value | `tests/gnu/18_record_self_cast.c` |
| 4 | `typedef enum E E;` before the enumerator list rejected | GNU forward enum; taken to have the type a nonnegative enum gets, and the definition must agree | `tests/gnu/19_forward_enum.c` |
| 5 | A struct ending in a flexible array member as a member of another struct rejected | allowed in gnu11 (GCC manual, "Arrays of Length Zero") | `tests/gnu/20_zero_size_records.c` |
| 6 | A record of only zero-length arrays (size 0) rejected | allowed in gnu11, including `= {}` | `tests/gnu/20_zero_size_records.c` |
| 7 | Range designators `[a ... b] = v` unsupported | expanded per index at parse time; a value with side effects is rejected rather than evaluated more than once | `tests/gnu/21_range_designators.c` |
| 8 | `static` and qualifiers in a parameter's array brackets (`double fields[static 9]`) unsupported | C11 6.7.6.2p1 / 6.7.6.3p7: qualifiers go to the adjusted pointer; misplaced uses are diagnosed | `tests/161_array_param_qualifiers.c` |
| 9 | `asm` statements unsupported | statements without operands: `call void asm sideeffect` with the clobbers (and x86's implicit `dirflag`/`fpsr`/`flags`, as in Clang's IR for the same statement); operands and `asm goto` are diagnosed | `tests/gnu/both/07_asm_statements.c` |
| 10 | **Wrong code**: an enumeration with no negative values had compatible type `int`; GCC documents `unsigned int`, and Clang agrees on every hcc target except MSVC Windows. A 3-bit `JSClosureTypeEnum` bit-field read 4..7 back as negative, `js_closure2` hit `abort()` on the first script | the compatible type is `unsigned int` (MSVC: `int`); the constants stay `int` | `tests/162_enum_compatible_type.c` |

Number 10 changes behaviour that existing programs can observe (enum
bit-fields, `e - 1 > 0` for an enum object `e == 0`), now matching GCC and
Clang. The other nine add GNU extensions or C11 syntax that hcc rejected.

## CI

`ci.yml` has a Linux-only `quickjs` job (20-minute cap): it caches
`tests/external/quickjs` keyed on both pins and runs
`run_quickjs.py --opt O0 --opt O2 --test262 O2`.

CI_PLACEHOLDER
