//! C11 preprocessor (translation phases 2–4).
//!
//! Phase 2 deletes each backslash that is immediately followed by a new-line.
//! Phase 3 splits the result into preprocessing tokens and replaces each
//! comment with one space. Phase 4 runs directives and expands macros
//! (C11 6.10). `#include` pulls another file through the same phases.
//!
//! A macro name is not expanded again while its own replacement list is being
//! rescanned (C11 6.10.3.4). Each token carries that set of blocked names.
//! Argument prescan expands arguments that are not operands of `#` or `##`
//! before they are substituted; the rescan then sees the replacement spliced
//! back into the rest of the file, so a macro can take arguments from tokens
//! that follow the invocation.

use std::collections::{HashMap, HashSet};

use crate::fxhash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::ast::{sign_extend, TypeName};
use crate::diag::Diagnostic;
use crate::target::{LongDouble, Target};
use crate::token::{Token, TokenKind};

mod directive;
mod expand;
mod ifexpr;
mod predefined;
mod scan;
use directive::*;
use expand::*;
use ifexpr::*;
pub use predefined::*;
use scan::*;

const MAX_EXPANSIONS: u32 = 100_000;
const DEFAULT_INCLUDE_DEPTH: usize = 200;

/// One `-D` or `-U` from the command line, in the order they appeared.
#[derive(Debug, Clone)]
pub enum CliMacro {
    /// `-D NAME` defines `NAME` as `1`. `-D NAME=` defines it as empty.
    /// `-D NAME=tokens` uses `tokens` as the replacement list. A `name`
    /// written `NAME(PARAM, ...)` defines a function-like macro, as GCC's
    /// `-D 'NAME(args...)=definition'` does (GCC manual, "Preprocessor
    /// Options").
    Define {
        name: String,
        replacement: Option<String>,
    },
    Undef(String),
}

/// Search paths and command-line macros for one translation unit.
#[derive(Debug, Clone)]
pub struct PpOptions {
    /// `-I` directories, quote includes after the including file's directory,
    /// and angle includes before the system directories.
    pub include_dirs: Vec<PathBuf>,
    /// Directories for `#include <...>`, after every `-I` directory.
    pub system_dirs: Vec<PathBuf>,
    pub definitions: Vec<CliMacro>,
    /// Nested `#include` limit. The main file is depth 0.
    pub max_include_depth: usize,
    /// The machine being compiled for. It selects the predefined macros,
    /// and [`crate::compile_dialect`] passes it on to the rest of the
    /// frontend.
    pub target: Target,
    /// `-fgnu89-inline`: GNU89 `inline` semantics (GCC manual, "Options
    /// Controlling C Dialect"). The preprocessor defines
    /// `__GNUC_GNU_INLINE__` instead of `__GNUC_STDC_INLINE__`, and
    /// [`crate::compile_dialect`] tells the parser.
    pub gnu89_inline: bool,
}

impl Default for PpOptions {
    fn default() -> Self {
        Self {
            include_dirs: Vec::new(),
            system_dirs: default_system_dirs(),
            definitions: Vec::new(),
            max_include_depth: DEFAULT_INCLUDE_DEPTH,
            target: Target::build_default(),
            gnu89_inline: false,
        }
    }
}

/// Preprocess `source` and return the text of the remaining tokens.
///
/// Spacing is only required to keep tokens from gluing back together. It does
/// not match any other preprocessor byte for byte.
pub fn preprocess(source: &str, filename: &str, options: &PpOptions) -> Result<String, Diagnostic> {
    let tokens = run(source, filename, options)?;
    Ok(format_tokens(&tokens))
}

/// Preprocess `source` into the tokens the parser already accepts.
pub fn preprocess_tokens(
    source: &str,
    filename: &str,
    options: &PpOptions,
) -> Result<Vec<Token>, Diagnostic> {
    let pp_tokens = run(source, filename, options)?;
    let mut out = Vec::new();
    let mut eof_file: Rc<str> = Rc::from(filename);
    let mut eof_line = 1usize;
    let mut eof_col = 1usize;
    for tok in &pp_tokens {
        if tok.kind == PpKind::Newline || tok.kind == PpKind::Placemarker {
            eof_file = tok.display.clone();
            eof_line = tok.line;
            eof_col = tok.column;
            continue;
        }
        eof_file = tok.display.clone();
        eof_line = tok.line;
        eof_col = tok.column;
        out.push(convert_token(tok)?);
    }
    out.push(Token {
        kind: TokenKind::Eof,
        spelling: String::new(),
        line: eof_line,
        column: eof_col,
        filename: eof_file,
        value: None,
        float_hi: 0,
        int_ty: None,
        float_kind: None,
        bytes: None,
    });
    Ok(out)
}

