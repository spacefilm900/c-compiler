//! `--link`: compile each C source to LLVM IR, then link the IR and the
//! other inputs into an executable with Clang.
//!
//! Clang is only the LLVM backend and the linker driver here: it reads the
//! `.ll` files this compiler wrote, and never sees the C sources. It is
//! `$HCC_CLANG` when set, else `clang` on `PATH`, and is called with the
//! target's LLVM triple.
//!
//! Inputs and clang options keep their command-line order, so options
//! such as `-Wl,--whole-archive` apply to the files after them.
//!
//! C11 threads (7.26) need a runtime on some targets. When the IR (hcc's,
//! or a `.ll` input) declares one of the `<threads.h>` functions, POSIX
//! targets link with `-pthread`, and when the target's
//! C library has no `<threads.h>` (macOS, Windows, Linux before glibc 2.28)
//! `lib/threads.c`, the compiler's own runtime, is compiled by this
//! compiler and linked too. That is the same choice the compiler's
//! `<threads.h>` makes when it decides whether to use the C library's.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::driver::{compile_source, read_source, LinkArg, Options};
use crate::target::Target;

/// Link `options.inputs` into `options.output`. Returns the exit status.
pub(crate) fn link(options: &Options) -> i32 {
    let output = options.output.as_deref().expect("checked by parse_args");
    let scratch = match Scratch::new() {
        Ok(scratch) => scratch,
        Err(err) => {
            let _ = writeln!(
                io::stderr(),
                "hcc: error: cannot create a temporary directory for the IR: {err}"
            );
            return 1;
        }
    };
    let mut clang_args: Vec<std::ffi::OsString> = Vec::new();
    let mut uses_threads = false;
    for (index, item) in options.link_line.iter().enumerate() {
        let input = match item {
            LinkArg::Option(option) => {
                clang_args.extend(option.iter().map(Into::into));
                continue;
            }
            LinkArg::File(input) => input,
        };
        if is_foreign_source(input) {
            let _ = writeln!(
                io::stderr(),
                "{input}: error: hcc --link takes C sources (.c, .i), LLVM IR, objects and \
                 libraries; this file would be compiled by Clang, not hcc"
            );
            return 1;
        }
        if !is_c_source(input) {
            // IR from hcc (or anywhere else) may use <threads.h> too. A file
            // that cannot be read is left for Clang to report.
            uses_threads |= ir_input_uses_threads(input);
            clang_args.push(literal_path(input).into());
            continue;
        }
        let source = match read_source(input) {
            Ok(text) => text,
            Err(status) => return status,
        };
        let compiled = match compile_source(options, input, &source) {
            Ok(compiled) => compiled,
            Err(status) => return status,
        };
        uses_threads |= declares_c11_threads(&compiled.ir);
        let ll = scratch.path(index, input);
        if let Err(err) = std::fs::write(&ll, &compiled.ir) {
            let _ = writeln!(io::stderr(), "{}: error: {err}", ll.display());
            return 1;
        }
        clang_args.push(ll.into());
    }
    let target = options.pp.target;
    let mut extra = Vec::new();
    if uses_threads {
        if needs_threads_runtime(target, &options.pp.system_dirs) {
            let Some(runtime) = threads_runtime() else {
                let _ = writeln!(
                    io::stderr(),
                    "hcc: error: the program uses <threads.h>, which needs lib/threads.c on {}, \
                     but it was not found next to hcc (expected ../lib/threads.c)",
                    target.name()
                );
                return 1;
            };
            let status = compile_runtime(options, &runtime, &scratch, &mut clang_args);
            if status != 0 {
                return status;
            }
        }
        if !target.is_windows() {
            extra.push("-pthread".to_string());
        }
    }
    let clang = std::env::var("HCC_CLANG").unwrap_or_else(|_| "clang".to_string());
    let mut command = Command::new(&clang);
    command.arg(format!("--target={}", target.triple()));
    // On macOS `-isysroot` is the SDK, which the link needs too. Elsewhere
    // it only names the C library headers; the libraries are Clang's
    // defaults, or `-L` options.
    if let (Some(root), true) = (&options.sysroot, target.is_apple()) {
        command.arg("-isysroot").arg(root);
    }
    // The IR is this compiler's output: Clang's warnings about it are not
    // the user's.
    command.arg("-w");
    command.args(&clang_args);
    command.args(&extra);
    command.arg("-o").arg(output);
    match command.status() {
        Ok(status) if status.success() => 0,
        Ok(status) => {
            let _ = writeln!(
                io::stderr(),
                "hcc: error: {clang} failed to link '{output}' ({status})"
            );
            1
        }
        Err(err) => {
            let _ = writeln!(
                io::stderr(),
                "hcc: error: cannot run '{clang}' to link: {err}; install Clang 18 or newer, \
                 or set HCC_CLANG to its path"
            );
            1
        }
    }
}

