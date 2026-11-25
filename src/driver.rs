//! Command-line driver.
//!
//! `hcc INPUT.c -o OUTPUT.ll` writes textual LLVM IR.
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

/// The release version, from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The short usage shown after a command-line error.
const SHORT_USAGE: &str = "\
usage: hcc [options] FILE.c -o FILE.ll
       hcc --link [options] FILE... -o PROGRAM
       hcc -E [options] FILE.c [-o FILE.i]
       hcc --dump-STAGE [options] FILE.c [-o FILE]";

fn help_text() -> String {
    format!(
        "\
hcc {VERSION}: the Haviland C Compiler, C11 to LLVM IR

{SHORT_USAGE}

hcc compiles one C translation unit to textual LLVM IR. LLVM 18 or newer
(clang, or llc and opt) turns the IR into machine code; --link does that
step for you.

Output:
  -o FILE         write the result to FILE
  --link          compile each .c input to IR, then link the IR and any
                  other inputs (.ll, .o, .a, ...) into an executable with
                  clang ($HCC_CLANG, else clang on PATH); when the program
                  uses <threads.h>, adds -pthread on POSIX targets and
                  compiles and links lib/threads.c where the C library has
                  no <threads.h> (macOS, Windows, older Linux)
  -E              preprocess only (also --dump-pp)
  --dump-tokens   stop after preprocessing and print the token stream
  --dump-ast      stop after parsing and print the syntax tree
  --dump-sema     stop after semantic analysis and print the checked tree
                  (types, conversions, constants, record layouts)
  --dump-ir       print the LLVM IR (to standard output unless -o is given)

Linking (with --link only; passed to clang):
  -O0, -O1, -O2, -O3, -Os, -Oz
                  optimisation level for the IR and the program
  -l NAME, -L DIR, -Wl,ARGS, -pthread, -static
                  libraries, library directories and linker options

Preprocessor:
  -I DIR          add DIR to the include search path
  -D NAME[=text]  define NAME as text, or as 1 if text is omitted
  -U NAME         undefine NAME

Language:
  -std=c11        strict ISO C11 (the default)
  -std=gnu11      C11 plus the GNU extensions this compiler supports
  -trigraphs      replace trigraphs under -std=gnu11 too (-std=c11 always
                  does)
  -fgnu89-inline  GNU89 inline semantics: plain `inline` definitions are
                  external, `extern inline` ones are only for inlining

Target:
  -target TRIPLE  compile for TRIPLE (also --target=TRIPLE): {targets};
                  the default is the host ({host})
  -isysroot DIR   find the C library headers under DIR/usr/include
                  (DIR/include for the Windows targets)

Diagnostics:
  -fno-caret-diagnostics
                  do not print the source line and a caret under messages

Other:
  --time-phases   print frontend phase times as JSON on stderr (not with
                  --link)
  --version       print the version, the default target and the targets
  -h, --help      show this help

Exit status: 0 success, 1 error in the program, 2 command-line error,
3 internal compiler error. The user guide is docs/USER_GUIDE.md.",
        targets = TARGET_LIST,
        host = crate::target::Target::host().name(),
    )
}

/// Every target `-target` accepts, by its canonical name.
const TARGET_LIST: &str = "x86_64-linux-gnu, aarch64-linux-gnu, arm64-apple-macos[VERSION], \
x86_64-apple-macos[VERSION], x86_64-w64-windows-gnu (MinGW-w64), x86_64-pc-windows-msvc";

fn version_text() -> String {
    let commit = option_env!("HCC_COMMIT")
        .filter(|commit| !commit.is_empty())
        .map(|commit| format!(" (commit {commit})"))
        .unwrap_or_default();
    format!(
        "hcc {VERSION}{commit}\n\
         Haviland C Compiler: C11 (and -std=gnu11) to textual LLVM IR\n\
         default target: {}\n\
         targets: {TARGET_LIST}\n\
         backend: LLVM 18 or newer (opaque pointers); --link uses clang\n\
         license: MIT",
        crate::target::Target::host().name()
    )
}

