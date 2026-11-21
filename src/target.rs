//! The machine a translation unit is compiled for.
//!
//! One compiler binary serves every target. The driver picks a [`Target`]
//! with `-target` / `--target` (the host by default), and the frontend reads
//! it through [`current`] wherever the C type model, the predefined macros,
//! or the calling convention differ. The selection is per thread, so unit
//! tests that run in parallel can each compile for a different target.
//!
//! Sources, by target:
//!
//! - `x86_64-linux-gnu`: the System V AMD64 psABI.
//! - `arm64-apple-macos`: Arm's AAPCS64 with the deviations in Apple's
//!   public "Writing ARM64 code for Apple platforms": `char` is signed,
//!   `long double` is the IEEE double format, the caller extends integer
//!   arguments narrower than 32 bits, every variadic argument is passed on
//!   the stack in 8-byte slots, and `va_list` is a plain `char *`.
//! - `x86_64-apple-macos`: the System V AMD64 psABI, as Apple's public
//!   "Writing 64-bit Intel code for Apple Platforms" adopts it, with Mach-O
//!   symbol names (a leading `_`), `int64_t` as `long long`, and `wint_t`
//!   as `int`. `long double` is the x87 80-bit format, as on Linux.
//! - `aarch64-linux-gnu`: AAPCS64 and its Linux (GNU) C type mapping:
//!   `char` and `wchar_t` are unsigned, `long double` is IEEE binary128,
//!   and `va_list` is the AAPCS64 register-save structure. It exists to run
//!   AArch64 code under user-mode emulation on a Linux host.
//! - `x86_64-pc-windows-msvc` and `x86_64-w64-windows-gnu` (MinGW-w64):
//!   Microsoft's published "x64 calling convention" and "x64 software
//!   conventions", and its LLP64 data model ("Data Type Ranges"): `int` and
//!   `long` are 32 bits, `long long` and pointers 64, `size_t` is
//!   `unsigned long long`, and `wchar_t` is a 16-bit `unsigned short`.
//!   `long double` is `double` for MSVC; MinGW keeps the x87 80-bit format
//!   in 16 bytes (observed with Clang as a black box). Records use the
//!   Microsoft bit-field layout that the GCC manual describes under
//!   `-mms-bitfields`, the default for both.
//!
//! LLVM data layout strings are the ones LLVM 18 expects for each triple.
//! LLVM 19 reads them unchanged and adds its function-pointer alignment
//! itself (`-Fn32` on AArch64), so one string serves both versions.

use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    /// `x86_64-pc-linux-gnu`, System V AMD64.
    X86_64Linux,
    /// `arm64-apple-macosx`, Apple's arm64 ABI. `macos_major` is the
    /// minimum macOS version in the triple (11 and later run on Apple
    /// silicon).
    Arm64Apple { macos_major: u32, macos_minor: u32 },
    /// `aarch64-unknown-linux-gnu`, AAPCS64 with glibc.
    Aarch64Linux,
    /// `x86_64-apple-macosx`, System V AMD64 on Darwin (Intel Macs).
    /// `macos_major.macos_minor` is the minimum macOS version.
    X86_64Apple { macos_major: u32, macos_minor: u32 },
    /// 64-bit Windows: `x86_64-pc-windows-msvc` when `msvc`, otherwise
    /// `x86_64-w64-windows-gnu` (MinGW-w64). Both use the Microsoft x64
    /// calling convention and LLP64.
    X86_64Windows { msvc: bool },
}

/// How `long double` is represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LongDouble {
    /// x87 80-bit extended, stored in 16 bytes (`x86_fp80`).
    X87,
    /// The same format as `double` (`double`, 8 bytes).
    Double,
    /// IEEE binary128 (`fp128`, 16 bytes).
    Quad,
}

/// The calling convention family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abi {
    SysV,
    /// AAPCS64. `apple` selects Apple's deviations.
    Aapcs64 {
        apple: bool,
    },
    /// The Microsoft x64 calling convention.
    Win64,
}

/// What `__builtin_va_list` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaListKind {
    /// System V AMD64: `[1 x struct __va_list_tag]`, the register-save
    /// descriptor.
    SysV,
    /// Apple arm64 and Windows x64: a `char *` into the argument area.
    CharPtr,
    /// AAPCS64 (AArch64 Linux): `struct __va_list`, passed by value.
    Aapcs64,
}

