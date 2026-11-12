# Design

Main decisions for this compiler, taken from `PLAN.md`, `STATUS.md`, `README.md`, `BOT.md`, and the code on `main`. A line that says "approved by the owner" uses the date of the owner's merge commit in the git history. `STATUS.md` and that history do not record a separate approval date for every sentence below. This file does not quote chat transcripts.

## Clean-room

Frontend code (lexer, preprocessor, parser, semantic analysis, IR generation) is original. It is written from the C standard, the LLVM Language Reference, and the System V ABI. Allowed references, from `PLAN.md`, are those documents, Kaleidoscope-style tutorials, and other public specs. For the GNU-compatibility work, the GCC user manual's public documentation of builtins and attributes is also allowed. Clang and GCC source are not copied, translated, or closely paraphrased. Clang is a black-box oracle in the test harness and the LLVM backend that links the emitted `.ll` file. The compiler does not call into Clang's or GCC's frontend.

Approved by the owner on 2026-11-09 (pull request 1). The rule is also in `PLAN.md` (repository since the initial commit on 2026-11-08) and `BOT.md`.

## Language

Rust, edition 2021, standard library only. `Cargo.lock` lists this package and no crates.io dependencies, so a checkout builds without fetching packages. `rust-toolchain.toml` pins Rust 1.85.1 with `rustfmt` and `clippy`. Rust was chosen as a break from implementing a C compiler in C or C++.

Approved by the owner on 2026-11-09 (pull request 1).

## Output

The compiler writes textual LLVM IR. `PLAN.md` also allows building IR through the LLVM libraries; the implementation, the CLI in `BOT.md`, and the driver write text. The contract is `build/compiler INPUT.c -o OUTPUT.ll` (and `-E` for preprocessed text). Success exits 0. Failure exits non-zero, prints one diagnostic to stderr, and does not create the output file.

LLVM (`clang`, and optionally `opt` / `llc`) optimizes that IR and generates native code. There is no custom backend. `PLAN.md` keeps LLVM as the backend until a solid release; a small non-optimising backend of our own may be considered only after 0.1, alongside LLVM. The module uses opaque pointers, so Clang/LLVM must be 15 or newer.

Approved by the owner on 2026-11-09 (pull request 1).

## Target

The module triple is `x86_64-pc-linux-gnu`. That triple was in the imported compiler.

Approved by the owner on 2026-11-09 (pull request 1).

Integer types are LP64. `char` is signed. `char` / `signed char` / `unsigned char` are 8 bits, `short` / `unsigned short` are 16, `int` / `unsigned int` are 32, and `long` / `unsigned long` / `long long` / `unsigned long long` are 64. `sizeof` has type `unsigned long`. Pointer difference has type `long`.

Approved by the owner on 2026-11-09 (pull request 4).

The module sets `target datalayout` to the System V AMD64 layout for this triple (`src/irgen.rs`), so `i64` stays 8-byte aligned. A narrow integer parameter or return keeps that width and carries an LLVM attribute. `char`, `signed char`, and `short` use `signext`. `unsigned char`, `unsigned short`, and `_Bool` use `zeroext`. `int` and wider integers have no extension attribute. The attribute is written on the `define`, the `declare`, and the `call` (`TypeName::abi_attr` in `src/ast.rs`). The `ret` instruction and the call result use the narrow type. This compiler does not emit a separate extend to 32 bits, and it does not define the excess bits of a narrow return.

Struct and union layout follows the same ABI. Each struct member is aligned to its alignment, the struct alignment is the strictest member alignment, and the size is rounded up to that alignment. A zero-width unnamed bit-field aligns the bit cursor and does not raise that alignment. An array member uses the element alignment. A local or global array of 16 bytes or more has alignment at least 16; a member does not. Union members start at offset 0, and the size is the largest member rounded up to the strictest alignment. Whole-struct and whole-union assignment is `llvm.memmove` of the object size, so the copy includes padding.

The data layout and the struct layout were approved by the owner on 2026-11-09 (pull request 7).

## Aggregate classification

`src/abi.rs` classifies a struct or union passed or returned by value. The rules are System V AMD64 psABI section 3.2.3. The aggregate is split into eightbyte chunks. Each eightbyte is INTEGER, SSE, x87, or MEMORY. INTEGER wins when it shares an eightbyte with SSE, so `struct { int i; float f; }` is one `i64`. A pure SSE eightbyte is an XMM register (`float`, `double`, or `<2 x float>`). A scalar `long double` is returned on the x87 stack. An x87 aggregate argument is passed in memory. A return whose eightbytes are `X87` then `X87UP` is `x86_fp80` in `%st0`. Any other aggregate the classification marks `MEMORY` is `sret`. An `X87UP` eightbyte that does not follow `X87` is memory. More than 16 bytes is memory for every aggregate this subset can form. A field that is not naturally aligned is memory. A struct that contains a bit-field is a byte array in LLVM; an integer bit-field of a type wider than one byte occupies its eightbyte as an integer.

