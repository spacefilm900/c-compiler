# Crash points in non-test code

Total: 340. By category: a (invariant) 300, b (reachable from bad input) 0, c (unclear) 40.

By kind: expect 224, unreachable 79, unwrap 28, assert 6, panic 2, assert_eq 1

Category a by basis: relies on an earlier phase or match (rule, not hand-checked) 189, checked locally 111

| File | Total | a | b | c |
|---|---:|---:|---:|---:|
| src/irgen.rs | 141 | 104 | 0 | 37 |
| src/sema.rs | 124 | 123 | 0 | 1 |
| src/parser.rs | 32 | 32 | 0 | 0 |
| src/pp.rs | 22 | 22 | 0 | 0 |
| src/float.rs | 6 | 6 | 0 | 0 |
| src/ast.rs | 5 | 5 | 0 | 0 |
| src/lexer.rs | 3 | 3 | 0 | 0 |
| src/abi.rs | 2 | 1 | 0 | 1 |
| src/builtins.rs | 2 | 2 | 0 | 0 |
| src/aapcs64.rs | 1 | 0 | 0 | 1 |
| src/fp80.rs | 1 | 1 | 0 | 0 |
| src/fxhash.rs | 1 | 1 | 0 | 0 |
