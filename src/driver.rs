//! Command-line driver.
//!
//! `compiler INPUT.c -o OUTPUT.ll` writes textual LLVM IR.
//! `-E` writes the preprocessed text instead. `--dump-tokens`, `--dump-ast`,
//! `--dump-sema` and `--dump-ir` stop after that stage and write its dump
//! (to `-o FILE`, or standard output); see `docs/DEBUGGING.md`. `-I`, `-D`, and `-U` are
//! passed to the preprocessor. A bare `-D NAME` defines `NAME` as `1`.
//! `-std=c11` (the default) is strict ISO C11; `-std=gnu11` enables the GNU
//! extensions listed in [`crate::dialect`]. The last `-std` wins.
//! `-target TRIPLE` (or `--target=TRIPLE`) selects the machine; the host is
//! the default. `-isysroot DIR` names the C library headers' root (the
//! macOS SDK when compiling for `arm64-apple-macos` or `x86_64-apple-macos`).

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::compile_and_exit;
use crate::dialect::Dialect;
use crate::pp::{preprocess, CliMacro, PpOptions};

const USAGE: &str = "\
usage: compiler [options] INPUT.c -o OUTPUT.ll
       compiler -E [options] INPUT.c [-o OUTPUT.i]
       compiler --dump-STAGE [options] INPUT.c [-o OUTPUT]
options:
  -I DIR          add DIR to the include search path
  -D NAME[=text]  define NAME as text, or as 1 if text is omitted
  -U NAME         undefine NAME
  -E              preprocess only (also --dump-pp)
  --dump-tokens   stop after preprocessing and print the token stream
  --dump-ast      stop after parsing and print the syntax tree
  --dump-sema     stop after semantic analysis and print the checked tree
                  (types, conversions, constants, record layouts)
  --dump-ir       print the LLVM IR (to standard output unless -o is given)
  -std=c11        strict ISO C11 (the default)
  -std=gnu11      C11 plus the GNU extensions this compiler supports
  -fgnu89-inline  GNU89 inline semantics: plain `inline` definitions are
                  external, `extern inline` ones are only for inlining
  -target TRIPLE  compile for TRIPLE (also --target=TRIPLE): x86_64-linux-gnu,
                  x86_64-apple-macos[VERSION], arm64-apple-macos[VERSION],
                  or aarch64-linux-gnu;
                  the default is the host
  -isysroot DIR   find the C library headers under DIR/usr/include
  --time-phases   print frontend phase times as JSON on stderr
  -h, --help      show this help
Compile a C subset to textual LLVM IR.";

struct Options {
    input: String,
    output: Option<String>,
    preprocess_only: bool,
    /// `--dump-tokens`, `--dump-ast`, `--dump-sema` or `--dump-ir`.
    dump: Option<crate::Stage>,
    time_phases: bool,
    pp: PpOptions,
    dialect: Dialect,
}

pub fn run(args: &[String]) -> i32 {
    if args.len() == 2 && (args[1] == "-h" || args[1] == "--help") {
        println!("{USAGE}");
        return 0;
    }
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(message) => {
            let _ = writeln!(io::stderr(), "{message}");
            return 2;
        }
    };
    let source = match fs::read(&options.input) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => {
                let _ = writeln!(
                    io::stderr(),
                    "{}: error: source is not valid UTF-8",
                    options.input
                );
                return 1;
            }
        },
        Err(err) => {
            let _ = writeln!(io::stderr(), "{}: error: {err}", options.input);
            return 1;
        }
    };
    if options.preprocess_only {
        return emit_preprocessed(&options, &source);
    }
    if let Some(stage) = options.dump {
        return emit_dump(&options, &source, stage);
    }
    let Some(output) = options.output.as_deref() else {
        let _ = writeln!(io::stderr(), "{USAGE}");
        return 2;
    };
    match compile_and_exit(&source, &options.input, &options.pp, options.dialect) {
        Ok(mut compiled) => {
            for warning in &compiled.warnings {
                let _ = writeln!(io::stderr(), "{warning}");
            }
            let start = std::time::Instant::now();
            let written = fs::write(output, &compiled.ir);
            compiled.times.write = start.elapsed();
            if options.time_phases {
                let _ = writeln!(io::stderr(), "time-phases: {}", compiled.times.to_json());
            }
            match written {
                Ok(()) => 0,
                Err(err) => {
                    let _ = writeln!(io::stderr(), "{output}: error: {err}");
                    1
                }
            }
        }
        Err(diag) => {
            let _ = writeln!(io::stderr(), "{diag}");
            error_status(&diag)
        }
    }
}

