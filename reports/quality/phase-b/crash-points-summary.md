# Crash points in non-test code

Total: 286. By category: a (invariant) 286, b (reachable from bad input) 0, c (unclear) 0.

By kind: expect 182, unreachable 75, unwrap 28, assert 1

Category a by basis: relies on an earlier phase or match (rule, not hand-checked) 181, checked locally 105

| File | Total | a | b | c |
|---|---:|---:|---:|---:|
| src/sema.rs | 123 | 123 | 0 | 0 |
| src/irgen.rs | 90 | 90 | 0 | 0 |
| src/parser.rs | 32 | 32 | 0 | 0 |
| src/pp.rs | 22 | 22 | 0 | 0 |
| src/float.rs | 6 | 6 | 0 | 0 |
| src/ast.rs | 5 | 5 | 0 | 0 |
| src/lexer.rs | 3 | 3 | 0 | 0 |
| src/builtins.rs | 2 | 2 | 0 | 0 |
| src/abi.rs | 1 | 1 | 0 | 0 |
| src/fp80.rs | 1 | 1 | 0 | 0 |
| src/fxhash.rs | 1 | 1 | 0 | 0 |
