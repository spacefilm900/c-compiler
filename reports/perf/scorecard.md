# Compile-time performance scorecard

- Before: `d57f313` ([baseline.md](baseline.md), frozen); after: `44cc353` ([after.md](after.md))
- Reference: Debian clang version 19.1.7 (3+b1)
- Machine: x86_64, 8 CPUs, Linux-6.12.113+-x86_64-with-glibc2.41
- Median of 3 runs per TU; group rows sum the TUs; peak memory is the largest TU

Ours = this compiler's frontend plus `clang -O<n> -c` on its IR. Clang = Clang compiling the C directly.

## Totals, before -> after

| Input | TUs | Frontend | Frontend peak | O0 ours | O0 Clang | O0 ratio | O2 ours | O2 Clang | O2 ratio |
|---|---|---|---|---|---|---|---|---|---|
| doom | 82 | 7.33 s -> 2.70 s (3x) | 68 -> 25 MiB | 9.15 s -> 4.64 s | 2.58 s | 3.87x -> 1.79x | 11.82 s -> 7.39 s | 5.65 s | 2.24x -> 1.31x |
| lua | 34 | 4.45 s -> 1.54 s (3x) | 113 -> 35 MiB | 5.36 s -> 2.51 s | 1.35 s | 4.36x -> 1.86x | 8.01 s -> 5.19 s | 3.83 s | 2.15x -> 1.36x |
| shell | 1 | 5.59 s -> 0.40 s (14x) | 241 -> 100 MiB | 5.84 s -> 0.67 s | 0.35 s | 18.24x -> 1.90x | 8.63 s -> 3.38 s | 3.17 s | 2.76x -> 1.07x |
| sqlite3 | 1 | 111.35 s -> 1.34 s (83x) | 698 -> 320 MiB | 112.37 s -> 2.34 s | 1.33 s | 90.40x -> 1.77x | 129.99 s -> 20.51 s | 19.87 s | 6.46x -> 1.03x |
| torture | 5 | 0.10 s -> 0.06 s (2x) | 16 -> 15 MiB | 0.24 s -> 0.23 s | 0.18 s | 1.60x -> 1.25x | 0.60 s -> 0.60 s | 0.56 s | 1.16x -> 1.07x |

## Frontend phases, before -> after (seconds)

| Input | lex | preprocess | parse | sema | irgen | write |
|---|---|---|---|---|---|---|
| doom | 3.41 -> 0.99 | 2.91 -> 1.15 | 0.40 -> 0.26 | 0.25 -> 0.10 | 0.08 -> 0.06 | 0.01 -> 0.01 |
| lua | 1.88 -> 0.51 | 2.05 -> 0.75 | 0.22 -> 0.14 | 0.10 -> 0.04 | 0.05 -> 0.04 | 0.01 -> 0.01 |
| shell | 0.41 -> 0.11 | 4.84 -> 0.16 | 0.09 -> 0.05 | 0.11 -> 0.03 | 0.05 -> 0.04 | 0.01 -> 0.01 |
| sqlite3 | 1.26 -> 0.32 | 108.06 -> 0.36 | 0.57 -> 0.29 | 0.71 -> 0.15 | 0.24 -> 0.19 | 0.03 -> 0.03 |
| torture | 0.03 -> 0.01 | 0.04 -> 0.02 | 0.01 -> 0.01 | 0.01 -> 0.00 | 0.01 -> 0.01 | 0.00 -> 0.00 |

## LLVM backend on our IR vs Clang's whole compile (after)

| Input | O0 backend | O0 Clang | O2 backend | O2 Clang |
|---|---|---|---|---|
| doom | 1.93 s | 2.58 s | 4.69 s | 5.65 s |
| lua | 0.97 s | 1.35 s | 3.65 s | 3.83 s |
| shell | 0.27 s | 0.35 s | 2.99 s | 3.17 s |
| sqlite3 | 1.01 s | 1.33 s | 19.17 s | 19.87 s |
| torture | 0.17 s | 0.18 s | 0.54 s | 0.56 s |

## Notes

**Headline.** `sqlite3.c`'s frontend went from 111.35 s to 1.34 s (83x faster) and its peak memory from 698 MiB to 320 MiB. The 10 s target is met with a wide margin. Whole-compile ratios against Clang are now 1.8-1.9x at `-O0` and 1.03-1.36x at `-O2` on every input (they were 3.9-90x and 2.2-6.5x).

**What it was.** Profiling with `perf` (release build with debug symbols) found one quadratic algorithm and a lot of copying:

1. *Preprocessor splice* (the big one). Every macro expansion was inserted into the middle of the whole remaining token vector, moving the rest of the file each time: about 80% of the time was `memmove`. Replacements are now written over the already-consumed prefix. 111 s -> 3.7 s on `sqlite3.c`.
2. *Punctuator matching* built a heap vector for each of 55 candidate punctuators at every punctuation character. It now peeks four bytes once into a stack array.
3. *File names* were copied into every preprocessing token and every parser token. They are now shared (`Rc<str>`).
4. *Struct members* were cloned out of the record on every `.`/`->` member lookup and assignment check; they are now borrowed.
5. *Sema's location map* (2.5M SipHash inserts) is built only if a diagnostic needs it.
6. *Process teardown*: the command-line driver no longer frees the token list and syntax tree just before it exits.
7. *Preprocessor copies*: the cursor token was cloned before checking whether it was a macro, macro definitions were cloned on every expansion, and every output token was copied.
8. *Parser copies*: skipping a token no longer copies it.
9. *Token conversion*: identifiers, keywords, and punctuators become parser tokens without re-running the lexer.
10. *Hashing*: the macro table and hide sets use a small FxHash-style hasher, and hide-set unions are reused for neighbouring tokens.

Each fix is one commit with its own before and after numbers. The generated IR is byte-identical to the baseline compiler's on all 275 benchmark and `tests/` translation units after every fix, so no program can behave differently.

**What limits it now.** The profile is flat: no item is above about 5%. What is left is per-token allocation (each token still owns its spelling as a `String`) and a tree-walking design that copies types (`CType` clones) in sema and IR generation. Removing those means interning spellings and changing the token and AST types across the whole frontend, which is a bigger refactor than this phase. For Lua and DOOM, the frontend's time is mostly re-lexing and re-preprocessing the same system headers for every file (about 20-35 ms per file), which Clang does faster.

**No optimiser added.** The LLVM backend on our IR takes less time than Clang's whole compile of the same C at both levels (for example `sqlite3.c` at `-O2`: 19.17 s against 19.87 s), so IR volume does not dominate LLVM time and no IR clean-up pass was added. The IR text is still written to a file and handed to `clang`; with the backend this close to Clang, pipes would not change the picture.

**CI guard.** `tools/perf_guard.py` runs at the end of the SQLite job. It fails if the frontend takes more than 6x `clang -O0 -c` (or 30 s) on `sqlite3.c` or `shell.c`. Locally the ratios are 1.06x and 1.23x; the baseline compiler fails at 82.6x and 14.8x.

**Reproduce.** `tools/bench_compile.py --runs 3 --json OUT.json --md OUT.md` (sources fetched by the usual `scripts/fetch_*.sh`), then `tools/perf_scorecard.py`. Timings are wall clock on a shared 8-CPU box and move by a few percent between runs. The earlier milestone scorecards (DOOM, Lua, SQLite) were not re-measured with their own harnesses here, so their numbers are left as recorded.