#[derive(Clone)]
pub(crate) struct Options {
    /// The source files (one, except with `--link`) and, with `--link`, the
    /// other files passed to clang, in command-line order.
    pub(crate) inputs: Vec<String>,
    pub(crate) output: Option<String>,
    preprocess_only: bool,
    /// `--dump-tokens`, `--dump-ast`, `--dump-sema` or `--dump-ir`.
    dump: Option<crate::Stage>,
    time_phases: bool,
    pub(crate) pp: PpOptions,
    pub(crate) dialect: Dialect,
    /// `--link`: compile, then link an executable with clang.
    pub(crate) link: bool,
    /// With `--link`: the inputs and the clang options (`-O`, `-l`, `-L`,
    /// `-Wl,` and the others) together, in
    /// command-line order, because a linker reads them in order
    /// (`-Wl,--whole-archive lib.a -Wl,--no-whole-archive`).
    pub(crate) link_line: Vec<LinkArg>,
    pub(crate) sysroot: Option<PathBuf>,
    /// Print the source line and a caret under each diagnostic.
    caret: bool,
}

/// One item of the `--link` command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LinkArg {
    /// An input file.
    File(String),
    /// A clang option, with its separate value if it has one (`-l m`).
    Option(Vec<String>),
}

impl Options {
    fn input(&self) -> &str {
        &self.inputs[0]
    }
}

/// Run the compiler with the command-line `args` (`args[0]` is the program
/// name). Returns the exit status.
pub fn run(args: &[String]) -> i32 {
    if args.len() == 2 && (args[1] == "-h" || args[1] == "--help") {
        println!("{}", help_text());
        return 0;
    }
    if args.len() == 2 && args[1] == "--version" {
        println!("{}", version_text());
        return 0;
    }
    if args.len() == 1 {
        let _ = writeln!(
            io::stderr(),
            "{SHORT_USAGE}\nRun 'hcc --help' for the options."
        );
        return 2;
    }
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(message) => {
            let _ = writeln!(
                io::stderr(),
                "hcc: error: {message}\nRun 'hcc --help' for the options."
            );
            return 2;
        }
    };
    if options.link {
        return crate::link::link(&options);
    }
    let source = match read_source(options.input()) {
        Ok(text) => text,
        Err(status) => return status,
    };
    if options.preprocess_only {
        return emit_preprocessed(&options, &source);
    }
    if let Some(stage) = options.dump {
        return emit_dump(&options, &source, stage);
    }
    let output = options.output.as_deref().expect("checked by parse_args");
    match compile_source(&options, options.input(), &source) {
        Ok(compiled) => {
            let mut compiled = compiled;
            let start = std::time::Instant::now();
            let status = write_to(Some(output), &compiled.ir);
            compiled.times.write = start.elapsed();
            if options.time_phases {
                let _ = writeln!(io::stderr(), "time-phases: {}", compiled.times.to_json());
            }
            status
        }
        Err(status) => status,
    }
}

/// Read a source file as UTF-8. On failure the message is printed and the
/// exit status returned.
pub(crate) fn read_source(path: &str) -> Result<String, i32> {
    match fs::read(path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => Ok(text),
            Err(_) => {
                let _ = writeln!(io::stderr(), "{path}: error: source is not valid UTF-8");
                Err(1)
            }
        },
        Err(err) => {
            let _ = writeln!(io::stderr(), "{path}: error: {err}");
            Err(1)
        }
    }
}

/// Compile one translation unit and print its warnings, or print the
/// error and return the exit status.
pub(crate) fn compile_source(
    options: &Options,
    path: &str,
    source: &str,
) -> Result<crate::Output, i32> {
    match compile_and_exit(source, path, &options.pp, options.dialect) {
        Ok(compiled) => {
            for warning in &compiled.warnings {
                report(options, source, path, &warning.to_string(), &warning.0);
            }
            Ok(compiled)
        }
        Err(diag) => {
            report(options, source, path, &diag.to_string(), &diag);
            Err(error_status(&diag))
        }
    }
}

