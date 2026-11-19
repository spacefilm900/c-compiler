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

Env: WINE (default wine), MINGW_GCC (default x86_64-w64-mingw32-gcc).
"""
import os
import shutil
import subprocess

GNU = "x86_64-w64-windows-gnu"
MSVC = "x86_64-pc-windows-msvc"
TARGETS = (GNU, MSVC)
WINE = os.environ.get("WINE", "wine")
MINGW_GCC = os.environ.get("MINGW_GCC", "x86_64-w64-mingw32-gcc")
MINGW_ROOT = "/usr/x86_64-w64-mingw32"
# The MSVC stand-in: MinGW-w64's headers without MSVC's identification
# macros, which would select code paths for Microsoft's own compiler.
# printf and friends come from msvcrt.dll, as with MSVC, not MinGW-w64's
# own (__USE_MINGW_ANSI_STDIO), which reads `%Lf` as the x87 format.
MSVC_STANDIN_FLAGS = ["-isysroot", MINGW_ROOT, "-U_MSC_VER", "-U_MSC_FULL_VER", "-U_MSC_BUILD",
                      "-D__MINGW32__", "-D__MINGW64__", "-D__MSVCRT__", "-D__GNUC__=4",
                      "-D__GNUC_MINOR__=2", "-D__declspec(a)=__attribute__((a))",
                      "-D__cdecl=__attribute__((__cdecl__))", "-D__USE_MINGW_ANSI_STDIO=0"]
# Clang for the MSVC triple: the same stand-in, and no MSVC compatibility
# extensions, so both compilers see the same headers the same way.
CLANG_MSVC_FLAGS = ["-isystem", MINGW_ROOT + "/include", "-fno-ms-compatibility",
                    "-fno-ms-extensions", "-U_MSC_VER", "-U_MSC_FULL_VER", "-U_MSC_BUILD",
                    "-D__MINGW32__", "-D__MINGW64__", "-D__MSVCRT__", "-D__GNUC__=4",
                    "-D__GNUC_MINOR__=2", "-D__declspec(a)=__attribute__((a))",
                    "-D__cdecl=__attribute__((__cdecl__))", "-D__USE_MINGW_ANSI_STDIO=0",
                    "-fno-stack-protector"]


def ours_flags(target):
    return list(MSVC_STANDIN_FLAGS) if target == MSVC else []


def clang_flags(target):
    return list(CLANG_MSVC_FLAGS) if target == MSVC else []


def available():
    """An error string when the MinGW-w64 toolchain or Wine is missing."""
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
    extra = []
    if target == MSVC:
        shim = f"{out}.chkstk.s"
        with open(shim, "w") as handle:
            handle.write(CHKSTK_SHIM)
        extra.append(shim)
    return [MINGW_GCC, *[str(o) for o in objs], *extra, "-o", str(out)]


def wine_env():
    env = dict(os.environ)
    env.setdefault("WINEDEBUG", "-all")
    # No Gecko/Mono installers, and no debugger on a crash: winedbg would
    # wait for input until the harness's timeout.
    env.setdefault("WINEDLLOVERRIDES", "mscoree,mshtml=;winedbg.exe=d")
    return env


def start_server():
    """Keep one wineserver alive for the whole run, so each program does
    not pay for starting it."""
    # The server outlives this call, so it must not hold a pipe of ours.
    quiet = {"stdin": subprocess.DEVNULL, "stdout": subprocess.DEVNULL,
             "stderr": subprocess.DEVNULL}
    subprocess.run(["wineserver", "-p"], env=wine_env(), **quiet)
    subprocess.run([WINE, "cmd", "/c", "exit"], env=wine_env(), timeout=300, **quiet)


def run_command(binary):
    return [WINE, str(binary)]