/// Exit status for a diagnostic that stopped compilation.
fn error_status(diag: &crate::Diagnostic) -> i32 {
    if diag.is_internal() {
        crate::diag::EXIT_INTERNAL_ERROR
    } else {
        1
    }
}

fn emit_preprocessed(options: &Options, source: &str) -> i32 {
    match preprocess(source, &options.input, &options.pp) {
        Ok(text) => write_text(options, &text),
        Err(diag) => {
            let _ = writeln!(io::stderr(), "{diag}");
            1
        }
    }
}

/// `--dump-STAGE`: run the frontend up to the stage and write its dump.
/// Warnings and errors are reported as in a normal compilation.
fn emit_dump(options: &Options, source: &str, stage: crate::Stage) -> i32 {
    match crate::dump_stage(source, &options.input, &options.pp, options.dialect, stage) {
        Ok(dumped) => {
            for warning in &dumped.warnings {
                let _ = writeln!(io::stderr(), "{warning}");
            }
            write_text(options, &dumped.ir)
        }
        Err(diag) => {
            let _ = writeln!(io::stderr(), "{diag}");
            error_status(&diag)
        }
    }
}

/// Write `text` to `-o FILE`, or to standard output.
fn write_text(options: &Options, text: &str) -> i32 {
    if let Some(output) = &options.output {
        match fs::write(output, text) {
            Ok(()) => 0,
            Err(err) => {
                let _ = writeln!(io::stderr(), "{output}: error: {err}");
                1
            }
        }
    } else {
        let mut stdout = io::stdout().lock();
        match stdout
            .write_all(text.as_bytes())
            .and_then(|()| stdout.flush())
        {
            Ok(()) => 0,
            Err(err) => {
                let _ = writeln!(io::stderr(), "<stdout>: error: {err}");
                1
            }
        }
    }
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut input = None;
    let mut output = None;
    let mut preprocess_only = false;
    let mut dump = None;
    let mut time_phases = false;
    let mut pp = PpOptions::default();
    let mut dialect = Dialect::default();
    let mut target = crate::target::Target::build_default();
    let mut sysroot: Option<PathBuf> = None;
    let mut index = 1;
    while index < args.len() {
        let arg = &args[index];
        if output_kind(arg, &mut preprocess_only, &mut dump)? {
            index += 1;
            continue;
        }
        if arg == "--time-phases" {
            time_phases = true;
            crate::timing::enable();
            index += 1;
            continue;
        }
        if arg == "-o" {
            index += 1;
            let Some(path) = args.get(index) else {
                return Err(USAGE.to_string());
            };
            output = Some(path.clone());
            index += 1;
            continue;
        }
        if let Some(name) = arg.strip_prefix("-std=") {
            dialect = Dialect::from_std(name)
                .ok_or_else(|| format!("unsupported -std value '{name}'; use c11 or gnu11"))?;
            index += 1;
            continue;
        }
        if arg == "-fgnu89-inline" || arg == "-fno-gnu89-inline" {
            pp.gnu89_inline = arg == "-fgnu89-inline";
            index += 1;
            continue;
        }
        if arg == "-target" || arg == "--target" {
            index += 1;
            let Some(triple) = args.get(index) else {
                return Err(USAGE.to_string());
            };
            target = parse_target(triple)?;
            index += 1;
            continue;
        }
        if let Some(triple) = arg
            .strip_prefix("--target=")
            .or_else(|| arg.strip_prefix("-target="))
        {
            target = parse_target(triple)?;
            index += 1;
            continue;
        }
        if arg == "-isysroot" {
            index += 1;
            let Some(dir) = args.get(index) else {
                return Err(USAGE.to_string());
            };
            sysroot = Some(PathBuf::from(dir));
            index += 1;
            continue;
        }
        if let Some(dir) = take_attached(arg, "-I", args, &mut index)? {
            pp.include_dirs.push(PathBuf::from(dir));
            continue;
        }
        if let Some(text) = take_attached(arg, "-D", args, &mut index)? {
            pp.definitions.push(parse_define(&text)?);
            continue;
        }
        if let Some(name) = take_attached(arg, "-U", args, &mut index)? {
            if !is_macro_name(&name) {
                return Err(format!("invalid macro name '{name}'"));
            }
            pp.definitions.push(CliMacro::Undef(name));
            continue;
        }
        if arg.starts_with('-') {
            return Err(USAGE.to_string());
        }
        if input.is_some() {
            return Err(USAGE.to_string());
        }
        input = Some(arg.clone());
        index += 1;
    }
    let Some(input) = input else {
        return Err(USAGE.to_string());
    };
    if preprocess_only && dump.is_some() {
        return Err("-E and --dump-STAGE cannot be combined".to_string());
    }
    if !preprocess_only && dump.is_none() && output.is_none() {
        return Err(USAGE.to_string());
    }
    pp.target = target;
    pp.system_dirs = crate::pp::system_dirs_for(target, sysroot.as_deref());
    Ok(Options {
        input,
        output,
        preprocess_only,
        dump,
        time_phases,
        pp,
        dialect,
    })
}

