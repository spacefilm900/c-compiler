//! Predefined macros (C11 6.10.8 and the GNU ones the target headers expect)
//! and the system include directories for each target.

use super::*;

pub(super) fn default_system_dirs() -> Vec<PathBuf> {
    system_dirs_for(Target::build_default(), None)
}

/// The `#include <...>` search list for `target`.
///
/// The compiler's own headers (stddef.h, stdarg.h, stdbool.h, limits.h,
/// float.h) come before the C library, the same way a hosted compiler
/// searches its builtin include directory first. The C library is the
/// host's on x86_64 Linux and AArch64 Linux (the Debian/Ubuntu multiarch
/// cross directory when compiling for AArch64 on another host). For macOS
/// it is the SDK: `sysroot` (`-isysroot`), else `$SDKROOT`, else, on a Mac,
/// the path `xcrun --show-sdk-path` prints.
pub fn system_dirs_for(target: Target, sysroot: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    push_compiler(&mut dirs, PathBuf::from("include"));
    push_compiler(
        &mut dirs,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("include"),
    );
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(Path::to_path_buf);
        for _ in 0..8 {
            let Some(current) = dir else { break };
            push_compiler(&mut dirs, current.join("include"));
            dir = current.parent().map(Path::to_path_buf);
        }
    }
    let mut library: Vec<PathBuf> = Vec::new();
    match target {
        Target::X86_64Linux => {
            if sysroot.is_none() && Target::host() != Target::X86_64Linux {
                // Debian/Ubuntu cross headers (libc6-dev-amd64-cross).
                library.push("/usr/x86_64-linux-gnu/include".into());
            } else {
                let root = sysroot.map(Path::to_path_buf).unwrap_or_else(|| "/".into());
                library.push(root.join("usr/include/x86_64-linux-gnu"));
                library.push(root.join("usr/include"));
            }
        }
        Target::Aarch64Linux => {
            if let Some(root) = sysroot {
                library.push(root.join("usr/include/aarch64-linux-gnu"));
                library.push(root.join("usr/include"));
            } else if Target::host() == Target::Aarch64Linux {
                library.push("/usr/include/aarch64-linux-gnu".into());
                library.push("/usr/include".into());
            } else {
                library.push("/usr/aarch64-linux-gnu/include".into());
            }
        }
        Target::Arm64Apple { .. } | Target::X86_64Apple { .. } => {
            if let Some(sdk) = apple_sdk(sysroot) {
                library.push(sdk.join("usr/include"));
            }
        }
        Target::X86_64Windows { msvc: false } => {
            // A MinGW-w64 sysroot keeps its headers in `include`; Debian and
            // Ubuntu install the cross headers (mingw-w64-x86-64-dev) at
            // /usr/x86_64-w64-mingw32.
            let root = sysroot
                .map(Path::to_path_buf)
                .unwrap_or_else(|| "/usr/x86_64-w64-mingw32".into());
            library.push(root.join("include"));
        }
        Target::X86_64Windows { msvc: true } => {
            // The MSVC and Windows SDK headers, as a Visual Studio developer
            // prompt lists them in `INCLUDE` (separated by `;`), or under
            // `sysroot/include` when one is given.
            if let Some(root) = sysroot {
                library.push(root.join("include"));
            } else if let Some(list) = std::env::var_os("INCLUDE") {
                let list = list.to_string_lossy().into_owned();
                library.extend(
                    list.split(';')
                        .filter(|dir| !dir.is_empty())
                        .map(PathBuf::from),
                );
            }
        }
    }
    for path in library {
        if path.is_dir() && !dirs.iter().any(|dir| dir == &path) {
            dirs.push(path);
        }
    }
    dirs
}

pub(super) fn apple_sdk(sysroot: Option<&Path>) -> Option<PathBuf> {
    if let Some(root) = sysroot {
        return Some(root.to_path_buf());
    }
    if let Some(root) = std::env::var_os("SDKROOT").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(root));
    }
    if !cfg!(target_os = "macos") {
        return None;
    }
    let output = std::process::Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let path = text.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// Predefined macros that depend on the target (C11 6.10.8 leaves the
