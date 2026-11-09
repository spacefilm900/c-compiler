# src

Rust sources for the clean-room frontend. `make` (or `cargo build --release`) produces `build/compiler`.

| File | Role |
| --- | --- |
| `lexer.rs` | Tokens, integer suffixes, and character constants. Rejects preprocessor lines, strings, floats, and other input this milestone does not lex. |
| `parser.rs`, `ast.rs` | Recursive-descent parser and AST, including pointers, arrays, casts, and the integer operators. |
| `sema.rs` | Scopes, integer promotions, pointer conversions, array decay, calls, lvalues, linkage, and constant initializers. |
| `irgen.rs` | Textual LLVM IR (`define`, `declare`, globals, pointer arithmetic, and the integer operators). |
| `driver.rs`, `main.rs` | `compiler INPUT.c -o OUTPUT.ll`. |
| `diag.rs`, `token.rs` | Diagnostics and token kinds. |

See the repository `README.md` for the supported subset and `STATUS.md` for test results.
