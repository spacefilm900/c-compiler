#![no_main]
#[path = "common.rs"]
mod common;

use hcc::Dialect;
use std::io::Write;
use std::process::{Command, Stdio};

/// Whether to run LLVM's verifier (`llvm-as`, or `$LLVM_AS`) on every
/// module the compiler accepts. It is a process per input, so it is slower;
/// set `HCC_FUZZ_VERIFY=1` to turn it on.
fn verify_enabled() -> bool {
    std::env::var_os("HCC_FUZZ_VERIFY").is_some_and(|v| v == "1")
}

fn verify(ir: &str, text: &str) {
    let tool = std::env::var("LLVM_AS").unwrap_or_else(|_| "llvm-as".to_string());
    let mut child = Command::new(&tool)
        .args(["-o", "/dev/null", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("cannot run {tool}: {e}"));
    // Write the module, then close stdin so llvm-as sees end of input. If
    // llvm-as stops reading early (it rejected the IR), the write fails with
    // a broken pipe; ignore that and report llvm-as's own diagnostic below.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(ir.as_bytes());
    }
    let out = child.wait_with_output().expect("wait for llvm-as");
    if !out.status.success() {
        panic!(
            "accepted input produced invalid IR:\n{}\n--- source ---\n{text}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Some(text) = common::source(data) {
        let options = common::options();
        for dialect in [Dialect::C11, Dialect::Gnu11] {
            match hcc::compile_dialect(text, "fuzz.c", &options, dialect) {
                Ok(output) => {
                    if verify_enabled() {
                        verify(&output.ir, text);
                    }
                }
                Err(diag) => common::check(Err(diag)),
            }
        }
    }
});