impl Default for Target {
    fn default() -> Self {
        Target::build_default()
    }
}

pub const APPLE_DEFAULT: Target = Target::Arm64Apple {
    macos_major: 11,
    macos_minor: 0,
};

/// The default Intel macOS target: macOS 10.15, the first release that
/// runs only 64-bit programs. Earlier versions select older C library
/// symbol variants in the SDK headers (32-bit `ino_t` among them).
pub const APPLE_X86_64_DEFAULT: Target = Target::X86_64Apple {
    macos_major: 10,
    macos_minor: 15,
};

/// `x86_64-pc-windows-msvc`.
pub const WINDOWS_MSVC: Target = Target::X86_64Windows { msvc: true };

/// `x86_64-w64-windows-gnu` (MinGW-w64).
pub const WINDOWS_GNU: Target = Target::X86_64Windows { msvc: false };

impl Target {
    /// The machine this compiler binary runs on, when it is a supported
    /// target; otherwise `x86_64-linux-gnu`.
    /// The target used when none is named: the host. Unit tests use
    /// x86_64 Linux instead, on any machine, because their expected IR and
    /// layouts are written for it; the arm64 targets are tested by name
    /// (`target_tests.rs`).
    pub fn build_default() -> Target {
        if cfg!(test) {
            Target::X86_64Linux
        } else {
            Target::host()
        }
    }

