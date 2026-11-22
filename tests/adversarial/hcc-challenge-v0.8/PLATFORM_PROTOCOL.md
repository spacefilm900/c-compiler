# Platform evidence protocol

The target matrix is deliberately separated from the main native suite. All targets use 64-bit pointers and 32-bit int in the documented contract. The data-model fixture asserts the following values in bytes against both HCC and the independent Clang oracle:

| Target | sizeof(long) | sizeof(long double) | _Alignof(long double) |
|---|---:|---:|---:|
| x86_64-linux-gnu | 8 | 16 | 16 |
| aarch64-linux-gnu | 8 | 16 | 16 |
| arm64-apple-macos11.0 | 8 | 8 | 8 |
| x86_64-apple-macos10.15 | 8 | 16 | 16 |
| x86_64-w64-windows-gnu | 4 | 16 | 16 |
| x86_64-pc-windows-msvc | 4 | 8 | 8 |

The bitfield-containing 16-byte-aligned nested record and the 64-byte-aligned nested union specifically challenge the v0.7 repair's propagation of C alignment into emitted LLVM representations. LLVM integer alignment is target/data-layout dependent; a syntactically valid object is not proof of the physical member offsets. For genuine runtime evidence, execute the fixture and check exit 0 rather than relying on static assertions alone.

## Additional runtime evidence requested from Grok

For each available native or emulator environment, build **each fixture's listed source files**, run each produced executable and compare with its platform oracle. Record exact target triple/deployment version, sysroot/SDK, compiler and linker hashes, host OS/ISA, runtime/OS or emulator, commands, output, exit code and timeouts. Cross-compiled object-only results must remain separate from runtime results. Preserve the original emitted IR even when an existing project stand-in transforms it later.

For the aggregate-call fixture (two translation units), additionally compile the caller with HCC and definition with reference Clang, and then swap the roles; link objects for the same actual target ABI and execute on the appropriate runtime when feasible. Use O0 and O2. Keep platform-native and emulated runs separate. If only the project’s Apple-on-Linux stand-in is available, label it as such, and record any IR retargeting or emulated TLS flag. It is not a test of native Darwin relocations or macOS thread runtime. For MinGW versus MSVC, do not treat using MinGW libraries under Wine as validation of native MSVC linkage. Use the project's genuine native Windows or macOS CI where available. For Linux ARM64 distinguish native execution from qemu. No platform-specific missing tools or documented stand-in limitations should be misreported as compiler failures or passes. Record not-run cells explicitly.

For the native Linux 8 concurrency cases, repeat execution 20 times per case as supplemental stress, including compare-exchange retry, floating RMW, recursive mutexes, condition variables, independent TLS/TSS and release sequences. These are deterministic forbidden-outcome tests, not exhaustive C11 memory model validation. The runner has a bounded timeout. Repeated passes do not establish absence of weak-memory or schedule-dependent bugs. If possible, exercise equivalent cases using the project’s platform runtime on actual macOS/Windows or distinguish any adapted harness run as non-gating; the native runner intentionally refuses other hosts. On macOS and Windows use the documented `lib/threads.c` implementation and platform linker flags; Linux glibc threads use `-pthread`. Do not link glibc's thread runtime in place of the platform runtime and call the result native.

No sysroot/SDKs, emulator or native cross-platform environment are assumed to exist locally. Independent reference validation is object generation for all six target families and execution for the six matching Linux x86-64 cells only. Report ABI interoperability on non-host targets only when mixed objects have actually linked and executed.