/// Compile `lib/threads.c` in strict C11 for the program's target and add
/// its IR to the link.
fn compile_runtime(
    options: &Options,
    runtime: &Path,
    scratch: &Scratch,
    clang_args: &mut Vec<std::ffi::OsString>,
) -> i32 {
    let path = runtime.display().to_string();
    let source = match read_source(&path) {
        Ok(text) => text,
        Err(status) => return status,
    };
    let mut pp = options.pp.clone();
    pp.definitions.clear();
    pp.include_dirs.clear();
    pp.gnu = false;
    let mut runtime_options = options.clone();
    runtime_options.pp = pp;
    runtime_options.dialect = crate::Dialect::C11;
    let compiled = match compile_source(&runtime_options, &path, &source) {
        Ok(compiled) => compiled,
        Err(status) => return status,
    };
    let ll = scratch.path(usize::MAX, "threads.c");
    if let Err(err) = std::fs::write(&ll, &compiled.ir) {
        let _ = writeln!(io::stderr(), "{}: error: {err}", ll.display());
        return 1;
    }
    clang_args.push(ll.into());
    0
}

/// A C source file, or preprocessed C written by `hcc -E` (`.i`):
/// compiled by this compiler. Line markers from other preprocessors
/// (`# 1 "main.c"`) are not directives hcc accepts. Anything else (`.ll`,
/// `.o`, `.a`, `.so`, `.obj`, `.lib`, ...) goes to Clang unchanged.
pub(crate) fn is_c_source(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|ext| ext == "c" || ext == "i")
}

/// A source file in a language other than C that Clang would compile
/// itself (C++, Objective-C, CUDA, OpenCL, headers, or preprocessed
/// C++/Objective-C), so hcc would not check it. Assembly (`.s`, `.S`) and
/// LLVM IR or bitcode are left to Clang, like objects and libraries.
pub(crate) fn is_foreign_source(path: &str) -> bool {
    const FOREIGN: [&str; 20] = [
        "C", "cc", "cp", "cpp", "cxx", "c++", "CPP", "CC", "CXX", "ii", "m", "mm", "M", "mi",
        "mii", "h", "hh", "hpp", "cu", "cl",
    ];
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| FOREIGN.contains(&ext))
}

/// `path` as Clang should see it: a relative name that starts with `@`
/// would be read as a response file, so it becomes `./@...`.
fn literal_path(path: &str) -> String {
    if path.starts_with('@') {
        format!("./{path}")
    } else {
        path.to_string()
    }
}

/// Whether `path` is a textual LLVM IR input (`.ll`) that declares a C11
/// `<threads.h>` function. Objects and archives are not read.
fn ir_input_uses_threads(path: &str) -> bool {
    Path::new(path).extension().is_some_and(|ext| ext == "ll")
        && std::fs::read_to_string(path).is_ok_and(|ir| declares_c11_threads(&ir))
}

/// The functions of C11 `<threads.h>` (7.26.2 to 7.26.6).
const C11_THREAD_FUNCTIONS: [&str; 25] = [
    "call_once",
    "cnd_broadcast",
    "cnd_destroy",
    "cnd_init",
    "cnd_signal",
    "cnd_timedwait",
    "cnd_wait",
    "mtx_destroy",
    "mtx_init",
    "mtx_lock",
    "mtx_timedlock",
    "mtx_trylock",
    "mtx_unlock",
    "thrd_create",
    "thrd_current",
    "thrd_detach",
    "thrd_equal",
    "thrd_exit",
    "thrd_join",
    "thrd_sleep",
    "thrd_yield",
    "tss_create",
    "tss_delete",
    "tss_get",
    "tss_set",
];