    pub fn host() -> Target {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            APPLE_DEFAULT
        } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            APPLE_X86_64_DEFAULT
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            Target::Aarch64Linux
        } else if cfg!(all(windows, target_arch = "x86_64", target_env = "gnu")) {
            WINDOWS_GNU
        } else if cfg!(all(windows, target_arch = "x86_64")) {
            WINDOWS_MSVC
        } else {
            Target::X86_64Linux
        }
    }

    /// Parse a `-target` triple. Accepted spellings follow the usual
    /// `arch-vendor-os[-env]` form:
    ///
    /// - `x86_64-linux-gnu`, `x86_64-pc-linux-gnu`, `x86_64-unknown-linux-gnu`
    /// - `arm64-apple-macos`, `arm64-apple-macosx`, `arm64-apple-darwin`,
    ///   `aarch64-apple-darwin`, with an optional version such as
    ///   `arm64-apple-macos13` or `arm64-apple-macosx11.0.0`
    /// - `aarch64-linux-gnu`, `aarch64-unknown-linux-gnu`, `arm64-linux-gnu`
    /// - `x86_64-apple-macos`, `x86_64-apple-macosx`, `x86_64-apple-darwin`,
    ///   with an optional version of 10.0 or later, such as
    ///   `x86_64-apple-macos13` or `x86_64-apple-macosx10.15.0`
    /// - `x86_64-pc-windows-msvc`, `x86_64-windows-msvc`, `x86_64-pc-windows`
    ///   (MSVC is the default Windows environment, as in Clang), with an
    ///   optional MSVC version such as `x86_64-pc-windows-msvc19.33.0`
    /// - `x86_64-w64-windows-gnu`, `x86_64-pc-windows-gnu`,
    ///   `x86_64-w64-mingw32`
    ///
    /// A `darwinN` version is a Darwin kernel version and is translated to
    /// its macOS version (`darwin25` is macOS 26, `darwin20` is macOS 11,
    /// `darwin19` is 10.15).
    pub fn parse(triple: &str) -> Option<Target> {
        let parts: Vec<&str> = triple.split('-').collect();
        let arch = *parts.first()?;
        let rest = &parts[1..];
        match arch {
            "x86_64" | "amd64" => {
                if rest.first() == Some(&"apple") {
                    if rest.len() != 2 || arch != "x86_64" {
                        return None;
                    }
                    let (major, minor) = parse_apple_os(rest[1], 10)?;
                    return Some(match (major, minor) {
                        (0, _) => APPLE_X86_64_DEFAULT,
                        (macos_major, macos_minor) => Target::X86_64Apple {
                            macos_major,
                            macos_minor,
                        },
                    });
                }
                let rest: Vec<&str> = rest
                    .iter()
                    .copied()
                    .filter(|part| *part != "pc" && *part != "unknown" && *part != "w64")
                    .collect();
                if let Some(windows) = parse_windows(&rest) {
                    return (arch == "x86_64").then_some(windows);
                }
                (rest == ["linux", "gnu"] || rest == ["linux"]).then_some(Target::X86_64Linux)
            }
            "arm64" | "aarch64" => {
                if rest.first() == Some(&"apple") {
                    if rest.len() != 2 {
                        return None;
                    }
                    let (major, minor) = parse_apple_os(rest[1], 11)?;
                    return Some(match (major, minor) {
                        (0, _) => APPLE_DEFAULT,
                        (macos_major, macos_minor) => Target::Arm64Apple {
                            macos_major,
                            macos_minor,
                        },
                    });
                }
                let rest: Vec<&str> = rest
                    .iter()
                    .copied()
                    .filter(|part| *part != "unknown")
                    .collect();
                (rest == ["linux", "gnu"] || rest == ["linux"]).then_some(Target::Aarch64Linux)
            }
            _ => None,
        }
    }

    /// The canonical name used in messages and in the test harness.
    pub fn name(self) -> &'static str {
        match self {
            Target::X86_64Linux => "x86_64-linux-gnu",
            Target::Arm64Apple { .. } => "arm64-apple-macos",
            Target::Aarch64Linux => "aarch64-linux-gnu",
            Target::X86_64Apple { .. } => "x86_64-apple-macos",
            Target::X86_64Windows { msvc: true } => "x86_64-pc-windows-msvc",
            Target::X86_64Windows { msvc: false } => "x86_64-w64-windows-gnu",
        }
    }

    /// The LLVM target triple.
    pub fn triple(self) -> String {
        match self {
            Target::X86_64Linux => "x86_64-pc-linux-gnu".to_string(),
            Target::Arm64Apple {
                macos_major,
                macos_minor,
            } => format!("arm64-apple-macosx{macos_major}.{macos_minor}.0"),
            Target::Aarch64Linux => "aarch64-unknown-linux-gnu".to_string(),
            Target::X86_64Apple {
                macos_major,
                macos_minor,
            } => format!("x86_64-apple-macosx{macos_major}.{macos_minor}.0"),
            Target::X86_64Windows { .. } => self.name().to_string(),
        }
    }

    /// The LLVM data layout string (LLVM LangRef, "Data Layout").
    ///
    /// `i64` must be 8-byte aligned. LLVM's default layout, used when this
    /// line is absent, aligns `i64` to 4 bytes, and a struct
    /// `getelementptr` is then folded with the wrong field offset.
    pub fn data_layout(self) -> &'static str {
        match self {
            Target::X86_64Linux => {
                "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
            }
            // `m:o` is Mach-O mangling: LLVM adds the leading underscore
            // to every C symbol.
            Target::Arm64Apple { .. } => "e-m:o-i64:64-i128:128-n32:64-S128",
            Target::Aarch64Linux => "e-m:e-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128",
            Target::X86_64Apple { .. } => {
                "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
            }
            // `m:w` is Windows COFF mangling: no prefix on x86-64 C names.
            Target::X86_64Windows { .. } => {
                "e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
            }
        }
    }

    /// A 64-bit Windows target, MSVC or MinGW.
    pub fn is_windows(self) -> bool {
        matches!(self, Target::X86_64Windows { .. })
    }

    /// The MSVC environment of 64-bit Windows.
    pub fn is_msvc(self) -> bool {
        matches!(self, Target::X86_64Windows { msvc: true })
    }

    /// `long` is 64 bits (LP64), except on Windows (LLP64), where it is 32.
    pub fn long_is_64(self) -> bool {
        !self.is_windows()
    }

    /// `size_t`: `unsigned long` under LP64, `unsigned long long` under
    /// LLP64.
    pub fn size_type(self) -> crate::ast::TypeName {
        if self.long_is_64() {
            crate::ast::TypeName::ULong
        } else {
            crate::ast::TypeName::ULongLong
        }
    }

    /// `ptrdiff_t` and `intptr_t`: `long` under LP64, `long long` under
    /// LLP64.
    pub fn ptrdiff_type(self) -> crate::ast::TypeName {
        if self.long_is_64() {
            crate::ast::TypeName::Long
        } else {
            crate::ast::TypeName::LongLong
        }
    }

    /// The width of `wchar_t` in bits: 16 on Windows (UTF-16 code units),
    /// 32 elsewhere.
    pub fn wchar_bits(self) -> u32 {
        if self.is_windows() {
            16
        } else {
            32
        }
    }

    /// The type of `wchar_t` (and of a wide character constant).
    pub fn wchar_type(self) -> crate::ast::TypeName {
        if self.is_windows() {
            crate::ast::TypeName::UShort
        } else if self.wchar_is_signed() {
            crate::ast::TypeName::Int
        } else {
            crate::ast::TypeName::UInt
        }
    }

    pub fn va_list_kind(self) -> VaListKind {
        match self {
            Target::X86_64Linux | Target::X86_64Apple { .. } => VaListKind::SysV,
            Target::Arm64Apple { .. } | Target::X86_64Windows { .. } => VaListKind::CharPtr,
            Target::Aarch64Linux => VaListKind::Aapcs64,
        }
    }

    /// Records use the Microsoft layout for bit-fields (GCC manual,
    /// `-mms-bitfields`).
    pub fn ms_layout(self) -> bool {
        self.is_windows()
    }

    /// An Apple (Darwin, Mach-O) target on either architecture.
    pub fn is_apple(self) -> bool {
        matches!(self, Target::Arm64Apple { .. } | Target::X86_64Apple { .. })
    }

    /// The minimum macOS version of an Apple target, `(major, minor)`.
    pub fn macos_version(self) -> Option<(u32, u32)> {
        match self {
            Target::Arm64Apple {
                macos_major,
                macos_minor,
            }
            | Target::X86_64Apple {
                macos_major,
                macos_minor,
            } => Some((macos_major, macos_minor)),
            _ => None,
        }
    }

    pub fn is_aarch64(self) -> bool {
        matches!(self, Target::Arm64Apple { .. } | Target::Aarch64Linux)
    }

    pub fn abi(self) -> Abi {
        match self {
            Target::X86_64Linux | Target::X86_64Apple { .. } => Abi::SysV,
            Target::Arm64Apple { .. } => Abi::Aapcs64 { apple: true },
            Target::Aarch64Linux => Abi::Aapcs64 { apple: false },
            Target::X86_64Windows { .. } => Abi::Win64,
        }
    }

    /// Plain `char` is signed on x86_64 and on Apple arm64, and unsigned
    /// in the AAPCS64 Linux mapping.
    pub fn char_is_signed(self) -> bool {
        !matches!(self, Target::Aarch64Linux)
    }

    /// `wchar_t` (and `wint_t`) is `int`, except on AArch64 Linux, where it
    /// is `unsigned int`, and on Windows, where it is `unsigned short`.
    pub fn wchar_is_signed(self) -> bool {
        !matches!(self, Target::Aarch64Linux | Target::X86_64Windows { .. })
    }

    pub fn long_double(self) -> LongDouble {
        match self {
            Target::X86_64Linux
            | Target::X86_64Apple { .. }
            | Target::X86_64Windows { msvc: false } => LongDouble::X87,
            Target::Arm64Apple { .. } | Target::X86_64Windows { msvc: true } => LongDouble::Double,
            Target::Aarch64Linux => LongDouble::Quad,
        }
    }

    /// An array object of 16 bytes or more is aligned to at least 16 bytes
    /// (System V AMD64 psABI 3.1.2), on Linux and on Intel Macs. AAPCS64
    /// and Apple's arm64 ABI align an array as its element type. Clang does
    /// the same on 64-bit Windows (observed as a black box).
    pub fn boosts_large_arrays(self) -> bool {
        matches!(
            self,
            Target::X86_64Linux | Target::X86_64Apple { .. } | Target::X86_64Windows { .. }
        )
    }

    /// The caller extends an integer argument or return value narrower
    /// than 32 bits (`signext` / `zeroext` in LLVM). System V AMD64 and
    /// Apple arm64 require it; AAPCS64 leaves the upper bits unspecified,
    /// and so does the Microsoft x64 convention.
    pub fn extends_narrow_args(self) -> bool {
        !matches!(self, Target::Aarch64Linux | Target::X86_64Windows { .. })
    }

    /// An unnamed bit-field, including a zero-width one, raises the
    /// record's alignment to that of its declared type. AAPCS64 says so
    /// for AArch64 Linux; System V AMD64 and Apple arm64 ignore unnamed
    /// bit-fields for alignment (observed with Clang as a black box).
    pub fn unnamed_bitfield_aligns(self) -> bool {
        matches!(self, Target::Aarch64Linux)
    }

    /// Function attributes every definition carries. Apple's arm64 ABI
    /// requires a valid frame record in x29 at all times, so frame
    /// pointers are kept in non-leaf functions. On Intel Macs every
    /// function keeps its frame pointer, so that Apple's tools can walk
    /// the stack (Clang does the same, observed as a black box).
    pub fn function_attrs(self) -> &'static str {
        match self {
            Target::Arm64Apple { .. } => "\"frame-pointer\"=\"non-leaf\"",
            Target::X86_64Apple { .. } => "\"frame-pointer\"=\"all\"",
            _ => "",
        }
    }

    /// Whether 16-byte lock-free atomics need the `cx16` x86-64 feature
    /// (`cmpxchg16b`). Without it, LLVM emits `__atomic_*_16` libcalls.
    pub fn needs_cx16(self) -> bool {
        matches!(
            self,
            Target::X86_64Linux | Target::X86_64Apple { .. } | Target::X86_64Windows { .. }
        )
    }
}

