"""The Linux stand-in for Intel macOS (x86_64-apple-macos) in the harnesses.

There is no Mac here. Intel macOS uses the System V AMD64 calling
convention (Apple's "Writing 64-bit Intel code for Apple Platforms"), so
code built for it can run on x86_64 Linux once three Mach-O and SDK
details are set aside:

1. Headers. Both compilers read the x86_64 glibc headers through one
   merged sysroot (`/usr/include` and `/usr/include/x86_64-linux-gnu`, with
   harness/macos_x86_64/include/bits/floatn.h first, because Clang for
   Darwin has no `__float128`). Both get `-D__USER_LABEL_PREFIX__=` so that
   glibc's asm-label redirects (`__isoc99_scanf`) name the ELF symbol
   rather than a Mach-O one with a leading `_`.
2. Objects. Each IR file, written for `x86_64-apple-macosx10.15.0` with
   the Mach-O data layout, has its triple changed to
   `x86_64-apple-darwin-elf` and its mangling to ELF (`m:e`), and `llc`
   builds an ELF object: the backend still applies its Darwin rules, but
   the symbols are the plain C names glibc exports.
3. Linking and running use the host's glibc and Linux.

What this cannot show: the macOS SDK headers, Apple's C library, the Mach-O
linker and dyld, and the `$INODE64`-style symbol variants. The Intel macOS
job in .github/workflows/macos.yml checks those on a real Mac.
"""
import os
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET = "x86_64-apple-macos"
CLANG_TRIPLE = "x86_64-apple-macos10.15"
IR_TRIPLE = "x86_64-apple-macosx10.15.0"
PROXY_TRIPLE = "x86_64-apple-darwin-elf"
OVERLAY = ROOT / "harness" / "macos_x86_64" / "include"
HOST_INCLUDES = [Path("/usr/include/x86_64-linux-gnu"), Path("/usr/include")]
# Clang predefines these as nullability keywords for Darwin; glibc uses
# __nonnull as an attribute macro. The label prefix is explained above.
# Clang turns on the stack protector for Darwin, which reads the Darwin
# global `__stack_chk_guard`; glibc keeps the guard elsewhere, and this
# compiler has no stack protector, so the stand-in builds Clang's code
# without it.
CLANG_FLAGS = ["-U__nonnull", "-U__nullable", "-U__null_unspecified", "-D__USER_LABEL_PREFIX__=",
               "-fno-stack-protector"]
OURS_FLAGS = ["-D__USER_LABEL_PREFIX__="]


def _merge(sources, dest):
    """Fill `dest` with symlinks to the entries of `sources`; a name that
    is a directory in more than one source is merged; otherwise the first
    source wins."""
    dest.mkdir(parents=True, exist_ok=True)
    names = {}
    for src in sources:
        if src.is_dir():
            for entry in sorted(src.iterdir()):
                names.setdefault(entry.name, []).append(entry)
    for name, entries in names.items():
        dirs = [e for e in entries if e.is_dir()]
        if len(dirs) > 1 and entries[0].is_dir():
            _merge(dirs, dest / name)
        else:
            (dest / name).symlink_to(entries[0])


def make_sysroot(tmp):
    """A sysroot whose usr/include merges the overlay and glibc's x86_64
    headers. Returns its path, or None when the glibc headers are absent."""
    if not all(p.is_dir() for p in HOST_INCLUDES):
        return None
    root = Path(tmp) / "intel-mac-sysroot"
    if not root.exists():
        _merge([OVERLAY] + HOST_INCLUDES, root / "usr" / "include")
    return root


def proxy_ir(text):
    """The IR text retargeted to the ELF stand-in (see the module doc)."""
    text = re.sub(r'^target triple = ".*"$', f'target triple = "{PROXY_TRIPLE}"', text, flags=re.M)
    return re.sub(r'^(target datalayout = "e)-m:o-', r"\1-m:e-", text, flags=re.M)


def find_llc():
    if os.environ.get("LLC"):
        return os.environ["LLC"]
    for name in ["llc"] + [f"llc-{v}" for v in (21, 20, 19, 18)]:
        if shutil.which(name):
            return name
    return None
