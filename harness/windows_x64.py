"""64-bit Windows in the harnesses, run on Linux under Wine.

Two targets, both with the Microsoft x64 calling convention and LLP64:

- `x86_64-w64-windows-gnu` (MinGW-w64) is checked for real. Both compilers
  read the MinGW-w64 headers (Debian/Ubuntu `mingw-w64-x86-64-dev`, found
  by both compilers by default), LLVM builds COFF objects
  (`clang -target x86_64-w64-windows-gnu -c` on either compiler's IR),
  `x86_64-w64-mingw32-gcc` links them against the MinGW-w64 C runtime
  and msvcrt.dll, and the programs run under Wine.
- `x86_64-pc-windows-msvc` is a stand-in, because the MSVC and Windows SDK
  headers and libraries are not on Linux. The type model and calling
  convention are the MSVC target's (above all, `long double` is the 8-byte
  `double`), but both compilers read the MinGW-w64 headers with the MinGW
  identification macros (MSVC_STANDIN_FLAGS) and the objects are linked
  like MinGW's. MinGW's `long double` functions expect the x87 format, so
  programs that call them cannot match; `printf` with `%Lf` comes from
  msvcrt.dll, which reads an 8-byte `long double`, as MSVC's does. The
  real headers, CRT and linker are checked by the manual Windows job in
  .github/workflows/windows.yml.

On a Windows host (NATIVE) the same harnesses run the programs directly:
the MSVC target uses the real MSVC headers (this compiler reads the INCLUDE
variable a Visual Studio developer prompt sets; Clang finds them itself)
and Clang links with the MSVC linker and CRT; the MinGW-w64 target reads
the headers under MINGW_SYSROOT and links with MINGW_GCC.

Env: WINE (default wine), MINGW_GCC (default x86_64-w64-mingw32-gcc, or gcc
on Windows), MINGW_SYSROOT (Windows only: the MinGW-w64 directory holding
include/, default derived from MINGW_GCC's location).
"""
import os
import shutil
import subprocess

GNU = "x86_64-w64-windows-gnu"
MSVC = "x86_64-pc-windows-msvc"
TARGETS = (GNU, MSVC)
NATIVE = os.name == "nt"
WINE = os.environ.get("WINE", "wine")
MINGW_GCC = os.environ.get("MINGW_GCC", "gcc" if NATIVE else "x86_64-w64-mingw32-gcc")
CLANG = os.environ.get("CLANG", "clang")
MINGW_ROOT = "/usr/x86_64-w64-mingw32"
# The MSVC stand-in: MinGW-w64's headers without MSVC's identification
# macros, which would select code paths for Microsoft's own compiler.
# printf and friends come from msvcrt.dll, as with MSVC, not MinGW-w64's
# own (__USE_MINGW_ANSI_STDIO), which reads `%Lf` as the x87 format.
MSVC_STANDIN_FLAGS = ["-isysroot", MINGW_ROOT, "-U_MSC_VER", "-U_MSC_FULL_VER", "-U_MSC_BUILD",
                      "-D__MINGW32__", "-D__MINGW64__", "-D__MSVCRT__", "-D__GNUC__=4",
                      "-D__GNUC_MINOR__=2", "-D__declspec(a)=__attribute__((a))",
                      "-D__cdecl=__attribute__((__cdecl__))", "-D__USE_MINGW_ANSI_STDIO=0"]
# This compiler's Microsoft keyword macros for the MSVC target
# (MSVC_KEYWORDS in src/pp/predefined.rs): <_mingw.h> defines its own.
MSVC_KEYWORD_MACROS = ["__int8", "__int16", "__int32", "__int64", "__cdecl", "__stdcall",
                       "__fastcall", "__thiscall", "__vectorcall", "__clrcall", "__ptr32",
                       "__ptr64", "__unaligned", "__w64", "__sptr", "__uptr", "__inline",
                       "__forceinline", "__declspec", "__pragma", "__restrict", "__alignof"]
MSVC_STANDIN_FLAGS = [f"-U{name}" for name in MSVC_KEYWORD_MACROS] + MSVC_STANDIN_FLAGS
# Clang for the MSVC triple: the same stand-in, and no MSVC compatibility
# extensions, so both compilers see the same headers the same way.
CLANG_MSVC_FLAGS = ["-isystem", MINGW_ROOT + "/include", "-fno-ms-compatibility",
                    "-fno-ms-extensions", "-U_MSC_VER", "-U_MSC_FULL_VER", "-U_MSC_BUILD",
                    "-D__MINGW32__", "-D__MINGW64__", "-D__MSVCRT__", "-D__GNUC__=4",
                    "-D__GNUC_MINOR__=2", "-D__declspec(a)=__attribute__((a))",
                    "-D__cdecl=__attribute__((__cdecl__))", "-D__USE_MINGW_ANSI_STDIO=0",
                    "-fno-stack-protector"]