/// Print a diagnostic, then, unless `-fno-caret-diagnostics`, the source
/// line it points at and a caret under its column.
fn report(options: &Options, source: &str, path: &str, text: &str, diag: &crate::Diagnostic) {
    let mut stderr = io::stderr().lock();
    let _ = writeln!(stderr, "{text}");
    if !options.caret {
        return;
    }
    let file_text;
    let text_of_file = if diag.filename == path {
        Some(source)
    } else {
        file_text = fs::read_to_string(&diag.filename).ok();
        file_text.as_deref()
    };
    if let Some(snippet) = text_of_file.and_then(|text| caret_snippet(text, diag.line, diag.column))
    {
        let _ = writeln!(stderr, "{snippet}");
    }
}

/// The source line `line` (1-based) of `text` and, under it, a caret at
/// byte column `column` (1-based). A tab before the column stays a tab so
/// the caret lines up, and each other character is as many spaces as the
/// cells it takes on a terminal (two for East Asian wide characters, none
/// for combining marks). `None` when the position is not in the text.
pub(crate) fn caret_snippet(text: &str, line: usize, column: usize) -> Option<String> {
    if line == 0 || column == 0 {
        return None;
    }
    let source_line = text.split('\n').nth(line - 1)?.trim_end_matches('\r');
    let before = source_line.get(..column - 1)?;
    let mut pad = String::new();
    for ch in before.chars() {
        if ch == '\t' {
            pad.push('\t');
        } else {
            pad.extend(std::iter::repeat_n(' ', display_width(ch)));
        }
    }
    Some(format!("{source_line}\n{pad}^"))
}

