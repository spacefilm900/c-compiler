# HCC v0.8 case index

All 72 runtime cases require `ok\n`, empty stderr and exit 0; all eight diagnostic cases require rejection. Six GNU programs also require rejection under strict C11. The 12 two-translation-unit ABI cases additionally require native and both mixed configurations on HCC. Expected outcomes are specified in `manifest.json`.

| ID | Group | Test | Mode | Gate | Basis |
|---|---|---|---|---|---|
| HCC08-001 | layout | union nested record, alignment 8 | c11 | runtime | 6.7.2.1; 6.7.5; 6.5.16.1 |
| HCC08-002 | layout | bitfield nested record, alignment 8 | c11 | runtime | 6.7.2.1; 6.7.5; 6.5.16.1 |
| HCC08-003 | layout | matrix nested record, alignment 8 | c11 | runtime | 6.7.2.1; 6.7.5; 6.5.16.1 |
| HCC08-004 | layout | Flexible array element alignment 8 | c11 | runtime | 6.7.2.1p18; 6.7.5 |
| HCC08-005 | layout | union nested record, alignment 16 | c11 | runtime | 6.7.2.1; 6.7.5; 6.5.16.1 |
| HCC08-006 | layout | bitfield nested record, alignment 16 | c11 | runtime | 6.7.2.1; 6.7.5; 6.5.16.1 |
| HCC08-007 | layout | matrix nested record, alignment 16 | c11 | runtime | 6.7.2.1; 6.7.5; 6.5.16.1 |
| HCC08-008 | layout | Flexible array element alignment 16 | c11 | runtime | 6.7.2.1p18; 6.7.5 |
| HCC08-009 | layout | union nested record, alignment 32 | c11 | runtime | 6.7.2.1; 6.7.5; 6.5.16.1 |
| HCC08-010 | layout | bitfield nested record, alignment 32 | c11 | runtime | 6.7.2.1; 6.7.5; 6.5.16.1 |
| HCC08-011 | layout | matrix nested record, alignment 32 | c11 | runtime | 6.7.2.1; 6.7.5; 6.5.16.1 |
| HCC08-012 | layout | Flexible array element alignment 32 | c11 | runtime | 6.7.2.1p18; 6.7.5 |
| HCC08-013 | layout | union nested record, alignment 64 | c11 | runtime | 6.7.2.1; 6.7.5; 6.5.16.1 |
| HCC08-014 | layout | bitfield nested record, alignment 64 | c11 | runtime | 6.7.2.1; 6.7.5; 6.5.16.1 |
| HCC08-015 | layout | matrix nested record, alignment 64 | c11 | runtime | 6.7.2.1; 6.7.5; 6.5.16.1 |
| HCC08-016 | layout | Flexible array element alignment 64 | c11 | runtime | 6.7.2.1p18; 6.7.5 |
| HCC08-017 | ABI | Over-aligned return (16 bytes) | c11 | runtime + two mixed directions | 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI |
| HCC08-018 | ABI | Over-aligned argument (16 bytes) | c11 | runtime + two mixed directions | 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI |
| HCC08-019 | ABI | Over-aligned variadic (16 bytes) | c11 | runtime + two mixed directions | 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI |
| HCC08-020 | ABI | Over-aligned callback (16 bytes) | c11 | runtime + two mixed directions | 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI |
| HCC08-021 | ABI | Over-aligned return (32 bytes) | c11 | runtime + two mixed directions | 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI |
| HCC08-022 | ABI | Over-aligned argument (32 bytes) | c11 | runtime + two mixed directions | 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI |
| HCC08-023 | ABI | Over-aligned variadic (32 bytes) | c11 | runtime + two mixed directions | 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI |
| HCC08-024 | ABI | Over-aligned callback (32 bytes) | c11 | runtime + two mixed directions | 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI |
| HCC08-025 | ABI | Over-aligned return (64 bytes) | c11 | runtime + two mixed directions | 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI |
| HCC08-026 | ABI | Over-aligned argument (64 bytes) | c11 | runtime + two mixed directions | 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI |
| HCC08-027 | ABI | Over-aligned variadic (64 bytes) | c11 | runtime + two mixed directions | 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI |
| HCC08-028 | ABI | Over-aligned callback (64 bytes) | c11 | runtime + two mixed directions | 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI |
| HCC08-029 | atomics | CAS success and failure with distinct expected values | c11 | runtime | 6.5.16.2; 7.17.3; 7.17.7; 7.17.8 |
| HCC08-030 | atomics | Weak CAS retry after mismatch | c11 | runtime | 6.5.16.2; 7.17.3; 7.17.7; 7.17.8 |
| HCC08-031 | atomics | Atomic pointer exchange with side-effecting new value | c11 | runtime | 6.5.16.2; 7.17.3; 7.17.7; 7.17.8 |
| HCC08-032 | atomics | Atomic pointer CAS with typed expected | c11 | runtime | 6.5.16.2; 7.17.3; 7.17.7; 7.17.8 |
| HCC08-033 | atomics | Unsigned bitwise fetch sequence | c11 | runtime | 6.5.16.2; 7.17.3; 7.17.7; 7.17.8 |
| HCC08-034 | atomics | Nonconstant order expression evaluated once | c11 | runtime | 6.5.16.2; 7.17.3; 7.17.7; 7.17.8 |
| HCC08-035 | atomics | Atomic floating multiply and division | c11 | runtime | 6.5.16.2; 7.17.3; 7.17.7; 7.17.8 |
| HCC08-036 | atomics | Atomic signed short compound operations | c11 | runtime | 6.5.16.2; 7.17.3; 7.17.7; 7.17.8 |
| HCC08-037 | atomics | Atomic aggregate exchange without padding | c11 | runtime | 6.5.16.2; 7.17.3; 7.17.7; 7.17.8 |
| HCC08-038 | atomics | Atomic 16-byte aggregate CAS | c11 | runtime | 6.5.16.2; 7.17.3; 7.17.7; 7.17.8 |
| HCC08-039 | atomics | Atomic elements in a VLA | c11 | runtime | 6.5.16.2; 7.17.3; 7.17.7; 7.17.8 |
| HCC08-040 | atomics | Atomic flag in nested aligned record | c11 | runtime | 6.5.16.2; 7.17.3; 7.17.7; 7.17.8 |
| HCC08-041 | threads | CAS contention exact count | c11 | runtime | 5.1.2.4; 6.2.4; 7.17; 7.26 |
| HCC08-042 | threads | Atomic floating multiply contended | c11 | runtime | 5.1.2.4; 6.2.4; 7.17; 7.26 |
| HCC08-043 | threads | Recursive mutex depth and trylock | c11 | runtime | 5.1.2.4; 6.2.4; 7.17; 7.26 |
| HCC08-044 | threads | Condition variable broadcast predicate | c11 | runtime | 5.1.2.4; 6.2.4; 7.17; 7.26 |
| HCC08-045 | threads | TLS nested over-aligned object in workers | c11 | runtime | 5.1.2.4; 6.2.4; 7.17; 7.26 |
| HCC08-046 | threads | Release sequence RMW publishes payload | c11 | runtime | 5.1.2.4; 6.2.4; 7.17; 7.26 |
| HCC08-047 | threads | TSS independent keys and deletion | c11 | runtime | 5.1.2.4; 6.2.4; 7.17; 7.26 |
| HCC08-048 | threads | Thread exit publishes return code and TLS | c11 | runtime | 5.1.2.4; 6.2.4; 7.17; 7.26 |
| HCC08-049 | VLA | VLA with aligned record stride | c11 | runtime | 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6 |
| HCC08-050 | VLA | VLA typedef shadowing and capture | c11 | runtime | 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6 |
| HCC08-051 | VLA | VLA pointer difference after bound changes | c11 | runtime | 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6 |
| HCC08-052 | VLA | VLA parameter captures bound and modifies independently | c11 | runtime | 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6 |
| HCC08-053 | VLA | VLA sizeof type evaluates dimension once | c11 | runtime | 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6 |
| HCC08-054 | VLA | VLA typedef declaration side effect once | c11 | runtime | 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6 |
| HCC08-055 | VLA | VLA multidimensional compound assignment pointer | c11 | runtime | 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6 |
| HCC08-056 | VLA | VLA loop with early return from nested scopes | c11 | runtime | 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6 |
| HCC08-057 | VLA | VLA goto backward reevaluates bound | c11 | runtime | 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6 |
| HCC08-058 | VLA | VLA nested switch exits with outer storage intact | c11 | runtime | 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6 |
| HCC08-059 | VLA | VLA and function pointer with runtime stride | c11 | runtime | 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6 |
| HCC08-060 | VLA | VLA pointer casts with runtime row type | c11 | runtime | 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6 |
| HCC08-061 | GNU | GNU typeof on VLA pointer captures stride | gnu11 | runtime + strict rejection | GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable |
| HCC08-062 | GNU | GNU statement expression returns record by value | gnu11 | runtime + strict rejection | GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable |
| HCC08-063 | GNU | GNU omitted conditional with pointer side effects | gnu11 | runtime + strict rejection | GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable |
| HCC08-064 | GNU | GNU packed nested aligned object offsets | gnu11 | runtime | GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable |
| HCC08-065 | GNU | GNU overflow builtin narrow output | gnu11 | runtime | GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable |
| HCC08-066 | GNU | GNU statement expression nested loop VLA exit | gnu11 | runtime + strict rejection | GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable |
| HCC08-067 | GNU | GNU computed goto exits inner ordinary scope | gnu11 | runtime + strict rejection | GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable |
| HCC08-068 | GNU | GNU case range mixed with enum constant | gnu11 | runtime + strict rejection | GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable |
| HCC08-069 | diagnostics | Atomic type specifier rejects const-qualified type | c11 | diagnostic | 6.7.2.4p3 |
| HCC08-070 | diagnostics | Atomic qualifier on function typedef | c11 | diagnostic | 6.7.3p3 |
| HCC08-071 | diagnostics | Atomic type specifier rejects already atomic type | c11 | diagnostic | 6.7.2.4p3 |
| HCC08-072 | diagnostics | Alignment on register object | c11 | diagnostic | 6.7.5p2 |
| HCC08-073 | diagnostics | Alignment on function declaration | c11 | diagnostic | 6.7.5p2 |
| HCC08-074 | diagnostics | Thread-local member | c11 | diagnostic | 6.7.1; 6.7.2.1 |
| HCC08-075 | diagnostics | VLA file scope via variable bound | c11 | diagnostic | 6.7.6.2p2 |
| HCC08-076 | diagnostics | VLA named goto into nested block | c11 | diagnostic | 6.8.6.1p1 |
| HCC08-077 | K&R / tgmath | K&R float promotion into type generic math | c11 | runtime | 6.5.2.2; 6.7.6.3; 6.9.1; 7.25 |
| HCC08-078 | K&R / tgmath | Old-style parameter adjusted array | c11 | runtime | 6.5.2.2; 6.7.6.3; 6.9.1; 7.25 |
| HCC08-079 | K&R / tgmath | Type generic fma mixes integer and float | c11 | runtime | 6.5.2.2; 6.7.6.3; 6.9.1; 7.25 |
| HCC08-080 | K&R / tgmath | Type generic real math on VLA elements | c11 | runtime | 6.5.2.2; 6.7.6.3; 6.9.1; 7.25 |

## Separate cross-target matrix
Six fixtures per target (36 target/fixture cells); two translation units for each aggregate-call cell. These are not part of the 80-case denominator. See `cross-target/manifest.json` for target arguments and sources.

| Fixture | Intent |
|---|---|
| data-model | ABI-specific long/long double sizes, alignment, nested offset |
| nested-16 | Bit-field plus alignas(16), nested array offset and stride |
| nested-64 | Union containing over-aligned records, copy and physical offset |
| aggregate-call | Separate caller and definition passing and returning over-aligned aggregates |
| atomic64 | Plain atomic compound and postfix operations |
| tls-layout | Nested alignment and thread-local array storage |

## Separate observational probe
`probes/atomic-pointer-fetch/test.c`: C11 atomic generics for pointer arithmetic; differing GCC/Clang outcomes. Excluded from gates.