A register aggregate is coerced to one or two pieces. The piece is the span of the fields in that eightbyte, rounded up to the strictest field alignment there: three bytes are `i24`, `struct { long a; char b; }` is `i64` plus `i8`, and an eightbyte that is exactly one pointer is `ptr`. A memory parameter is `byval`, aligned to at least 8. A memory return is a hidden `sret` pointer, aligned to the aggregate, and the LLVM return type is `void`. The x86-64 backend returns that pointer in `%rax`. A call result is a temporary, so `s = f()`, `f().x`, and `c ? a : b` copy it with `llvm.memmove`. A prototype may name an incomplete struct or union. A definition and a call need a complete type. Six general registers are available. The hidden `sret` pointer takes the first. A variadic argument uses the same classification and the registers still free. A multi-eightbyte aggregate that does not fit in those registers is passed entirely in memory, and the registers that were still free stay free.

Approved by the owner on 2026-11-10 (pull request 13).

`float`, `double`, and `long double` are computed: constants, usual arithmetic conversions, casts, comparisons, and the calling convention. `long double` is `x86_fp80`, stored in 16 bytes. A scalar is passed in memory and returned on the x87 stack. An aggregate that contains one is passed in memory. Returning the aggregate follows the same classification: `X87`/`X87UP` comes back in `%st0`, and `MEMORY` uses `sret`. `<stdarg.h>` uses `__builtin_va_list` and `__builtin_va_start` / `__builtin_va_arg` / `__builtin_va_end` / `__builtin_va_copy`. `va_list` is the x86_64 register-save descriptor. `va_arg` is lowered from the psABI, not from LLVM's `va_arg` instruction. `goto` names a label in the same function. Designated initializers (`.field` and `[const]`) are accepted; a following positional initializer continues at the next subobject. Bit-fields are packed least-significant bit first. `volatile` is a `load volatile` or `store volatile`.

## Preprocessor and include search

The preprocessor is built in. It runs translation phases 2–4 before the parser: line splicing, comments, `#define` / `#undef` (object-like and function-like macros, `#`, `##`, `__VA_ARGS__`), `#if` / `#ifdef` / `#ifndef` / `#elif` / `#else` / `#endif`, `#include`, `#include_next`, `#error`, `#line`, `#pragma once`, and the null directive. `-E`, `-I`, `-D`, and `-U` are accepted. `-D` and `-U` apply in order, after the predefined macros. The nested-include limit is 200.

Approved by the owner on 2026-11-09 (pull request 8). That step's `include/` was a temporary stand-in for `stdio.h`, `stdlib.h`, and `string.h`, and the search stopped before `/usr/include`.

Search order in the current code (`src/pp.rs`):

- `#include "file"` searches the including file's directory, then each `-I` directory, then every compiler `include/` directory, then the host directories.
- `#include <file>` skips the including file's directory and uses the rest of that list.
- Every compiler directory that contains `stddef.h` is appended, in order, and a path already listed is skipped: `include/` in the working directory, the crate's `include/`, then `include/` beside the executable and beside each of up to eight parent directories. A header missing from an earlier directory can come from a later compiler directory, still before the host directories.
- The host directories, after every compiler directory, are `/usr/include/x86_64-linux-gnu` and `/usr/include`, when those directories exist.
- `#include_next` resumes after the directory that provided the current file. If that directory is not in the search list, the directive behaves like `#include`.

## Headers

