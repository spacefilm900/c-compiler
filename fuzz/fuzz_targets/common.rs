//! Shared by the fuzz targets.
//!
//! The oracle: no panic, no hang (libFuzzer's `-timeout`), no internal
//! compiler error, and, in the `frontend` target with `HCC_FUZZ_VERIFY=1`,
//! IR that LLVM's own verifier accepts whenever the compiler accepts the
//! input.

#![allow(dead_code)]

use hcc::{Diagnostic, PpOptions};
use std::path::PathBuf;

/// Preprocessor options for the fuzz targets. By default `#include <...>`
/// finds only this compiler's own headers (`include/`), so an input that
/// names `<stdio.h>` fails at once instead of spending most of a second
/// in the C library's headers; the real headers are covered by the test
/// suites. Set `HCC_FUZZ_HOST_HEADERS=1` to search the host's headers too.
pub fn options() -> PpOptions {
    let mut options = PpOptions::default();
    if std::env::var_os("HCC_FUZZ_HOST_HEADERS").is_some_and(|v| v == "1") {
        return options;
    }
    let ours = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../include"));
    let ours = ours.canonicalize().unwrap_or(ours);
    options.system_dirs = vec![ours];
    options
}

/// The input as source text, or None when it should be skipped: not UTF-8,
/// or it names a path outside the source (an `#include` of a device such
/// as `/dev/zero` would hang for a reason that is not a compiler bug).
pub fn source(data: &[u8]) -> Option<&str> {
    if data.len() > 64 * 1024 {
        return None;
    }
    let text = std::str::from_utf8(data).ok()?;
    if text.contains("/dev") || text.contains("/proc") || text.contains("/sys") {
        return None;
    }
    Some(text)
}

/// An internal compiler error is a bug, like a panic.
pub fn check(result: Result<(), Diagnostic>) {
    if let Err(diag) = result {
        if diag.is_internal() {
            panic!("{diag}");
        }
    }
}