/// Terminal cells for `ch`: 0 for combining marks and zero-width
/// characters, 2 for the East Asian Wide and Fullwidth blocks (Unicode
/// Standard Annex #11) and emoji, else 1.
fn display_width(ch: char) -> usize {
    let code = ch as u32;
    let zero = matches!(code,
        0x0300..=0x036F | 0x0483..=0x0489 | 0x0591..=0x05BD | 0x0610..=0x061A
        | 0x064B..=0x065F | 0x0E31 | 0x0E34..=0x0E3A | 0x1AB0..=0x1AFF
        | 0x1DC0..=0x1DFF | 0x200B..=0x200F | 0x20D0..=0x20FF | 0xFE00..=0xFE0F
        | 0xFE20..=0xFE2F);
    if zero {
        return 0;
    }
    let wide = matches!(code,
        0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF | 0xA000..=0xA4CF | 0xA960..=0xA97F | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F | 0x1F900..=0x1F9FF | 0x20000..=0x2FFFD
        | 0x30000..=0x3FFFD);
    if wide {
        2
    } else {
        1
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
    match preprocess(source, options.input(), &options.pp) {
        Ok(text) => write_text(options, &text),
        Err(diag) => {
            report(options, source, options.input(), &diag.to_string(), &diag);
            1
        }
    }
}

/// `--dump-STAGE`: run the frontend up to the stage and write its dump.
/// Warnings and errors are reported as in a normal compilation.
fn emit_dump(options: &Options, source: &str, stage: crate::Stage) -> i32 {
    match crate::dump_stage(source, options.input(), &options.pp, options.dialect, stage) {
        Ok(dumped) => {
            for warning in &dumped.warnings {
                report(
                    options,
                    source,
                    options.input(),
                    &warning.to_string(),
                    &warning.0,
                );
            }
            write_text(options, &dumped.ir)
        }
        Err(diag) => {
            report(options, source, options.input(), &diag.to_string(), &diag);
            error_status(&diag)
        }
    }
}

/// Write `text` to `-o FILE`, or to standard output.
fn write_text(options: &Options, text: &str) -> i32 {
    write_to(options.output.as_deref(), text)
}

/// Write `text` to `output`, or to standard output when there is no `-o`
/// or it is `-o -`, as with other compilers. Returns the exit status.
fn write_to(output: Option<&str>, text: &str) -> i32 {
    if let Some(output) = output.filter(|path| *path != "-") {
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

/// The value after an option such as `-o`, or an error naming the option.
fn value_of(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    let value = args
        .get(*index)
        .cloned()
        .ok_or_else(|| format!("missing argument to '{flag}'"))?;
    *index += 1;
    Ok(value)
}

/// An option passed to clang with `--link`, and whether it takes the next
/// argument as its value.
fn link_option(arg: &str) -> Option<bool> {
    if matches!(arg, "-l" | "-L") {
        return Some(true);
    }
    let attached = ["-l", "-L", "-Wl,"]
        .iter()
        .any(|prefix| arg.len() > prefix.len() && arg.starts_with(prefix));
    let level = matches!(arg, "-O" | "-O0" | "-O1" | "-O2" | "-O3" | "-Os" | "-Oz");
    (attached || level || matches!(arg, "-pthread" | "-static")).then_some(false)
}

pub(crate) fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut inputs = Vec::new();
    let mut output = None;
    let mut preprocess_only = false;
    let mut dump = None;
    let mut time_phases = false;
    let mut pp = PpOptions::default();
    let mut dialect = Dialect::default();
    let mut target = crate::target::Target::build_default();
    let mut sysroot: Option<PathBuf> = None;
    let mut link = false;
    let mut link_args = Vec::new();
    let mut link_line = Vec::new();
    let mut caret = true;
    let mut index = 1;
    while index < args.len() {
        let arg = &args[index];
        if output_kind(arg, &mut preprocess_only, &mut dump)? {
            index += 1;
            continue;
        }
        match arg.as_str() {
            "--time-phases" => {
                time_phases = true;
                crate::timing::enable();
            }
            "--link" => link = true,
            "-fcaret-diagnostics" => caret = true,
            "-fno-caret-diagnostics" => caret = false,
            "-fgnu89-inline" | "-fno-gnu89-inline" => pp.gnu89_inline = arg == "-fgnu89-inline",
            "-trigraphs" => pp.trigraphs = true,
            "-h" | "--help" | "--version" => {
                return Err(format!("'{arg}' must be the only argument"));
            }
            "-o" => {
                output = Some(value_of(args, &mut index, "-o")?);
                continue;
            }
            "-target" | "--target" => {
                target = parse_target(&value_of(args, &mut index, arg)?)?;
                continue;
            }
            "-isysroot" => {
                sysroot = Some(PathBuf::from(value_of(args, &mut index, "-isysroot")?));
                continue;
            }
            _ => {
                if let Some(name) = arg.strip_prefix("-std=") {
                    dialect = Dialect::from_std(name).ok_or_else(|| {
                        format!("unsupported -std value '{name}'; use c11 or gnu11")
                    })?;
                } else if let Some(triple) = arg
                    .strip_prefix("--target=")
                    .or_else(|| arg.strip_prefix("-target="))
                {
                    target = parse_target(triple)?;
                } else if let Some(dir) = take_attached(arg, "-I", args, &mut index)? {
                    pp.include_dirs.push(PathBuf::from(dir));
                    continue;
                } else if let Some(text) = take_attached(arg, "-D", args, &mut index)? {
                    pp.definitions.push(parse_define(&text)?);
                    continue;
                } else if let Some(name) = take_attached(arg, "-U", args, &mut index)? {
                    if !is_macro_name(&name) {
                        return Err(format!("invalid macro name '{name}'"));
                    }
                    pp.definitions.push(CliMacro::Undef(name));
                    continue;
                } else if let Some(takes_value) = link_option(arg) {
                    let mut option = vec![arg.clone()];
                    if takes_value {
                        option.push(value_of(args, &mut index, arg)?);
                    }
                    link_args.extend(option.iter().cloned());
                    link_line.push(LinkArg::Option(option));
                    if takes_value {
                        continue;
                    }
                } else if arg.starts_with('-') {
                    return Err(format!("unknown option '{arg}'"));
                } else {
                    inputs.push(arg.clone());
                    link_line.push(LinkArg::File(arg.clone()));
                }
            }
        }
        index += 1;
    }
    if inputs.is_empty() {
        return Err("no input file".to_string());
    }
    if preprocess_only && dump.is_some() {
        return Err("-E and --dump-STAGE cannot be combined".to_string());
    }
    if link {
        if preprocess_only || dump.is_some() {
            return Err("--link cannot be combined with -E or --dump-STAGE".to_string());
        }
        if output.is_none() {
            return Err("--link needs an output program name: -o PROGRAM".to_string());
        }
        if time_phases {
            return Err("--time-phases cannot be combined with --link".to_string());
        }
        if output.as_deref() == Some("-") {
            return Err(
                "--link cannot write a program to standard output; name it with -o PROGRAM"
                    .to_string(),
            );
        }
    } else {
        if let Some(flag) = link_args.first() {
            return Err(format!("'{flag}' is a link option; it needs --link"));
        }
        if inputs.len() > 1 {
            return Err(format!(
                "more than one input file ('{}' and '{}'); compile them one at a time, or use --link",
                inputs[0], inputs[1]
            ));
        }
        if !preprocess_only && dump.is_none() && output.is_none() {
            return Err(
                "no output file; use -o FILE.ll (or --dump-ir to print the IR)".to_string(),
            );
        }
    }
    pp.target = target;
    pp.gnu = dialect.gnu();
    pp.system_dirs = crate::pp::system_dirs_for(target, sysroot.as_deref());
    Ok(Options {
        inputs,
        output,
        preprocess_only,
        dump,
        time_phases,
        pp,
        dialect,
        link,
        link_line,
        sysroot,
        caret,
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
    crate::target::Target::parse(triple)
        .ok_or_else(|| format!("unsupported target '{triple}'; use one of: {TARGET_LIST}"))
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
            return Err(format!("missing argument to '{flag}'"));
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
    let mut name_len = name_len;
    if !is_macro_name(&text[..name_len]) {
        return Err(format!("invalid macro name '{text}'"));
    }
    // GCC's `-D 'NAME(args...)=definition'` form: a function-like macro.
    if text.as_bytes().get(name_len) == Some(&b'(') {
        let close = text[name_len..]
            .find(')')
            .ok_or_else(|| format!("invalid macro name '{text}'"))?;
        let params_ok = text[name_len + 1..name_len + close]
            .split(',')
            .map(str::trim)
            .all(|param| param.is_empty() || is_macro_name(param));
        if !params_ok {
            return Err(format!("invalid macro name '{text}'"));
        }
        name_len += close + 1;
    }
    let name = &text[..name_len];
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
        assert_eq!(run(&["hcc".into()]), 2);
        assert_eq!(run(&["hcc".into(), "--help".into()]), 0);

        let (src, out) = temp_paths();
        std::fs::write(&src, "int main(void) { return 42; }\n").unwrap();
        assert_eq!(
            run(&[
                "hcc".into(),
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
                "hcc".into(),
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
            std::iter::once("hcc")
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
                "hcc".into(),
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
            let mut args = vec!["hcc".to_string()];
            args.extend(flags.iter().map(|flag| flag.to_string()));
            args.extend([path.clone(), "-o".to_string(), ll.clone()]);
            assert_eq!(run(&args), 0, "{flags:?}");
            let text = std::fs::read_to_string(&out).unwrap();
            assert!(
                text.contains(&format!("target triple = \"{triple}\"")),
                "{text}"
            );
        }
        let args: Vec<String> = ["hcc", "-target", "mips-linux-gnu", &path, "-o", &ll]
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
                "hcc".into(),
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
                "hcc".into(),
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
                    "hcc".into(),
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
        // An SDK without <limits.h>: the compiler's own definitions stand
        // alone, with its MB_LEN_MAX.
        std::fs::remove_file(sdk.join("usr/include/limits.h")).unwrap();
        std::fs::write(
            &src,
            "#include <limits.h>\nint w[] = { CHAR_MAX, MB_LEN_MAX, INT_MAX };\n",
        )
        .unwrap();
        for target in ["x86_64-apple-macos", "arm64-apple-macos"] {
            assert_eq!(
                run(&[
                    "hcc".into(),
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
            assert!(text.contains("{ 127, 16, 2147483647 }"), "{target}: {text}");
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
            let mut args: Vec<String> = vec!["hcc".into()];
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

    fn argv(list: &[&str]) -> Vec<String> {
        std::iter::once("hcc")
            .chain(list.iter().copied())
            .map(String::from)
            .collect()
    }

    #[test]
    fn version_and_help() {
        assert_eq!(run(&argv(&["--version"])), 0);
        assert_eq!(run(&argv(&["-h"])), 0);
        assert!(version_text().starts_with(&format!("hcc {VERSION}")));
        assert!(version_text().contains(crate::target::Target::host().name()));
        let help = help_text();
        for flag in [
            "--link",
            "-fno-caret-diagnostics",
            "--version",
            "-std=gnu11",
            "-O2",
        ] {
            assert!(help.contains(flag), "{flag}");
        }
        // --version and --help stand alone.
        let err = parse_args(&argv(&["--version", "a.c"]))
            .err()
            .expect("rejected");
        assert!(err.contains("only argument"), "{err}");
    }

    #[test]
    fn command_line_errors_say_what_is_wrong() {
        let err = |list: &[&str]| parse_args(&argv(list)).err().expect("rejected");
        assert_eq!(err(&["-o", "a.ll"]), "no input file");
        assert_eq!(err(&["-frobnicate", "a.c"]), "unknown option '-frobnicate'");
        assert_eq!(err(&["a.c", "-o"]), "missing argument to '-o'");
        assert_eq!(err(&["a.c", "-I"]), "missing argument to '-I'");
        assert!(err(&["a.c"]).contains("no output file"));
        assert!(err(&["a.c", "b.c", "-o", "x.ll"]).contains("more than one input file"));
        assert!(err(&["-lm", "a.c", "-o", "a.ll"]).contains("needs --link"));
        assert!(err(&["-O2", "a.c", "-o", "a.ll"]).contains("needs --link"));
        assert!(err(&["--link", "a.c"]).contains("-o PROGRAM"));
        assert!(err(&["--link", "-E", "a.c", "-o", "a"]).contains("cannot be combined"));
        assert!(err(&["--link", "a.c", "-o", "-"]).contains("standard output"));
        assert!(err(&["--link", "--time-phases", "a.c", "-o", "a"])
            .contains("--time-phases cannot be combined with --link"));
        assert!(err(&["-target", "mips-linux-gnu", "a.c", "-o", "a.ll"])
            .contains("x86_64-pc-windows-msvc"));
        // A bare run prints the short usage.
        assert_eq!(run(&argv(&[])), 2);
    }

    #[test]
    fn link_mode_collects_inputs_and_clang_options() {
        let options = parse_args(&argv(&[
            "--link",
            "main.c",
            "util.ll",
            "-O2",
            "-l",
            "m",
            "-lz",
            "-L",
            "/opt/lib",
            "-Wl,--as-needed",
            "-pthread",
            "lib.a",
            "-o",
            "prog",
        ]))
        .expect("parsed");
        assert!(options.link);
        assert_eq!(options.inputs, ["main.c", "util.ll", "lib.a"]);
        let file = |name: &str| LinkArg::File(name.to_string());
        let option =
            |parts: &[&str]| LinkArg::Option(parts.iter().map(|p| p.to_string()).collect());
        assert_eq!(
            options.link_line,
            [
                file("main.c"),
                file("util.ll"),
                option(&["-O2"]),
                option(&["-l", "m"]),
                option(&["-lz"]),
                option(&["-L", "/opt/lib"]),
                option(&["-Wl,--as-needed"]),
                option(&["-pthread"]),
                file("lib.a"),
            ]
        );
        assert_eq!(options.output.as_deref(), Some("prog"));
    }

    #[test]
    fn caret_under_the_column() {
        let text = "int a;\n\tint b = c;\r\nint é = d;\n";
        assert_eq!(
            caret_snippet(text, 2, 10).as_deref(),
            Some("\tint b = c;\n\t        ^")
        );
        // Columns count bytes; the caret counts terminal cells.
        assert_eq!(
            caret_snippet(text, 3, 10).as_deref(),
            Some("int é = d;\n        ^")
        );
        let wide = "/* 界 */ x;\n";
        assert_eq!(
            caret_snippet(wide, 1, 11).as_deref(),
            Some("/* 界 */ x;\n         ^")
        );
        let combining = "/* e\u{301} */ y;\n";
        assert_eq!(
            caret_snippet(combining, 1, 11).as_deref(),
            Some("/* e\u{301} */ y;\n        ^")
        );
        assert_eq!(
            caret_snippet(text, 1, 7).as_deref(),
            Some("int a;\n      ^")
        );
        assert!(caret_snippet(text, 0, 1).is_none());
        assert!(caret_snippet(text, 9, 1).is_none());
        assert!(caret_snippet(text, 1, 40).is_none());
    }

    /// Whether `clang` can be the backend: it compiles a known-good module
    /// with opaque pointers (LLVM 15 and later). An older Clang, or none,
    /// skips the `--link` test rather than failing it.
    fn clang_reads_opaque_pointer_ir(clang: &str, dir: &std::path::Path) -> bool {
        let probe = dir.with_extension("probe.ll");
        let object = dir.with_extension("probe.o");
        std::fs::write(
            &probe,
            "define ptr @f(ptr %p) {\n  store ptr null, ptr %p\n  ret ptr %p\n}\n",
        )
        .unwrap();
        let ok = std::process::Command::new(clang)
            .args(["-w", "-c"])
            .arg(&probe)
            .arg("-o")
            .arg(&object)
            .output()
            .is_ok_and(|done| done.status.success());
        let _ = std::fs::remove_file(&probe);
        let _ = std::fs::remove_file(&object);
        ok
    }

    /// `--link` end to end, when Clang is installed: a program from a C
    /// file and an IR file, and a `<threads.h>` program.
    #[test]
    fn link_builds_and_runs_programs() {
        if crate::target::Target::host() != crate::target::Target::X86_64Linux {
            return;
        }
        let clang = std::env::var("HCC_CLANG").unwrap_or_else(|_| "clang".to_string());
        let (src, out) = temp_paths();
        if !clang_reads_opaque_pointer_ir(&clang, &out) {
            eprintln!("skipped: {clang} cannot compile opaque-pointer IR");
            return;
        }
        let helper = out.with_extension("helper.c");
        let helper_ll = out.with_extension("helper.ll");
        let program = out.with_extension("prog");
        std::fs::write(&helper, "int twice(int x) { return 2 * x; }\n").unwrap();
        assert_eq!(
            run(&argv(&[
                &helper.display().to_string(),
                "-o",
                &helper_ll.display().to_string()
            ])),
            0
        );
        std::fs::write(
            &src,
            "#include <threads.h>\nint twice(int);\nstatic int hits;\n\
             static int body(void *arg) { hits = twice(*(int *)arg); return 0; }\n\
             int main(void) { int v = 21; thrd_t t;\n\
             if (thrd_create(&t, body, &v) != thrd_success) return 1;\n\
             thrd_join(t, 0); return hits; }\n",
        )
        .unwrap();
        assert_eq!(
            run(&argv(&[
                "--link",
                &src.display().to_string(),
                &helper_ll.display().to_string(),
                "-O1",
                "-o",
                &program.display().to_string(),
            ])),
            0
        );
        let status = std::process::Command::new(&program).status().unwrap();
        assert_eq!(status.code(), Some(42));
        for path in [&src, &helper, &helper_ll, &program] {
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn trigraphs_flag_in_gnu_mode() {
        let (src, out) = temp_paths();
        // `??!` is `|` only when trigraphs are replaced; otherwise the
        // program is a syntax error.
        std::fs::write(&src, "int main(void) { return 0 ??! 0; }\n").unwrap();
        let compile = |flags: &[&str]| {
            let mut args: Vec<String> = vec!["hcc".into()];
            args.extend(flags.iter().map(|flag| flag.to_string()));
            args.extend([
                src.display().to_string(),
                "-o".into(),
                out.display().to_string(),
            ]);
            run(&args)
        };
        assert_eq!(compile(&[]), 0);
        assert_eq!(compile(&["-std=gnu11"]), 1);
        assert_eq!(compile(&["-std=gnu11", "-trigraphs"]), 0);
        assert_eq!(compile(&["-trigraphs", "-std=gnu11"]), 0);
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn dash_output_is_standard_output() {
        // `-o -` writes to standard output, as other compilers do, rather
        // than to a file named `-` in the working directory.
        let (src, _) = temp_paths();
        std::fs::write(&src, "int main(void) { return 0; }\n").unwrap();
        let path = src.display().to_string();
        let dash = std::path::Path::new("-");
        let existed = dash.exists();
        assert_eq!(run(&argv(&[&path, "-o", "-"])), 0);
        assert_eq!(run(&argv(&["-E", &path, "-o", "-"])), 0);
        assert_eq!(run(&argv(&["--dump-tokens", &path, "-o", "-"])), 0);
        if !existed {
            assert!(!dash.exists());
        }
        let _ = std::fs::remove_file(&src);
    }
}
