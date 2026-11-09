# src

Rust sources for the clean-room frontend. `make` (or `cargo build --release`) produces `build/compiler`.

| File | Role |
| --- | --- |
| `pp.rs` | Translation phases 2–4: splicing, preprocessing tokens, macros, conditionals, and `#include`. |
| `lexer.rs` | Phase-7 tokens, integer suffixes, character constants, and string literals. Rejects a stray `#`, wide strings, floats, and other input this milestone does not lex. |
| `parser.rs`, `ast.rs` | Recursive-descent parser and AST, including pointers, arrays, structs, `const`/`volatile`, and variadic prototypes. |
| `sema.rs` | Scopes, tag scopes, promotions, pointer conversions, array decay, struct layout, qualifiers, string initializers, calls, lvalues, linkage, and constant initializers. |
| `irgen.rs` | Textual LLVM IR (`define`, `declare`, globals, named struct types, `getelementptr`, string constants, pointer arithmetic, and variadic calls). |
| `driver.rs`, `main.rs` | `compiler [options] INPUT.c -o OUTPUT.ll`, including `-E`, `-I`, `-D`, and `-U`. |
| `diag.rs`, `token.rs` | Diagnostics and token kinds. |

See the repository `README.md` for the supported subset and `STATUS.md` for test results.