/// rest to the implementation). The sets were chosen by comparing with
/// `clang -dM -E` for each triple as a black box; macros for features this
/// compiler lacks (Blocks, NEON vectors, Objective-C) are left out so a
/// header does not take a path it cannot compile.
pub(super) fn target_macros(target: Target) -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    data_model_macros(target, &mut out);
    match target {
        Target::X86_64Linux | Target::X86_64Apple { .. } | Target::X86_64Windows { .. } => out
            .extend([
                ("__x86_64__", "1"),
                ("__x86_64", "1"),
                ("__amd64__", "1"),
                ("__amd64", "1"),
            ]),
        Target::Arm64Apple { .. } | Target::Aarch64Linux => out.extend([
            ("__aarch64__", "1"),
            ("__AARCH64EL__", "1"),
            ("__ARM_64BIT_STATE", "1"),
            ("__ARM_ARCH", "8"),
            ("__ARM_ARCH_ISA_A64", "1"),
            ("__ARM_ARCH_PROFILE", "'A'"),
            ("__ARM_PCS_AAPCS64", "1"),
            ("__ARM_SIZEOF_WCHAR_T", "4"),
            ("__ARM_SIZEOF_MINIMAL_ENUM", "4"),
        ]),
    }
    match target {
        Target::X86_64Linux | Target::Aarch64Linux => out.extend([
            ("__linux__", "1"),
            ("__linux", "1"),
            ("__unix__", "1"),
            ("__unix", "1"),
            ("__gnu_linux__", "1"),
            ("__ELF__", "1"),
            ("__USER_LABEL_PREFIX__", ""),
        ]),
        Target::Arm64Apple { .. } | Target::X86_64Apple { .. } => out.extend([
            ("__APPLE__", "1"),
            ("__APPLE_CC__", "6000"),
            ("__MACH__", "1"),
            ("__DYNAMIC__", "1"),
            ("__USER_LABEL_PREFIX__", "_"),
            // <libkern/OSByteOrder.h> (reached from <mach/mach.h>) stops
            // with "#error Unknown endianess" without it; Clang defines it.
            ("__LITTLE_ENDIAN__", "1"),
        ]),
        Target::X86_64Windows { msvc } => {
            out.extend([
                ("_WIN32", "1"),
                ("_WIN64", "1"),
                ("__USER_LABEL_PREFIX__", ""),
                // MinGW-w64's <_mingw.h> otherwise declares `__int128`
                // itself with `__mode__(TI)`.
                ("__SIZEOF_INT128__", "16"),
                // GCC and Clang define this when they inline nothing (at
                // -O0). MinGW-w64's headers then declare their CRT
                // functions instead of defining inline versions, some of
                // which are x87 extended asm (`fabsl` in <math.h>) that this
                // compiler does not accept. The library has every one.
                ("__NO_INLINE__", "1"),
            ]);
            if msvc {
                out.extend(msvc_macros());
            } else {
                out.extend(mingw_macros());
            }
        }
    }
    if let Target::Arm64Apple { .. } = target {
        out.extend([("__arm64__", "1"), ("__arm64", "1")]);
    }
    // The macOS SDK's <libkern/i386/_OSByteOrder.h> uses __builtin_bswap32
    // and __builtin_bswap64 only when __llvm__ is defined and falls back to
    // GNU extended asm otherwise, which this compiler does not accept. The
    // compiler emits LLVM IR and provides both builtins, so it says so on
    // Intel macOS. Other targets are left as they were.
    if let Target::X86_64Apple { .. } = target {
        out.push(("__llvm__", "1"));
    }
    if let Some((major, minor)) = target.macos_version() {
        let version: &'static str = Box::leak(macos_version_macro(major, minor).into_boxed_str());
        out.push(("__ENVIRONMENT_MAC_OS_X_VERSION_MIN_REQUIRED__", version));
        out.push(("__ENVIRONMENT_OS_VERSION_MIN_REQUIRED__", version));
    }
    if !target.char_is_signed() {
        out.push(("__CHAR_UNSIGNED__", "1"));
    }
    if target.is_windows() {
        out.extend([
            ("__WCHAR_TYPE__", "unsigned short"),
            ("__WCHAR_MAX__", "65535"),
            ("__WCHAR_UNSIGNED__", "1"),
            ("__SIZEOF_WCHAR_T__", "2"),
            ("__SIZEOF_WINT_T__", "2"),
        ]);
    } else if target.wchar_is_signed() {
        out.extend([
            ("__WCHAR_TYPE__", "int"),
            ("__WCHAR_MAX__", "2147483647"),
            ("__WCHAR_MIN__", "(-2147483647 - 1)"),
        ]);
    } else {
        out.extend([
            ("__WCHAR_TYPE__", "unsigned int"),
            ("__WCHAR_MAX__", "4294967295U"),
            ("__WCHAR_MIN__", "0U"),
            ("__WCHAR_UNSIGNED__", "1"),
        ]);
    }
    // wint_t is `int` on Apple, `unsigned int` in glibc, and `unsigned
    // short` on Windows.
    if target.is_windows() {
        out.push(("__WINT_TYPE__", "unsigned short"));
    } else if target.is_apple() {
        out.push(("__WINT_TYPE__", "int"));
    } else {
        out.push(("__WINT_TYPE__", "unsigned int"));
    }
    float_macros(target.long_double(), &mut out);
    out
}

