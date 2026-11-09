# src

Rust sources for the clean-room frontend. `make` (or `cargo build --release`) produces `build/compiler`.

| File | Role |
| --- | --- |
| `lexer.rs` | Tokens, integer suffixes, character constants, and string literals. Rejects preprocessor lines, wide strings, floats, and other input this milestone does not lex. |
| `parser.rs`, `ast.rs` | Recursive-descent parser and AST, including pointers, arrays, `const`/`volatile`, and variadic prototypes. |
| `sema.rs` | Scopes, promotions, pointer conversions, array decay, qualifiers, string initializers, calls, lvalues, linkage, and constant initializers. |
| `irgen.rs` | Textual LLVM IR (`define`, `declare`, globals, string constants, pointer arithmetic, and variadic calls). |
| `driver.rs`, `main.rs` | `compiler INPUT.c -o OUTPUT.ll`. |
| `diag.rs`, `token.rs` | Diagnostics and token kinds. |

See the repository `README.md` for the supported subset and `STATUS.md` for test results.
