# C Compiler Project – High-Level Plan & Bot Instructions

## High-level plan

### Completed

- **Stage 1**: C subset (no preprocessor, integers, pointers, functions, control flow, simple structs).
- **Stage 2**: Preprocessor, more types, basic standard headers, aggregate calls, floating types, variadic definitions, `goto`, designated initializers, bit-fields, and `volatile`.
- **Stage 3**: Testing, before any new platform. Csmith random differential testing against clang. A few small open-source C programs, with One True AWK as the flagship: build it here and with clang, run its own test suite and the AWK Builder project's suites, and compare the two builds. The GCC torture tests (`gcc.c-torture/execute`) are inputs only, with a known-failures list; they are not used to shape the compiler. First performance numbers: compile time against clang, and run time of the compiled programs against clang at the same optimisation level, tracked in CI.

### Forward plan

The owner approved this revised order on 2026-11-12, and on 2026-11-13, after the adversarial challenge closed, approved this sequence: a Csmith coverage step, then the GNU-compatibility cheap tier, then big programs with a per-program scorecard, then macOS on Apple Silicon. Each step is ordinary work on this compiler. A new platform is the same compiler choosing its target at compile time.

1. **Done.** Small C11 gaps. Flexible array members. `u` and `U` character and string literals, and `u8` string literals (`char16_t` and `char32_t` are the target's unsigned 16-bit and 32-bit types; `u8` strings are arrays of `char`). `u8` character constants are not in C11 and stay rejected. Returning a struct or union that contains `long double`. File-scope initializers that nest a bit-field struct inside another struct or array. A follow-up closed eight more gaps: `sizeof` in integer constant expressions, compound literals, non-prototype `int f();` declarations, `_Static_assert` / `_Generic` / `_Alignof`, duplicate asm-label declarations, brace elision after a designator, UTF-8 and universal character names in literals, and prefixed character constants in `#if`.
2. **Done.** Adversarial review. A separate reviewer agent, given only the C11 standard, the System V ABI, and `STATUS.md`, and never the compiler source, writes tricky test programs (integer promotions, conversions, initializers, bit-fields, struct passing, `long double`, varargs, preprocessor edge cases). Clang is the referee. A mismatch counts as a bug only where Clang's behaviour is what the standard requires or the implementation-defined choice recorded in `STATUS.md`. The reviewer does not change compiler code. The builder fixes findings in normal pull requests. This phase is closed: v0.6 was accepted as the final planned adversarial release, with 724/724 at both `-O0` and `-O2`, and pull request 27 merged (`d728a94`).
3. **Done.** Csmith coverage. `tools/csmith_campaign.py` is the in-tree runner: it always links with `-lm`, at both `-O0` and `-O2`, and its default `gnu11` mode compiles the candidate with `-std=gnu11` and the references in their default GNU mode, without `-pedantic-errors`. Csmith is given the same size caps as the CI runner, plus `--no-packed-struct`. A local campaign of seeds 1–2000 produced **1,492** valid differential comparisons at `-O0` and **1,490** at `-O2`. The CI Csmith job now also uses `-std=gnu11`; its runtime is unchanged. Candidate defects found by the campaign are recorded in `reports/csmith-coverage/`. The follow-up fixed both (a struct-return overrun at `-O0`, and GNU-mode distinct-pointer comparisons and pointer-sign assignments); the rerun has **1,821** and **1,822** valid comparisons, all agreeing, and no rejects.
4. **Done.** GNU-compatibility cheap tier. A separate, opt-in layer, clearly labelled as outside strict C11 and selected with `-std=gnu11`. This step is `__typeof__`, statement expressions, `__attribute__` handling, case ranges, zero-length arrays, the `?:` shorthand, and more `__builtin_` functions (those that map onto library calls or LLVM intrinsics, such as `__builtin_abort`, the `memcpy` family, `__builtin_expect`, overflow builtins, `clz` / `ctz` / `popcount`, `bswap`, `__builtin_unreachable`, and `__builtin_constant_p`). Other cheap GCC spellings (`__extension__`, `__inline__`, `__asm__` labels) belong here too. Equality between a `void *` and a function pointer is already allowed in GNU mode (pull request 27), and so are distinct-pointer comparisons and pointer-sign or qualifier-dropping assignments (the Csmith follow-up). Progress is measured against `harness/gcc_torture_known_failures.txt` (829 known failures at `2a2ce0d`, 827 after the Csmith follow-up). A local triage of an earlier list (918 known at `3c22ec4`) found that GNU spellings and builtins that map onto library calls accounted for about a third of those entries; that triage is not in the repository. All of this work is clean-room and is written only from the GCC user manual's public documentation of builtins, attributes, and language extensions, never from GCC or Clang source. Progress: `typeof`, statement expressions, `__attribute__` handling (with meaning for `aligned`, `packed`, `noreturn`, `used`, `section`, `weak`, `deprecated`, `cold` / `hot`, `const` / `pure`, `noinline` / `always_inline`, `constructor` / `destructor`, and `mode`), case ranges, zero-length arrays, `?:`, empty initializers, and the builtins listed in README are implemented. The known-failure list went from **827** to **432**, with **1,314** torture tests passing (was 919). Two tests that passed only because `vector_size` and an aligned `int` typedef were silently ignored are now known failures. The largest remaining groups are inline assembly, implicit `int` and implicit function declarations, `vector_size`, and bit-fields in packed structs.
5. **Big programs on Linux x86_64.** DOOM, via doomgeneric, comes first: build it with this compiler and with Clang, run the same recorded demo headless, and compare frame hashes one by one. Then Lua, using its own test suite, and SQLite, using its own tests. Programs are taken in graded complexity, from small to multi-file, with immutable baselines. Each program gets a short scorecard: whether it builds, whether its output matches Clang at `-O0` and `-O2`, defects found by category, and a minimal regression test kept in the repository for each bug. Third-party sources are test inputs only: they are fetched by script and are not vendored. The middle tier of GNU compatibility — inline assembly, computed `goto` and label addresses (`&&label`), and GNU vector types — is added only when a real target program needs it. Nested functions stay out of scope. GNU features these programs actually need drive priorities in the cheap tier as well. Progress: **DOOM done** (pull request 32, merged as `060ca52`). doomgeneric is fetched at a pinned commit and the Freedoom 0.13.0 WAD by SHA-256; a headless backend of our own plays each recorded demo to its end and hashes every frame. The first run (frozen in `reports/doom/baseline.md`) had one rejected file, `info.c` (function addresses in a union inside a static table); that is fixed with a regression test. DOOM now builds with this compiler alone, and all eight demos of `freedoom1.wad` and `freedoom2.wad` (39,209 frames) match Clang at both `-O0` and `-O2` (`reports/doom/scorecard.md`); CI plays all of them at both levels. Code review of the fix found two union-initializer bugs that DOOM does not reach; both are fixed with regression tests. No inline assembly, vector types, or other deferred features were needed. **Lua done (pending review).** Lua 5.4.7 and its official test suite are fetched from lua.org by SHA-256. The first run (frozen in `reports/lua/baseline.md`, strict `-std=c11`) had one rejected file, `lvm.c`: Lua's interpreter loop uses a jump table built from GNU labels as values and computed `goto` when the compiler defines `__GNUC__`. That is the first middle-tier GNU feature a target program needed, so it is now implemented in `-std=gnu11` (`&&label`, `goto *p`, label addresses in static initializers), with regression tests. Lua now builds with this compiler alone in `-std=gnu11`, both as released and with Lua's internal checks (`ltests`) on; the official suite passes in its user, portable and internal modes, nine workload scripts match Clang byte for byte, and `luac` output is identical, at `-O0` and `-O2` (`reports/lua/scorecard.md`); CI runs all of it. Label-address differences (`&&a - &&b`) are deferred: Lua does not use them. SQLite is next.
6. **macOS on Apple Silicon** (`arm64-apple-darwin`). This is the first new platform. The platform list was reordered on 2026-11-12 so that macOS ARM comes before Linux ARM and macOS Intel, because it is the owner's own device (a MacBook Air with Apple Silicon). The work covers the target triple and data layout, the Apple ARM64 calling convention (which differs from AAPCS64 for variadic functions), Mach-O symbol naming, and Apple's SDK headers, and it adds a macOS ARM CI job. CI on that job runs every existing suite: unit tests, differential tests against Clang, the Writing a C Compiler suite, Csmith, GCC torture with a per-platform known-failures list, One True AWK, and the AWK Builder suites. New cross-compiler calling-convention tests link this compiler's code with Clang's in both directions. Each new platform starts with a short adversarial round focused on its calling rules. Acceptance is that the harness and the challenge suites pass, and that DOOM runs on the owner's MacBook Air, with a short step-by-step user guide in `docs/`.
7. **Linux ARM (AArch64).** The same suites and the same checks.
8. **macOS on Intel.** The same suites and the same checks.
9. **Windows x64.** LLP64 and the Microsoft ABI. The same suites, adapted. This is last because linking and headers are hardest.
10. **Bigger C11 features.** Variable-length arrays, then old-style K&R function definitions (non-prototype `int f();` declarations are already accepted), then `_Alignas`. `_Complex` is optional and last, if at all. Take these earlier if a program needs them.
11. **Performance tuning,** using the timing CI already collects.
12. **Later options.** C23, WebAssembly, and RISC-V. None of these starts before the steps above.

- **Target**: correct LLVM IR that produces efficient native binaries.

### 2. Clean-room rule (mandatory)

- All frontend code (lexer, parser, semantic analysis, IR generation) must be original.
- Do **not** copy, translate, or closely paraphrase Clang or GCC source, or Claude's C Compiler, or nqcc.
- Allowed references: C standard, LLVM Language Reference, System V ABI, Kaleidoscope-style tutorials, public specs, and, for the GNU-compatibility work, the GCC user manual's public documentation of builtins, attributes, and language extensions. GCC and Clang source remain off-limits.
- Clang/GCC may be used only as black-box oracles for differential testing. GCC torture, Csmith, and One True AWK are test inputs only: never shape compiler code around them and never patch them. DOOM, Lua, and SQLite are test inputs in the same way: they are fetched by script, are not vendored, and are not patched. The GNU features they need may set priorities in the GNU-compatibility milestone.

### 3. Technical approach

- Frontend emits textual LLVM IR (or builds it via LLVM libraries).
- LLVM (`opt` / `llc` / `clang`) performs optimisation and native code generation.
- LLVM stays the backend until a solid release. A small non-optimising backend of our own may be considered only after 0.1, alongside LLVM. No custom backend is required to achieve good performance.
- Rust standard library only.

### 4. Agent workflow

- Repository with clear layout and a test harness the bot can run.
- Every change must pass existing tests.
- Bot maintains a living status document (`STATUS.md`).
- Frequent small commits.

### 5. Milestones

- Lexer + parser → AST.
- Semantic analysis.
- IR generation that can compile and run “hello world”, arithmetic, loops, and function calls.
- Growing test suite (hand-written → GCC torture subset → csmith differential testing).
- Self-hosting (stretch goal).
- Performance comparison against `clang -O2` on selected benchmarks.

### 6. Testing

- Hand-written smoke tests.
- Differential testing: compile the same program with the project compiler and with Clang; compare runtime results and exit codes.
- No inspection of Clang’s or GCC’s source code.

## Bot instructions

You are building a C compiler that targets LLVM IR.
(Follow the clean-room rules above.)
Work incrementally; keep STATUS.md up to date.
After every significant change, run the test harness.