The compiler supplies the freestanding headers in `include/`: `stddef.h` (`size_t`, `ptrdiff_t`, `wchar_t`, `NULL`, `offsetof`, including glibc's `__need_*` protocol), `stdarg.h`, `stdbool.h`, and `limits.h` (the ISO limits for this LP64 target, with `char` signed). Those directories are searched before the C library.

The C library headers come from the host glibc. Programs can `#include` the host `stdio.h`, `stdlib.h`, `string.h`, `stdint.h`, `ctype.h`, and `assert.h`. The temporary `stdio.h`, `stdlib.h`, and `string.h` stand-ins are gone. Clang, when it compiles the same test, uses those same host headers. Predefined macros (`__x86_64__`, `__linux__`, `__LP64__`, `__GNUC__` 4.2.1, `__SIZE_TYPE__`, and the `__SIZEOF_*__` macros) select the LP64 path in those headers.

Approved by the owner on 2026-11-10 (pull request 11).

## Tests

Unit tests live in the Rust sources. `cargo test --offline --lib` runs the lexer, preprocessor, parser, diagnostic, layout, and IR checks.

`harness/run_tests.py` is the differential runner. It compiles each `tests/*.c` with `build/compiler`, links the `.ll` with Clang, compiles the same source with `clang -std=c11`, runs both programs, and compares exit status, stdout, and stderr. `cargo test` also compares those programs with Clang. Clang is the oracle only. Its source is not inspected.

Approved by the owner on 2026-11-09 (pull request 1).

The Writing a C Compiler suite (Nora Sandler) is fetched at pinned commit `ae12014d2dec14488f3f80d14df4b6d8e4634d7d` (2026-02-17) by `scripts/fetch_wacc_tests.sh`. The tree is gitignored (`tests/external/wacc/`) and is not stored in this repository. Its licence is recorded in `tests/external/README.md`. `harness/run_wacc.py` does not call the suite's `test_compiler`. A valid program is compiled here, linked with Clang, and compared with `expected_results.json`, or with `clang -std=c11` when that entry is missing. Failures that are not fixed yet are listed in `harness/wacc_known_failures.txt`, one path per line plus a reason category. A listed test that passes is reported so the line can be removed. A compiler panic fails the run even when the test is listed. Chapter skips and the latest counts are in `STATUS.md`.

Approved by the owner on 2026-11-09 (pull request 9).

### CI

`.github/workflows/ci.yml` runs on `push` and `pull_request`. Permissions are `contents: read`. A newer push to the same ref cancels the older run. The runner is `ubuntu-24.04`. Actions are pinned to commit SHAs.

- `fmt, clippy, tests, harness` (20 minutes): Rust 1.85.1 with `rustfmt` and `clippy`, apt packages `clang`, `llvm`, `make`, and `python3`, a check that Clang's major version is at least 15, then `cargo fmt --all -- --check`, `cargo clippy --offline --all-targets -- -D warnings`, `cargo test --offline --lib`, `make`, and `python3 harness/run_tests.py`.
- `writing-a-c-compiler suite` (30 minutes): the same toolchain, a cache of the fetched suite keyed by the pinned commit, `scripts/fetch_wacc_tests.sh`, `make`, and `python3 harness/run_wacc.py`.

Completed runs are recorded in `STATUS.md`. The workflow file was approved by the owner on 2026-11-09 (pull request 1). The suite job was approved by the owner on 2026-11-09 (pull request 9).

### CodeRabbit

`.coderabbit.yaml` is review configuration (schema `https://coderabbit.ai/integrations/schema.v2.json`, profile `chill`, auto-review on). The file does not install the GitHub App. Path instructions tell the reviewer to stay inside the documented subset and not to suggest copying Clang or GCC. What the App said on each pull request is in `STATUS.md`. The configuration was approved by the owner on 2026-11-09 (pull request 1).

## Tie-break

Follow the C standard. Where the standard leaves a choice, follow the gcc and clang majority.

`STATUS.md` and the git history do not record a separate approval date for this rule. The oracle in the harness and in CI is Clang. gcc is not run there. Cases those files already record:

- The standard wins where Clang is more permissive. Dropping `const` on assignment, arithmetic on `void *`, a relational comparison of a pointer with an integer (`p < 0`), and an evaluated comma in a constant initializer are diagnostics. Equality with a null pointer constant (`p == 0`, `p != (void *)0`) is allowed.
- A multi-character character constant such as `'ab'` has an implementation-defined value (C11 6.4.4.4). This subset rejects it. `STATUS.md` records that as a limitation. Clang accepts it as an `int`.
- An implementation-defined choice that is recorded as matching Clang on this target: an out-of-range conversion to a signed type keeps the low bits and reads them as two's complement. An omitted `__VA_ARGS__` deletes a comma written as `, ## __VA_ARGS__`, matching Clang's `-std=c11`.
- A completed enumeration is compatible with `int`, including when every enumerator is nonnegative. `STATUS.md` records that choice (C11 6.7.2.2). A value that does not fit in `int` is a diagnostic. gcc and clang use `unsigned int` when there is no negative enumerator, so this choice differs from that majority.

## Platforms

Only x86_64 Linux is supported today. The triple, the data layout, the integer ABI, and the host include directories are the System V AMD64 / glibc paths for that system.

The forward plan is in `PLAN.md`. After the remaining C11 work (an adversarial review, then variable-length arrays, old-style definitions, and `_Alignas`), the order is a separate opt-in GNU-compatibility milestone, then big programs on Linux x86_64 (DOOM via doomgeneric, then Lua and SQLite), then macOS on Apple Silicon, Linux ARM, macOS on Intel, and Windows x64. The platform list was reordered on 2026-11-12 so that macOS ARM comes first, because it is the owner's own device. Performance tuning uses the timing CI already collects. C23, WebAssembly, and RISC-V come later.