/// A Windows OS and environment, after the vendor (`pc`, `w64`) is
/// removed: `windows` or `windows-msvc[VERSION]` is MSVC, `windows-gnu`
/// and `mingw32` are MinGW-w64.
fn parse_windows(rest: &[&str]) -> Option<Target> {
    match rest {
        ["windows"] => Some(WINDOWS_MSVC),
        ["windows", env] => {
            if *env == "gnu" {
                return Some(WINDOWS_GNU);
            }
            let version = env.strip_prefix("msvc")?;
            let valid = version.is_empty()
                || version
                    .split('.')
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
            valid.then_some(WINDOWS_MSVC)
        }
        ["mingw32"] => Some(WINDOWS_GNU),
        _ => None,
    }
}

/// The version in an Apple OS name (`macos`, `macosx`, `darwin`), as
/// `(major, minor)`; `(0, 0)` means none was given, so the target's default
/// applies. A version below `min_major` is rejected.
fn parse_apple_os(os: &str, min_major: u32) -> Option<(u32, u32)> {
    let version = if let Some(rest) = os.strip_prefix("macosx") {
        rest
    } else if let Some(rest) = os.strip_prefix("macos") {
        rest
    } else if let Some(rest) = os.strip_prefix("darwin") {
        // A Darwin kernel version names a macOS version: Darwin 25 and
        // later are macOS N+1 (Darwin 25 is macOS 26, when Apple moved to
        // year numbers), Darwin 20 to 24 are macOS N-9 (Darwin 20 is macOS
        // 11), earlier ones are 10.(N-4) (Darwin 19 is 10.15). Only the
        // major number counts, as with Clang (observed as a black box). Bare
        // `darwin` is the default, and so is a version below the target's
        // minimum (an arm64 Darwin before 20), as before.
        if rest.is_empty() {
            return Some((0, 0));
        }
        let mut numbers = rest.split('.');
        let kernel: u32 = numbers.next()?.parse().ok()?;
        if numbers.any(|part| part.parse::<u32>().is_err()) || !(4..=98).contains(&kernel) {
            return None;
        }
        let (major, minor) = match kernel {
            25.. => (kernel + 1, 0),
            20..=24 => (kernel - 9, 0),
            _ => (10, kernel - 4),
        };
        return Some(if major >= min_major {
            (major, minor)
        } else {
            (0, 0)
        });
    } else {
        return None;
    };
    if version.is_empty() {
        return Some((0, 0));
    }
    let mut numbers = version.split('.');
    let major: u32 = numbers.next()?.parse().ok()?;
    let minor: u32 = match numbers.next() {
        Some(text) => text.parse().ok()?,
        None => 0,
    };
    // A patch level other than 0 is rejected rather than silently dropped
    // from the triple and the deployment-target macros.
    if let Some(patch) = numbers.next() {
        let patch: u32 = patch.parse().ok()?;
        if patch != 0 {
            return None;
        }
    }
    // Two digits each, as the deployment-target macros encode them.
    if numbers.next().is_some() || major < min_major || major > 99 || minor > 99 {
        return None;
    }
    Some((major, minor))
}