/// Whether the IR declares (uses without defining) a C11 `<threads.h>`
/// function.
pub(crate) fn declares_c11_threads(ir: &str) -> bool {
    ir.lines()
        .filter(|line| line.starts_with("declare "))
        .filter_map(|line| declared_name(&line[line.find('@')? + 1..]))
        .any(|name| C11_THREAD_FUNCTIONS.contains(&name.as_str()))
}

/// The global name at the start of `text` (just after `@`): either a plain
/// identifier ending at `(`, or a quoted one (`"thrd_yield"`) whose `\HH`
/// escapes are decoded, as in the LLVM LangRef's identifier syntax.
fn declared_name(text: &str) -> Option<String> {
    let Some(quoted) = text.strip_prefix('"') else {
        return Some(text[..text.find('(')?].to_string());
    };
    let body = &quoted[..quoted.find('"')?];
    let bytes = body.as_bytes();
    let mut name = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            let hex = body.get(index + 1..index + 3)?;
            name.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            name.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(name).ok()
}

/// Whether the target needs `lib/threads.c`: its C library has no
/// `<threads.h>` in the system include directories.
pub(crate) fn needs_threads_runtime(target: Target, system_dirs: &[PathBuf]) -> bool {
    if target.is_apple() || target.is_windows() {
        return true;
    }
    let compiler_include = crate::pp::compiler_include_dirs();
    !system_dirs
        .iter()
        .filter(|dir| !compiler_include.contains(dir))
        .any(|dir| dir.join("threads.h").is_file())
}

/// `lib/threads.c` next to the compiler: `../lib` from the executable (an
/// installed release or `build/hcc`), or the source tree it was built from.
fn threads_runtime() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(Path::to_path_buf);
        for _ in 0..4 {
            let Some(current) = dir else { break };
            candidates.push(current.join("lib").join("threads.c"));
            dir = current.parent().map(Path::to_path_buf);
        }
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lib")
            .join("threads.c"),
    );
    candidates.into_iter().find(|path| path.is_file())
}

