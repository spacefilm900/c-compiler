# hcc rebuild specification (hcc2)

**Status:** draft for the owner's review (PLAN.md step 16). It is not approval to run a rebuild. Whether and when a rebuild runs is the owner's decision.

**Written from:** hcc at `main` `d848f46` (the v1.0.0 release, 25 November 2026), its git and pull-request history (pull requests 1 to 77), `PLAN.md`, `STATUS.md`, `DESIGN.md`, `CHANGELOG.md`, `reports/`, `docs/C11_CONFORMANCE.md`, the outside reviews of the project, and the builder's own session notes. Appendix A maps every lesson to that evidence.

**Who it is for:** a fresh builder agent that has not worked on hcc, the independent reviewer, and the owner, Keith Haviland.

## Contents

1. [What this specification is for](#1-what-this-specification-is-for)
2. [Plain-English summary](#2-plain-english-summary)
3. [Roles and governance](#3-roles-and-governance)
4. [Clean-room rules](#4-clean-room-rules)
5. [The product to build](#5-the-product-to-build)
6. [Architecture: modules and the diagnostics model](#6-architecture-modules-and-the-diagnostics-model)
7. [Budgets](#7-budgets)
8. [An oracle for every phase](#8-an-oracle-for-every-phase)
9. [Quality tools in CI from the first milestone](#9-quality-tools-in-ci-from-the-first-milestone)
10. [The independent adversarial reviewer](#10-the-independent-adversarial-reviewer)
11. [Milestones and gates](#11-milestones-and-gates)
12. [Test assets and how they are fetched](#12-test-assets-and-how-they-are-fetched)
13. [Comparing the two builds](#13-comparing-the-two-builds)
14. [Working rules for the builder](#14-working-rules-for-the-builder)
15. [Risks](#15-risks)
16. [Decisions for the owner](#16-decisions-for-the-owner)
- [Appendix A: lessons and their evidence](#appendix-a-lessons-and-their-evidence)
- [Appendix B: hcc 1.0 baseline](#appendix-b-hcc-10-baseline)
- [Appendix C: words used here](#appendix-c-words-used-here)

## 1. What this specification is for

hcc is a C compiler. It was built by an agent, milestone by milestone, from 8 to 25 November 2026, through 77 pull requests and 660 commits on `main`, ending in release v1.0.0 after successive interim releases. The main aim of the work was an experiment in truly agentic software engineering.

The work was planned in phases, and these phases were adjusted as experience was gained. The use of oracles, review and testing by an independent reviewer, the use of quality tools, the control of budgets, the target platforms beyond Linux on Intel (x86-64), and the full C11 audit were each added incrementally.

This specification describes the same product in a way that can be built again from scratch by a fresh agent, incorporating the major lessons the original hcc agent gained on the way. We call this new compiler **hcc2**.

The intent is to create a more optimised process, to see whether a new agent can build a functionally equivalent compiler with fewer escaped defects, less rework and less cost, and whether the result is easier to maintain.

To answer this, hcc2 should use hcc's test suites unchanged and be measured the same way (section 13). The new agent may write new tests of its own, but the existing tests are the formal check that hcc2 is equivalent to hcc. This document fixes the rules, the plan and the measures before the rebuild starts, so that neither build is judged by rules made up afterwards.

The words **must**, **must not** and **should** have their usual meaning: *must* is a rule, *should* is a strong default that the builder may depart from only with a written reason in the decision log.

## 2. Plain-English summary

- **Same product, same core tests.** hcc2 is a C11 compiler, written in Rust, that turns C into LLVM IR for the same six targets as hcc 1.0. It is tested with exactly the same test suites, fetched the same way, so the two builds can be compared fairly. The new agent can create new tests, but must use the existing tests as the formal verification of equivalence.
- **Same clean-room rules.** The code is written only from public standards and manuals. Other compilers are used only as black boxes, to compare answers. The new agent must not read the original hcc's compiler code either.
- **A judge for every step from day one.** Every part of the compiler, from the first line of preprocessor output to the final program, is checked against Clang's answer automatically. In hcc most mistakes were caught this way, not by the agent's own reasoning. This reinforces the power of oracles.
- **The independent reviewer starts at the beginning.** Another agent, assumed to be ChatGPT, which never sees the code, writes tricky tests from the first milestone. In hcc it only started after the basics were built, and it found defects in every round.
- **The whole C11 standard is tracked from the start.** A clause-by-clause checklist exists on day one. In hcc the full audit only happened at release time, and two C11 features (multi-character constants and international identifiers) were found missing shortly before the release.
- **Budgets are set up front.** Money, CI minutes and compile speed have limits from the start, and CI fails when they are broken. In hcc, cost and speed were raised by the owner, not the agent.
- **The quality tools run from the first milestone.** Coverage, mutation testing, fuzzing, lint and size limits are part of CI from the first milestone, not a clean-up phase after the code has grown. Review findings that keep coming back become written rules and, where cheap, automatic checks (14.5).
- **The structure is planned.** The module layout and the way errors are reported are designed before the code is written, including one shared model of struct layout, the area where the last three review rounds kept finding bugs.
- **Agent summaries should be facts, not claims.** Every number in a status report must link to the evidence (a commit, a CI run or a report file), and is checked before it is published. hcc had several summaries that overstated results or went stale.
- **The owner stays in control.** Every hcc2 merge, tag, release and settings change still needs the owner's OK, as in hcc.

## 3. Roles and governance

### 3.1 Roles

| Role | Who | Does | Does not |
| --- | --- | --- | --- |
| Owner | Keith Haviland | Sets priorities, approves merges, tags, releases, settings and spending, settles disputed rulings, decides whether the rebuild runs | Write or review code |
| Builder | a fresh agent with its own box | Writes the compiler, its tests and its documentation; runs CI and the campaigns; reports with evidence | Merge, tag, release, change settings or spend money without the owner's OK; read the code of hcc, Clang, GCC or any C library |
| Independent reviewer | another agent, assumed to be ChatGPT, relayed by the owner | Writes adversarial test kits from the standard, the ABI documents and the builder's published `STATUS.md` and conformance matrix; rules on disputed cases | See the compiler source; change compiler code |
| Automated reviewer | CodeRabbit on every pull request | Reviews diffs | Approve merges |
| Oracles | Clang 19.1.7 (CI), GCC 14.2 (second opinion, by hand), LLVM's own verifier | Give reference answers as black boxes | Have their source read |

### 3.2 Approvals

These need the owner's explicit OK, given for that specific action:

- merging any pull request, including the builder's own and Dependabot's;
- creating a tag, a GitHub Release or a release build;
- pushing to `main` (which is protected and should never need a direct push);
- changing repository settings, rulesets, secrets or the CodeRabbit configuration;
- posting a pull-request comment on the owner's behalf, other than routine review replies;
- spending money: raising a budget, buying a seat or a service, or launching a cloud agent;
- changing this specification, the test kit (section 12.1) or a gate threshold.

These are normal workflow and need no extra OK: pushing to the builder's own pull-request branches, replying to and resolving CodeRabbit threads, asking CodeRabbit for a review, and running local tools and campaigns on the builder's box.

A "yes" to one action is not a "yes" to the next. An OK to merge is not an OK to tag.

### 3.3 Merge procedure

1. The pull request's head commit is the one that was reviewed. Record its SHA (the unique identifier Git gives every commit: a 40-character code, usually shortened to its first 7 characters, such as `d848f46`).
2. GitHub reports the merge state as clean, every required check has passed on that SHA, and the only skipped checks are ones the workflow skips by design.
3. CodeRabbit's check shows *Finished* on that SHA (a "review rate limited" result does not count), and the pull request has 0 unresolved review threads.
4. The owner has said yes.
5. Merge with the SHA pinned, so a late push cannot be merged by accident. If GitHub's API keeps failing (hcc hit repeated server errors when merging #77), stop and ask the owner to use the Merge button in the web page; do not retry in a loop.

### 3.4 Reporting to the owner

- The owner is not intended to act as a programmer or tester. Reports use plain words, short sentences and one recommendation, with the reasons.
- Quiet by default: report when a pull request is fully green and reviewed, when the owner asks, or when something is blocked or the plan has to change.
- A builder's choice is never presented as an "owner decision". Section 16 lists the real owner decisions.

### 3.5 Evidence rule: summaries must be factual

Every statement of a result, by any agent including the builder's own helpers, is a claim to be checked against evidence before it is published.

- Every number in `STATUS.md`, a report, a pull-request description or a message to the owner must carry its evidence: the commit SHA it was measured on, and a CI run link, a report file in the repository, or a command that reproduces it.
- A count must carry its denominator and its scope. Write "2,775 of 3,000 generated programs compared, 0 mismatches", never "3,000 passed". Write "92.9% of viable mutants caught in 7 selected files", never "92.9% mutation score".
- A claim about the standard cites the clause (for example "C11 6.4.4.4p10").
- A claim that something is "full", "complete", "all" or "none" must name the check that proves it. If there is no such check, the claim must be qualified.
- When a helper agent returns, the builder checks its stated results against the branch, the CI run and the files before reporting them. A helper's summary is not evidence.
- The documentation is checked against the compiler automatically. Where the README, a user guide or `docs/LANGUAGE.md` says that the compiler accepts or rejects some piece of C, a small test program goes with that statement, and a script compiles each one and fails CI when the compiler no longer does what the document says. It runs on every pull request and again before an outside review or a release. hcc's `tools/check_docs.py` does this; hcc2 writes its own.

## 4. Clean-room rules

These carry over from hcc unchanged in spirit. They are mandatory.

1. **Allowed sources for compiler code.**
   1. the C11 standard (N1570, the final public draft of ISO/IEC 9899:2011);
   2. the LLVM Language Reference;
   3. the platform ABI documents (System V AMD64 psABI, AAPCS64 and Apple's arm64 rules, the Microsoft x64 calling convention);
   4. the Rust documentation;
   5. other public specifications that the compiler's output or input must follow: the IEEE 754 floating-point standard, the Unicode standard (for the characters allowed in identifiers), the object-file and debug formats (ELF, Mach-O, COFF, DWARF) and the C library standards (POSIX for `<threads.h>` over pthreads, and the Win32 thread API documentation);
   6. and, for the GNU compatibility layer only, the public GCC user manual's documentation of builtins, attributes and language extensions.
2. **Forbidden sources.** The source code of Clang, GCC, any other C compiler (including Claude's C Compiler and nqcc) and any C library must not be read, copied, translated or closely paraphrased.
3. **The original hcc is also off limits.** For the experiment to be fair, the builder must not read hcc's compiler source (`src/`, `lib/`, `include/`, `build.rs`, `fuzz/`) or its design notes beyond this specification. It may read and run the reused test kit (section 12.1) and the public documents named in this specification. Interface names given here (command-line options, exit codes, file names) are not code.
4. **Oracles are black boxes.** Clang and GCC may be run, and their output, diagnostics, `-E` output, `-dM -E` macro lists and emitted IR may be compared. Their source may not be read. The same holds for LLVM's tools.
5. **Test inputs stay test inputs.** WACC, GCC torture, Csmith, YARPGen, One True AWK, the AWK suites, DOOM, Lua, SQLite, QuickJS and zlib are inputs only. They are fetched by script at a pinned version, not vendored, and never patched. Compiler code must not be shaped around one of them; a failure is fixed in the general C feature, with its own test.
6. **No compiler code from other models.** Only the builder agent writes compiler code. ChatGPT and other assistants may write test programs and reviews. Their compiler code, if offered, must not be used.
7. **Third-party tools are black boxes.** cargo-fuzz, cargo-mutants, cargo-llvm-cov, Csmith, YARPGen, C-Reduce or C-Vise, gitleaks and similar tools are run, never read or adapted.
8. **Provenance is recorded, not just asserted.** The repository keeps a provenance section listing the sources used. Each pull request that adds a language feature names the clauses or manual sections it was written from. This is a record of how the code was made, not proof; an outside review noted that inspecting code cannot prove the absence of copying.

## 5. The product to build

### 5.1 Language

- **C11 (N1570), strict by default** (`-std=c11`). The whole language, including the parts hcc added later: `_Alignas`, `_Thread_local`, K&R definitions, `<tgmath.h>`, variable-length arrays, `_Atomic` with `<stdatomic.h>`, `<threads.h>`, trigraphs, `_Pragma`, `__DATE__` and `__TIME__`, universal character names and UTF-8 in identifiers, and multi-character character constants.
- **Optional features** follow the standard's feature macros. `_Complex` is left out, with `__STDC_NO_COMPLEX__` defined, exactly as in hcc 1.0; implementing it later is an owner decision (section 16).
- **Invalid programs are rejected.** Every constraint or syntax violation gets an error and a non-zero exit status. This is stricter than 5.1.1.3 requires; hcc used the same rule and its conformance probes check it.
- **The tie-break rule** for anything the standard leaves to the implementation: the standard first, then the target's ABI document, then a choice made by hand and written down with its reason, usually to match GCC and Clang on that target so objects link. Clang is the CI oracle. hcc's first wording ("follow the GCC and Clang majority") was not a real rule with two compilers and had to be corrected.
- **GNU compatibility** is a separate, opt-in layer, `-std=gnu11`, with the same scope as hcc 1.0's (statement expressions, `__typeof__`, `__attribute__`, case ranges, range designators, labels as values, `__int128`, the builtins that map onto library calls or LLVM intrinsics, and the object-size checking builtins the macOS SDK uses). Every GNU feature is switched on in one dialect table, so strict mode cannot accept it by accident.

### 5.2 Output and toolchain

- Textual LLVM IR, turned into machine code by LLVM's own tools. Clang is the linker driver for `--link`.
- Rust, stable, with the standard library only. The toolchain is pinned in `rust-toolchain.toml`. For a fair comparison it should be Rust 1.85.1, with Clang and LLVM 19.1.7 as the oracle, as in hcc; a newer pin is an owner decision.
- No own back end. LLVM is the back end; an experimental own back end is outside this specification (PLAN.md step 18 covers it for hcc). It may also be added to hcc2 later, as a separate stage after this specification is complete.

### 5.3 Target platforms

All six of hcc 1.0's targets, with the same target triples. A triple is the standard name LLVM and Clang use for a platform, written as processor, vendor and operating system, for example `x86_64-linux-gnu` (64-bit Intel or AMD, Linux, GNU C library):

| Target | ABI | How it is tested |
| --- | --- | --- |
| `x86_64-linux-gnu` (default) | System V AMD64 | natively on every pull request |
| `aarch64-linux-gnu` | AAPCS64 | qemu on pull requests; native arm64 runner on `main` and weekly |
| `arm64-apple-macos` | Apple arm64 | IR and object checks on pull requests; native Apple Silicon runner on `main` and weekly; the owner's MacBook Air for DOOM |
| `x86_64-apple-macos` | System V with Apple's object format | stand-in on pull requests; native Intel Mac runner by hand |
| `x86_64-w64-windows-gnu` (MinGW-w64) | Microsoft x64 | Wine on pull requests; native Windows runner weekly |
| `x86_64-pc-windows-msvc` | Microsoft x64, MSVC headers and libraries | Wine stand-in on pull requests; native Windows runner weekly |

The difference from hcc: **the target description covers all six from the first milestone** (section 6.3), even though programs run only on x86-64 Linux at first. In hcc each new platform was added to a design that assumed x86-64 Linux, which meant retrofitting: `long` is 32 bits on Windows, `long double` is a different format on different targets, and plain `char` is unsigned on arm64 Linux.

### 5.4 Command-line interface

The reused test kit drives the compiler through its command line, so hcc2 must keep hcc 1.0's interface exactly. The kit's contract is `build/hcc INPUT.c -o OUTPUT.ll`, producing textual IR with exit status 0; the binary's path can be overridden with the `CC_UNDER_TEST` environment variable.

- **Build:** `make` writes the hcc2 binary to `build/hcc`, the path the reused kit expects (not every reused script reads `CC_UNDER_TEST`). `--version` names it hcc2.
- **Options:** `-o`, `--link`, `-E` (also `--dump-pp`), `--dump-tokens`, `--dump-ast`, `--dump-sema`, `--dump-ir`, `-O0` to `-O3`, `-Os`, `-Oz`, `-l`, `-L`, `-Wl,`, `-pthread`, `-static`, `-I`, `-D`, `-U`, `-std=c11`, `-std=gnu11`, `-trigraphs`, `-fgnu89-inline`, `-target TRIPLE` (also `--target=TRIPLE`), `-isysroot`, `-fno-caret-diagnostics`, `--time-phases`, `--version`, `-h` and `--help`.
- **Environment:** `HCC_CLANG` names the Clang used by `--link`; `SOURCE_DATE_EPOCH` fixes `__DATE__` and `__TIME__`.
- **Exit status:** 0 for success, 1 for errors in the program, 2 for a command-line error (with a one-line reason), 3 for an internal compiler error.
- **Runtime pieces:** compiler-provided headers (for example `<stdarg.h>`, `<stddef.h>`, `<stdalign.h>`, `<tgmath.h>`, and `<threads.h>` on targets whose C library has none) and the `<threads.h>` runtime for macOS and Windows, linked automatically by `--link`. They are written fresh.
- The dump formats may differ from hcc's; section 12.1 says how that is handled.

## 6. Architecture: modules and the diagnostics model

### 6.1 Planned module structure

The builder writes `docs/ARCHITECTURE.md` in the first milestone, before the code grows, with one section per module: what it owns, what it may call, its dump format and its tests. The layout below is required in substance; names may differ if the decision log says why.

| Module | Owns | Depends on |
| --- | --- | --- |
| `driver` | command line, target selection, phase order, `--link` | the modules below; nothing depends on it |
| `diag` | diagnostics, source positions, exit status (section 6.2) | `source` |
| `source` | files, the include stack, line maps (`#line`), macro-expansion history | none |
| `target` | the target description (section 6.3) | none |
| `lex` | tokens, literals, character sets, universal character names | `source`, `diag` |
| `pp` | preprocessing: directives, macros, `#if` evaluation, includes, `_Pragma` | `source`, `diag`, `target`, `lex` |
| `parse` | the syntax tree and declarators, and the constant folding the grammar needs | `lex`, `diag` |
| `types` | the C type model, compatibility and composite types | `target` |
| `layout` | the single record-layout model (section 6.3) | `types`, `target` |
| `sema` | name lookup, type checking, conversions, constant evaluation, initializers, statement rules | `parse`, `types`, `layout`, `diag` |
| `abi` | calling-convention lowering for each target, reading `layout` | `types`, `layout`, `target` |
| `irgen` | lowering the checked tree to IR | the output of `sema`, `abi`, `layout` |
| `ir` | the IR text writer, and the hook that runs LLVM's verifier in tests | none |
| `float` | exact compile-time arithmetic for every floating format (`float`, `double`, x87 80-bit, binary128) | none |

Rules:

- **Dependencies point one way.** A later phase never re-derives a fact an earlier phase owns. In particular `abi` and `irgen` read struct layout from `layout` and never compute it again.
- **Size limits from the first commit:** no source file over 1,500 lines and, if possible, no function over 120 lines or above cyclomatic complexity 25, unless the decision log records an exception and why. hcc had to split three files of over 6,000 lines (the largest 8,766) after the fact.
- **One owner per rule.** Each C rule (for example "the address of a `register` object may not be taken") is checked in one place, named in the conformance matrix. hcc's late `register` findings came from the same rule being checked separately for parameters, K&R declarations and `_Generic`.
- **Every phase can be dumped.** `--dump-tokens`, `--dump-pp`, `--dump-ast`, `--dump-sema` and `--dump-ir` exist from the milestone that adds the phase, with deterministic, documented formats and golden tests. hcc added them only in its quality phase.
- **Deterministic output.** The same input, options and `SOURCE_DATE_EPOCH` give byte-identical IR, run to run and machine to machine (without `SOURCE_DATE_EPOCH`, `__DATE__` and `__TIME__` change with the clock). A CI check compiles a fixed set twice, with `SOURCE_DATE_EPOCH` set to the same fixed value both times, and compares.
- **Design and history are separate.** `docs/ARCHITECTURE.md` and `docs/LANGUAGE.md` hold the current design; dated decisions go in `docs/DECISIONS.md`; `STATUS.md` holds the current state only. hcc's `DESIGN.md` and `STATUS.md` mixed the two, and `STATUS.md` grew to over 1,300 lines; an outside review flagged it.

### 6.2 Diagnostics model

Designed in the first milestone and used by every module.

- **A diagnostic has** a severity (error, warning or note), a primary source position (file, line and column, after `#line` mapping), the macro-expansion and include history that led there, a message, an optional C11 clause reference, and a stable identifier that tests use. Notes attach to their error.
- **Format:** `file:line:column: severity: message`, then the source line and a caret unless `-fno-caret-diagnostics` is given. Messages go to standard error in source order, deterministically.
- **Recovery:** after an error the compiler continues when it is safe to, but a cascade guard limits follow-on errors from one cause.
- **No crashes on any input.** Compiler code must not panic. `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!` and indexing that can fail are not allowed in non-test code, enforced by Clippy lints from the first milestone; each justified exception is commented and listed. hcc started with 333 such crash points and spent a phase removing the unclear ones.
- **Internal errors are diagnostics too.** When a later phase finds something an earlier phase should have rejected, it reports `file:line:column: internal compiler error: message (in PHASE)` and exits with status 3. A fuzzer finding an internal error has found a bug, but never a crash.
- **Dialects:** a GNU feature used under `-std=c11` is an error that names the GNU mode, and a GNU-mode relaxation (for example a pointer-sign mismatch) is a warning, never silent.
- **Required diagnostics are tested both ways:** each constraint in the conformance matrix has a probe that must be rejected and a nearby valid program that must be accepted. hcc's v0.1 to v0.6 review rounds found mostly missing required diagnostics, and its pre-release audit found 12 more accepted constraint violations. Over-strict rejections were found too (the 16 v0.9 failures were valid programs that hcc rejected).

### 6.3 One target description and one record-layout model

The outside review of hcc noted that its v0.7 to v0.9 defects clustered in record layout and over-aligned bit-fields: three rounds in a row found the same family (`_Alignas` members in nested arrays, then bit-fields next to `_Alignas` members, twice). hcc's layout was computed in one module, and the calling-convention code classified the laid-out records again in three more (`DESIGN.md`, "After 1.0"). The rebuild must avoid this.

- **`target`** is one description per triple: type sizes and alignments (including `long`, `long double`, `wchar_t` and the signedness of plain `char`), the LLVM data layout and triple, predefined macros (taken from `clang -dM -E` as a black box), object-format details (symbol prefixes, section names), and the C library's header search. All six targets exist from the first milestone.
- **`layout`** is the only place struct and union layout is decided: member offsets, bit-field storage units, padding, `_Alignas`, flexible array members, and GNU `packed` and `aligned`, with each target's rules (System V, AAPCS64, Apple arm64, MSVC). It produces one layout record that `sema` (`sizeof`, `offsetof`, constant initializers), `abi` (argument and return classification) and `irgen` (member access and static images) all read.
- **Layout is checked automatically.** For every record in the test corpus, the offsets and size in `layout` must equal what Clang reports for the same target (a probe program that prints `sizeof`, `_Alignof` and `offsetof`) and what the emitted IR implies. Bit-fields need their own oracle, because `offsetof` may not name a bit-field (C11 7.19p3): for every bit-field record, the bytes of initialized objects must equal those of Clang's build for the same target, and cross-compiler access tests read and write each bit-field from Clang-built code and from the compiler's code, in both directions. A layout fuzzer, and random calling-convention pairs linking the compiler's objects with Clang's in both directions, run in CI from the milestone that adds structs.

## 7. Budgets

Budgets are fixed before the first milestone and checked by CI or a monthly report. Breaking one is reported to the owner the same day with a proposal; it is never handled silently.

### 7.1 Money

| Item | Budget | Notes from hcc |
| --- | --- | --- |
| GitHub Actions | the owner's account limit ($50 a month, as on hcc's account now) | CI was blocked on 14 November 2026 by a $0 spending limit until the owner set $30, now expanded. Standard runners for a public repository do not use paid minutes. |
| CodeRabbit | one seat, as in hcc | about one review an hour; see 7.3 |
| Cloud agents | none, unless the owner approves a specific one | hcc's first 19 pull requests came from cloud agents; the owner paused them on 11 November 2026 and the rest of the project ran on the builder's own box |
| Anything else | none without the owner's OK | |

A monthly cost report goes in `reports/budget/`: Actions minutes by runner type (from the billing data), the CodeRabbit seat and any other spend, against the budget.

### 7.2 CI minutes

- **Pull-request CI** must finish within 25 minutes wall-clock and should stay under about 60 billed Linux minutes per run. It runs once per change: on the pull request, not again for the branch push. hcc's CI first ran twice per push; fixing that cut a pull request's checks from 19 to 10. Documentation-only changes run only a short documentation job.
- **Expensive runners** (macOS, Windows, and native arm64 if billed) run only on merges to `main`, weekly, or by hand. In a private repository, macOS minutes cost about ten times Linux minutes. hcc's weekly real-platform run cost about 37 billed minutes, about $0.44 a run.
- **Long campaigns** (hours of fuzzing, thousands of Csmith or YARPGen programs, whole-crate mutation testing) run on the builder's box or as a manual or weekly job, never in pull-request CI.
- CI records its own duration. A pull request that makes the pull-request workflow more than 20% slower than the 7-day median must explain why in its description.

### 7.3 Review throughput

- CodeRabbit reviews about once an hour per account, and a "review rate limited" result is not a review. The builder batches fixes, asks for a review only when the head is ready, and plans the waits. In hcc one pull request (#70) waited through several review slots across an evening.
- Large pull requests draw more findings. hcc's 494 CodeRabbit review threads across pull requests 1 to 77 were concentrated in the largest changes (29 on #49, 25 on #56, 23 on #70). A pull request should be small enough to review in one pass: as a guide, under 1,500 changed lines of compiler code.

### 7.4 Compile time and memory

hcc had no compile-time budget until the owner asked for one, after Lua. SQLite's 271,131-line `sqlite3.c` then took 111 seconds in the frontend, and the whole compile was 90 times slower than Clang's at `-O0`, because of one quadratic algorithm in macro expansion. The rebuild sets the budget on day one:

| Measure | Gate | From |
| --- | --- | --- |
| Frontend time per translation unit | at most 3x `clang -O0 -c` on the same file, and never over 30 s | M2, on the largest inputs that compile at that point; `sqlite3.c` from the first milestone where it compiles |
| Scaling | doubling a generated input (more functions, more macro uses, deeper includes) takes at most 2.5x the time | M2 |
| Whole compile against Clang | at most 2.0x at `-O0` and 1.4x at `-O2` on DOOM, Lua and SQLite | M5 (hcc 1.0: 1.8 to 1.9x and 1.03 to 1.36x) |
| Peak memory | at most 400 MiB on `sqlite3.c` | M5 (hcc 1.0: 320 MiB) |
| Speed of the compiled programs | within 10% of Clang's at the same optimisation level on the DOOM, Lua, SQLite, QuickJS and zlib workloads | M5 |

`--time-phases` exists from the first milestone so every phase can be timed, and CI fails when a gate is broken.

### 7.5 Agent sessions

The builder's helper agents were interrupted often in hcc, roughly every 50 minutes in the last days before the release, and resumed from the git state. No work was lost because everything went through commits. The rebuild plans for this:

- Work in steps that finish, commit and push within about 30 minutes.
- Keep a working-notes file outside the repository (section 14.4) with the current branch, head SHA, what is running and the next step, updated at every step.
- Start long runs in the background with their own logs and a results file, so a resumed agent reads the results rather than rerunning.
- Record each interruption in the notes (time and what was lost), so the comparison can count them. hcc did not record them.

## 8. An oracle for every phase

In hcc almost every real mistake was caught by comparison with an outside judge (Clang's output, a test suite, a real program), not by the agent's own reasoning. The rebuild has a judge for every phase from the milestone that adds it.

| Phase | Oracle | Check | Runs |
| --- | --- | --- | --- |
| Lexing | Clang's token dump, as a black box, and the standard's grammar | token kinds and spellings agree on every test file | every pull request |
| Preprocessing | `clang -E` | the output is token-for-token equivalent (ignoring whitespace and line markers) on every test file and every header the suites use; predefined macros agree with `clang -dM -E` | every pull request |
| Parsing and semantic checks | `clang -std=c11 -pedantic-errors -fsyntax-only`, and the conformance matrix | accept or reject agrees on every probe; where Clang and the standard differ, the standard wins and the case is listed | every pull request |
| Types and layout | probe programs built by Clang that print `sizeof`, `_Alignof` and `offsetof`; the layout fuzzer | identical numbers on all six targets | every pull request |
| Calling conventions | objects from Clang, linked with hcc2's objects in both directions | hand-written and random argument and return pairs agree | every pull request (x86-64 natively, other targets under qemu or Wine) |
| IR | `llvm-as` and `opt -passes=verify` | every module the compiler writes is valid IR | every pull request, and on every fuzzer input the compiler accepts |
| Code generation | Clang's build of the same program | the same exit status and output, at `-O0` and `-O2` | every pull request |
| Whole programs | Clang's builds of DOOM, Lua, SQLite, QuickJS, zlib and One True AWK | the same frame hashes, test results, database files and outputs | from the milestone that adds them; then on `main` and weekly |
| Performance | Clang's compile time and the compiled programs' run time | the budgets in 7.4 | every pull request (frontend), weekly (whole programs) |
| Behaviour the standard leaves open | the standard, the ABI document, then a written decision (5.1) | recorded in `docs/LANGUAGE.md` with the reason | when the choice is made |

**The harness is tested too.** Before the first compiler code, the differential harness is run with Clang itself as the compiler under test and must pass everything. The harness calls the compiler as `COMPILER INPUT.c -o OUTPUT.ll` and expects textual IR, and calls it with `-E` to preprocess, so `CC_UNDER_TEST` names a small adapter script rather than Clang itself: for a compile it runs `clang -S -emit-llvm` with the same input, options and output, adding `-std=c11 -pedantic-errors` when the call names no `-std` (Clang's own default is a GNU dialect, and the compiler's default is strict C11), adding `-pedantic-errors` when the call already names `-std=c11` (Clang without that flag still accepts some GNU extensions), and keeping `-std=gnu11` unchanged; it passes `-E` and every other mode through to Clang the same way. The adapter is the rebuild's own file and lives outside the read-only `testkit/` (for example `tools/clang_as_candidate.sh`). Seeded faults in the adapter must be caught, including a wrong constant in the IR, a dropped diagnostic, and one that drops `-pedantic-errors` from an explicit `-std=c11` call on a GNU-extension input. A harness that cannot fail proves nothing.

## 9. Quality tools in CI from the first milestone

hcc ran these tools for the first time in its quality phase (PLAN.md step 8) and its hardening phase (step 14), after about 38,600 lines had been written. In the rebuild they are part of CI from the first milestone, and their numbers are gates that may rise but must not fall.

| Tool | Pull-request CI | Weekly or by hand | Gate |
| --- | --- | --- | --- |
| `cargo fmt --check` | yes | | clean |
| Clippy with warnings as errors, plus the no-panic lints of 6.2 | yes | | clean |
| Unit tests | yes | | all pass |
| Differential harness, conformance probes, documentation-claims check | yes | | all pass, with known failures listed |
| Line and branch coverage (`cargo llvm-cov`) | yes, on the unit tests and the harness | the full test set | at least 80% of lines in M1; 85% of lines and 75% of branches from M2; 92% and 83% by release (hcc 1.0: 92.7% and 83.4%). A pull request may not lower either by more than 0.5 points without a reason |
| Mutation testing (`cargo-mutants`) | on the files the pull request changes, with the unit tests as the oracle and a 20-minute cap | the whole crate, monthly | at least 80% of viable mutants caught in changed files; the monthly whole-crate score is reported with its scope (hcc 1.0: 92.9% in 7 selected files) |
| Fuzzing (`cargo-fuzz`) of the lexer, the parser and the whole frontend from M1, and of the preprocessor from M2 | 2 minutes per target, as a smoke test | 1 hour per target, weekly | no panic, no hang, no invalid IR from an accepted input |
| Csmith and YARPGen | a fixed set of Csmith seeds from M4, because hcc's runner compiles them with `-std=gnu11` and the preprocessor (hcc: 40 seeds) | 3,000 Csmith and 2,000 YARPGen programs before each release | 0 mismatches and a minimum valid-comparison count, as defined below |
| Complexity and size report | yes | | the limits in 6.1 |
| Sanitizers (ASan, UBSan, TSan) on compiled test programs | | weekly | no new findings |
| Secret scanning (GitHub's own, or gitleaks as a black box) | yes | | clean |
| Dependabot for the workflow actions | | weekly | each update reviewed like any pull request |

A Csmith or YARPGen campaign passes only when there are 0 mismatches and the valid-comparison count meets hcc's recorded baseline for the same seed range. Csmith's counts are per optimisation level (seeds 200001–203000: 2,775 at `-O0` and 2,774 at `-O2`). YARPGen's count is once per seed, not per level: a seed counts only when both `-O0` and `-O2` agree with the reference (seeds 1–2000: 1,651). The figure 1,651 is not applied to each level. If the rebuild wants YARPGen per-level counts, it records them and sets their baselines from its own hcc 1.0 runs. A seed is a valid comparison only when both compilers accepted the program and both programs ran to a compared result. A candidate that rejects valid GNU C, a timeout, or a missing checksum is not a valid comparison and is not a pass. Generated, compared and mismatch counts are reported together. hcc's campaign scripts record a reject or a timeout without treating it as a mismatch, and they exit 0 even when no seed was compared, so the rebuild wraps them with a check that applies this gate; the reused scripts themselves are not edited.

The fuzz crate needs a nightly Rust toolchain. It lives outside the stable build, as in hcc, and only the fuzz job installs nightly.

A test-case reducer (hcc wrote its own, using the published ddmin algorithm) is written in the first milestone that has real programs, so every crash or miscompile is shrunk before it is fixed. hcc reduced its early Csmith failures by hand.

## 10. The independent adversarial reviewer

### 10.1 Rules

- The reviewer never sees the compiler source. It works from N1570 (the final public draft of the C11 standard, with the same technical content as ISO/IEC 9899:2011), the ABI documents (each platform's published rules for how compiled code calls functions and lays out data, listed in section 4), the builder's `STATUS.md`, `docs/LANGUAGE.md` and the conformance matrix found at `docs/C11_CONFORMANCE.md` in hcc2's repository (section 10.3).
- The builder never changes the reviewer's tests. If a test is wrong (it relies on undefined behaviour, or the standard says otherwise), the builder sends the evidence back with the clause, and the reviewer rules.
- **Freeze before fixing.** The first run of each new kit is committed, untouched, before any analysis or fix. That first-run score is a main comparison number (section 13).
- Every failure is reduced, gets a regression test first, and is fixed in a normal pull request.
- The reviewer's kits are kept byte-for-byte as received, with their checksums.
- The main agent and the reviewer communicate in human-readable memos or reports, relayed by the owner, so the owner can read every exchange.

### 10.2 From the start

In hcc the reviewer started after the basics were built, and found defects in every round. v0.1 to v0.6 (87 to 724 cases) found nine root-cause families, mostly missing required diagnostics. v0.7 to v0.9 found record-layout defects in each of three rounds. Then an outside review of the release candidate found two C11 features missing (universal character names in identifiers, and multi-character constants), and the clause-by-clause audit at release time found two wrong behaviours and 12 accepted constraint violations.

So in the rebuild:

1. **M0:** the reviewer receives this specification and writes a short kit for the first milestone's subset, and reviews the structure of the conformance matrix.
2. **Every milestone:** a new round on what that milestone added, run and frozen before the milestone is accepted. Each round covers required diagnostics as well as behaviour.
3. **Each platform:** a short round on its calling rules and type model.
4. **Before release:** a review of the whole repository, including the documentation's claims, as hcc had at the end.

How hcc's existing kits v0.6 to v0.9 are reused is in section 12.2. They are held-out comparison material, not a substitute for new rounds.

### 10.3 The C11 conformance matrix

`docs/C11_CONFORMANCE.md` exists from M0. It lists every subclause of N1570 clauses 5 to 7 with what it requires (behaviour, constraints, limits), the module that owns it, the probes that test it, and its status: not started, partial, done, or not applicable with a reason. Each milestone's acceptance names the clauses it completes. "Full C11" may be claimed only when every row is done or justified and the matrix's own runner passes. hcc wrote its audit in its last pull request before the release.

## 11. Milestones and gates

Each milestone ends with every gate measured on one commit and recorded in `reports/milestones/MN.md` with that commit's SHA, and with the owner's OK to move on. A gate that is not met is reported, not redefined. The numbers in square brackets are hcc 1.0's, from Appendix B.

The order differs from hcc's on purpose: all of C11 comes before the GNU layer and the big programs, and every target is in the type model from the start.

### M0: Foundations (no compiler code yet)

- The repository with branch protection, the merge procedure of 3.3, CodeRabbit, Dependabot, secret scanning, `SECURITY.md`, `CONTRIBUTING.md`, and a licence chosen by the owner.
- The test kit (12.1) imported at its pinned commit with its checksums; every fetch script runs and verifies its checksums.
- The differential harness runs with Clang, through the adapter of section 8, as the compiler under test and passes, and catches seeded faults.
- A CI skeleton with the budgets of section 7 wired in; the pull-request workflow under 10 minutes.
- `docs/ARCHITECTURE.md` (6.1), the diagnostics model (6.2), the target description for all six targets (6.3), a `docs/LANGUAGE.md` skeleton, the conformance matrix with every row present, and `docs/DECISIONS.md`.
- The reviewer has the specification and has returned its first kit.

**Gate:** all of the above exist and CI is green; the owner approves the architecture and the budgets.

### M1: A first C subset

Integers, pointers, arrays, functions, control flow and simple structs, without a preprocessor: hcc's Stage 1.

**Gates:** the WACC chapters that cover the subset pass with no known failures; the matching harness programs pass; every pull-request job of section 9 that applies to M1 is on and green (the preprocessor fuzz target starts in M2, and the Csmith and YARPGen jobs in M4), with line coverage at least 80%; the fuzz smoke test of the M1 targets is clean; every compile's IR is verified; the first reviewer round is frozen and then fixed; `CODING_STANDARDS.md` exists and the first review of recurring findings (14.5) is recorded; no file is over 1,500 lines.

### M2: Preprocessor, types, headers and the ABI

The preprocessor, the full type system, floating types in every format, varargs, `goto`, `switch`, designated initializers, bit-fields, `volatile`, typedefs, unions, system headers, struct passing, and the single layout model on all six targets.

**Gates:** WACC chapters 1 to 18 all pass [1,384 passed, 233 skipped]; `clang -E` equivalence on every test file; layout agrees with Clang on all six targets for every record in the corpus, including the bit-field byte and access checks of 6.3; random calling-convention pairs pass in both directions, natively on x86-64 and under qemu or Wine on the others; One True AWK builds and matches Clang's build; coverage at least 85% of lines and 75% of branches; the frontend and scaling gates of 7.4 hold on One True AWK, the torture tests that compile without the GNU layer, and a generated stress file (and on `sqlite3.c` as soon as it compiles); the second reviewer round is frozen and fixed.

### M3: All of C11

Everything in 5.1: `_Generic`, `_Static_assert`, `_Alignas` and `_Alignof`, compound literals, flexible array members, Unicode literals, universal character names and UTF-8 in identifiers, multi-character constants, trigraphs, `_Pragma`, K&R definitions, `<tgmath.h>`, `_Thread_local`, variable-length arrays, `_Atomic` and `<stdatomic.h>`, `<threads.h>` with its runtime for macOS and Windows, and the translation limits of 5.2.4.1.

**Gates:** the conformance matrix is complete for clauses 5 to 7 and its probes pass [286 of 288; the 2 are the inline-definition gap of 6.7.4p3]; held-out kit v0.6 has its first run recorded, then passes 724 of 724 at `-O0` and `-O2` [hcc's first run of v0.6: 722 of 724]; held-out kits v0.7, v0.8 and v0.9 have their first runs recorded, then pass 65, 80 and 100 at both levels [hcc's first runs: 64 of 65, 76 of 80, 84 of 100]; the reviewer's own M3 round is frozen and fixed.

### M4: The GNU layer

`-std=gnu11`, as in 5.1.

**Gates:** the Csmith CI seeds pass; GCC torture `execute` has 0 unexpected failures and at most 334 listed known failures of 1,746 executed [1,412 passed, 334 known]; strict mode still rejects every GNU feature, each with a strict-mode rejection test; a Csmith campaign of 2,000 seeds at `-O0` and `-O2` has 0 mismatches and at least as many valid comparisons, at each level, as a rerun of the same seeds with hcc 1.0, with the counts reported in full.

### M5: Big programs

DOOM (doomgeneric with the Freedoom demos), Lua 5.4.7, SQLite 3.54.1, QuickJS, zlib, One True AWK and the AWK suites, each with a scorecard like hcc's (`reports/<program>/scorecard.md`) and a frozen first-build baseline.

**Gates:** DOOM's frame hashes match Clang's build on every frame at `-O0` and `-O2` [39,209 of 39,209]; Lua's test suite and workloads, SQLite's workloads and database files, QuickJS's tests, zlib's tests and round trips, and One True AWK's tests and the AWK suites agree with Clang's builds at both levels; every compile-time and run-time gate of 7.4 holds.

### M6: Native platforms

Native runs on all six targets, using the target description that has existed since M0.

**Gates:** on native arm64 Linux, Apple Silicon macOS and Windows (MinGW-w64 and MSVC), the unit tests, the harness, random calling-convention pairs and the v0.7 to v0.9 kit cases pass, with platform known failures listed and justified [hcc's weekly platform run]; on Intel macOS, the harness, Lua, SQLite, DOOM and GCC torture pass when run by hand; the owner plays DOOM on his MacBook Air; a short reviewer round per platform is frozen and fixed; the weekly platform workflow costs at most about $1 a run if its minutes are billed.

### M7: Hardening

**Gates:** at least 13 hours of fuzzing across the targets with 0 failures [13.2 hours, 11 million inputs, 0 failures]; Csmith 3,000 programs at `-O0` and `-O2` with 0 mismatches and at least 2,775 and 2,774 valid comparisons at those levels, and YARPGen 2,000 programs with 0 mismatches and at least 1,651 valid comparisons counted once per seed (not per level), with the counts in full; coverage at least 92% of lines and 83% of branches; the whole-crate mutation score reported with its scope; sanitizer runs clean.

### M8: Release candidate and release

A version number and `--version`, `CHANGELOG.md`, user guides, `--help`, release builds for four platforms (Linux x86-64 and arm64, macOS arm64, Windows x64) with `SHA256SUMS`, a per-target status table, and the provenance section.

**Gates:** every suite is rerun on the exact release-candidate commit and recorded against it [`reports/release-1.0/README.md`]; the documentation-claims check passes; an outside review of the candidate is answered point by point; the changelog's release date is set when the tag is made, not when the pull request is written (hcc's 1.0.0 entry says 24 November; the tag was made on 25 November); the owner approves the tag and the release; the published downloads are checked against their checksums and run hello world.

## 12. Test assets and how they are fetched

### 12.1 The reused test kit

The rebuild reuses hcc's tests unchanged, so the two builds can be compared. They are copied from hcc's `v1.0.0` tag (`d848f46`) into a read-only `testkit/` directory in hcc2's repository, with a `SHA256SUMS` file. Nothing in `testkit/` is edited; a change needs the owner's OK and a rerun of both builds.

| Taken from hcc | What it is | Notes |
| --- | --- | --- |
| `tests/` (the `.c` programs and `gnu/`, `hosted/`, `posix/`, `xabi/`, `arm64/`, `inc/`, `data/`) | the differential harness programs | written for hcc but compared against Clang, so they work for any compiler; `tests/adversarial/` and the regression tests listed in 12.2 are held out until M3 |
| `harness/` runners | `run_tests.py`, `run_wacc.py`, `run_gcc_torture.py`, `run_csmith.py`, `run_conformance.py`, `run_doom.py`, `run_lua.py`, `run_sqlite.py`, `run_quickjs.py`, `run_zlib.py`, `run_onetrueawk.py`, `run_awk_extra.py`, `run_cross.py` and the platform runners | the runners are used unchanged; the known-failure lists are not (see below) |
| `conformance/` | the C11 probes and diagnostic cases | hcc2 adds its own probes beside them and never edits these |
| `scripts/fetch_*.sh` | the fetch scripts, with their pins and checksums | |
| `tools/csmith_campaign.py`, `tools/yarpgen_campaign.py`, `tools/abi_fuzz.py`, `tools/layout_fuzz.py`, `tools/constfold_fuzz.py`, `tools/perf_guard.py`, `tools/perf_scorecard.py`, `tools/bench_compile.py`, `tools/platform/` | campaign, fuzzing and timing tools | black-box tools that drive a compiler through its command line |

**Known-failure lists are hcc2's own.** hcc's lists (`harness/*known_failures*.txt`, `conformance/known_gaps.txt`) are kept for comparison only. The rebuild starts with empty lists in its own directory outside `testkit/`, pointed to by the runners' options or environment variables, or, where a runner reads a fixed path, by replacing that one list file and recording the replacement in `testkit/SHA256SUMS`. Every entry needs an issue and a reason, and once a suite's milestone gate has passed, its list only shrinks.

Not reused, because they test hcc's insides rather than its behaviour: hcc's unit tests (they live inside `src/`), `harness/dumps/` (hcc's own dump formats; hcc2 writes its own golden dumps) and `fuzz/` (compiler code). The rebuild writes its own versions of these.

### 12.2 Held-out material

The adversarial kits v0.6 to v0.9 are held out. The builder does not read their cases before the milestone gate that runs them (M3), so that their first-run scores mean the same as hcc's. They are run, frozen, then fixed. After M3 they are ordinary regression suites. Earlier kits v0.1 to v0.5 are contained in v0.6, which grew from 87 cases to 724.

hcc's regression tests for the kits' failures are held out with them, because they reveal the cases: `tests/128` to `tests/136`, `tests/157` to `tests/159`, and `tests/hosted/01` to `tests/hosted/04` (added in pull requests 21 to 25 and 58 to 60), and `docs/adversarial/`. They join the test kit at M3.

### 12.3 Fetched suites and programs

None of these is vendored. Each is fetched by script at a pin, checked against a recorded SHA-256 where the source publishes a file, and cached in CI by its pin. Licences stay with the fetched trees.

| Asset | Pin | Fetched by | Used from |
| --- | --- | --- | --- |
| Writing a C Compiler test suite (WACC) | commit `ae12014d2dec14488f3f80d14df4b6d8e4634d7d` | `scripts/fetch_wacc_tests.sh` | M1 |
| GCC torture, `gcc.c-torture/execute` only | GCC `releases/gcc-15.2.0`, commit `5115c7e447fc07457443df874bf57840e8316d5f`, sparse checkout | `scripts/fetch_gcc_torture.sh` | measured from M2, gated at M4 |
| Csmith | tag `csmith-2.3.0`, commit `30dccd73b78652c4719f36572994778a5b233a4e`, built from source | `scripts/fetch_csmith.sh` | M4 (it needs the preprocessor and `-std=gnu11`) |
| YARPGen | commit `a65fa1d79f9860546a858f14733a1621c9c01c84`, the one hcc's campaigns used | a new `scripts/fetch_yarpgen.sh` (hcc cloned it without a pin) | M4 |
| One True AWK | tag `20260426`, commit `5739fd79bcfc75ba7526773d0cf634521f8aca3c` | `scripts/fetch_onetrueawk.sh` | M2 |
| Independent POSIX AWK Challenge | `spacefilm900/awk-posix-challenge`, tag `v0.6`, commit `b46ea063c6d25372b6a2010ac537ca19a8fd1b1e` | `scripts/fetch_awk_posix_challenge.sh` | M5 |
| Shakespeare volume test | `spacefilm900/awk-volume-tests`, commit `46701e477fe48a7e0592b8022c77ac29c4114582` | `scripts/fetch_awk_volume.sh` | M5 |
| DOOM | `ozkl/doomgeneric`, commit `e257a09527062bfb986b6e5e0780839b6cbd5b71` | `scripts/fetch_doomgeneric.sh` | M5 |
| Freedoom WADs | release 0.13.0, zip SHA-256 `3f9b264f3e3ce503b4fb7f6bdcb1f419d93c7b546f4df3e874dd878db9688f59` | `scripts/fetch_freedoom.sh` | M5 |
| Lua | 5.4.7 source and test tarballs, SHA-256 in the script | `scripts/fetch_lua.sh` | M5 |
| SQLite | 3.54.1 amalgamation, SHA-256 `0a9c8e34352dad46006aa82836d5383bff9dc6bc9ce5a60553c05aa30e6ad1f9` | `scripts/fetch_sqlite.sh` | M2 (compile time), M5 |
| QuickJS | release 2026-06-04, with test262 commit `5c8206929d81b2d3d727ca6aac56c18358c8d790`, SHA-256 in the script | `scripts/fetch_quickjs.sh` | M5 |
| zlib | 1.3.1, SHA-256 in the script | `scripts/fetch_zlib.sh` | M5 |
| ChatGPT C11 kit v0.6 | `C11_Challenge_v0.6.zip`, SHA-256 `07e26e6c29a48f1a605d4cc1da38d53db10f424689b95b9dc17030a63b7ff182` | from the owner: it belongs to the reviewer and is not in a public repository | M3 |
| ChatGPT kits v0.7, v0.8 and v0.9 | as in hcc's `tests/adversarial/`, each with its own `SHA256SUMS.txt` | from the owner, or from hcc's `v1.0.0` tag, at M3 | M3 |

The two AWK repositories were private when hcc's CI was written, and CI read them with a read-only token. They are public as of 25 November 2026, so the rebuild needs no token for them. If any suite is private, its token is a repository secret, read-only, never printed and never put in code, chat or memory.

## 13. Comparing the two builds

### 13.1 Fair conditions

- The same host type (x86-64 Linux with 8 CPUs, as in hcc's reports), the same Rust, Clang and LLVM versions, the same pins, seeds and timeouts, and the same CI runner images.
- The same test kit, byte for byte, checked by its checksums.
- hcc is measured at `v1.0.0` (`d848f46`). Its release results are in `reports/release-1.0/README.md`, measured on `2e80ec0`, whose source tree is the release's. Any measure in 13.2 that was not recorded for hcc is measured again on `v1.0.0` with the same scripts before the comparison.
- hcc is not used as an oracle during the rebuild, so hcc2's results are its own (an owner decision; see section 16).

### 13.2 What is measured

| Area | Measure | hcc evidence |
| --- | --- | --- |
| Correctness at the end | every suite in Appendix B, with denominators | `reports/release-1.0/README.md` |
| Defects found by others | first-run score of every held-out kit; failures per kit and per root cause | `STATUS.md` sections on v0.7 to v0.9; `docs/adversarial/v0.6/`; Appendix A |
| Escaped defects | defects found after the milestone that should have caught them was accepted (by a later suite, the reviewer, CodeRabbit or a user), each tagged with that milestone | git history, the `STATUS.md` repair sections, issue #68 |
| Rework | fix-only pull requests; lines changed in code less than 7 days old; known-failure entries added for something that used to pass | git history |
| Review | CodeRabbit threads per 1,000 changed lines, the share that led to a code change, and the findings of a kind that `CODING_STANDARDS.md` already covered (14.5) | GitHub review data (hcc: 494 threads across 60 of 77 pull requests) |
| Maintainability | largest file and function, functions over 100 lines, functions over cyclomatic complexity 25, crash points, line and branch coverage, and mutation score on the same 7 files hcc measured and on the whole crate | `reports/quality/`, `reports/hardening/` |
| Performance | the compile-time and run-time measures of 7.4 | `reports/perf/scorecard.md` |
| Cost | Actions minutes by runner type, in minutes and dollars; CodeRabbit; any cloud agent | billing data; hcc's partial record is in Appendix A |
| Effort | elapsed days, pull requests, commits, builder sessions and interruptions, owner messages needed and owner decisions taken | git history and the builder's notes (hcc recorded neither its interruptions nor the owner's time) |
| Process | gates passed at the first attempt; summaries later found wrong | the milestone reports and their corrections |

### 13.3 Cross-checking the two compilers

At the end, each compiler is also an oracle for the other. Every program in the test kit and every big program is built by both, and the programs' behaviour is compared at `-O0` and `-O2`, as well as each against Clang. Every difference is examined: it is a bug in one of them, or a recorded implementation-defined choice. This costs little and may find defects one build has that neither Clang comparison caught.

### 13.4 Reporting

The comparison is one report, `reports/rebuild-comparison/README.md` in hcc2's repository, with every number linked to its evidence as in 3.5, and a short plain-English summary for the owner. It states plainly where the rebuild did worse. The independent reviewer reviews it before it is final, because the builder is judging its own work.

## 14. Working rules for the builder

### 14.1 Changes

- One milestone is several small pull requests, each with a clear purpose, its tests and its documentation.
- Every bug gets a regression test that fails first, then the fix.
- Known-failure lists only shrink once a suite's milestone gate has passed (12.1). Adding an entry needs a reason in the pull request and an issue.
- A refactor that should not change output proves it: the IR of the whole test corpus is byte-identical before and after (hcc did this over 13,148 compilations).
- Gaps and bugs are GitHub Issues with labels, not lists inside `STATUS.md`.

### 14.2 Documentation

- `STATUS.md` is the current state only, under 300 lines, with evidence links. History goes in `docs/DECISIONS.md` and the changelog.
- Every user-facing claim in the README, the guides and `docs/LANGUAGE.md` that can be compiled is checked by the documentation-claims job. hcc's `LANGUAGE.md` contradicted its user guide on GNU range designators until an outside review found it.
- The platform table always says what was tested natively, under emulation or with a stand-in, and on which commit.

### 14.3 Release discipline

- Every suite is rerun on the exact release commit, and the results are recorded with that commit's SHA.
- Release dates and version numbers are set at tag time.
- If the owner agrees, the release workflow is tried on a pre-release tag first; the published files are downloaded and checked.

### 14.4 The box

- Worktrees and scratch directories are named after their branch and removed after merge. By the release, the box held 73 `cc-*` working copies of hcc, and an earlier review linked stale copies to the agent working from stale context.
- The builder keeps its working notes outside the repository, in a folder that survives a box refresh (not one named `build`, `dist`, `out`, `target` or `coverage`).
- The builder never prints, copies or stores a token, and uses the box's GitHub login only for the repository's normal work.

### 14.5 Recurring review findings become rules

From M1, hcc2's repository has a `CODING_STANDARDS.md` that the builder and its helpers read at the start of every session, before writing code.

- **Grouping.** At the end of every milestone, and as soon as the same kind of finding has appeared three times, the builder groups the findings from CodeRabbit, the independent reviewer and the owner by kind (for example "record offsets computed outside the layout module", "a register name reused across calls", "a count reported without its denominator"). The grouping, with counts and links to the threads or kit cases, is recorded in `reports/findings/` for that milestone.
- **Written rule.** Each kind that has recurred becomes a rule in `CODING_STANDARDS.md`, with a one-line reason and a link to the findings it came from.
- **Automatic check where cheap.** Where a rule can be checked mechanically for little cost, it also gets a check in CI: a Clippy lint setting, a small lint script, a regression test or a harness case. The rule then says which check enforces it; a rule with no check says so.
- **Measured.** Section 13.2's review measure also counts findings of a kind that already had a rule, so the comparison shows whether the rules work.

Why: an agent does not carry review comments from one session to the next, so without a written rule it makes the same mistake again and the reviewer writes the same comment again. The owner points to the Anthropic Messages-API Rust migration of November 2026 as an example of this memory loop, broken by turning recurring findings into standing written rules and checks. hcc showed the same pattern: struct-layout defects were found in three review rounds in a row before one layout model fixed the family (A4), and CodeRabbit raised the IR register rule in three rounds before it was fixed structurally (A7). hcc's own `CODING_STANDARDS.md` (pull request 81, not yet merged) is the model: rules already enforced, with the check that enforces each; soft guidelines; and a section on turning recurring findings into rules.

## 15. Risks

| Risk | Effect | Mitigation |
| --- | --- | --- |
| The builder reads hcc's source, or hcc's code leaks in through memory or notes | the comparison is unfair and the rebuild is not independent | rule 4.3; the builder starts with no memory of hcc; the provenance section; the owner may ask for an outside provenance review |
| The builder tunes to the held-out kits | first-run scores look better than they are | the kits and hcc's regression tests for them are held out until M3 (12.2); frozen first runs; new reviewer rounds at every milestone |
| This specification's lessons point at the defect families the kits found (for example record layout) | the rebuild's first-run scores are partly informed by hcc's results | intended: applying lessons is the experiment; the comparison report says so next to the scores |
| The early quality gates slow the first milestones | the rebuild takes longer, and the comparison favours hcc on time | gates start at realistic levels (80% line coverage in M1) and rise; their time cost is measured, not hidden |
| CI cost grows with the gates from day one | the budget is exceeded | the limits in 7.2; expensive work weekly or local; a public repository if the owner agrees; the monthly report |
| CodeRabbit's rate limit and GitHub errors stall merges | idle time | small pull requests, batched review requests, the web-page fallback of 3.3 |
| Agent interruptions | lost time and stale context | 7.5 and 14.4 |
| The planned architecture is wrong in places | rework anyway | the owner and the reviewer review the architecture at M0; changes go through the decision log, not silently |
| Toolchain drift (a new Clang gives different answers) | false differences | everything pinned; a toolchain change needs the owner's OK and a rerun of both builds |
| A fetched source disappears or changes | a suite cannot run | pins and checksums; cached copies on the box and in CI caches |
| Oracle bugs, or Clang disagreeing with the standard | wrong "fixes" | the standard wins (5.1); disputed cases go to the reviewer with the clause; GCC as a second opinion |
| The two builds share a blind spot | defects neither finds | cross-checking (13.3), the reviewer, real programs |
| Owner time | the owner is the single point of approval | quiet-by-default reporting; approval requests batched, each with one recommendation |
| The builder judges its own comparison | bias | 13.4: the independent reviewer reviews the comparison report |

## 16. Decisions for the owner

Only the owner can make these. Everything else is the builder's choice, recorded in the decision log. The owner's review of 27 November 2026 settled some of them.

**Settled**

- **Scope:** `_Complex` is left out, with `__STDC_NO_COMPLEX__` defined, as in hcc 1.0 (5.1). Adding it later is a new owner decision.
- **Name:** the new compiler is called hcc2 (section 1). Its binary keeps the path `build/hcc` so the reused kit runs unchanged (5.4).
- **The reviewer:** another agent, assumed to be ChatGPT, relayed by the owner as before; the builder and the reviewer exchange human-readable memos or reports (3.1, 10.1).
- **GitHub Actions money limit:** the owner's account limit is now $50 a month (7.1).

**Still open**

1. **Whether and when to run the rebuild**, and with which agent.
2. **The held-out rule:** may the builder see the v0.6 to v0.9 kits before M3? This specification says no.
3. **hcc as an oracle:** may hcc2 be compared against hcc 1.0's binary during development? This specification says no, so hcc2's results are its own.
4. **The rest of the budgets:** confirm the other limits of section 7 (CodeRabbit, cloud agents, CI minutes and compile time), and whether hcc2's repository is public from the start (free standard runners) or private until a milestone.
5. **Toolchain:** the same Rust 1.85.1 and Clang 19.1.7 as hcc for fairness, or current versions.
6. **Licence** of hcc2 and its repository.

## Appendix A: lessons and their evidence

Pull-request numbers and commit SHAs refer to `spacefilm900/c-compiler`. Sources outside the repository, all kept on the builder's box:

- **Release notes**: the builder's session record for the 1.0 release (`RELEASE1_NOTES.md`), 24 and 25 November 2026.
- **Project review**: the builder's report to the owner of 15 November 2026, "Agentic AI in Software Engineering: Project Review".
- **Outside review**: ChatGPT's review of the 1.0 candidate, 25 November 2026, and the builder's point-by-point response. Its fixes are in pull requests 76 and 77.

| # | Lesson | Evidence | Acted on in |
| --- | --- | --- | --- |
| A1 | Outside oracles caught most mistakes | Project review, "What worked": Clang's output, WACC, GCC torture, DOOM frame hashes, Lua's suite and SQLite's database files caught most of the agent's real mistakes | 8 |
| A2 | The independent reviewer found what self-testing missed, in every round | first runs v0.1 85/87, v0.2 205/206, v0.3 335/336, v0.4 456/476, v0.5 635/636, v0.6 722/724 (project review, table 2.3; `docs/adversarial/v0.6/`); fixed in pull requests 21 to 25 and 27 | 10 |
| A3 | Missing required diagnostics were the commonest defect | project review, section 3(3); v0.1 (`_Generic` unselected branches), v0.2 (`!` in constant expressions), v0.6 (`void *` against a function pointer) | 6.2, 10.3 |
| A4 | Record layout was the late defect cluster | v0.7 HCC07-001 (`_Alignas` in nested record arrays; pull request 58); v0.8 four cases (bit-field next to `_Alignas`; pull request 59); v0.9 sixteen cases of the same family (pull request 60) (`STATUS.md`); the outside review's architectural advice; `DESIGN.md`, "After 1.0" | 6.3 |
| A5 | C11 features were found missing at release time | outside review items 1 and 2: identifier UCNs (fixed in pull request 76) and multi-character constants (pull request 77), days before the tag, while the README said "full C11 except `_Complex`" | 5.1, 10.3, 3.5 |
| A6 | The clause-by-clause audit came only at the end | `docs/C11_CONFORMANCE.md` (pull request 77): 2 wrong behaviours (macro rescanning past a replacement list, the `int a[];` tentative definition) and 12 accepted constraint violations; 10 fixed, 2 left as the 6.7.4p3 gap | 10.3, M3 |
| A7 | One C rule checked in several places drifts | the `register` address rule: CodeRabbit round 1 on pull request 77 (parameters, `_Generic`), round 2 (nested `_Generic` rejecting valid code), then a K&R case after the final review, left for 1.0.1 (release notes, 25 November) | 6.1, "one owner per rule" |
| A8 | Details surfaced late in real programs | issue #68 (a GNU forward-declared enum, opened 24 November, fixed in pull request 69); the enum compatible type changed during the QuickJS work (`DESIGN.md`, tie-break section) | M5 before release hardening; 13.2, escaped defects |
| A9 | Performance was not considered until the owner asked | `PLAN.md` step 6: the owner added the phase on 14 November; the `sqlite3.c` frontend took 111.35 s because of a quadratic macro-expansion algorithm, fixed to 1.34 s (pull request 35; `reports/perf/scorecard.md`) | 7.4 |
| A10 | Cost was not considered until prompted | project review, "Where the agent fell short": CI ran twice per push, and a pull request's checks dropped from 19 to 10 in pull request 40; the macOS job ran only on `main` because its minutes cost about 10x | 7.2 |
| A11 | CI stopped on a spending limit | Actions were blocked by a $0 limit until the owner set $30 a month on 14 November 2026 (the builder's project memory) | 7.1 |
| A12 | Cloud agents were paused | pull requests 1 to 19 came from cloud-agent branches (`cursor/...` in the merge history); the owner paused cloud agents on 11 November 2026, after pull request 19; the spend is not recorded in the repository | 7.1 |
| A13 | Platforms were retrofitted | `PLAN.md` steps 7, 9, 10 and 11; project review: "GNU mode and the Mac target were added onto it rather than built in"; step 7 had to keep x86-64 IR byte-identical over 2,172 inputs while adding the target layer | 5.3, 6.3 |
| A14 | Quality tools came after the code had grown | `PLAN.md` step 8; `reports/quality/baseline-2026-11/`: 333 crash points, files of 8,766, 7,786 and 6,528 lines, 2 functions over 200 lines, 69% of a mutant sample caught | 6.1, 6.2, 9 |
| A15 | Review of a refactor found old wrong-code bugs | Phase C (pull request 45): three wrong-code defects already on `main`, fixed in pull request 46, whose own review found five more; all 17 smaller CodeRabbit findings on pull request 45 were real (`STATUS.md`, quality section) | 9, 14.1 |
| A16 | Coverage work found real bugs | `reports/hardening/coverage.md`: three bugs in compile-time `long double` arithmetic and one in `float`/`double` folding, plus a rejected `offsetof` idiom and a wrong message | 9 |
| A17 | Mutation testing showed weak unit tests | `reports/hardening/mutation.md`: 74.0% to 92.9% of viable mutants caught in 7 files after new tests (pull request 67); in step 8 the differential suite caught only 4 of 20 sampled surviving mutants | 9 |
| A18 | Agent summaries overstated or went stale | project review, section 4: an overstated DOOM frame count; the v0.6 report listed finished Csmith work as open (corrected in pull request 39); "two wrong-code bugs" from Csmith was one; the phase C report said 15 findings where there were 18 (`STATUS.md`, quality section) | 3.5 |
| A19 | Headline numbers lost their scope and denominators | outside review items 7 and 8 (mutation scope; generated-program counts), fixed in the README and release notes in pull request 77 | 3.5 |
| A20 | Documentation contradicted itself | outside review item 3 (`LANGUAGE.md` against the user guide on range designators); `tools/check_docs.py` added in pull request 77 | 14.2 |
| A21 | The tie-break rule was worded wrongly | outside review item 6; the `DESIGN.md` tie-break section rewritten in pull request 77 | 5.1 |
| A22 | Design and history were mixed | outside review item 9; `STATUS.md` is 1,327 lines at `d848f46`; the split is planned for after 1.0 (`DESIGN.md`, "After 1.0") | 6.1, 14.2 |
| A23 | The clean-room claim is a process, not a proof | outside review item 10; project review, section 4; the README's provenance section (pull request 77) | 4.8 |
| A24 | Helper agents were interrupted often | project review, section 4 ("interrupted often", number not recorded); release notes: resumes through 24 and 25 November; box pauses counted as about 190 false fuzzing timeouts (`reports/hardening/README.md`); load timeouts in the release-candidate Csmith and YARPGen runs that agreed when rerun (`reports/release-1.0/README.md`) | 7.5 |
| A25 | Review throughput set the pace | CodeRabbit about one review an hour; release notes: pull request 70 opened at 18:16 on 24 November and merged at 01:49 on 25 November after several review slots; 494 CodeRabbit review threads across 60 of 77 pull requests, most on #49 (29), #56 (25) and #70 (23) (GitHub review data, counted 25 November) | 7.3 |
| A26 | GitHub itself can fail at the last step | release notes, 25 November: the SHA-pinned merge of pull request 77 returned server errors and then "merge already in progress"; the owner merged it in the web page | 3.3 |
| A27 | Release dates should be set at tag time | `CHANGELOG.md` dates 1.0.0 as 2026-11-24; the tag and release were made on 25 November (release notes) | M8, 14.3 |
| A28 | Rerunning everything on the release commit was worth it | `reports/release-1.0/README.md`: every suite on `2e80ec0`; the outside review asked for it | M8 |
| A29 | Freeze-before-fix keeps the evidence honest | carried over from the AWK project (`advice_for_c_compiler_bot.md`); frozen first-run logs for every C11 round (project review) | 10.1 |
| A30 | Real programs found what the suites missed | project review: the 90x slowdown only on `sqlite3.c`; a union-initializer case in DOOM's `info.c`; labels as values and `__uint128_t` in Lua and SQLite | M2 compile-time gate, M5 |
| A31 | Unpinned tools weaken reproducibility | YARPGen was cloned without a pin (`docs/FUZZING.md`); the commit used is recorded only on the builder's box | 12.3 |
| A32 | Stale working copies confused the agent | project review, section 8 (33 working copies then); 73 `cc-*` working copies on the box at the release | 14.4 |
| A33 | Sanitizers and real platforms found bugs CI did not | the weekly platform run (pull request 61): a struct-parameter store-order bug exposed by AddressSanitizer, and the macOS SDK's `_chk` builtins (`STATUS.md`) | 9, M6 |
| A34 | The same kinds of review finding kept coming back | struct-layout families in three review rounds in a row (A4); the IR register rule in three CodeRabbit rounds (A7); hcc wrote its `CODING_STANDARDS.md` only after the release (pull request 81) | 14.5 |

## Appendix B: hcc 1.0 baseline

From `reports/release-1.0/README.md` (commit `2e80ec0`, whose source tree is the release's) unless another source is named.

| Measure | hcc 1.0 |
| --- | --- |
| Unit tests | 366 passed |
| Differential harness | 230 of 230 |
| C11 conformance probes | 286 of 288 (the 2 are the 6.7.4p3 gap); 20 of 20 policy cases diagnosed |
| WACC | 1,384 passed, 0 known failures, 233 skipped |
| GCC torture `execute` | 1,746 executed: 1,412 passed, 334 known failures, 0 unexpected |
| DOOM | 39,209 of 39,209 frames identical at `-O0` and `-O2` |
| Lua, SQLite, QuickJS, zlib | match Clang's builds at both levels |
| One True AWK | both builds agree (with the same 9 golden-file mismatches, from the host environment) |
| Kit v0.6 | 724 of 724 at `-O0` and `-O2` (first run 722 of 724) |
| Kits v0.7, v0.8, v0.9 | 65, 80 and 100 at both levels (first runs 64, 76 and 84) |
| Csmith | 3,000 generated: 2,775 compared at `-O0` and 2,774 at `-O2`, 0 mismatches |
| YARPGen 2 | 2,000 generated: 1,651 compared, 0 mismatches |
| Fuzzing (`reports/hardening/README.md`, `main` `90e83d9`) | 13.2 hours, 11 million inputs, 0 failures |
| Coverage (`reports/hardening/coverage.md`) | 92.7% of lines, 96.3% of functions, 83.4% of branches |
| Mutation (`reports/hardening/mutation.md`) | 92.9% of viable mutants caught in 7 selected files |
| Compile time (`reports/perf/scorecard.md`) | 1.8 to 1.9x Clang at `-O0` and 1.03 to 1.36x at `-O2`; `sqlite3.c` peak memory 320 MiB |
| Size (`d848f46`) | 56,907 lines of Rust in `src/`, unit tests included |
| Effort (git history) | 8 to 25 November 2026; 77 pull requests; 660 commits on `main` |

## Appendix C: words used here

- **ABI:** the rules for how compiled code passes arguments, returns values and lays out data on a platform, so that code from different compilers works together.
- **Black box:** a tool whose answers are used but whose inside is never looked at.
- **CI:** the automatic checks GitHub runs on every change.
- **Coverage:** how much of the compiler's code the tests actually run.
- **Differential testing:** building the same program with this compiler and with Clang and checking that both behave the same.
- **Escaped defect:** a bug that got past the stage meant to catch it.
- **Fuzzing:** feeding the compiler huge numbers of random or damaged inputs to find crashes.
- **Held-out tests:** tests the builder does not see until they are run, so the score is honest.
- **IR:** LLVM's intermediate representation, the text the compiler writes and LLVM turns into machine code.
- **Mutation testing:** planting small deliberate bugs in the compiler and checking that the tests catch them.
- **Oracle:** a trusted reference that gives the right answer to compare against.
- **Required diagnostic:** an error the C standard says a compiler must report.
