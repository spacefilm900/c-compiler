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

The oracle: no panic, no hang (libFuzzer's `-timeout`, 10 seconds), no
internal compiler error, and with `HCC_FUZZ_VERIFY=1` the `frontend`
target runs `llvm-as` (or `$LLVM_AS`) on every module the compiler
accepts, so invalid IR from an accepted input is a failure. An ordinary
error diagnostic for an invalid program is the expected result, not a
failure. Inputs over 64 KiB, inputs that are not UTF-8, and inputs that
mention `/dev`, `/proc` or `/sys` are skipped: an `#include` of a device
file would hang for a reason that is not a compiler bug.

Setup, once:

    rustup toolchain install nightly --profile minimal
    cargo +nightly install cargo-fuzz --locked
    # libFuzzer is C++: Debian needs g++ (or clang with libstdc++ headers)
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
every input that declares such a struct.

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

## Results

See `reports/hardening/README.md`.