/// `-E`, `--dump-pp` and `--dump-STAGE`: what to write instead of IR.
/// `Ok(false)` when `arg` is none of them.
fn output_kind(
    arg: &str,
    preprocess_only: &mut bool,
    dump: &mut Option<crate::Stage>,
) -> Result<bool, String> {
    if arg == "-E" || arg == "--dump-pp" {
        *preprocess_only = true;
        return Ok(true);
    }
    let Some(stage) = dump_flag(arg) else {
        return Ok(false);
    };
    if dump.is_some_and(|seen| seen != stage) {
        return Err("only one --dump-STAGE option may be given".to_string());
    }
    *dump = Some(stage);
    Ok(true)
}

fn dump_flag(arg: &str) -> Option<crate::Stage> {
    Some(match arg {
        "--dump-tokens" => crate::Stage::Tokens,
        "--dump-ast" => crate::Stage::Ast,
        "--dump-sema" => crate::Stage::Sema,
        "--dump-ir" => crate::Stage::Ir,
        _ => return None,
    })
}

fn parse_target(triple: &str) -> Result<crate::target::Target, String> {
    crate::target::Target::parse(triple).ok_or_else(|| {
        format!(
            "unsupported target '{triple}'; use x86_64-linux-gnu, x86_64-apple-macos, arm64-apple-macos, or aarch64-linux-gnu"
        )
    })
}

/// `-I dir` and `-Idir` are both accepted. The returned string is the payload
/// and `index` points at the next argument.
fn take_attached(
    arg: &str,
    flag: &str,
    args: &[String],
    index: &mut usize,
) -> Result<Option<String>, String> {
    if arg == flag {
        *index += 1;
        let Some(value) = args.get(*index) else {
            return Err(USAGE.to_string());
        };
        *index += 1;
        return Ok(Some(value.clone()));
    }
    if let Some(rest) = arg.strip_prefix(flag) {
        if !rest.is_empty() {
            *index += 1;
            return Ok(Some(rest.to_string()));
        }
    }
    Ok(None)
}

fn parse_define(text: &str) -> Result<CliMacro, String> {
    let name_len = text
        .bytes()
        .position(|byte| !(byte.is_ascii_alphanumeric() || byte == b'_'))
        .unwrap_or(text.len());
    let name = &text[..name_len];
    if !is_macro_name(name) {
        return Err(format!("invalid macro name '{text}'"));
    }
    let replacement = if name_len == text.len() {
        None
    } else if text.as_bytes().get(name_len) == Some(&b'=') {
        Some(text[name_len + 1..].to_string())
    } else {
        return Err(format!("invalid macro name '{text}'"));
    };
    Ok(CliMacro::Define {
        name: name.to_string(),
        replacement,
    })
}

