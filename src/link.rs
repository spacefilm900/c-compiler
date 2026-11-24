//! `--link`: compile each C source to LLVM IR, then link the IR and the
//! other inputs into an executable with Clang.
//!
//! Clang is only the LLVM backend and the linker driver here: it reads the
//! `.ll` files this compiler wrote, and never sees the C sources. It is
//! `$HCC_CLANG` when set, else `clang` on `PATH`, and is called with the
//! target's LLVM triple.
//!
//! C11 threads (7.26) need a runtime on some targets. When the IR declares
//! a `<threads.h>` function (`thrd_*`, `mtx_*`, `cnd_*`, `tss_*` or
//! `call_once`), POSIX targets link with `-pthread`, and when the target's
//! C library has no `<threads.h>` (macOS, Windows, Linux before glibc 2.28)
//! `lib/threads.c`, the compiler's own runtime, is compiled by this
//! compiler and linked too. That is the same choice the compiler's
//! `<threads.h>` makes when it decides whether to use the C library's.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::driver::{compile_source, read_source, Options};
use crate::target::Target;

/// Link `options.inputs` into `options.output`. Returns the exit status.
pub(crate) fn link(options: &Options) -> i32 {
    let output = options.output.as_deref().expect("checked by parse_args");
    let scratch = Scratch::new();
    let mut clang_inputs: Vec<PathBuf> = Vec::new();
    let mut uses_threads = false;
    for (index, input) in options.inputs.iter().enumerate() {
        if !is_c_source(input) {
            clang_inputs.push(PathBuf::from(input));
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
        clang_inputs.push(ll);
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
            let status = compile_runtime(options, &runtime, &scratch, &mut clang_inputs);
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
    command.args(&clang_inputs);
    command.args(&options.link_args);
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
    clang_inputs: &mut Vec<PathBuf>,
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
    clang_inputs.push(ll);
    0
}

/// A C source file: compiled by this compiler. Anything else (`.ll`, `.o`,
/// `.a`, `.so`, `.obj`, `.lib`, ...) goes to Clang unchanged.
pub(crate) fn is_c_source(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|ext| ext == "c" || ext == "i")
}

/// Whether the IR declares a C11 `<threads.h>` function.
pub(crate) fn declares_c11_threads(ir: &str) -> bool {
    const PREFIXES: [&str; 5] = ["@thrd_", "@mtx_", "@cnd_", "@tss_", "@call_once("];
    ir.lines()
        .filter(|line| line.starts_with("declare "))
        .any(|line| PREFIXES.iter().any(|prefix| line.contains(prefix)))
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

/// Temporary `.ll` files, removed when dropped.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "hcc-link-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::create_dir_all(&dir);
        Self { dir }
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
        assert!(!declares_c11_threads("declare i32 @printf(ptr, ...)\n"));
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
}
