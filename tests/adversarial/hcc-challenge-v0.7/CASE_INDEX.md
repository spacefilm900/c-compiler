# v0.7 test case index

Each runtime case asserts its expected values and prints `ok` only after all assertions pass. Diagnostic cases require rejection. Detailed per-case rationales are in `manifest.json` and the C files. `+ strict gate` denotes an additional C11 rejection check. `multi-TU` denotes both mixed-compiler directions in candidate runs.

| ID | Group | Test | Mode | Expected | Basis |
|---|---|---|---|---|---|
| HCC07-001 | alignment | Over-aligned nested record arrays | c11 | ok; exit 0 | 6.2.8; 6.7.5; 6.7.2.1 |
| HCC07-002 | alignment | Multiple specifiers with weaker and zero | c11 | ok; exit 0 | 6.7.5p4,p6 |
| HCC07-003 | alignment | Aligned union and enclosing record copy | c11 | ok; exit 0 | 6.7.5; 6.7.2.1; 6.5.16.1 |
| HCC07-004 | alignment | Extern alignment and one definition | c11 | ok; exit 0 | 6.7.5p7; 6.2.2 |
| HCC07-005 | alignment | Alignment cannot weaken natural alignment | c11 | diagnostic | 6.7.5p4 |
| HCC07-006 | alignment | Non-power-of-two alignment | c11 | diagnostic | 6.7.5p3; 6.2.8p4 |
| HCC07-007 | alignment | Aligned typedef constraint | c11 | diagnostic | 6.7.5p2 |
| HCC07-008 | alignment | Aligned bit-field constraint | c11 | diagnostic | 6.7.5p2 |
| HCC07-009 | alignment | Aligned parameter constraint | c11 | diagnostic | 6.7.5p2 |
| HCC07-010 | TLS | TLS extern across translation units | c11 | ok; exit 0; multi-TU | 6.7.1p3; 6.2.4p4 |
| HCC07-011 | TLS | Aligned TLS inside a worker | c11 | ok; exit 0 | 6.2.4p4; 6.7.5; 7.26.5 |
| HCC07-012 | TLS | TLS missing at block scope | c11 | diagnostic | 6.7.1p3 |
| HCC07-013 | TLS | TLS forbidden on function | c11 | diagnostic | 6.7.1p2 |
| HCC07-014 | TLS | Mixed TLS declarations | c11 | diagnostic | 6.7.1p3 |
| HCC07-015 | K&R | Prior prototype uses promoted unsigned short and float | c11 | ok; exit 0 | 6.7.6.3p15; 6.9.1; 6.5.2.2 |
| HCC07-016 | K&R | Old-style definition declaration order differs | c11 | ok; exit 0 | 6.9.1p6; 6.5.2.2 |
| HCC07-017 | K&R | K&R caller and prototype definition separate units | c11 | ok; exit 0; multi-TU | 6.7.6.3p14,p15; 6.5.2.2 |
| HCC07-018 | K&R | Prototype incompatible with promoted float | c11 | diagnostic | 6.7.6.3p15 |
| HCC07-019 | K&R | Extraneous K&R parameter declaration | c11 | diagnostic | 6.9.1p6 |
| HCC07-020 | K&R | Duplicate K&R parameter declaration | c11 | diagnostic | 6.9.1p6 |
| HCC07-021 | tgmath | Type-generic math with side effects and qualifiers | c11 | ok; exit 0 | 7.25p3; 6.5.1.1 |
| HCC07-022 | tgmath | Non-generic parameters do not select precision | c11 | ok; exit 0 | 7.25p3; 7.12.6.4; 7.12.10.3 |
| HCC07-023 | tgmath | Ternary real precision and integer argument | c11 | ok; exit 0 | 7.25p3 |
| HCC07-024 | VLA | VLA typedef captured across changed bounds | c11 | ok; exit 0 | 6.7.6.2p5; 6.5.3.4p2 |
| HCC07-025 | VLA | VLA pointer stride across three dimensions | c11 | ok; exit 0 | 6.7.6.2; 6.5.6p8,p9 |
| HCC07-026 | VLA | sizeof operand with VLA type evaluates pointer increment | c11 | ok; exit 0 | 6.5.3.4p2; 6.5.2.4 |
| HCC07-027 | VLA | Nested break and continue release scopes | c11 | ok; exit 0 | 6.2.4p7; 6.8.6.2; 6.8.6.3 |
| HCC07-028 | VLA | Named goto leaves only inner VLA declaration | c11 | ok; exit 0 | 6.2.4p7; 6.8.6.1 |
| HCC07-029 | VLA | Static pointer to VLA type in block | c11 | ok; exit 0 | 6.7.6.2p2; 6.2.4 |
| HCC07-030 | VLA | Prototype star replaced by runtime bound in definition | c11 | ok; exit 0 | 6.7.6.2p4,p5; 6.7.6.3p7 |
| HCC07-031 | VLA | VLA element type containing bit-fields | c11 | ok; exit 0 | 6.7.6.2; 6.7.2.1 |
| HCC07-032 | VLA | Named goto enters scope of VM typedef | c11 | diagnostic | 6.8.6.1p1 |
| HCC07-033 | VLA | Switch bypasses VM pointer declaration | c11 | diagnostic | 6.8.4.2p2 |
| HCC07-034 | VLA | Variably modified generic association | c11 | diagnostic | 6.5.1.1p2 |
| HCC07-035 | VLA | VLA member constraint | c11 | diagnostic | 6.7.2.1p9 |
| HCC07-036 | VLA | VLA initializer constraint | c11 | diagnostic | 6.7.9p3 |
| HCC07-037 | VLA | VLA static storage constraint | c11 | diagnostic | 6.7.6.2p2 |
| HCC07-038 | atomics | Failed compare exchange updates expected | c11 | ok; exit 0 | 7.17.7.4 |
| HCC07-039 | atomics | Atomic floating exchange and compound RMW | c11 | ok; exit 0 | 6.5.16.2p3; 7.17.7.3 |
| HCC07-040 | atomics | Atomic pointer increment and decrement preserve element stride | c11 | ok; exit 0 | 6.5.2.4p2; 6.5.3.1; 6.5.6 |
| HCC07-041 | atomics | Atomic aggregate load store and exchange | c11 | ok; exit 0 | 6.5.16.1; 7.17.7.2,p3 |
| HCC07-042 | atomics | Nonconstant memory order evaluated once | c11 | ok; exit 0 | 7.17.7.2; 6.5.2.2 |
| HCC07-043 | atomics | VLA of atomic members and alignment | c11 | ok; exit 0 | 6.7.6.2; 6.7.3; 7.17.7.1 |
| HCC07-044 | atomics | Atomic type specifier rejects qualified type | c11 | diagnostic | 6.7.2.4p3 |
| HCC07-045 | atomics | Atomic type specifier rejects array | c11 | diagnostic | 6.7.2.4p3 |
| HCC07-046 | atomics | Atomic bit-field constraint | c11 | diagnostic | 6.7.3p3 |
| HCC07-047 | threads | Concurrent atomic increment with exact total | c11 | ok; exit 0 | 6.5.2.4p2; 7.17; 7.26.5 |
| HCC07-048 | threads | call_once publishes ordinary data to concurrent callers | c11 | ok; exit 0 | 7.26.2.1 |
| HCC07-049 | threads | Thread exit and TSS destructor iteration | c11 | ok; exit 0 | 7.26.5.5; 7.26.6 |
| HCC07-050 | threads | Release/acquire publishes non-atomic payload | c11 | ok; exit 0 | 5.1.2.4; 7.17.3; 7.26.5 |
| HCC07-051 | GNU | typeof VLA captures operand type from runtime bound | gnu11 | ok; exit 0 + strict gate | N1570 6.7.6.2; GCC manual Typeof |
| HCC07-052 | GNU | Statement expression and VLA scope | gnu11 | ok; exit 0 + strict gate | N1570 6.2.4; GCC manual Statement Exprs |
| HCC07-053 | GNU | Omitted conditional evaluates first operand once | gnu11 | ok; exit 0 + strict gate | N1570 6.5.15; GCC manual Conditionals |
| HCC07-054 | GNU | Case range with negative endpoints | gnu11 | ok; exit 0 + strict gate | N1570 6.8.4.2; GCC manual Case Ranges |
| HCC07-055 | GNU | Computed goto with VLA declared outside targets | gnu11 | ok; exit 0 + strict gate | N1570 6.2.4; GCC manual Labels as Values |
| HCC07-056 | GNU | Zero-length tail member and padding | gnu11 | ok; exit 0 + strict gate | N1570 6.7.6.2; GCC manual Zero Length |
| HCC07-057 | GNU | GNU overflow builtin single evaluation | gnu11 | ok; exit 0 | N1570 6.5.2.2; GCC manual Integer Overflow Builtins |
| HCC07-058 | GNU | Packed record nested with explicit aligned member | gnu11 | ok; exit 0 | N1570 6.7.2.1; GCC manual Common Type Attributes |
| HCC07-059 | GNU | Choose expression unselected side effect | gnu11 | ok; exit 0 | N1570 6.5.1.1 analogous; GCC manual Other Builtins |
| HCC07-060 | alignment | stdalign header and type-name alignment | c11 | ok; exit 0 | 7.15; 6.7.5p5 |
| HCC07-061 | alignment | Aligned compound literal and record ABI across translation units | c11 | ok; exit 0; multi-TU | 6.7.5; 6.5.2.5; 6.5.2.2; 6.9.1 |
| HCC07-062 | atomics | Volatile atomic operand and atomic flag state | c11 | ok; exit 0 | 6.7.3; 7.17.7.5; 7.17.8 |
| HCC07-063 | GNU | Empty initializer for fixed-size record and array | gnu11 | ok; exit 0 + strict gate | N1570 6.7.9; GCC manual C extensions |
| HCC07-064 | GNU | GNU constructor runs before main | gnu11 | ok; exit 0 | N1570 5.1.2.2.1; GCC manual Common Function Attributes |
| HCC07-065 | TLS | Thread feature-test macros and independent block scope TLS | c11 | ok; exit 0 | 6.10.8.3; 6.2.4p4; 7.26 |