/// Temporary `.ll` files in a directory this run created, removed when
/// dropped.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    /// A new directory under the temporary directory. `create_dir` fails if
    /// the path exists, so the directory is always this run's own; on Unix
    /// only the user can read it.
    fn new() -> io::Result<Self> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos());
        let mut last_err = None;
        for _ in 0..100 {
            let dir = std::env::temp_dir().join(format!(
                "hcc-link-{}-{nanos:x}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            #[cfg(unix)]
            let builder = {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = std::fs::DirBuilder::new();
                builder.mode(0o700);
                builder
            };
            #[cfg(not(unix))]
            let builder = std::fs::DirBuilder::new();
            match builder.create(&dir) {
                Ok(()) => return Ok(Self { dir }),
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => last_err = Some(err),
                Err(err) => return Err(err),
            }
        }
        Err(last_err.unwrap_or_else(|| io::Error::other("no free name")))
    }

    fn path(&self, index: usize, input: &str) -> PathBuf {
        let stem = Path::new(input)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "input".to_string());
        self.dir.join(format!("{index}-{stem}.ll"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_sources_and_other_inputs() {
        assert!(is_c_source("main.c"));
        assert!(is_c_source("dir/pre.i"));
        for other in ["lib.o", "x.ll", "libz.a", "a.obj", "noext", "c"] {
            assert!(!is_c_source(other), "{other}");
        }
    }

    #[test]
    fn thread_functions_in_ir_declarations() {
        assert!(declares_c11_threads(
            "define i32 @main() {\n}\ndeclare i32 @thrd_create(ptr, ptr, ptr)\n"
        ));
        assert!(declares_c11_threads("declare void @call_once(ptr, ptr)\n"));
        assert!(declares_c11_threads("declare i32 @mtx_lock(ptr)\n"));
        // A definition, or a name that only starts the same, is not a use
        // of the C library's threads.
        assert!(!declares_c11_threads(
            "define i32 @thrd_create(ptr %0) {\n}\n"
        ));
        assert!(!declares_c11_threads("declare i32 @call_once_more(ptr)\n"));
        // Only the C11 names: an application's `thrd_Custom` is not one.
        assert!(!declares_c11_threads("declare i32 @thrd_Custom()\n"));
        // A quoted name is the same symbol (LangRef identifiers), escapes too.
        assert!(declares_c11_threads("declare void @\"thrd_yield\"()\n"));
        assert!(declares_c11_threads("declare void @\"thrd\\5Fyield\"()\n"));
        assert!(!declares_c11_threads("declare void @\"thrd_Custom\"()\n"));
        assert!(!declares_c11_threads("define void @\"thrd_yield\"() {\n"));
        assert!(!declares_c11_threads("declare i32 @mtx_lockless(ptr)\n"));
        assert!(declares_c11_threads("declare void @thrd_yield()\n"));
        assert!(!declares_c11_threads("declare i32 @printf(ptr, ...)\n"));
    }

    #[test]
    fn thread_functions_in_ir_inputs() {
        let dir = Scratch::new().unwrap();
        let worker = dir.dir.join("worker.ll");
        std::fs::write(&worker, "declare i32 @thrd_create(ptr, ptr, ptr)\n").unwrap();
        let plain = dir.dir.join("plain.ll");
        std::fs::write(&plain, "declare i32 @puts(ptr)\n").unwrap();
        // An object is passed through unread, whatever it holds.
        let object = dir.dir.join("worker.o");
        std::fs::write(&object, "declare i32 @thrd_create(ptr, ptr, ptr)\n").unwrap();
        assert!(ir_input_uses_threads(&worker.display().to_string()));
        assert!(!ir_input_uses_threads(&plain.display().to_string()));
        assert!(!ir_input_uses_threads(&object.display().to_string()));
        assert!(!ir_input_uses_threads("missing.ll"));
    }

    #[test]
    fn scratch_directories_are_new_and_private() {
        let first = Scratch::new().unwrap();
        let second = Scratch::new().unwrap();
        assert_ne!(first.dir, second.dir);
        assert!(first.dir.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&first.dir).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
        let dir = first.dir.clone();
        drop(first);
        assert!(!dir.exists());
    }

    #[test]
    fn threads_runtime_by_target() {
        let dir = std::env::temp_dir().join(format!("hcc-threads-{}", std::process::id()));
        let with = dir.join("with");
        let without = dir.join("without");
        std::fs::create_dir_all(&with).unwrap();
        std::fs::create_dir_all(&without).unwrap();
        std::fs::write(with.join("threads.h"), "").unwrap();
        for target in [
            crate::target::APPLE_DEFAULT,
            crate::target::APPLE_X86_64_DEFAULT,
            crate::target::WINDOWS_GNU,
            crate::target::WINDOWS_MSVC,
        ] {
            assert!(
                needs_threads_runtime(target, std::slice::from_ref(&with)),
                "{target:?}"
            );
        }
        for target in [Target::X86_64Linux, Target::Aarch64Linux] {
            assert!(!needs_threads_runtime(target, std::slice::from_ref(&with)));
            assert!(needs_threads_runtime(
                target,
                std::slice::from_ref(&without)
            ));
        }
        // The compiler's own <threads.h> is not the C library's.
        assert!(needs_threads_runtime(
            Target::X86_64Linux,
            &crate::pp::compiler_include_dirs()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(threads_runtime().is_some());
    }

    #[test]
    fn foreign_sources_and_response_file_names() {
        for path in [
            "main.cpp", "a.cc", "b.cxx", "c.C", "d.m", "e.mm", "f.h", "g.cu", "h.ii",
        ] {
            assert!(is_foreign_source(path), "{path}");
        }
        for path in [
            "main.c", "main.i", "a.ll", "a.bc", "a.o", "libx.a", "x.so", "x.obj", "s.s", "s.S",
            "noext",
        ] {
            assert!(!is_foreign_source(path), "{path}");
        }
        assert_eq!(literal_path("@main.o"), "./@main.o");
        assert_eq!(literal_path("dir/@main.o"), "dir/@main.o");
        assert_eq!(literal_path("main.o"), "main.o");
    }
}
