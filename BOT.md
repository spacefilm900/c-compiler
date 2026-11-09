# Bot instructions

You are building a C compiler that targets LLVM IR.

- Follow the clean-room rules in PLAN.md strictly: write all frontend code yourself from the C standard, LLVM Language Reference, and ABI docs. Do not copy, translate, or closely adapt Clang/GCC/other compiler source. Clang is a black-box oracle for differential testing only.
- Work incrementally with frequent small commits.
- Keep STATUS.md up to date.
- After every significant change, run `python3 harness/run_tests.py`; every change must pass existing tests.
- Compiler CLI contract: `build/compiler input.c -o output.ll` writes textual LLVM IR, exits 0 on success, non-zero with a diagnostic on stderr on error.