/// The data-model macros: LP64 everywhere except Windows, which is LLP64
/// (`long` is 32 bits; `size_t`, `ptrdiff_t`, `intptr_t` and `intmax_t`
/// are `long long`), and the GNU version macros, which every target but
/// MSVC defines (Clang leaves them out for MSVC, observed as a black box).
fn data_model_macros(target: Target, out: &mut Vec<(&'static str, &'static str)>) {
    if target.long_is_64() {
        out.extend([
            ("__LP64__", "1"),
            ("_LP64", "1"),
            ("__SIZEOF_LONG__", "8"),
            ("__LONG_WIDTH__", "64"),
            ("__LONG_MAX__", "9223372036854775807L"),
            ("__INTMAX_MAX__", "9223372036854775807L"),
            ("__UINTMAX_MAX__", "18446744073709551615UL"),
            ("__SIZE_TYPE__", "long unsigned int"),
            ("__PTRDIFF_TYPE__", "long int"),
            ("__INTMAX_TYPE__", "long int"),
            ("__UINTMAX_TYPE__", "long unsigned int"),
            ("__INTPTR_TYPE__", "long int"),
            ("__UINTPTR_TYPE__", "long unsigned int"),
        ]);
    } else {
        out.extend([
            ("__SIZEOF_LONG__", "4"),
            ("__LONG_WIDTH__", "32"),
            ("__LONG_MAX__", "2147483647L"),
            ("__INTMAX_MAX__", "9223372036854775807LL"),
            ("__UINTMAX_MAX__", "18446744073709551615ULL"),
            ("__SIZE_MAX__", "18446744073709551615ULL"),
            ("__PTRDIFF_MAX__", "9223372036854775807LL"),
            ("__INTPTR_MAX__", "9223372036854775807LL"),
            ("__UINTPTR_MAX__", "18446744073709551615ULL"),
            ("__SIZE_TYPE__", "long long unsigned int"),
            ("__PTRDIFF_TYPE__", "long long int"),
            ("__INTMAX_TYPE__", "long long int"),
            ("__UINTMAX_TYPE__", "long long unsigned int"),
            ("__INTPTR_TYPE__", "long long int"),
            ("__UINTPTR_TYPE__", "long long unsigned int"),
            ("__INT64_TYPE__", "long long int"),
            ("__UINT64_TYPE__", "long long unsigned int"),
            ("__INT64_C_SUFFIX__", "LL"),
            ("__UINT64_C_SUFFIX__", "ULL"),
        ]);
    }
    if !target.is_msvc() {
        out.extend([
            ("__GNUC__", "4"),
            ("__GNUC_MINOR__", "2"),
            ("__GNUC_PATCHLEVEL__", "1"),
            ("__GNUC_STDC_INLINE__", "1"),
        ]);
    }
}

/// MinGW-w64 (`x86_64-w64-windows-gnu`): the macros Clang predefines for
/// the triple (observed as a black box). The Microsoft calling-convention
/// keywords become GNU attributes, which this compiler accepts and, on
/// x86-64 where there is one convention, ignores.
fn mingw_macros() -> [(&'static str, &'static str); 22] {
    [
        ("__MINGW32__", "1"),
        ("__MINGW64__", "1"),
        ("__MSVCRT__", "1"),
        ("__SEH__", "1"),
        ("__WIN32", "1"),
        ("__WIN32__", "1"),
        ("__WIN64", "1"),
        ("__WIN64__", "1"),
        ("__WINNT", "1"),
        ("__WINNT__", "1"),
        ("__declspec(a)", "__attribute__((a))"),
        ("__cdecl", "__attribute__((__cdecl__))"),
        ("__stdcall", "__attribute__((__stdcall__))"),
        ("__fastcall", "__attribute__((__fastcall__))"),
        ("__thiscall", "__attribute__((__thiscall__))"),
        ("__pascal", "__attribute__((__pascal__))"),
        ("_cdecl", "__attribute__((__cdecl__))"),
        ("_stdcall", "__attribute__((__stdcall__))"),
        ("_fastcall", "__attribute__((__fastcall__))"),
        ("_thiscall", "__attribute__((__thiscall__))"),
        ("_pascal", "__attribute__((__pascal__))"),
        ("__GXX_TYPEINFO_EQUALITY_INLINE", "0"),
    ]
}

