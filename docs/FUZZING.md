# Fuzzing and random-program testing

Step 14 of `PLAN.md` (hardening). None of this runs in pull-request CI.
It runs locally, or by hand through the manual-only `fuzz` workflow, so
it costs nothing unless someone starts it.

All the tools here are used as black boxes. Their source is not read or
adapted into this compiler: cargo-fuzz and libFuzzer drive the fuzz
targets, Csmith and YARPGen generate programs, and Clang (and GCC for
Csmith) are the reference compilers.

## Fuzzing the compiler itself (cargo-fuzz)

The `fuzz/` crate is separate from the compiler's build: it has its own
`Cargo.toml` and workspace, and needs a nightly toolchain, while the
compiler stays on the stable toolchain in `rust-toolchain.toml`.

| Target | What it runs on each input |
| --- | --- |
| `lex` | `lexer::lex` |
| `preprocess` | `pp::preprocess` with the default options (host headers) |
| `parse` | preprocessing and parsing (`dump_stage(.., Stage::Ast)`) in `-std=c11` and `-std=gnu11` |
| `frontend` | the whole front end to LLVM IR, in both dialects |
| `roundtrip` | the front end on the input and on its `-E` output; the two IR modules must be identical |

The oracle: no panic, no hang (libFuzzer's `-timeout`, 10 seconds), no
internal compiler error, and with `HCC_FUZZ_VERIFY=1` the `frontend`
target runs `llvm-as` (or `$LLVM_AS`) on every module the compiler
accepts, so invalid IR from an accepted input is a failure. `roundtrip` adds its own check: when an
input compiles, its preprocessed text must compile to the same IR. An ordinary
error diagnostic for an invalid program is the expected result, not a
failure. Inputs over 64 KiB, inputs that are not UTF-8, and inputs that
mention `/dev`, `/proc` or `/sys` are skipped: an `#include` of a device
file would hang for a reason that is not a compiler bug.

By default `#include <...>` finds only this compiler's own headers
(`include/`), so an input naming `<stdio.h>` fails at once instead of
spending most of a second in the C library's headers; that made the
targets about twenty times faster. `HCC_FUZZ_HOST_HEADERS=1` searches the
host's headers as well.

Setup, once:

    rustup toolchain install nightly --profile minimal
    cargo +nightly install cargo-fuzz --locked
    # libFuzzer is C++ (needs g++ or libstdc++ headers); llvm-as is the
    # verifier for HCC_FUZZ_VERIFY=1 (Debian/Ubuntu package names)
    sudo apt-get install -y g++ llvm
    python3 tools/fuzz_seed.py        # seed fuzz/corpus/* from tests/

Run one target for an hour, continuing past failures so every distinct
one is collected in `fuzz/artifacts/<target>/`:

    tools/fuzz_run.sh parse 3600
    HCC_FUZZ_VERIFY=1 tools/fuzz_run.sh frontend 3600

Reproduce and minimise a failure:

    cargo +nightly fuzz run -O parse fuzz/artifacts/parse/crash-...
    cargo +nightly fuzz tmin -O parse fuzz/artifacts/parse/crash-...

Leak detection is off. Record types are reference counted and a record
can point at itself, so a compilation keeps them until the process exits.
In a compiler process that is not a bug, but LeakSanitizer reports it for
every input that declares such a struct. A fuzz worker does keep that memory
until it exits; `tools/fuzz_run.sh` uses `-fork=1`, so each worker runs one
short job and is replaced, and `-rss_limit_mb=4096` would still stop a run
that grew without bound (the first campaign had no out-of-memory exits).

Each failure gets a regression test in the ordinary test suite first
(usually a unit test next to the code, or a `tests/` program), then the
fix.

## Random programs at scale

### Csmith

`tools/csmith_campaign.py` (see its docstring) is the existing runner. CI
runs a small fixed batch; the hardening campaigns run thousands of seeds
locally in shards:

    python3 tools/csmith_campaign.py --compiler build/hcc \
        --seed-start 200001 --count 1500 --out /tmp/csmith-hardening \
        --discard-binaries

### YARPGen

`tools/yarpgen_campaign.py` builds each `yarpgen --std=c` program with
Clang at `-O0` and `-O2` as the reference, then with this compiler (IR
linked by Clang at `-O0` and `-O2`), and compares the printed checksums.
YARPGen is fetched and built separately (CMake); it is not vendored:

    git clone https://github.com/intel/yarpgen && cmake -S yarpgen -B yarpgen/build \
        -DCMAKE_BUILD_TYPE=Release && cmake --build yarpgen/build
    python3 tools/yarpgen_campaign.py --yarpgen yarpgen/build/yarpgen \
        --seed-start 1 --count 500 --jobs 4 --out /tmp/yarpgen-campaign

### Constant folding (own generator)

`tools/constfold_fuzz.py` writes one random arithmetic expression per
program (every integer type, `float` and `double`, casts, shifts,
division, `?:`, logical operators) and evaluates it twice: as a static
initializer, which this compiler folds, and at run time from `volatile`
copies of the constants. Clang builds the same program with
`-fsanitize=undefined`; programs with undefined behaviour or that Clang
does not accept as constant are discarded.

    python3 tools/constfold_fuzz.py --count 20000 --jobs 2 --out /tmp/constfold

### Record layout (own generator)

`tools/layout_fuzz.py` declares random structs and unions (bit-fields of
every integer type including zero-width ones, `_Alignas`, arrays, nesting;
with `--gnu` also `packed` and `aligned` on records without bit-fields)
and prints `sizeof`, `_Alignof`, `offsetof`, each member read from a byte
pattern, and the bytes after storing every member.

    python3 tools/layout_fuzz.py --count 5000 --out /tmp/layout [--gnu]

## Results

See `reports/hardening/README.md`.