fn is_macro_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    match bytes.next() {
        Some(byte) if byte.is_ascii_alphabetic() || byte == b'_' => {}
        _ => return false,
    }
    bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') && name != "defined"
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

    #[test]
    fn dump_options_parse_and_conflict() {
        let args = |list: &[&str]| -> Vec<String> {
            std::iter::once("compiler")
                .chain(list.iter().copied())
                .map(String::from)
                .collect()
        };
        for (flag, stage) in [
            ("--dump-tokens", crate::Stage::Tokens),
            ("--dump-ast", crate::Stage::Ast),
            ("--dump-sema", crate::Stage::Sema),
            ("--dump-ir", crate::Stage::Ir),
        ] {
            // A dump needs no -o: it goes to standard output by default.
            let options = parse_args(&args(&[flag, "a.c"])).expect("parsed");
            assert_eq!(options.dump, Some(stage));
            assert!(options.output.is_none() && !options.preprocess_only);
            // The same flag twice is fine.
            assert!(parse_args(&args(&[flag, flag, "a.c"])).is_ok());
        }
        assert!(
            parse_args(&args(&["--dump-pp", "a.c"]))
                .expect("parsed")
                .preprocess_only
        );
        let err = parse_args(&args(&["--dump-ast", "--dump-sema", "a.c"]))
            .err()
            .expect("two stages");
        assert!(err.contains("only one"), "{err}");
        let err = parse_args(&args(&["-E", "--dump-ir", "a.c"]))
            .err()
            .expect("-E and a dump");
        assert!(err.contains("cannot be combined"), "{err}");
        assert!(parse_args(&args(&["--dump-everything", "a.c"])).is_err());
    }

    #[test]
    fn dumps_stop_after_their_stage() {
        let (src, out) = temp_paths();
        // Parses, but semantic analysis rejects the undeclared name.
        std::fs::write(&src, "int main(void) { return missing; }\n").unwrap();
        let path = src.display().to_string();
        let dumped = out.with_extension("dump");
        let dump = |flag: &str| {
            let _ = std::fs::remove_file(&dumped);
            let status = run(&[
                "compiler".into(),
                flag.into(),
                path.clone(),
                "-o".into(),
                dumped.display().to_string(),
            ]);
            (status, std::fs::read_to_string(&dumped).unwrap_or_default())
        };
        let (status, tokens) = dump("--dump-tokens");
        assert_eq!(status, 0);
        assert!(tokens.contains(":1:25 Ident 'missing'"), "{tokens}");
        let (status, ast) = dump("--dump-ast");
        assert_eq!(status, 0);
        assert!(ast.contains("var missing @1:25"), "{ast}");
        assert_eq!(dump("--dump-sema").0, 1);
        assert_eq!(dump("--dump-ir").0, 1);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&dumped);
    }

    #[test]
    fn internal_errors_exit_with_their_own_status() {
        let ice = crate::Diagnostic::internal("a.c", 1, 1, "irgen", "lost");
        assert_eq!(error_status(&ice), crate::diag::EXIT_INTERNAL_ERROR);
        assert_eq!(crate::diag::EXIT_INTERNAL_ERROR, 3);
        assert_eq!(error_status(&crate::Diagnostic::new("a.c", 1, 1, "bad")), 1);
    }

    #[test]
    fn target_option_spellings() {
        let (src, out) = temp_paths();
        std::fs::write(&src, "int main(void) { return 0; }\n").unwrap();
        let path = src.display().to_string();
        let ll = out.display().to_string();
        for (flags, triple) in [
            (
                vec!["-target", "arm64-apple-macos"],
                "arm64-apple-macosx11.0.0",
            ),
            (
                vec!["--target", "arm64-apple-macos14"],
                "arm64-apple-macosx14.0.0",
            ),
            (
                vec!["--target=aarch64-linux-gnu"],
                "aarch64-unknown-linux-gnu",
            ),
            (vec!["-target", "x86_64-linux-gnu"], "x86_64-pc-linux-gnu"),
            (
                vec!["-target", "x86_64-apple-macos"],
                "x86_64-apple-macosx10.15.0",
            ),
            (
                vec!["--target=x86_64-apple-macos13"],
                "x86_64-apple-macosx13.0.0",
            ),
        ] {
            let mut args = vec!["compiler".to_string()];
            args.extend(flags.iter().map(|flag| flag.to_string()));
            args.extend([path.clone(), "-o".to_string(), ll.clone()]);
            assert_eq!(run(&args), 0, "{flags:?}");
            let text = std::fs::read_to_string(&out).unwrap();
            assert!(
                text.contains(&format!("target triple = \"{triple}\"")),
                "{text}"
            );
        }
        let args: Vec<String> = ["compiler", "-target", "mips-linux-gnu", &path, "-o", &ll]
            .iter()
            .map(|arg| arg.to_string())
            .collect();
        assert_eq!(run(&args), 2);
        assert!(parse_target("arm64-apple-macos10.15").is_err());
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn dash_e_define_and_include() {
        let (src, out) = temp_paths();
        std::fs::write(&src, "int x = NAME;\n").unwrap();
        let pre = out.with_extension("i");
        assert_eq!(
            run(&[
                "compiler".into(),
                "-DNAME=7".into(),
                "-E".into(),
                src.display().to_string(),
                "-o".into(),
                pre.display().to_string(),
            ]),
            0
        );
        let text = std::fs::read_to_string(&pre).unwrap();
        assert!(text.contains("int x = 7;"), "{text}");

        let header_dir = out.parent().unwrap().join(format!(
            "cc-inc-{}",
            src.file_name().unwrap().to_string_lossy()
        ));
        let _ = std::fs::remove_dir_all(&header_dir);
        std::fs::create_dir_all(&header_dir).unwrap();
        std::fs::write(header_dir.join("one.h"), "int from_i = 4;\n").unwrap();
        std::fs::write(&src, "#include <one.h>\n").unwrap();
        let included = pre.with_extension("inc.i");
        assert_eq!(
            run(&[
                "compiler".into(),
                format!("-I{}", header_dir.display()),
                "-E".into(),
                src.display().to_string(),
                "-o".into(),
                included.display().to_string(),
            ]),
            0
        );
        let included_text = std::fs::read_to_string(&included).unwrap();
        assert!(included_text.contains("int from_i = 4;"), "{included_text}");
        let _ = std::fs::remove_file(&included);
        let _ = std::fs::remove_dir_all(&header_dir);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&pre);
    }

    #[test]
    fn apple_limits_h_reads_the_sdk_limits_first() {
        // The macOS SDK's <machine/limits.h> spells CHAR_MAX as 127. Before
        // the compiler's <limits.h> pulled the SDK's in first, a program that
        // reached both saw two different CHAR_MAX definitions and failed
        // (SQLite on Intel macOS).
        let (src, out) = temp_paths();
        let sdk = out.with_extension("sdk");
        let _ = std::fs::remove_dir_all(&sdk);
        let machine = sdk.join("usr/include/machine");
        std::fs::create_dir_all(&machine).unwrap();
        std::fs::write(
            sdk.join("usr/include/limits.h"),
            "#include <machine/limits.h>\n#define PATH_FROM_SDK 1\n",
        )
        .unwrap();
        std::fs::write(
            machine.join("limits.h"),
            "#ifndef MACHINE_LIMITS\n#define MACHINE_LIMITS\n#define CHAR_MAX 127\n\
             #define MB_LEN_MAX 6\n#define INT_MAX 0x7fffffff\n#endif\n",
        )
        .unwrap();
        std::fs::write(
            &src,
            "#include <limits.h>\n#include <machine/limits.h>\n\
             int v[] = { CHAR_MAX, MB_LEN_MAX, INT_MAX, PATH_FROM_SDK };\n",
        )
        .unwrap();
        let pre = out.with_extension("lim.i");
        for target in ["x86_64-apple-macos", "arm64-apple-macos"] {
            assert_eq!(
                run(&[
                    "compiler".into(),
                    "-target".into(),
                    target.into(),
                    "-isysroot".into(),
                    sdk.display().to_string(),
                    "-E".into(),
                    src.display().to_string(),
                    "-o".into(),
                    pre.display().to_string(),
                ]),
                0,
                "{target}"
            );
            let text = std::fs::read_to_string(&pre).unwrap();
            assert!(
                text.contains("{ 127, 6, 2147483647, 1 }"),
                "{target}: {text}"
            );
        }
        let _ = std::fs::remove_dir_all(&sdk);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&pre);
    }

    #[test]
    fn std_flag_selects_dialect() {
        let (src, out) = temp_paths();
        std::fs::write(
            &src,
            "int f(void);\nint main(void) { void *v = 0; return v == f; }\n",
        )
        .unwrap();
        let compile = |flags: &[&str]| {
            let _ = std::fs::remove_file(&out);
            let mut args: Vec<String> = vec!["compiler".into()];
            args.extend(flags.iter().map(|flag| flag.to_string()));
            args.extend([
                src.display().to_string(),
                "-o".into(),
                out.display().to_string(),
            ]);
            run(&args)
        };
        // Strict C11 is the default and can be named explicitly.
        assert_eq!(compile(&[]), 1);
        assert_eq!(compile(&["-std=c11"]), 1);
        assert!(!out.exists());
        // GNU mode accepts the comparison (with a warning) and writes IR.
        assert_eq!(compile(&["-std=gnu11"]), 0);
        assert!(out.exists());
        // The last -std wins, as with other drivers.
        assert_eq!(compile(&["-std=gnu11", "-std=c11"]), 1);
        // Other standards are not supported.
        assert_eq!(compile(&["-std=c99"]), 2);
        assert_eq!(compile(&["-std="]), 2);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&out);
    }
}