/// MSVC (`x86_64-pc-windows-msvc`): the identification macros Clang
/// predefines for the triple (observed as a black box). `_MSC_VER` 1933 is
/// Visual Studio 2022 17.3, Clang's default.
fn msvc_macros() -> Vec<(&'static str, &'static str)> {
    let mut macros = vec![
        ("_MSC_VER", "1933"),
        ("_MSC_FULL_VER", "193300000"),
        ("_MSC_BUILD", "1"),
        ("_M_X64", "100"),
        ("_M_AMD64", "100"),
        ("_INTEGRAL_MAX_BITS", "64"),
        ("_ISO_VOLATILE", "1"),
    ];
    macros.extend(MSVC_KEYWORDS);
    macros
}

/// Microsoft's C keywords that the MSVC and Windows SDK headers use, as
/// macros over this compiler's own spellings (Microsoft's public C language
/// reference). Clang treats them as keywords; macros are enough for the
/// headers. On x64 the calling-convention keywords other than
/// `__vectorcall` mean the one convention, so they expand to nothing.
/// `__vectorcall` (more vector registers) would change the code, so it
/// becomes an attribute that the parser rejects. `__ptr32` (a 4-byte
/// pointer) is not supported and is erased, because <basetsd.h> uses it in
/// inline functions that every <windows.h> user sees; a record with a
/// `__ptr32` member gets the wrong layout (see the platform report). MinGW-w64's <_mingw.h> defines
/// most of these itself, so the MSVC stand-in (harness/windows_x64.py)
/// undefines them.
pub const MSVC_KEYWORDS: [(&str, &str); 22] = [
    ("__int8", "char"),
    ("__int16", "short"),
    ("__int32", "int"),
    ("__int64", "long long"),
    ("__cdecl", ""),
    ("__stdcall", ""),
    ("__fastcall", ""),
    ("__thiscall", ""),
    ("__vectorcall", "__attribute__((__vectorcall__))"),
    ("__clrcall", ""),
    ("__ptr32", ""),
    ("__ptr64", ""),
    ("__unaligned", ""),
    ("__w64", ""),
    ("__sptr", ""),
    ("__uptr", ""),
    ("__inline", "inline"),
    (
        "__forceinline",
        "__inline__ __attribute__((__always_inline__))",
    ),
    ("__declspec(spec)", "__attribute__((spec))"),
    ("__pragma(text)", "_Pragma(#text)"),
    ("__restrict", "restrict"),
    ("__alignof", "_Alignof"),
];

/// `__ENVIRONMENT_MAC_OS_X_VERSION_MIN_REQUIRED__` as Availability.h
/// expects it: `1090` for 10.9 (the old four-digit form, used before 10.10),
/// `101500` for 10.15, `110000` for 11.0.
pub(super) fn macos_version_macro(major: u32, minor: u32) -> String {
    if major == 10 && minor < 10 {
        format!("{}", 1000 + minor * 10)
    } else {
        format!("{}", major * 10000 + minor * 100)
    }
}