thread_local! {
    static CURRENT: Cell<Target> = Cell::new(Target::build_default());
}

/// The target of the translation unit being compiled on this thread.
pub fn current() -> Target {
    CURRENT.with(Cell::get)
}

/// Select the target for this thread. [`crate::compile_dialect`] and the
/// preprocessor call this from their options.
pub fn set(target: Target) {
    CURRENT.with(|cell| cell.set(target));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_triples() {
        assert_eq!(Target::parse("x86_64-linux-gnu"), Some(Target::X86_64Linux));
        assert_eq!(
            Target::parse("x86_64-pc-linux-gnu"),
            Some(Target::X86_64Linux)
        );
        assert_eq!(Target::parse("arm64-apple-macos"), Some(APPLE_DEFAULT));
        assert_eq!(Target::parse("aarch64-apple-darwin"), Some(APPLE_DEFAULT));
        assert_eq!(
            Target::parse("arm64-apple-macosx13.2.0"),
            Some(Target::Arm64Apple {
                macos_major: 13,
                macos_minor: 2
            })
        );
        assert_eq!(
            Target::parse("arm64-apple-macos14"),
            Some(Target::Arm64Apple {
                macos_major: 14,
                macos_minor: 0
            })
        );
        assert_eq!(
            Target::parse("aarch64-linux-gnu"),
            Some(Target::Aarch64Linux)
        );
        assert_eq!(
            Target::parse("aarch64-unknown-linux-gnu"),
            Some(Target::Aarch64Linux)
        );
        // A Darwin kernel version maps to its macOS version (Clang as a
        // black box); junk and out-of-range versions are rejected.
        for (triple, major, minor) in [
            ("x86_64-apple-darwin19", 10, 15),
            ("x86_64-apple-darwin15", 10, 11),
            ("x86_64-apple-darwin20", 11, 0),
            ("x86_64-apple-darwin21.6.0", 12, 0),
            ("x86_64-apple-darwin24.6.0", 15, 0),
            ("x86_64-apple-darwin25.0.0", 26, 0),
        ] {
            assert_eq!(
                Target::parse(triple),
                Some(Target::X86_64Apple {
                    macos_major: major,
                    macos_minor: minor
                }),
                "{triple}"
            );
        }
        assert_eq!(
            Target::parse("arm64-apple-darwin23"),
            Some(Target::Arm64Apple {
                macos_major: 14,
                macos_minor: 0
            })
        );
        // Below arm64's minimum, a Darwin spelling keeps meaning the
        // default, as before the versions were read.
        assert_eq!(
            Target::parse("arm64-apple-darwin19.0.0"),
            Some(APPLE_DEFAULT)
        );
        for triple in [
            "x86_64-apple-darwinjunk",
            "x86_64-apple-darwin20.x",
            "x86_64-apple-macos429497",
            "x86_64-apple-macos100",
        ] {
            assert_eq!(Target::parse(triple), None, "{triple}");
        }
        assert_eq!(Target::parse("arm64-apple-macos10.15"), None);
        assert_eq!(Target::parse("arm64-apple-macos13.2.1"), None);
        assert_eq!(Target::parse("arm64-apple-ios"), None);
        assert_eq!(Target::parse("riscv64-linux-gnu"), None);
        assert_eq!(
            Target::parse("x86_64-apple-darwin"),
            Some(APPLE_X86_64_DEFAULT)
        );
        assert_eq!(
            Target::parse("x86_64-apple-macos"),
            Some(APPLE_X86_64_DEFAULT)
        );
        assert_eq!(
            Target::parse("x86_64-apple-macosx10.9"),
            Some(Target::X86_64Apple {
                macos_major: 10,
                macos_minor: 9
            })
        );
        assert_eq!(
            Target::parse("x86_64-apple-macos13.0.0"),
            Some(Target::X86_64Apple {
                macos_major: 13,
                macos_minor: 0
            })
        );
        assert_eq!(Target::parse("x86_64-apple-macos9"), None);
        assert_eq!(Target::parse("x86_64-apple-macos10.15.1"), None);
        assert_eq!(Target::parse("amd64-apple-macos"), None);
        assert_eq!(Target::parse("x86_64-apple-ios"), None);
    }

    #[test]
    fn intel_mac_type_model() {
        let t = APPLE_X86_64_DEFAULT;
        assert_eq!(t.triple(), "x86_64-apple-macosx10.15.0");
        assert_eq!(
            t.data_layout(),
            "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
        );
        assert!(t.is_apple() && !t.is_aarch64());
        assert_eq!(t.abi(), Abi::SysV);
        assert_eq!(t.long_double(), LongDouble::X87);
        assert!(t.char_is_signed() && t.wchar_is_signed());
        assert!(t.boosts_large_arrays() && t.extends_narrow_args());
        assert!(!t.unnamed_bitfield_aligns());
        assert_eq!(t.function_attrs(), "\"frame-pointer\"=\"all\"");
        assert_eq!(t.macos_version(), Some((10, 15)));
        assert_eq!(Target::X86_64Linux.macos_version(), None);
    }

    #[test]
    fn windows_triples_and_type_model() {
        for triple in [
            "x86_64-pc-windows-msvc",
            "x86_64-windows-msvc",
            "x86_64-pc-windows",
            "x86_64-unknown-windows-msvc19.33.0",
        ] {
            assert_eq!(Target::parse(triple), Some(WINDOWS_MSVC), "{triple}");
        }
        for triple in [
            "x86_64-w64-windows-gnu",
            "x86_64-pc-windows-gnu",
            "x86_64-w64-mingw32",
        ] {
            assert_eq!(Target::parse(triple), Some(WINDOWS_GNU), "{triple}");
        }
        for triple in [
            "aarch64-pc-windows-msvc",
            "i686-w64-mingw32",
            "x86_64-w64-cygwin",
        ] {
            assert_eq!(Target::parse(triple), None, "{triple}");
        }
        assert_eq!(WINDOWS_MSVC.triple(), "x86_64-pc-windows-msvc");
        assert_eq!(WINDOWS_GNU.triple(), "x86_64-w64-windows-gnu");
        for t in [WINDOWS_MSVC, WINDOWS_GNU] {
            assert!(t.is_windows() && !t.is_apple() && !t.is_aarch64());
            assert!(t.data_layout().starts_with("e-m:w-"));
            assert_eq!(t.abi(), Abi::Win64);
            assert_eq!(t.va_list_kind(), VaListKind::CharPtr);
            assert!(!t.long_is_64() && t.ms_layout());
            assert_eq!(t.wchar_bits(), 16);
            assert!(t.char_is_signed() && !t.wchar_is_signed());
            assert!(t.boosts_large_arrays() && !t.extends_narrow_args());
        }
        assert!(WINDOWS_MSVC.is_msvc() && !WINDOWS_GNU.is_msvc());
        assert_eq!(WINDOWS_MSVC.long_double(), LongDouble::Double);
        assert_eq!(WINDOWS_GNU.long_double(), LongDouble::X87);
        for t in [Target::X86_64Linux, APPLE_DEFAULT, APPLE_X86_64_DEFAULT] {
            assert!(t.long_is_64() && !t.ms_layout() && !t.is_windows());
            assert_eq!(t.wchar_bits(), 32);
        }
    }

    #[test]
    fn triples_and_layouts() {
        assert_eq!(APPLE_DEFAULT.triple(), "arm64-apple-macosx11.0.0");
        assert!(APPLE_DEFAULT.data_layout().contains("m:o"));
        assert_eq!(Target::X86_64Linux.triple(), "x86_64-pc-linux-gnu");
        assert!(Target::Aarch64Linux
            .data_layout()
            .starts_with("e-m:e-i8:8:32"));
        assert!(APPLE_DEFAULT.char_is_signed());
        assert!(!Target::Aarch64Linux.char_is_signed());
        assert_eq!(APPLE_DEFAULT.long_double(), LongDouble::Double);
    }

    #[test]
    fn selection_is_per_thread() {
        set(APPLE_DEFAULT);
        assert!(current().is_apple());
        let other = std::thread::spawn(current).join().unwrap();
        assert_eq!(other, Target::build_default());
        set(Target::build_default());
    }
}
