# v0.9 case inventory

100 cases: 92 runtime and eight diagnostic. Includes five supplementary strict GNU rejection checks and nine two-TU runtime cases. Alignment extension support is target-contract rather than a universal C11 guarantee.

| ID | Group | Title | Basis | Classification | Stress |
|---|---|---|---|---|---|---|
| HCC09-001 | threads | join return value and argument identity | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-002 | threads | thrd_exit return value | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-003 | threads | thread identity across worker and creator | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-004 | threads | nested create/join | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-005 | threads | detached thread publication with static lifetime | 7.26; 6.2.4 | portable-c11 | yes |
| HCC09-006 | threads | thrd_sleep with valid interval | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-007 | threads | plain mutex serialises ordinary writes | 7.26; 6.2.4 | portable-c11 | yes |
| HCC09-008 | threads | trylock on mutex held by another thread | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-009 | threads | recursive mutex lock and trylock depth | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-010 | threads | recursive mutex retains ownership until final unlock | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-011 | threads | timed mutex acquisition on free lock | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-012 | threads | timed mutex deadline while held by another thread | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-013 | threads | condition signal guarded by predicate | 7.26; 6.2.4 | portable-c11 | yes |
| HCC09-014 | threads | condition broadcast to multiple waiters | 7.26; 6.2.4 | portable-c11 | yes |
| HCC09-015 | threads | timed condition wait expiry and mutex reacquisition | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-016 | threads | initialized TLS scalar | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-017 | threads | zero-initialized TLS scalar | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-018 | threads | initialized TLS aggregate | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-019 | threads | TLS pointer initial value | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-020 | threads | block static TLS persists within thread | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-021 | threads | external TLS across translation units | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-022 | threads | TSS default NULL and independent values | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-023 | threads | TSS destructor runs before join | 7.26; 6.2.4 | portable-c11 | yes |
| HCC09-024 | threads | TSS destructor resets key for another iteration | 7.26; 6.2.4 | portable-c11 | yes |
| HCC09-025 | threads | TSS null value suppresses destructor | 7.26; 6.2.4 | portable-c11 | no |
| HCC09-026 | threads | call_once publishes ordinary data to concurrent callers | 7.26; 6.2.4 | portable-c11 | yes |
| HCC09-027 | threads | independent once flags and repeated calls | 7.26; 6.2.4 | portable-c11 | yes |
| HCC09-028 | ordering | contended CAS increment int | 7.17.7.4 | portable-c11 | yes |
| HCC09-029 | ordering | contended CAS increment unsigned long | 7.17.7.4 | portable-c11 | yes |
| HCC09-030 | ordering | contended CAS increment unsigned char | 7.17.7.4 | portable-c11 | yes |
| HCC09-031 | ordering | contended CAS increment unsigned short | 7.17.7.4 | portable-c11 | yes |
| HCC09-032 | ordering | contended atomic fetch_xor | 7.17.7.5 | portable-c11 | yes |
| HCC09-033 | ordering | contended atomic fetch_or | 7.17.7.5 | portable-c11 | yes |
| HCC09-034 | ordering | contended atomic fetch_add | 7.17.7.5 | portable-c11 | yes |
| HCC09-035 | ordering | contended atomic fetch_sub | 7.17.7.5 | portable-c11 | yes |
| HCC09-036 | ordering | publication: release acquire | 5.1.2.4; 7.17.4 | portable-c11 | yes |
| HCC09-037 | ordering | publication: release fence | 5.1.2.4; 7.17.4 | portable-c11 | yes |
| HCC09-038 | ordering | publication: acquire fence | 5.1.2.4; 7.17.4 | portable-c11 | yes |
| HCC09-039 | ordering | publication: paired fences | 5.1.2.4; 7.17.4 | portable-c11 | yes |
| HCC09-040 | ordering | publication: seq_cst | 5.1.2.4; 7.17.4 | portable-c11 | yes |
| HCC09-041 | ordering | release sequence through separate RMW thread | 5.1.2.4 | portable-c11 | yes |
| HCC09-042 | ordering | SC store buffering forbidden both-zero outcome | 5.1.2.4; 7.17.3 | portable-c11 | yes |
| HCC09-043 | ordering | atomic flag protects ordinary critical section | 7.17.8 | portable-c11 | yes |
| HCC09-044 | concurrent | parallel histogram with 2 workers | 7.26; 7.17 | portable-c11 | no |
| HCC09-045 | concurrent | parallel histogram with 4 workers | 7.26; 7.17 | portable-c11 | yes |
| HCC09-046 | concurrent | parallel histogram with 8 workers | 7.26; 7.17 | portable-c11 | no |
| HCC09-047 | concurrent | parallel histogram with 16 workers | 7.26; 7.17 | portable-c11 | no |
| HCC09-048 | concurrent | parallel histogram with 32 workers | 7.26; 7.17 | portable-c11 | yes |
| HCC09-049 | concurrent | bounded producer consumer ring capacity 1 | 7.26.3; 7.26.4 | portable-c11 | yes |
| HCC09-050 | concurrent | bounded producer consumer ring capacity 7 | 7.26.3; 7.26.4 | portable-c11 | yes |
| HCC09-051 | concurrent | SPSC atomic ring buffer | 5.1.2.4; 7.17 | portable-c11 | yes |
| HCC09-052 | concurrent | parallel VLA matrix row computation | 6.7.6.2; 7.26 | portable-c11 | yes |
| HCC09-053 | concurrent | parallel task allocation through atomic index | 7.17; 7.26 | portable-c11 | yes |
| HCC09-054 | concurrent | aligned TLS scratch records in workers | 6.7.5; 6.2.4; 7.26 | target-contract | yes |
| HCC09-055 | concurrent | reference count final release publishes worker contributions | 5.1.2.4; 7.17 | portable-c11 | yes |
| HCC09-056 | layout | bit-fields around aligned member variant 1, 8 bytes | 6.7.2.1; 6.7.5 | target-contract | no |
| HCC09-057 | layout | bit-fields around aligned member variant 2, 8 bytes | 6.7.2.1; 6.7.5 | target-contract | no |
| HCC09-058 | layout | bit-fields around aligned member variant 3, 8 bytes | 6.7.2.1; 6.7.5 | target-contract | no |
| HCC09-059 | layout | bit-fields around aligned member variant 1, 16 bytes | 6.7.2.1; 6.7.5 | target-contract | no |
| HCC09-060 | layout | bit-fields around aligned member variant 2, 16 bytes | 6.7.2.1; 6.7.5 | target-contract | no |
| HCC09-061 | layout | bit-fields around aligned member variant 3, 16 bytes | 6.7.2.1; 6.7.5 | target-contract | no |
| HCC09-062 | layout | bit-fields around aligned member variant 1, 32 bytes | 6.7.2.1; 6.7.5 | target-contract | no |
| HCC09-063 | layout | bit-fields around aligned member variant 2, 32 bytes | 6.7.2.1; 6.7.5 | target-contract | no |
| HCC09-064 | layout | bit-fields around aligned member variant 3, 32 bytes | 6.7.2.1; 6.7.5 | target-contract | no |
| HCC09-065 | layout | bit-fields around aligned member variant 1, 64 bytes | 6.7.2.1; 6.7.5 | target-contract | no |
| HCC09-066 | layout | bit-fields around aligned member variant 2, 64 bytes | 6.7.2.1; 6.7.5 | target-contract | no |
| HCC09-067 | layout | bit-fields around aligned member variant 3, 64 bytes | 6.7.2.1; 6.7.5 | target-contract | no |
| HCC09-068 | layout | mixed ABI aligned bitfield aggregate return 8 | 6.7.2.1; 6.7.5; target ABI | target-contract | no |
| HCC09-069 | layout | mixed ABI aligned bitfield aggregate variadic 8 | 6.7.2.1; 7.16; target ABI | target-contract | no |
| HCC09-070 | layout | mixed ABI aligned bitfield aggregate return 16 | 6.7.2.1; 6.7.5; target ABI | target-contract | no |
| HCC09-071 | layout | mixed ABI aligned bitfield aggregate variadic 16 | 6.7.2.1; 7.16; target ABI | target-contract | no |
| HCC09-072 | layout | mixed ABI aligned bitfield aggregate return 32 | 6.7.2.1; 6.7.5; target ABI | target-contract | no |
| HCC09-073 | layout | mixed ABI aligned bitfield aggregate variadic 32 | 6.7.2.1; 7.16; target ABI | target-contract | no |
| HCC09-074 | layout | mixed ABI aligned bitfield aggregate return 64 | 6.7.2.1; 6.7.5; target ABI | target-contract | no |
| HCC09-075 | layout | mixed ABI aligned bitfield aggregate variadic 64 | 6.7.2.1; 7.16; target ABI | target-contract | no |
| HCC09-076 | vla | VLA typedef with runtime bound and nested aligned records 3 | 6.7.6.2; 6.7.5 | target-contract | no |
| HCC09-077 | vla | VLA typedef with runtime bound and nested aligned records 5 | 6.7.6.2; 6.7.5 | target-contract | no |
| HCC09-078 | vla | VLA typedef with runtime bound and nested aligned records 9 | 6.7.6.2; 6.7.5 | target-contract | no |
| HCC09-079 | vla | VLA typedef with runtime bound and nested aligned records 17 | 6.7.6.2; 6.7.5 | target-contract | no |
| HCC09-080 | vla | VLA typedef with runtime bound and nested aligned records 31 | 6.7.6.2; 6.7.5 | target-contract | no |
| HCC09-081 | vla | GNU statement expression and typeof VLA 3 | GNU typeof and statement expressions | gnu-contract | no |
| HCC09-082 | vla | GNU statement expression and typeof VLA 7 | GNU typeof and statement expressions | gnu-contract | no |
| HCC09-083 | vla | GNU statement expression and typeof VLA 13 | GNU typeof and statement expressions | gnu-contract | no |
| HCC09-084 | vla | GNU statement expression and typeof VLA 19 | GNU typeof and statement expressions | gnu-contract | no |
| HCC09-085 | vla | GNU statement expression and typeof VLA 29 | GNU typeof and statement expressions | gnu-contract | no |
| HCC09-086 | diagnostic | atomic bit-field | 6.7.2.4; 6.7.3; 6.7.5; 6.7.1; 6.7.6.2 | portable-c11 | no |
| HCC09-087 | diagnostic | aligned bit-field | 6.7.2.4; 6.7.3; 6.7.5; 6.7.1; 6.7.6.2 | portable-c11 | no |
| HCC09-088 | diagnostic | atomic array specifier | 6.7.2.4; 6.7.3; 6.7.5; 6.7.1; 6.7.6.2 | portable-c11 | no |
| HCC09-089 | diagnostic | atomic function specifier | 6.7.2.4; 6.7.3; 6.7.5; 6.7.1; 6.7.6.2 | portable-c11 | no |
| HCC09-090 | diagnostic | atomic qualified type specifier | 6.7.2.4; 6.7.3; 6.7.5; 6.7.1; 6.7.6.2 | portable-c11 | no |
| HCC09-091 | diagnostic | TLS automatic local | 6.7.2.4; 6.7.3; 6.7.5; 6.7.1; 6.7.6.2 | portable-c11 | no |
| HCC09-092 | diagnostic | TLS function declaration | 6.7.2.4; 6.7.3; 6.7.5; 6.7.1; 6.7.6.2 | portable-c11 | no |
| HCC09-093 | diagnostic | VLA static storage | 6.7.2.4; 6.7.3; 6.7.5; 6.7.1; 6.7.6.2 | portable-c11 | no |
| HCC09-094 | other | atomic exchange and failed compare-exchange expected update int | 7.17.7 | portable-c11 | no |
| HCC09-095 | other | atomic exchange and failed compare-exchange expected update unsigned long | 7.17.7 | portable-c11 | no |
| HCC09-096 | other | atomic exchange and failed compare-exchange expected update double | 7.17.7 | portable-c11 | no |
| HCC09-097 | other | atomic exchange and failed compare-exchange expected update float | 7.17.7 | portable-c11 | no |
| HCC09-098 | other | atomic flag clear and test sequence | 7.17.8 | portable-c11 | no |
| HCC09-099 | other | nonconstant memory order evaluated once | 7.17.3; 7.17.7 | portable-c11 | no |
| HCC09-100 | other | atomic signal fence preserves observable values | 7.17.4 | portable-c11 | no |
