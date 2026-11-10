# C Compiler Project – High-Level Plan & Bot Instructions

## High-level plan

### Completed

- **Stage 1**: C subset (no preprocessor, integers, pointers, functions, control flow, simple structs).
- **Stage 2**: Preprocessor, more types, basic standard headers, aggregate calls, floating types, variadic definitions, `goto`, designated initializers, bit-fields, and `volatile`.
- **Stage 3**: Testing, before any new platform. Csmith random differential testing against clang. A few small open-source C programs, with One True AWK as the flagship: build it here and with clang, run its own test suite and the AWK Builder project's suites, and compare the two builds. The GCC torture tests (`gcc.c-torture/execute`) are inputs only, with a known-failures list; they are not used to shape the compiler. First performance numbers: compile time against clang, and run time of the compiled programs against clang at the same optimisation level, tracked in CI.

### Forward plan

The owner approved this order. Each step is ordinary work on this compiler. A new platform is the same compiler choosing its target at compile time.

1. **Done.** Small C11 gaps. Flexible array members. `u` and `U` character and string literals, and `u8` string literals (`char16_t` and `char32_t` are the target's unsigned 16-bit and 32-bit types; `u8` strings are arrays of `char`). `u8` character constants are not in C11 and stay rejected. Returning a struct or union that contains `long double`. File-scope initializers that nest a bit-field struct inside another struct or array.
2. **Adversarial review.** A separate reviewer agent, given only the C11 standard, the System V ABI, and `STATUS.md`, and never the compiler source, writes tricky test programs (integer promotions, conversions, initializers, bit-fields, struct passing, `long double`, varargs, preprocessor edge cases). Clang is the referee. A mismatch counts as a bug only where Clang's behaviour is what the standard requires or the implementation-defined choice recorded in `STATUS.md`. The reviewer does not change compiler code. The builder fixes findings in normal pull requests.
3. **Bigger C11 features.** Variable-length arrays, then old-style K&R function definitions, then `_Alignas`. `_Complex` is optional and last, if at all.
4. **macOS on Intel, then ARM.** CI on GitHub macOS runners runs every existing suite: unit tests, differential tests against Clang, the Writing a C Compiler suite, Csmith, GCC torture with a per-platform known-failures list, One True AWK, and the AWK Builder suites. New cross-compiler calling-convention tests link this compiler's code with Clang's in both directions. Each new platform starts with a short adversarial round focused on its calling rules.
5. **Linux ARM (AArch64).** The same suites and the same checks.
6. **Windows x64.** LLP64 and the Microsoft ABI. The same suites, adapted. This is last because linking and headers are hardest.
7. **Performance tuning,** using the timing CI already collects.
8. **Later options.** C23, WebAssembly, and RISC-V. None of these starts before the steps above.

- **Target**: correct LLVM IR that produces efficient native binaries.

### 2. Clean-room rule (mandatory)

- All frontend code (lexer, parser, semantic analysis, IR generation) must be original.
- Do **not** copy, translate, or closely paraphrase Clang or GCC source, or Claude's C Compiler, or nqcc.
- Allowed references: C standard, LLVM Language Reference, System V ABI, Kaleidoscope-style tutorials, public specs.
- Clang/GCC may be used only as black-box oracles for differential testing. GCC torture, Csmith, and One True AWK are test inputs only: never shape compiler code around them and never patch them.

### 3. Technical approach

- Frontend emits textual LLVM IR (or builds it via LLVM libraries).
- LLVM (`opt` / `llc` / `clang`) performs optimisation and native code generation.
- No custom backend is required to achieve good performance.
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
