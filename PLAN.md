# C Compiler Project – High-Level Plan & Bot Instructions

## High-level plan

### 1. Scope (staged)
- **Stage 1**: C subset (no preprocessor, integers, pointers, functions, control flow, simple structs).
- **Stage 2**: Preprocessor, more types, basic standard headers.
- **Stage 3**: Testing, before any new platform. Csmith random differential testing against clang. A few small open-source C programs, with One True AWK as the flagship: build it here and with clang, run its own test suite and the AWK Builder project's suites, and compare the two builds. The GCC torture tests (`gcc.c-torture/execute`) are inputs only, with a known-failures list; they are not used to shape the compiler. First performance numbers: compile time against clang, and run time of the compiled programs against clang at the same optimisation level, tracked in CI.
- **Stage 4**: More targets of this same compiler, selected by a target option. macOS on x86_64 and arm64, then Linux arm64 (AArch64 ABI), then Windows x64 (LLP64, Microsoft ABI, MSVC or MinGW). WebAssembly and RISC-V are optional and come later.
- **Target**: correct LLVM IR that produces efficient native binaries.

### 2. Clean-room rule (mandatory)
- All frontend code (lexer, parser, semantic analysis, IR generation) must be original.
- Do **not** copy, translate, or closely paraphrase Clang or GCC source.
- Allowed references: C standard, LLVM Language Reference, System V ABI, Kaleidoscope-style tutorials, public specs.
- Clang/GCC may be used only as black-box oracles for differential testing.

### 3. Technical approach
- Frontend emits textual LLVM IR (or builds it via LLVM libraries).
- LLVM (`opt` / `llc` / `clang`) performs optimisation and native code generation.
- No custom backend is required to achieve good performance.

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