def mingw_sysroot():
    """Windows host: the MinGW-w64 directory with include/ (next to the
    compiler's bin/, as in the MSYS2 and WinLibs layouts)."""
    if os.environ.get("MINGW_SYSROOT"):
        return os.environ["MINGW_SYSROOT"]
    gcc = shutil.which(MINGW_GCC)
    if gcc is None:
        return None
    base = os.path.dirname(os.path.dirname(os.path.realpath(gcc)))
    for root in (os.path.join(base, "x86_64-w64-mingw32"), base):
        if os.path.isfile(os.path.join(root, "include", "_mingw.h")):
            return root
    return None


def ours_flags(target):
    if NATIVE:
        if target == GNU:
            root = mingw_sysroot()
            return ["-isysroot", root] if root else []
        return []
    return list(MSVC_STANDIN_FLAGS) if target == MSVC else []


def clang_flags(target):
    if NATIVE:
        # Name the MinGW-w64 root that ours_flags uses (MINGW_SYSROOT or the
        # one beside MINGW_GCC), so both compilers read the same headers.
        if target == GNU:
            root = mingw_sysroot()
            return ["--sysroot", root] if root else []
        return []
    return list(CLANG_MSVC_FLAGS) if target == MSVC else []


def available(target=GNU):
    """An error string when the toolchain (or Wine) is missing."""
    if NATIVE:
        if shutil.which(CLANG) is None:
            return f"{CLANG} not found"
        if target == GNU and (shutil.which(MINGW_GCC) is None or mingw_sysroot() is None):
            return f"MinGW-w64 not found ({MINGW_GCC}, MINGW_SYSROOT)"
        if target == MSVC and not os.environ.get("INCLUDE"):
            return "INCLUDE is not set: run from a Visual Studio developer environment"
        return None
    for tool in (MINGW_GCC, WINE):
        if shutil.which(tool) is None:
            return f"{tool} not found (apt: mingw-w64 wine64)"
    if not os.path.isdir(MINGW_ROOT + "/include"):
        return f"{MINGW_ROOT}/include is missing (apt: mingw-w64-x86-64-dev)"
    return None


def object_command(clang, target, ll, obj):
    """LLVM's COFF object for one IR file. The triple stays the IR's."""
    return [clang, "-target", target, "-w", "-c", str(ll), "-o", str(obj)]


# MSVC's stack probe. LLVM calls `__chkstk` (with the size in RAX; it
# probes the pages and keeps every register) before a large or dynamic
# stack allocation on the MSVC target; on MinGW the same routine is
# libgcc's `___chkstk_ms`. The stand-in links with MinGW, so it forwards one
# to the other.
CHKSTK_SHIM = """\
\t.text
\t.globl __chkstk
__chkstk:
\tjmp ___chkstk_ms
"""


def link_command(objs, out, target=GNU):
    if NATIVE and target == MSVC:
        # The UCRT defines the printf family only as inline functions in
        # <stdio.h>; Microsoft's documented legacy_stdio_definitions.lib
        # gives a body to a call that never saw the header
        # (__builtin_sprintf, for one).
        return [
            CLANG, "-target", MSVC, *[str(o) for o in objs],
            "-llegacy_stdio_definitions", "-o", str(out),
        ]
    if NATIVE:
        return [MINGW_GCC, *[str(o) for o in objs], "-o", str(out)]
    extra = []
    if target == MSVC:
        shim = f"{out}.chkstk.s"
        with open(shim, "w") as handle:
            handle.write(CHKSTK_SHIM)
        extra.append(shim)
    return [MINGW_GCC, *[str(o) for o in objs], *extra, "-o", str(out)]


def link_messages(out, err):
    """The linker's own error lines first (link.exe writes them to standard
    output; the Clang driver's summary is on standard error)."""
    text = (out or b"") + b"\n" + (err or b"")
    lines = text.decode(errors="replace").splitlines()
    picked = [line for line in lines if "error" in line.lower() or "LNK" in line]
    return ("\n".join(picked) if picked else text.decode(errors="replace")).encode()


def wine_env():
    env = dict(os.environ)
    env.setdefault("WINEDEBUG", "-all")
    # No Gecko/Mono installers, and no debugger on a crash: winedbg would
    # wait for input until the harness's timeout.
    env.setdefault("WINEDLLOVERRIDES", "mscoree,mshtml=;winedbg.exe=d")
    return env


def start_server():
    """Keep one wineserver alive for the whole run, so each program does
    not pay for starting it. Nothing to do on Windows itself."""
    if NATIVE:
        return
    # The server outlives this call, so it must not hold a pipe of ours.
    quiet = {"stdin": subprocess.DEVNULL, "stdout": subprocess.DEVNULL,
             "stderr": subprocess.DEVNULL}
    subprocess.run(["wineserver", "-p"], env=wine_env(), **quiet)
    subprocess.run([WINE, "cmd", "/c", "exit"], env=wine_env(), timeout=300, **quiet)


def run_command(binary):
    return [str(binary)] if NATIVE else [WINE, str(binary)]
