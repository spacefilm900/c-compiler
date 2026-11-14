## Defects

| Id | Category | Where | Repro test | State |
| --- | --- | --- | --- | --- |
| LUA-1 | frontend reject (missing GNU feature) | `lvm.c`: this compiler defines `__GNUC__`, so Lua turns on its jump table (`LUA_USE_JUMPTABLE`, `ljumptab.h`): `static const void *const disptab[] = { &&L_OP_MOVE, ... };` and `goto *disptab[op];`. Labels as values and computed `goto` were not supported in any mode (`ljumptab.h:28:1: error: expected an expression, found '&&'`). Clang accepts them even with `-std=c11` | [tests/gnu/10_labels_as_values.c](../../tests/gnu/10_labels_as_values.c); unit test `labels_as_values_and_computed_goto` in `src/lib.rs` | fixed (in `-std=gnu11`) |
| LUA-H1 | harness, not a compiler defect | lua.org sometimes drops TLS connections, so a single download failed about half the time | none | fixed in the fetch script (retries on both host names; CI caches the tree) |
| LUA-H2 | harness, not a compiler defect | the test suite prints random seeds, dates and timings, so its output differs between any two runs of the same binary | none | the suite's outcome is compared (exit status, "final OK !!!", and the list of test files reached), not its bytes; the workloads are compared byte for byte |

By category: frontend reject 1 (fixed); ICE 0; bad IR 0; wrong output 0; crash 0; performance 0.

**Language mode.** Lua 5.4.7 is written in C99 that strict C11 accepts, except for the jump table, which Lua selects only because the compiler says it is GNU-compatible. Labels as values are a GNU extension, so, like the other GNU extensions here, they are accepted only with `-std=gnu11`; strict C11 still rejects them with an error that names `-std=gnu11`. The final scorecard therefore builds both compilers with `-std=gnu11`. The frozen baseline used `-std=c11` for both: there every file except `lvm.c` in the release configuration compiled, and the ltests configuration (whose `ltests.h` turns the jump table off) built completely.

**What the fix does** (GCC manual, "Labels as Values"; LLVM Language Reference, `blockaddress` and `indirectbr`). `&&label` is a `void *` naming a label of the current function; it is lowered to `blockaddress(@function, %label)`. It is an address constant in a block-scope `static` initializer, which is how Lua builds its table. `goto *expr;` is lowered to `indirectbr`, listing every label of the function whose address is taken. These were also the only reason six GCC torture tests failed (`20040302-1.c`, `20041214-1.c`, `20071210-1.c`, `920501-4.c`, `920501-5.c`, `990208-1.c`); they now pass and were removed from the known-failure list.

**Deferred.** Differences of label addresses (`&&a - &&b`, used to make position-independent tables) and arithmetic on `void *`, which they need: Lua does not use them; GCC torture `comp-goto-1.c` and `pr70460.c` do. Local label declarations (`__label__`) and nested functions stay out of scope, as PLAN.md says.

**Not covered by this run.** The interactive interpreter tests in `main.lua` and the C-module loading tests in `attrib.lua` are skipped in the portable modes (`_port=true`), because they need readline-style terminal echo and shared libraries built by the suite's own makefile. Everything else in `all.lua` runs, including the long tests in portable mode and the white-box `T` tests in the ltests build.
