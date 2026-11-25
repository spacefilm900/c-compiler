# C11 conformance audit (N1570 clauses 5 to 7)

This is a clause-by-clause check of hcc against the C11 standard, using the
N1570 draft, which has the same technical content as ISO/IEC 9899:2011. It
was done for the 1.0 release. Each clause was read, then probe programs
were written for what it requires, and those programs were compared with
Clang (`clang -std=c11 -pedantic-errors`) as a black box.

This is not a certification or a validation suite. A formal conformance
claim needs a validated test suite such as Perennial or Plum Hall, and hcc
has not been run against one. What follows is what these probes show, on
the stated target, and nothing more.

## Method

- **Probes.** `conformance/run/` holds 34 programs, one or more per clause.
  Each program checks itself (`CHECK(...)`) and prints values whose exact
  form the standard leaves to the implementation. `harness/run_conformance.py`
  generates 20 more programs at the minimum translation limits of 5.2.4.1.
  `conformance/diag/` holds 254 cases, one per `//==` marker, grouped by
  clause. 234 are constraint or syntax violations, which the standard
  requires a diagnostic for. The other 20, marked `[policy]`, are not:
  they are undefined behaviour (such as an invalid `##` paste or
  `#line 0`), implementation-defined (concatenating `u"a" U"b"`), or an
  optional feature hcc does not provide (`_Complex`). hcc diagnoses them
  by choice, and they are counted separately.
- **Programs.** hcc compiles each program to IR, and Clang compiles and
  links that IR. `clang -std=c11 -pedantic-errors` builds the same source.
  Both builds must succeed. The two programs must exit with the same status
  and print the same output, and no `CHECK` may fail.
- **Violations.** The standard requires a diagnostic for every constraint
  or syntax violation (5.1.1.3). An error or a warning from hcc counts. A
  crash, a signal or a timeout does not. Clang's result (error, warning or
  nothing) is recorded but is not required. Where the standard and Clang
  differ, the standard wins.
- **Run it.** `python3 harness/run_conformance.py` (add `--json FILE` for
  the full results). CI runs it on every pull request, together with
  `tools/check_docs.py`. That script compiles the language claims in
  `docs/LANGUAGE.md`, `DESIGN.md`, `STATUS.md` and the user guide with hcc,
  and checks the wording those files use for them.
- **Target.** The audit ran on x86-64 Linux (glibc, Clang 19.1.7 as the
  oracle). Implementation-defined values printed by the probes, such as
  the signedness of `char` and the sizes of types, are those of that
  target. CI repeats the audit on x86-64 Linux with Clang 18.
- **Library.** hcc is the compiler half of the implementation. It ships
  its own `<stdarg.h>`, `<stddef.h>`, `<stdbool.h>`, `<stdalign.h>`,
  `<stdnoreturn.h>`, `<float.h>`, `<iso646.h>`, `<stdatomic.h>` and, where
  the C library lacks it, `<threads.h>`. The rest of the hosted library is
  the platform's C library, so clause 7 was checked only where it needs
  the compiler: every header must compile, and its macros and type-generic
  interfaces must work. The behaviour of the library functions themselves
  belongs to that library.

## Result