/// The floating-type characteristics, `__FLT_*__`, `__DBL_*__` and
/// `__LDBL_*__`, for a target whose `long double` is `long_double`.
fn float_macros(long_double: LongDouble, out: &mut Vec<(&'static str, &'static str)>) {
    match long_double {
        LongDouble::X87 => out.extend([
            ("__SIZEOF_LONG_DOUBLE__", "16"),
            ("__LDBL_MANT_DIG__", "64"),
        ]),
        LongDouble::Double => {
            out.extend([("__SIZEOF_LONG_DOUBLE__", "8"), ("__LDBL_MANT_DIG__", "53")])
        }
        LongDouble::Quad => out.extend([
            ("__SIZEOF_LONG_DOUBLE__", "16"),
            ("__LDBL_MANT_DIG__", "113"),
        ]),
    }
    // The floating characteristics GCC's manual lists among its common
    // predefined macros, with the values of include/float.h. macOS's
    // <math.h> uses `__FLT_MIN__` and friends directly.
    out.extend([
        ("__FLT_RADIX__", "2"),
        ("__FLT_MANT_DIG__", "24"),
        ("__FLT_DIG__", "6"),
        ("__FLT_MIN_EXP__", "(-125)"),
        ("__FLT_MIN_10_EXP__", "(-37)"),
        ("__FLT_MAX_EXP__", "128"),
        ("__FLT_MAX_10_EXP__", "38"),
        ("__FLT_DECIMAL_DIG__", "9"),
        ("__FLT_MAX__", "0x1.fffffep+127F"),
        ("__FLT_MIN__", "0x1p-126F"),
        ("__FLT_EPSILON__", "0x1p-23F"),
        ("__FLT_DENORM_MIN__", "0x1p-149F"),
        ("__DBL_MANT_DIG__", "53"),
        ("__DBL_DIG__", "15"),
        ("__DBL_MIN_EXP__", "(-1021)"),
        ("__DBL_MIN_10_EXP__", "(-307)"),
        ("__DBL_MAX_EXP__", "1024"),
        ("__DBL_MAX_10_EXP__", "308"),
        ("__DBL_DECIMAL_DIG__", "17"),
        ("__DBL_MAX__", "0x1.fffffffffffffp+1023"),
        ("__DBL_MIN__", "0x1p-1022"),
        ("__DBL_EPSILON__", "0x1p-52"),
        ("__DBL_DENORM_MIN__", "0x1p-1074"),
    ]);
    match long_double {
        LongDouble::Double => out.extend([
            ("__LDBL_DIG__", "15"),
            ("__LDBL_MIN_EXP__", "(-1021)"),
            ("__LDBL_MIN_10_EXP__", "(-307)"),
            ("__LDBL_MAX_EXP__", "1024"),
            ("__LDBL_MAX_10_EXP__", "308"),
            ("__LDBL_DECIMAL_DIG__", "17"),
            ("__LDBL_MAX__", "0x1.fffffffffffffp+1023L"),
            ("__LDBL_MIN__", "0x1p-1022L"),
            ("__LDBL_EPSILON__", "0x1p-52L"),
            ("__LDBL_DENORM_MIN__", "0x1p-1074L"),
        ]),
        LongDouble::Quad => out.extend([
            ("__LDBL_DIG__", "33"),
            ("__LDBL_MIN_EXP__", "(-16381)"),
            ("__LDBL_MIN_10_EXP__", "(-4931)"),
            ("__LDBL_MAX_EXP__", "16384"),
            ("__LDBL_MAX_10_EXP__", "4932"),
            ("__LDBL_DECIMAL_DIG__", "36"),
            (
                "__LDBL_MAX__",
                "1.18973149535723176508575932662800702e+4932L",
            ),
            (
                "__LDBL_MIN__",
                "3.36210314311209350626267781732175260e-4932L",
            ),
            (
                "__LDBL_EPSILON__",
                "1.92592994438723585305597794258492732e-34L",
            ),
            (
                "__LDBL_DENORM_MIN__",
                "6.47517511943802511092443895822764655e-4966L",
            ),
        ]),
        LongDouble::X87 => out.extend([
            ("__LDBL_DIG__", "18"),
            ("__LDBL_MIN_EXP__", "(-16381)"),
            ("__LDBL_MIN_10_EXP__", "(-4931)"),
            ("__LDBL_MAX_EXP__", "16384"),
            ("__LDBL_MAX_10_EXP__", "4932"),
            ("__LDBL_DECIMAL_DIG__", "21"),
            ("__LDBL_MAX__", "1.18973149535723176502e+4932L"),
            ("__LDBL_MIN__", "3.36210314311209350626e-4932L"),
            ("__LDBL_EPSILON__", "1.08420217248550443401e-19L"),
            ("__LDBL_DENORM_MIN__", "3.64519953188247460253e-4951L"),
        ]),
    }
}

/// Adds the compiler's header directory: the first candidate that has all
/// of its headers, in the order tried (relative to the working directory, from
/// the build, beside the executable). Only one tree is used: a second
/// copy, even at another path, would make `#include_next` in one of the
/// compiler's headers find its twin (whose guard skips it) instead of the
/// C library's header.
pub(super) fn push_compiler(dirs: &mut Vec<PathBuf>, path: PathBuf) {
    if dirs.is_empty()
        && COMPILER_HEADERS
            .iter()
            .all(|name| path.join(name).is_file())
    {
        dirs.push(path);
    }
}

/// The headers the compiler provides (`include/`). A candidate counts as
/// its header tree only when it has all of them, so a project's own
/// `include/` with a `stddef.h` is not mistaken for it.
const COMPILER_HEADERS: [&str; 8] = [
    "float.h",
    "iso646.h",
    "limits.h",
    "stdalign.h",
    "stdarg.h",
    "stdbool.h",
    "stddef.h",
    "stdnoreturn.h",
];