fn run(source: &str, filename: &str, options: &PpOptions) -> Result<Vec<PpToken>, Diagnostic> {
    crate::target::set(options.target);
    let mut processor = Processor::new(options)?;
    processor.push_main(filename);
    processor.tokens = crate::timing::lexing(|| lex_source(source, filename, 0))?;
    let tokens = processor.expand_all(false)?;
    remove_pragma_operators(tokens)
}

/// The `_Pragma ( string-literal )` operator (C11 6.10.9), applied after
/// macro expansion. Its pragma behaves as a `#pragma` line would; every
/// pragma except `once` is ignored by this implementation, and `once` in
/// this form is ignored too (it rarely comes from a macro). The operator
/// and its operand are removed.
fn remove_pragma_operators(tokens: Vec<PpToken>) -> Result<Vec<PpToken>, Diagnostic> {
    if !tokens
        .iter()
        .any(|tok| tok.kind == PpKind::Ident && tok.spelling == "_Pragma")
    {
        return Ok(tokens);
    }
    let skippable = |tok: &PpToken| matches!(tok.kind, PpKind::Newline | PpKind::Placemarker);
    let mut out = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while index < tokens.len() {
        let tok = &tokens[index];
        if tok.kind != PpKind::Ident || tok.spelling != "_Pragma" {
            out.push(tok.clone());
            index += 1;
            continue;
        }
        let mut parts = Vec::new();
        let mut next = index + 1;
        while parts.len() < 3 && next < tokens.len() {
            if !skippable(&tokens[next]) {
                parts.push(&tokens[next]);
            }
            next += 1;
        }
        let well_formed = parts.len() == 3
            && parts[0].is_lparen()
            && parts[1].kind == PpKind::String
            && !parts[1].spelling.starts_with(['u', 'U'])
            && parts[2].is_rparen();
        if !well_formed {
            return Err(error_at(
                tok,
                "_Pragma requires a parenthesized string literal",
            ));
        }
        index = next;
    }
    Ok(out)
}