On the head of the 1.0 pre-tag pull request: **286 of 288 probes pass**.
The 288 are the 54 programs and the 234 required diagnostics. All 54
programs pass, and 232 of the 234 required diagnostics are given. The two
that are not are known gaps, listed in `conformance/known_gaps.txt` and
described [below](#known-gaps). On the 234, Clang reports an error for 231
and warns for 3.

hcc also diagnoses all 20 `[policy]` cases. On those, Clang reports an
error for 17 and warns for 1. It accepts `_Complex`, which Clang supports
and hcc does not. For `#line 0` it did not finish within the runner's
10-second limit.

The audit found 12 constraint violations that hcc accepted, and 2 bugs in
accepted programs. 10 of the 12 were fixed in the same pull request, as
were both bugs (see the [changes](#changes-made-by-the-audit)).

| Clause | Subject | Programs | Required diagnostics | `[policy]` cases | Result |
| --- | --- | ---: | ---: | ---: | --- |
| 5.1.1 | Translation environment, translation phases | 1 | - | - | pass |
| 5.1.2 | Execution environments (hosted `main`, `argv`) | 1 | - | - | pass |
| 5.2.1-5.2.2 | Character sets, escape semantics | 1 | - | - | pass |
| 5.2.4.1 | Translation limits (minimums) | 21 | - | - | pass |
| 5.2.4.2 | `<limits.h>` and `<float.h>` values | 1 | - | - | pass |
| 6.2 | Concepts: scopes, linkage, name spaces, storage duration, types, representation, compatibility, alignment | 5 | - | - | pass |
| 6.3 | Conversions | 1 | - | - | pass |
| 6.4 | Lexical elements | 6 | 14 | 5 | pass |
| 6.5 | Expressions | 5 | 61 | 0 | pass |
| 6.6 | Constant expressions | 1 | 14 | 0 | pass |
| 6.7 | Declarations | 5 | 88 | 1 | 86 of 88 required (see gaps) |
| 6.8 | Statements and blocks | 1 | 23 | 0 | pass |
| 6.9 | External definitions | 1 | 11 | 1 | pass |
| 6.10 | Preprocessing directives | 2 | 21 | 11 | pass |
| 6.11 | Future language directions | - | - | - | not applicable |
| 7 | Library: the freestanding headers, the hosted headers as far as they need the compiler | 3 | 2 | 2 | pass |

The 5.2.4.1 row counts the 20 generated programs plus the `#include`
nesting test. All of them are at or above the standard's minimums: 127
nesting levels of blocks, 63 of parenthesised expressions, 63 of
conditional inclusion, 15 of `#include`, 12 pointer, array and function
declarators on one declaration, 63 nested structure definitions, 63
significant characters in an internal identifier, 31 in an external one,
511 identifiers in one block, 4095 external identifiers, 4095 macros, 127
parameters and 127 macro parameters, a 4095-character string literal and
logical line, a 32767-byte object, 1023 members, 1023 enumeration
constants and 1023 case labels.

## Clause notes

**5.1.1.2 Translation phases.** Trigraphs are replaced in phase 1 in
strict C11 (under `-std=gnu11` only with `-trigraphs`). Line splicing
works inside identifiers. A comment becomes one space, and comments are
not recognised inside string literals. Adjacent string literals are
concatenated after their escapes are converted, and a prefix on one of
them applies to all of them. `_Pragma` is carried out.

**5.1.2.2 Hosted environment.** `argv[argc]` is a null pointer, and
reaching the `}` of `main` returns 0. A function specifier on `main` is
now diagnosed (6.7.4p4).

**6.2 Concepts.** Block, file, function and prototype scopes, the
four name spaces, and an enumerator's scope starting after the enumerator
all behave as the standard requires. So do the 6.2.2 linkage rules
(`static` then `extern`, block-scope `extern`), tentative definitions
(now including `int a[];`, see below), and static versus automatic
storage (a compound literal in a loop body is a new object each time).
`max_align_t` is the strictest fundamental alignment. Composite types
complete an array size and add a prototype.

**6.3 Conversions.** The probe checks the integer promotions (bit-fields
included), conversion to `_Bool`, unsigned wrap-around, truncation of
floating values toward zero, the usual arithmetic conversions (including
`long` with `unsigned int`), array and function decay, null pointer
constants, and function pointers converted to another type and back.

**6.4 Lexical elements.** All 44 keywords, universal character names in
identifiers (with the Annex D ranges and the 6.4.3 restrictions), the type
of every integer constant suffix and base combination, hexadecimal
floating constants, character constants (including multi-character ones,
see below), every string literal prefix with concatenation, digraphs, and
pp-numbers. Diagnosed as required: constants too large for any type, bad
suffixes, `08`, a hex float with no exponent, empty character constants,
out-of-range escapes, UCNs that 6.4.3 forbids, `0xe+1`, and stray
characters. Also diagnosed, as `[policy]` cases: a UCN outside the Annex D
identifier ranges, a UCN above U+10FFFF in a string, `u"a" U"b"`, and
unterminated comments and string literals.

**6.5 Expressions.** `_Generic`, subscripting with either operand order,
argument promotions and default promotions for variadic calls, compound
literals, `sizeof` of a VLA (evaluated) and of other expressions (not
evaluated), casts, truncating division and the sign of `%`, pointer
arithmetic one past the end, the type of a shift (the promoted left
operand), relational and equality results of type `int`, short-circuit
evaluation, the result types of the conditional operator (null pointer
constants and combined qualifiers included), compound assignment computed
in the common type, and the comma operator. All 61 constraint probes are
diagnosed. They cover `_Generic` association rules, argument count and
type, member access, lvalue and modifiable-lvalue requirements, `&` of a
bit-field or a `register` object, operand types for every operator,
pointer compatibility, and simple assignment (6.5.16.1).

**6.6 Constant expressions.** Address constants with offsets, arithmetic
constant expressions, `offsetof`, a cast of a floating constant in an
integer constant expression, and file-scope compound literals. An
evaluated comma, assignment or call, an overflowing or dividing-by-zero
constant expression, and non-constant array sizes, case labels,
enumeration values, bit-field widths and static initializers are
diagnosed. Clang only warns for the comma and the overflow, and it
accepts some forms 6.6p10 allows an implementation to accept, such as
`&g[1] != 0` as a static initializer. hcc does not accept those forms.

**6.7 Declarations.** Storage classes, `_Thread_local`, typedef
redefinition (allowed in C11), specifier order, flexible array members,
anonymous structures and unions, bit-fields (zero-width, `_Bool`, a 32-bit
`unsigned`), enumerations, tags and forward declarations, `_Atomic` (both
forms), `restrict`, `_Alignas`, `inline` with an external definition,
`_Noreturn`, complex declarators, VLAs and `[*]`, the VLA typedef size
being fixed at its definition, K&R definitions, designated initializers
(including overrides and continuing after a designator), brace elision,
string initializers with no room for the null, and `_Static_assert`. 86
of the 88 constraint probes are diagnosed. The two that are not are the
6.7.4p3 inline rules, described below.

**6.8 Statements.** `switch` with case values converted to the promoted
controlling type, Duff's device, `continue` in `do` going to the
condition, `goto` within and into blocks, the dangling `else`, and a new
VLA on each loop iteration. All 23 constraint probes are diagnosed,
including jumps into the scope of a VLA and a label with no statement
after it (C11 needs a statement; C23 changed this).

**6.9 External definitions.** K&R definitions with promoted parameters, a
static function used before its definition, and `extern inline`. All 11
required probes are diagnosed, and so is the `[policy]` case, a static
tentative definition of incomplete type (undefined by 6.9.2p3).

**6.10 Preprocessing.** `#if` arithmetic in `intmax_t` and `uintmax_t`,
unknown identifiers as 0, skipped groups that are not tokenised as
directives, macro-expanded `#include`, identical redefinition, `#` and
`##`, `__VA_ARGS__`, `#line` changing `__LINE__` and `__FILE__`, the
`STDC` pragmas, the null directive, and the predefined macros. The
examples of 6.10.3.3 to 6.10.3.5 (EXAMPLE 3 to 7) are checked by a probe
and by unit tests. All 21 required probes are diagnosed, including
`#error`. So are the 11 `[policy]` cases, which the standard leaves
undefined: a `##` that does not form a token, `#line` 0, a `#line` above
2147483647 or not a number, an `#include` of neither form, an unterminated
macro invocation, `_Pragma` without a string, an unknown directive, and
`#define` or `#undef` of `defined` or a predefined macro.

**7 Library.** All nine freestanding headers compile and work, and
including a header twice is harmless. The hosted probes use
`<assert.h>` (`static_assert`), `<ctype.h>`, `<errno.h>`, `<fenv.h>`
with `FENV_ACCESS`, `<inttypes.h>`, `<locale.h>`, `<math.h>` (its
classification macros included), `<setjmp.h>`, `<signal.h>`,
`<stdatomic.h>`, `<stdio.h>`, `<stdlib.h>`, `<string.h>`, `<tgmath.h>`
(each macro must pick the function that matches its argument's type),
`<threads.h>`, `<time.h>`, `<uchar.h>`, `<wchar.h>` and `<wctype.h>`.
`<complex.h>` is not provided, and `__STDC_NO_COMPLEX__` is defined. That
is allowed: complex types are optional (6.10.8.3). Annex K
(`__STDC_LIB_EXT1__`) is optional and not provided.

## Changes made by the audit

Two accepted programs were handled wrongly. Both are fixed:

- **6.10.3.4p2 rescanning.** A macro stopped counting as "being replaced"
  only when its whole expansion had been read. The standard ends that at
  the end of its replacement list. So in 6.10.3.5p5's `f(2)(9)`, hcc gave
  `2*f(9)` instead of `2*9*g`. An invocation whose closing parenthesis
  comes from outside an enclosing expansion is now painted only with the
  names that both the macro name and the `)` carry.
- **6.9.2p2 tentative definitions.** `int a[];` at file scope, with no
  later size, was rejected. It now gets one element, with a warning, as
  in 6.9.2 EXAMPLE 2. With internal linkage (`static int a[];`) it is
  still an error. 6.9.2p3 makes that undefined; Clang gives a warning.

These constraint violations were accepted silently. Each is now a
diagnostic:

- `__VA_ARGS__` in a macro that is not variadic (6.10.3p5).
- `auto` or `register` at file scope, function definitions included
  (6.9p2, 6.9.1p4).
- `inline` or `_Noreturn` on any declaration of `main`, at file or block
  scope (6.7.4p4).
- A function definition whose function type comes from a typedef
  (6.9.1p2): `F f { ... }`. `F *make(void) { ... }`, where the typedef is
  only the pointed-to type, is still accepted.
- `&` applied to an object declared `register`, or to a member of one
  (6.5.3.2p1). This covers parameters, from the point where each is
  declared, and a `_Generic` selection whose selected association names
  such an object.
- The `_Atomic` qualifier on an array or function type named by a typedef
  (6.7.3p3).

Two undefined constructs are now diagnosed too (6.10.8p2), outside system
headers. One is `#undef` of `__LINE__`, `__FILE__`, `__DATE__`,
`__TIME__`, `__STDC__`, `__STDC_HOSTED__` or `__STDC_VERSION__`. The other
is `#define` of `__LINE__` or `__FILE__`. A `#define` of the other five
was already an error.

Multi-character character constants (`'ab'`, 6.4.4.4p10) were rejected.
The standard allows them, with an implementation-defined value. They are
now accepted, with the value and warnings GCC and Clang give.

## Known gaps

- **6.7.4p3, inline definitions.** An inline definition of a function
  with external linkage must not define a modifiable object with static or
  thread storage duration, and must not refer to an identifier with
  internal linkage. hcc does not diagnose either. It decides which
  definitions are inline definitions only at the end of the translation
  unit (6.7.4p7), and at that point it no longer knows which names a body
  used or how they were declared. The code generated for such a program
  is the same as Clang's.

Outside the probes, these are documented limitations rather than
conformance failures:

- `_Complex` and `<complex.h>`: optional, and `__STDC_NO_COMPLEX__` is
  defined.
- The run-time behaviour of the hosted library belongs to the platform's
  C library.
- Undefined behaviour is not diagnosed at run time. The standard does not
  require that.

## Implementation-defined choices the probes print

For x86-64 Linux, these match Clang and GCC. `docs/LANGUAGE.md` gives them
for every target.

- `char` is signed on x86-64 and on Apple arm64, and unsigned on arm64
  Linux. `int` is 32 bits. `long` is 64 bits, except 32 bits on Windows.
- Converting an out-of-range value to a signed type keeps the low bits
  (two's complement). `-16 >> 2` is `-4` (an arithmetic shift).
- A plain `int` bit-field is signed. An enumeration with no negative
  values has type `unsigned int`.
- A multi-character constant `'ab'` is `0x6162`: each byte is shifted in
  from the right, and the last four are kept.
- `long double` is the x87 80-bit format on x86-64 Linux and macOS,
  binary128 on arm64 Linux, and `double` on Windows and Apple arm64.
