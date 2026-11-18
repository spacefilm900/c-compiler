# Debugging the compiler

How to take a bug from a failing program to a fix with a regression test.
The tools are the stage dumps (`--dump-*`), the reducer
(`tools/reduce/reduce.py`) and the existing test harnesses. Clang and GCC
are used only as black-box oracles: compare their *output* with ours,
never their source.

## 1. Reproduce and classify

Run the compiler the way the failing build did (same `-std`, `-target`,
`-I` and `-D` flags) and look at the exit status:

| Status | Meaning | Reducer mode |
|---|---|---|
| 0, but the program behaves differently from Clang's build | wrong code | `miscompile` |
| 0, but Clang cannot compile our `.ll` | invalid IR | `bad-ir` |
| 0, but the program is not valid C (Clang rejects it) | missing diagnostic | `accept` |
| 1 (`error:`), but the program is valid C | wrong diagnostic | `reject` |
| 3 (`internal compiler error: ... (in PHASE)`) | a compiler invariant failed | `ice` |
| 101 or a signal | a panic or crash (should not happen; see phase B) | `crash` |

## 2. Reduce it

```sh
make
python3 tools/reduce/reduce.py big.c --mode miscompile -o small.c
python3 tools/reduce/reduce.py big.c --mode ice --message "in irgen" -o small.c
python3 tools/reduce/reduce.py big.c --mode reject --message "incompatible types" -o small.c
python3 tools/reduce/reduce.py big.c --mode cmd --cmd './still_bad.sh {}' -o small.c
```

Add `--std gnu11` for GNU C, `--flags '-target aarch64-linux-gnu'` for
another target, and `--clang-valid` with `ice`, `crash` or `cmd` to keep the
result valid C. A preprocessed file (`compiler -E big.c -o big.i`, renamed
to `.c`) reduces better than one with many `#include`s, because the
reducer can then remove unused header declarations too.
`tools/reduce/README.md` lists every mode and option. A typical result is
under 20 lines: the demo turns a 118-line program into 14 lines.

## 3. Find the stage at fault

Each dump stops after its stage and prints to standard output (or `-o FILE`).
Warnings and errors are reported as in a normal compilation, so a dump of a
later stage fails if an earlier stage rejects the program. Normal
compilation output is unchanged by these options. Compare each stage with
what C11 (N1570) says the program means, working forwards:

| Option | Stage | Check |
|---|---|---|
| `-E` (or `--dump-pp`) | preprocessed text | macro expansion, `#if`, `#include` (compare with `clang -E` as an oracle) |
| `--dump-tokens` | the phase-7 tokens the parser reads | constant values and types, string bytes, positions |
| `--dump-ast` | the syntax tree as parsed | operator precedence, declarator types, initializer shape |
| `--dump-sema` | the tree after semantic analysis | expression types, implicit conversions, member offsets, record layouts, folded constants |
| `--dump-ir` | the LLVM IR module | the IR a normal compilation writes (`-o`); check it with the LLVM LangRef, or compare behaviour with `clang -S -emit-llvm` |

If the sema dump is right and the IR is wrong, the bug is in `src/irgen/`;
if the sema dump has a wrong type, conversion or offset, it is in
`src/sema/` (or the layout code in `src/ast/record.rs`); a wrong tree is in
`src/parser/`; wrong tokens are in `src/pp/` or `src/lexer.rs`.
`--time-phases` prints how long each stage took, for performance bugs.

### Dump formats

All dumps are plain text, deterministic (the same input always gives the
same bytes) and in source order. `tests/dumps/` has a complete example of
each for four small programs.

**Tokens.** One line per token:
`FILE:LINE:COLUMN KIND 'SPELLING' [VALUE]`. `KIND` is the token kind
(`Ident`, `Int`, `Float`, `String`, `KwInt`, `Plus`, ...). An `Int` adds its
type and value (`Int '0x10u' unsigned int 16`), a `Float` its type and the
LLVM spelling of its bits, a `String` its decoded bytes in double quotes.
In the spelling and bytes, non-printing bytes are `\xNN`, and `\n`, `\t`,
`\\`, `\'` and `\"` are escaped. `FILE` is the file the token came from,
so tokens from a header name the header.

**AST.** One node per line, children indented by two spaces, each line
ending in `@LINE:COLUMN`. Declarations are `func`, `proto`, `global` and
`decl` with their type and name; statements are `block`, `if` / `cond` /
`then` / `else`, `while`, `for` / `init` / `cond` / `step` / `body`,
`return`, `expr` and so on; expressions are the operator (`+`, `assign`,
`+=`, `?:`, `call NAME`, `member F`, `arrow F`, `index`, `cast TYPE`, `neg`,
`addr`, `deref`, ...) or a leaf (`var NAME`, `int 42`, `double 0x...`,
`string "..."`). The unit tests in `src/parser/tests.rs` use the same tree
without the positions.

**Sema.** The AST format after semantic analysis, with more on each line:

- `: TYPE` after every expression: its type after analysis.
- `-> TYPE` after that when the context converts the value implicitly
  (C11 6.3): the usual arithmetic conversions of a binary operator or
  comparison, integer promotions of a shift or unary operator, assignment
  and initialization to the object's type, an argument to its parameter
  type (or the default argument promotions), and `return` to the function's
  return type. `decay` nodes are array-to-pointer conversions inserted by
  analysis.
- `[...]` details: a variable's storage (`[%v3]` for a local slot,
  `[@name]` for a global); a call's target and prototype
  (`[@printf int(const char *, ...)]`); a member's field index, byte
  offset, `bytewise` addressing and bit-field position
  (`bits 3 at bit 1 unit 32`); a string literal's global; a function's
  linkage and linker symbol.
- `case = VALUE` for each folded case label.
- A local's initializer as checked: `stores`, then one
  `store TYPE at OFFSET` per scalar written (with bit-field details).
- After the tree: `records` (each completed struct or union with its size,
  alignment and every member's offset or bit position), `globals` (every
  emitted object with its linkage and folded constant value: integers in
  their type, `&name + offset` for addresses, `aggregate` lists and `bytes`
  for bit-field images), `strings` (literal globals with their bytes,
  including the terminating NUL) and `declarations` (external functions).
  File-scope initializers in the tree itself appear as written; their
  values are under `globals`.

**IR.** Exactly what `-o FILE` writes.

## 4. Fix it, with a regression test

Write the fix from N1570, the LLVM LangRef and the ABI documents. Then add
the reduced program as a test where it belongs:

- Wrong code, runtime behaviour: `tests/NNN_name.c` (next free number),
  which `harness/run_tests.py` builds with this compiler and with Clang and
  compares. GNU C goes in `tests/gnu/`; a cross-ABI case in `tests/xabi/`.
- A diagnostic, an ICE or a type rule: a unit test beside the code
  (`src/sema/tests.rs`, `src/parser/tests.rs`, `src/irgen/ice_tests.rs`, ...).
- A dump format change: update the golden files with
  `python3 harness/run_dump_tests.py --update` and review the diff.

Check that the test fails without the fix and passes with it, then run the
usual checks:

```sh
cargo fmt --all -- --check
cargo clippy --offline --all-targets -- -D warnings
cargo test --offline --lib
make && python3 harness/run_tests.py
python3 harness/run_dump_tests.py
python3 -m unittest tools/reduce/test_reduce.py
```

CI runs these plus WACC, GCC torture, Csmith, DOOM, Lua, SQLite and
One True AWK.
