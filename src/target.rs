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
//! - `aarch64-linux-gnu`: AAPCS64 and its Linux (GNU) C type mapping:
//!   `char` and `wchar_t` are unsigned, `long double` is IEEE binary128,
//!   and `va_list` is the AAPCS64 register-save structure. It exists to run
//!   AArch64 code under user-mode emulation on a Linux host.
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
}

impl Default for Target {
    fn default() -> Self {
        Target::host()
    }
}

pub const APPLE_DEFAULT: Target = Target::Arm64Apple {
    macos_major: 11,
    macos_minor: 0,
};

impl Target {
    /// The machine this compiler binary runs on, when it is a supported
    /// target; otherwise `x86_64-linux-gnu`.
    pub fn host() -> Target {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            APPLE_DEFAULT
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            Target::Aarch64Linux
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
    pub fn parse(triple: &str) -> Option<Target> {
        let parts: Vec<&str> = triple.split('-').collect();
        let arch = *parts.first()?;
        let rest = &parts[1..];
        match arch {
            "x86_64" | "amd64" => {
                let rest: Vec<&str> = rest
                    .iter()
                    .copied()
                    .filter(|part| *part != "pc" && *part != "unknown")
                    .collect();
                (rest == ["linux", "gnu"] || rest == ["linux"]).then_some(Target::X86_64Linux)
            }
            "arm64" | "aarch64" => {
                if rest.first() == Some(&"apple") {
                    if rest.len() != 2 {
                        return None;
                    }
                    return parse_apple_os(rest[1]);
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
        }
    }

    pub fn is_apple(self) -> bool {
        matches!(self, Target::Arm64Apple { .. })
    }

    pub fn is_aarch64(self) -> bool {
        matches!(self, Target::Arm64Apple { .. } | Target::Aarch64Linux)
    }

    pub fn abi(self) -> Abi {
        match self {
            Target::X86_64Linux => Abi::SysV,
            Target::Arm64Apple { .. } => Abi::Aapcs64 { apple: true },
            Target::Aarch64Linux => Abi::Aapcs64 { apple: false },
        }
    }

    /// Plain `char` is signed on x86_64 and on Apple arm64, and unsigned
    /// in the AAPCS64 Linux mapping.
    pub fn char_is_signed(self) -> bool {
        !matches!(self, Target::Aarch64Linux)
    }

    /// `wchar_t` (and `wint_t`) is `int`, except on AArch64 Linux, where it
    /// is `unsigned int`.
    pub fn wchar_is_signed(self) -> bool {
        !matches!(self, Target::Aarch64Linux)
    }

    pub fn long_double(self) -> LongDouble {
        match self {
            Target::X86_64Linux => LongDouble::X87,
            Target::Arm64Apple { .. } => LongDouble::Double,
            Target::Aarch64Linux => LongDouble::Quad,
        }
    }

    /// The caller extends an integer argument or return value narrower
    /// than 32 bits (`signext` / `zeroext` in LLVM). System V AMD64 and
    /// Apple arm64 require it; AAPCS64 leaves the upper bits unspecified.
    pub fn extends_narrow_args(self) -> bool {
        !matches!(self, Target::Aarch64Linux)
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
    /// pointers are kept in non-leaf functions.
    pub fn function_attrs(self) -> &'static str {
        match self {
            Target::Arm64Apple { .. } => "\"frame-pointer\"=\"non-leaf\"",
            _ => "",
        }
    }
}

fn parse_apple_os(os: &str) -> Option<Target> {
    let version = if let Some(rest) = os.strip_prefix("macosx") {
        rest
    } else if let Some(rest) = os.strip_prefix("macos") {
        rest
    } else if let Some(rest) = os.strip_prefix("darwin") {
        // A Darwin kernel version is not a macOS version; Darwin 20 is
        // macOS 11, the first release for Apple silicon.
        let _ = rest;
        return Some(APPLE_DEFAULT);
    } else {
        return None;
    };
    if version.is_empty() {
        return Some(APPLE_DEFAULT);
    }
    let mut numbers = version.split('.');
    let major: u32 = numbers.next()?.parse().ok()?;
    let minor: u32 = match numbers.next() {
        Some(text) => text.parse().ok()?,
        None => 0,
    };
    if let Some(patch) = numbers.next() {
        let _: u32 = patch.parse().ok()?;
    }
    if numbers.next().is_some() || major < 11 {
        return None;
    }
    Some(Target::Arm64Apple {
        macos_major: major,
        macos_minor: minor,
    })
}

thread_local! {
    static CURRENT: Cell<Target> = Cell::new(Target::host());
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
        assert_eq!(Target::parse("arm64-apple-macos10.15"), None);
        assert_eq!(Target::parse("arm64-apple-ios"), None);
        assert_eq!(Target::parse("riscv64-linux-gnu"), None);
        assert_eq!(Target::parse("x86_64-apple-darwin"), None);
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
        assert_eq!(other, Target::host());
        set(Target::host());
    }
}
