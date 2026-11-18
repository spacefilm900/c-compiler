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
        Target::Arm64Apple { .. } => {
            if let Some(sdk) = apple_sdk(sysroot) {
                library.push(sdk.join("usr/include"));
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
    match target {
        Target::X86_64Linux => out.extend([
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
        Target::Arm64Apple { .. } => out.extend([
            ("__APPLE__", "1"),
            ("__APPLE_CC__", "6000"),
            ("__MACH__", "1"),
            ("__arm64__", "1"),
            ("__arm64", "1"),
            ("__DYNAMIC__", "1"),
            ("__USER_LABEL_PREFIX__", "_"),
        ]),
    }
    if let Target::Arm64Apple {
        macos_major,
        macos_minor,
    } = target
    {
        // `110000` for macOS 11.0, as Availability.h expects.
        let version: &'static str =
            Box::leak(format!("{}", macos_major * 10000 + macos_minor * 100).into_boxed_str());
        out.push(("__ENVIRONMENT_MAC_OS_X_VERSION_MIN_REQUIRED__", version));
        out.push(("__ENVIRONMENT_OS_VERSION_MIN_REQUIRED__", version));
    }
    if !target.char_is_signed() {
        out.push(("__CHAR_UNSIGNED__", "1"));
    }
    if target.wchar_is_signed() {
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
    // wint_t is `int` on Apple and `unsigned int` in glibc.
    if target.is_apple() {
        out.push(("__WINT_TYPE__", "int"));
    } else {
        out.push(("__WINT_TYPE__", "unsigned int"));
    }
    float_macros(target.long_double(), &mut out);
    out
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

pub(super) fn push_compiler(dirs: &mut Vec<PathBuf>, path: PathBuf) {
    if path.join("stddef.h").is_file() && !dirs.iter().any(|dir| dir == &path) {
        dirs.push(path);
    }
}
