# Crash points in non-test code

Total: 285. By category: a (invariant) 217, b (reachable from bad input) 0, c (unclear) 68.

By kind: expect 182, unreachable 74, unwrap 28, assert 1

Category a by basis: relies on an earlier phase or match (rule, not hand-checked) 180, checked locally 37

| File | Total | a | b | c |
|---|---:|---:|---:|---:|
| src/sema/expr.rs | 44 | 37 | 0 | 7 |
| src/irgen/expr.rs | 43 | 43 | 0 | 0 |
| src/sema/call.rs | 27 | 26 | 0 | 1 |
| src/irgen/call.rs | 16 | 16 | 0 | 0 |
| src/sema/decl.rs | 13 | 7 | 0 | 6 |
| src/parser/decl.rs | 12 | 4 | 0 | 8 |
| src/irgen/inline_builtins.rs | 11 | 11 | 0 | 0 |
| src/pp/directive.rs | 11 | 9 | 0 | 2 |
| src/sema/mod.rs | 10 | 9 | 0 | 1 |
| src/sema/stmt.rs | 10 | 6 | 0 | 4 |
| src/irgen/access.rs | 9 | 9 | 0 | 0 |
| src/sema/init.rs | 8 | 4 | 0 | 4 |
| src/parser/expr.rs | 7 | 0 | 0 | 7 |
| src/float.rs | 6 | 6 | 0 | 0 |
| src/pp/scan.rs | 6 | 0 | 0 | 6 |
| src/irgen/stmt.rs | 5 | 5 | 0 | 0 |
| src/pp/expand.rs | 5 | 4 | 0 | 1 |
| src/sema/consteval.rs | 5 | 5 | 0 | 0 |
| src/sema/convert.rs | 5 | 3 | 0 | 2 |
| src/ast/types.rs | 4 | 4 | 0 | 0 |
| src/parser/ice.rs | 4 | 0 | 0 | 4 |
| src/irgen/vaarg.rs | 3 | 3 | 0 | 0 |
| src/lexer.rs | 3 | 0 | 0 | 3 |
| src/parser/record.rs | 3 | 0 | 0 | 3 |
| src/builtins.rs | 2 | 1 | 0 | 1 |
| src/irgen/consts.rs | 2 | 2 | 0 | 0 |
| src/parser/declarator.rs | 2 | 0 | 0 | 2 |
| src/parser/specs.rs | 2 | 0 | 0 | 2 |
| src/parser/stmt.rs | 2 | 0 | 0 | 2 |
| src/abi.rs | 1 | 1 | 0 | 0 |
| src/ast/tree.rs | 1 | 1 | 0 | 0 |
| src/fp80.rs | 1 | 0 | 0 | 1 |
| src/fxhash.rs | 1 | 1 | 0 | 0 |
| src/sema/image.rs | 1 | 0 | 0 | 1 |