fn format_tokens(tokens: &[PpToken]) -> String {
    let mut out = String::new();
    let mut at_bol = true;
    let mut previous = String::new();
    for tok in tokens {
        if tok.kind == PpKind::Newline {
            out.push('\n');
            at_bol = true;
            previous.clear();
            continue;
        }
        if tok.kind == PpKind::Placemarker {
            continue;
        }
        if !at_bol && (tok.leading_space || tokens_glue(&previous, &tok.spelling)) {
            out.push(' ');
        }
        out.push_str(&tok.spelling);
        previous = tok.spelling.clone();
        at_bol = false;
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn tokens_glue(previous: &str, next: &str) -> bool {
    if previous.is_empty() || next.is_empty() {
        return false;
    }
    // `..` is still two tokens, but three dots become `...`. `??` is the
    // start of a trigraph in a preprocessor that implements them.
    if (previous == "." && next == ".") || (previous == "?" && next == "?") {
        return true;
    }
    let joined = format!("{previous}{next}");
    match lex_source(&joined, "<glue>", 0) {
        Ok(tokens) => {
            let kept: Vec<_> = tokens
                .iter()
                .filter(|tok| tok.kind != PpKind::Newline)
                .collect();
            !(kept.len() == 2 && kept[0].spelling == previous && kept[1].spelling == next)
        }
        Err(_) => true,
    }
}

fn convert_token(tok: &PpToken) -> Result<Token, Diagnostic> {
    if tok.spelling == "#" || tok.spelling == "##" {
        return Err(error_at(tok, "stray '#' in program"));
    }
    let spelling = match tok.spelling.as_str() {
        "<:" => "[",
        ":>" => "]",
        "<%" => "{",
        "%>" => "}",
        other => other,
    };
    // Identifiers, keywords, and punctuators are most tokens; they need no
    // value, so the full lexer is skipped for them.
    if matches!(tok.kind, PpKind::Ident | PpKind::Punct) {
        if let Some(kind) = crate::timing::lexing(|| crate::lexer::simple_kind(spelling)) {
            return Ok(Token {
                kind,
                spelling: spelling.to_string(),
                line: tok.line,
                column: tok.column,
                filename: Rc::clone(&tok.display),
                value: None,
                float_hi: 0,
                int_ty: None,
                float_kind: None,
                bytes: None,
            });
        }
    }
    match crate::timing::lexing(|| crate::lexer::lex_shared(spelling, &tok.display)) {
        Ok(mut tokens) => {
            tokens.pop();
            if tokens.len() != 1 {
                return Err(error_at(
                    tok,
                    format!("invalid preprocessing token '{}'", tok.spelling),
                ));
            }
            let mut converted = tokens.remove(0);
            converted.line = tok.line;
            converted.column = tok.column;
            converted.filename = Rc::clone(&tok.display);
            Ok(converted)
        }
        Err(mut err) => {
            err.filename = tok.display.to_string();
            err.line = tok.line;
            err.column = tok.column;
            Err(err)
        }
    }
}

fn error_at(tok: &PpToken, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(&*tok.display, tok.line, tok.column, message)
}

type Hide = Rc<FxHashSet<String>>;

fn hide_empty() -> Hide {
    Rc::new(FxHashSet::default())
}

fn hide_with(hide: &Hide, name: &str) -> Hide {
    if hide.contains(name) {
        return Rc::clone(hide);
    }
    let mut set = (**hide).clone();
    set.insert(name.to_string());
    Rc::new(set)
}

fn hide_union(left: &Hide, right: &Hide) -> Hide {
    if left.is_empty() {
        return Rc::clone(right);
    }
    if right.is_empty() {
        return Rc::clone(left);
    }
    let mut set = (**left).clone();
    set.extend(right.iter().cloned());
    Rc::new(set)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PpKind {
    Ident,
    Number,
    String,
    Char,
    Punct,
    Other,
    Newline,
    Placemarker,
}

#[derive(Clone)]
struct PpToken {
    kind: PpKind,
    spelling: String,
    display: Rc<str>,
    file_id: u32,
    line: usize,
    column: usize,
    leading_space: bool,
    hide: Hide,
}

impl PpToken {
    fn is_lparen(&self) -> bool {
        self.kind == PpKind::Punct && self.spelling == "("
    }

    fn is_rparen(&self) -> bool {
        self.kind == PpKind::Punct && self.spelling == ")"
    }

    fn is_comma(&self) -> bool {
        self.kind == PpKind::Punct && self.spelling == ","
    }

    fn is_hash(&self) -> bool {
        self.kind == PpKind::Punct && self.spelling == "#"
    }
}

#[derive(Clone)]
struct Macro {
    name: String,
    /// `None` is object-like. `Some` is function-like, possibly with no
    /// parameters. `__VA_ARGS__` is not stored here.
    params: Option<Vec<String>>,
    variadic: bool,
    body: Vec<PpToken>,
}

#[derive(Clone)]
enum MacroEntry {
    Defined(Rc<Macro>),
    Undefined,
}

struct FileInfo {
    real_path: PathBuf,
    canonical: PathBuf,
    depth: usize,
    /// Directory the file was found in, for `#include_next`. None for the
    /// main source file.
    found_dir: Option<PathBuf>,
    /// A system header: found in a system directory, or found next to the
    /// system header that included it with `#include "..."`.
    system: bool,
}

struct Cond {
    active: bool,
    seen_true: bool,
    /// C11 6.10.1 allows one `#else`, and no `#elif` after it.
    else_seen: bool,
    display: Rc<str>,
    line: usize,
    column: usize,
}

struct Part {
    /// The comma that opened this part. The first part has none.
    comma: Option<PpToken>,
    tokens: Vec<PpToken>,
}

struct Collected {
    positional: Vec<Vec<PpToken>>,
    va: Vec<PpToken>,
    /// `F(1)` for `#define F(a, ...)` omits the variable arguments. `F(1,)`
    /// provides an empty one. Clang deletes a comma before `## __VA_ARGS__`
    /// only when the arguments were omitted.
    va_omitted: bool,
}

enum Found {
    No,
    Special,
    Macro(Rc<Macro>),
}

enum Clause {
    Elif,
    Else,
    Endif,
}

struct Processor {
    tokens: Vec<PpToken>,
    index: usize,
    macros: FxHashMap<String, MacroEntry>,
    files: Vec<FileInfo>,
    once: HashSet<PathBuf>,
    cond: Vec<Cond>,
    expansions: u32,
    /// Free slots `splice` last put in front of the cursor when it rebuilt
    /// the token vector.
    headroom: usize,
    include_dirs: Vec<PathBuf>,
    system_dirs: Vec<PathBuf>,
    max_depth: usize,
    /// Expanding an `#if` line: the operand of a `defined` that a macro
    /// produced is kept as written (see `expand_all`).
    in_if: bool,
}

impl Processor {
    fn new(options: &PpOptions) -> Result<Self, Diagnostic> {
        let mut processor = Self {
            tokens: Vec::new(),
            index: 0,
            macros: FxHashMap::default(),
            files: Vec::new(),
            once: HashSet::new(),
            cond: Vec::new(),
            expansions: 0,
            headroom: 0,
            include_dirs: options.include_dirs.clone(),
            system_dirs: options.system_dirs.clone(),
            max_depth: options.max_include_depth,
            in_if: false,
        };
        processor.install_builtins()?;
        if options.gnu89_inline {
            processor.apply_cli(&CliMacro::Undef("__GNUC_STDC_INLINE__".to_string()))?;
            processor.apply_cli(&CliMacro::Define {
                name: "__GNUC_GNU_INLINE__".to_string(),
                replacement: None,
            })?;
        }
        for definition in &options.definitions {
            processor.apply_cli(definition)?;
        }
        Ok(processor)
    }

    fn install_builtins(&mut self) -> Result<(), Diagnostic> {
        const BUILTINS: &[(&str, &str)] = &[
            ("__STDC__", "1"),
            ("__STDC_HOSTED__", "1"),
            ("__STDC_VERSION__", "201112L"),
            ("__STDC_UTF_16__", "1"),
            ("__STDC_UTF_32__", "1"),
            // C11 6.10.8.3: features this implementation does not provide.
            ("__STDC_NO_COMPLEX__", "1"),
            ("__STDC_NO_THREADS__", "1"),
            ("__STRICT_ANSI__", "1"),
            ("__CHAR_BIT__", "8"),
            ("__SIZEOF_SHORT__", "2"),
            ("__SIZEOF_INT__", "4"),
            ("__SIZEOF_LONG_LONG__", "8"),
            ("__SIZEOF_POINTER__", "8"),
            ("__SIZEOF_SIZE_T__", "8"),
            ("__SIZEOF_PTRDIFF_T__", "8"),
            ("__SIZEOF_FLOAT__", "4"),
            ("__SIZEOF_DOUBLE__", "8"),
            ("__POINTER_WIDTH__", "64"),
            ("__INT_WIDTH__", "32"),
            ("__LONG_LONG_WIDTH__", "64"),
            ("__SCHAR_MAX__", "127"),
            ("__SHRT_MAX__", "32767"),
            ("__INT_MAX__", "2147483647"),
            ("__LONG_LONG_MAX__", "9223372036854775807LL"),
            ("__ORDER_LITTLE_ENDIAN__", "1234"),
            ("__ORDER_BIG_ENDIAN__", "4321"),
            ("__ORDER_PDP_ENDIAN__", "3412"),
            ("__BYTE_ORDER__", "__ORDER_LITTLE_ENDIAN__"),
        ];
        let target = target_macros(crate::target::current());
        for (name, body) in BUILTINS.iter().chain(target.iter()) {
            // A function-like builtin is written `NAME(PARAM, ...)`.
            let (name, params) = split_function_like(name);
            let mac = self.macro_from_parts(name, params, false, body_tokens(body)?, None)?;
            self.macros
                .insert(mac.name.clone(), MacroEntry::Defined(Rc::new(mac)));
        }
        Ok(())
    }

    fn apply_cli(&mut self, definition: &CliMacro) -> Result<(), Diagnostic> {
        let at = command_token();
        match definition {
            CliMacro::Undef(name) => {
                self.macros.insert(name.clone(), MacroEntry::Undefined);
                Ok(())
            }
            CliMacro::Define {
                name: spelling,
                replacement,
            } => {
                let (name, params) = split_function_like(spelling);
                let params_ok = params
                    .as_ref()
                    .is_none_or(|list| list.iter().all(|param| is_ident_text(param)));
                if !is_ident_text(name) || name == "defined" || !params_ok {
                    return Err(error_at(&at, format!("invalid macro name '{spelling}'")));
                }
                let body = match replacement {
                    None => body_tokens("1")?,
                    Some(text) => body_tokens(text)?,
                };
                let mac = self.macro_from_parts(name, params, false, body, Some(&at))?;
                self.macros
                    .insert(mac.name.clone(), MacroEntry::Defined(Rc::new(mac)));
                Ok(())
            }
        }
    }

    fn push_main(&mut self, filename: &str) {
        let path = PathBuf::from(filename);
        let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        self.files.push(FileInfo {
            real_path: path,
            canonical,
            depth: 0,
            found_dir: None,
            system: false,
        });
    }
}

/// Split `NAME(PARAM, ...)` into the name and its parameters; a plain
/// `NAME` has none. A malformed list (no closing parenthesis, text after
/// it, or an empty parameter between commas) yields an empty parameter,
/// which the caller rejects as not an identifier. `NAME()` has no
/// parameters.
fn split_function_like(spelling: &str) -> (&str, Option<Vec<String>>) {
    let Some((name, rest)) = spelling.split_once('(') else {
        return (spelling, None);
    };
    let Some(inner) = rest.strip_suffix(')') else {
        return (name, Some(vec![String::new()]));
    };
    if inner.contains(['(', ')']) {
        return (name, Some(vec![String::new()]));
    }
    if inner.trim().is_empty() {
        return (name, Some(Vec::new()));
    }
    let params = inner
        .split(',')
        .map(|param| param.trim().to_string())
        .collect();
    (name, Some(params))
}

#[cfg(test)]
mod tests;
