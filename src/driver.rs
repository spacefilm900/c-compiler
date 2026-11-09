//! Command-line driver. The harness contract is `compiler INPUT.c -o OUTPUT.ll`.

use std::fs;
use std::io::{self, Write};

use crate::compile_to_ir;

const USAGE: &str = "\
usage: compiler INPUT.c -o OUTPUT.ll
Compile a C subset to textual LLVM IR.";

pub fn run(args: &[String]) -> i32 {
    if args.len() == 2 && (args[1] == "-h" || args[1] == "--help") {
        println!("{USAGE}");
        return 0;
    }
    if args.len() != 4 || args[2] != "-o" {
        let _ = writeln!(io::stderr(), "{USAGE}");
        return 2;
    }
    let input = &args[1];
    let output = &args[3];
    let source = match fs::read(input) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => {
                let _ = writeln!(io::stderr(), "{input}: error: source is not valid UTF-8");
                return 1;
            }
        },
        Err(err) => {
            let _ = writeln!(io::stderr(), "{input}: error: {err}");
            return 1;
        }
    };
    match compile_to_ir(&source, input) {
        Ok(ir) => match fs::write(output, ir) {
            Ok(()) => 0,
            Err(err) => {
                let _ = writeln!(io::stderr(), "{output}: error: {err}");
                1
            }
        },
        Err(diag) => {
            let _ = writeln!(io::stderr(), "{diag}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_paths() -> (std::path::PathBuf, std::path::PathBuf) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        (
            dir.join(format!("cc-drv-{pid}-{id}.c")),
            dir.join(format!("cc-drv-{pid}-{id}.ll")),
        )
    }

    #[test]
    fn cli_usage_success_and_error() {
        assert_eq!(run(&["compiler".into()]), 2);
        assert_eq!(run(&["compiler".into(), "--help".into()]), 0);

        let (src, out) = temp_paths();
        std::fs::write(&src, "int main(void) { return 42; }\n").unwrap();
        assert_eq!(
            run(&[
                "compiler".into(),
                src.display().to_string(),
                "-o".into(),
                out.display().to_string(),
            ]),
            0
        );
        assert!(std::fs::read_to_string(&out)
            .unwrap()
            .contains("ret i32 42"));

        let bad = src.with_extension("bad.c");
        let bad_out = out.with_extension("bad.ll");
        std::fs::write(&bad, "int main(void) { return; }\n").unwrap();
        let _ = std::fs::remove_file(&bad_out);
        assert_eq!(
            run(&[
                "compiler".into(),
                bad.display().to_string(),
                "-o".into(),
                bad_out.display().to_string(),
            ]),
            1
        );
        assert!(!bad_out.exists());
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&bad);
    }
}
