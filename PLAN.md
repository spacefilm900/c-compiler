# C Compiler Project – High-Level Plan & Bot Instructions

## High-level plan

### 1. Scope (staged)
- **Stage 1**: C subset (no preprocessor, integers, pointers, functions, control flow, simple structs).
- **Stage 2**: Preprocessor, more types, basic standard headers.
- **Stage 3**: Optimisations (via LLVM) + broader language coverage.
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
