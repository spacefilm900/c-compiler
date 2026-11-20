# src

Rust sources for the clean-room frontend. `make` (or `cargo build --release`) produces `build/hcc`. Each directory is one module: `mod.rs` holds the shared types and entry points, and unit tests live in the directory's `tests.rs` (plus `ice_tests.rs` and `precise_tests.rs` in `irgen/`).

| File or directory | Role |
| --- | --- |
| `pp/` | Translation phases 2–4: splicing, preprocessing tokens, macros, conditionals, and `#include`. `mod.rs` holds the preprocessor state; `predefined.rs` the predefined macros and include list, `scan.rs` phase 2–3 scanning, `directive.rs` directives and include search, `ifexpr.rs` `#if` expressions, `expand.rs` macro expansion. |
| `lexer.rs` | Phase-7 tokens, integer suffixes, floating constants (including `long double`), character constants, and `L` / `u` / `U` / `u8` literals. Rejects a stray `#`, `u8` character constants, and other input this milestone does not lex. |
| `float.rs` | Decimal and hexadecimal floating constants, and conversions between floating values and integers. |
| `abi.rs` | System V AMD64 classification for aggregates, including SSE eightbytes. |
| `parser/`, `ast/` | Recursive-descent parser and AST, including pointers, arrays, structs, unions, typedefs, enums, `const`/`volatile`/`restrict`, and variadic prototypes. Parser: `decl.rs`, `specs.rs`, `record.rs`, `attrs.rs`, `declarator.rs`, `stmt.rs`, `expr.rs`, `ice.rs` (integer constant expressions), `uneval.rs` (checks and types for operands that are not evaluated). AST: `types.rs`, `record.rs`, `compat.rs` (type spelling, compatibility and composite types), `tree.rs` (expressions, statements, declarations), `dump.rs` (text dumps: the plain tree for the parser tests, and the `--dump-ast` and `--dump-sema` formats). |
| `sema/` | Scopes, tag scopes, typedefs, promotions, pointer conversions, array decay, struct and union layout, qualifiers, string initializers, calls, lvalues, linkage, and constant initializers. Split into `decl.rs`, `init.rs`, `image.rs` (static initializer bytes), `stmt.rs`, `expr.rs`, `call.rs`, `convert.rs`, and `consteval.rs`. |
| `irgen/` | Textual LLVM IR (`define`, `declare`, globals, named struct types, `getelementptr`, string constants, pointer arithmetic, and variadic calls). Split into `module.rs`, `consts.rs` (static initializers), `stmt.rs`, `expr.rs`, `call.rs`, `access.rs` (loads, stores, members, conversions), `vaarg.rs`, and `inline_builtins.rs`. |
| `driver.rs`, `main.rs` | `compiler [options] INPUT.c -o OUTPUT.ll`, including `-E`, `-I`, `-D`, `-U`, `--time-phases`, and the stage dumps `--dump-tokens`, `--dump-ast`, `--dump-sema` and `--dump-ir` (see `docs/DEBUGGING.md`). |
| `diag.rs`, `token.rs` | Diagnostics, token kinds, and the `--dump-tokens` format. |
| `timing.rs` | Frontend phase timing for `--time-phases`. |
| `fxhash.rs` | A small fast hasher for the preprocessor's macro table and hide sets. |

See the repository `README.md` for the supported subset and `STATUS.md` for test results.
