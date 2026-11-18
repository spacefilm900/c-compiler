"""The target a harness run builds for, read from the oracle Clang.

The harnesses compare this compiler with Clang for the same target. On an
x86_64 host that is x86_64 Linux; on a native AArch64 Linux host (or with
CLANG and CC_UNDER_TEST wrappers that add `--target=aarch64-linux-gnu`) it
is AArch64 Linux. Suites whose expected results were written for x86_64
(the Writing a C Compiler book's tests assume a signed `char`, and some link
x86-64 assembly) use this to pick Clang as the oracle elsewhere.
"""

import subprocess


def clang_triple(clang: str) -> str:
    """`clang -dumpmachine`, or "" when Clang cannot be run."""
    try:
        proc = subprocess.run([clang, "-dumpmachine"], capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.TimeoutExpired):
        return ""
    return proc.stdout.strip() if proc.returncode == 0 else ""


def arch_of(triple: str) -> str:
    """The architecture part of a triple: x86_64, aarch64, ..."""
    arch = triple.split("-", 1)[0]
    return "aarch64" if arch == "arm64" else arch
