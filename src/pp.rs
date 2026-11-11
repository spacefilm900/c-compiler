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
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::ast::{sign_extend, TypeName};
use crate::diag::Diagnostic;
use crate::token::{Token, TokenKind};

const MAX_EXPANSIONS: u32 = 100_000;
const DEFAULT_INCLUDE_DEPTH: usize = 200;

/// One `-D` or `-U` from the command line, in the order they appeared.
#[derive(Debug, Clone)]
pub enum CliMacro {
    /// `-D NAME` defines `NAME` as `1`. `-D NAME=` defines it as empty.
    /// `-D NAME=tokens` uses `tokens` as the replacement list.
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
}

impl Default for PpOptions {
    fn default() -> Self {
        Self {
            include_dirs: Vec::new(),
            system_dirs: default_system_dirs(),
            definitions: Vec::new(),
            max_include_depth: DEFAULT_INCLUDE_DEPTH,
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
    let mut eof_file = filename.to_string();
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

fn default_system_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    // The compiler's own headers (stddef.h, stdarg.h, stdbool.h, limits.h)
    // come before the C library, the same way a hosted compiler searches its
    // builtin include directory first.
    push_compiler(&mut dirs, PathBuf::from("include"));
    push_compiler(
        &mut dirs,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("include"),
    );
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(Path::to_path_buf);
        for _ in 0..8 {
            let Some(current) = dir else { break };
            push_compiler(&mut dirs, current.join("include"));
            dir = current.parent().map(Path::to_path_buf);
        }
    }
    for path in ["/usr/include/x86_64-linux-gnu", "/usr/include"] {
        let path = PathBuf::from(path);
        if path.is_dir() && !dirs.iter().any(|dir| dir == &path) {
            dirs.push(path);
        }
    }
    dirs
}

fn push_compiler(dirs: &mut Vec<PathBuf>, path: PathBuf) {
    if path.join("stddef.h").is_file() && !dirs.iter().any(|dir| dir == &path) {
        dirs.push(path);
    }
}

fn run(source: &str, filename: &str, options: &PpOptions) -> Result<Vec<PpToken>, Diagnostic> {
    let mut processor = Processor::new(options)?;
    processor.push_main(filename);
    processor.tokens = lex_source(source, filename, 0)?;
    processor.expand_all(false)
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
    match crate::lexer::lex(spelling, &tok.display) {
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
            converted.filename = tok.display.clone();
            Ok(converted)
        }
        Err(mut err) => {
            err.filename = tok.display.clone();
            err.line = tok.line;
            err.column = tok.column;
            Err(err)
        }
    }
}

fn error_at(tok: &PpToken, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(&tok.display, tok.line, tok.column, message)
}

type Hide = Rc<HashSet<String>>;

fn hide_empty() -> Hide {
    Rc::new(HashSet::new())
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
    display: String,
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
    Defined(Macro),
    Undefined,
}

struct FileInfo {
    real_path: PathBuf,
    canonical: PathBuf,
    depth: usize,
    /// Directory the file was found in, for `#include_next`. None for the
    /// main source file.
    found_dir: Option<PathBuf>,
}

struct Cond {
    active: bool,
    seen_true: bool,
    /// C11 6.10.1 allows one `#else`, and no `#elif` after it.
    else_seen: bool,
    display: String,
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
    Macro(Macro),
}

enum Clause {
    Elif,
    Else,
    Endif,
}

struct Processor {
    tokens: Vec<PpToken>,
    index: usize,
    macros: HashMap<String, MacroEntry>,
    files: Vec<FileInfo>,
    once: HashSet<PathBuf>,
    cond: Vec<Cond>,
    expansions: u32,
    include_dirs: Vec<PathBuf>,
    system_dirs: Vec<PathBuf>,
    max_depth: usize,
}

impl Processor {
    fn new(options: &PpOptions) -> Result<Self, Diagnostic> {
        let mut processor = Self {
            tokens: Vec::new(),
            index: 0,
            macros: HashMap::new(),
            files: Vec::new(),
            once: HashSet::new(),
            cond: Vec::new(),
            expansions: 0,
            include_dirs: options.include_dirs.clone(),
            system_dirs: options.system_dirs.clone(),
            max_depth: options.max_include_depth,
        };
        processor.install_builtins()?;
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
            ("__STDC_NO_ATOMICS__", "1"),
            ("__STDC_NO_COMPLEX__", "1"),
            ("__STDC_NO_THREADS__", "1"),
            ("__STDC_NO_VLA__", "1"),
            ("__STRICT_ANSI__", "1"),
            ("__x86_64__", "1"),
            ("__x86_64", "1"),
            ("__amd64__", "1"),
            ("__amd64", "1"),
            ("__LP64__", "1"),
            ("_LP64", "1"),
            ("__linux__", "1"),
            ("__linux", "1"),
            ("__unix__", "1"),
            ("__unix", "1"),
            ("__gnu_linux__", "1"),
            ("__ELF__", "1"),
            ("__CHAR_BIT__", "8"),
            ("__SIZEOF_SHORT__", "2"),
            ("__SIZEOF_INT__", "4"),
            ("__SIZEOF_LONG__", "8"),
            ("__SIZEOF_LONG_LONG__", "8"),
            ("__SIZEOF_POINTER__", "8"),
            ("__SIZEOF_SIZE_T__", "8"),
            ("__SIZEOF_PTRDIFF_T__", "8"),
            ("__SIZEOF_FLOAT__", "4"),
            ("__SIZEOF_DOUBLE__", "8"),
            ("__SIZEOF_LONG_DOUBLE__", "16"),
            ("__POINTER_WIDTH__", "64"),
            ("__INT_WIDTH__", "32"),
            ("__LONG_WIDTH__", "64"),
            ("__LONG_LONG_WIDTH__", "64"),
            ("__SCHAR_MAX__", "127"),
            ("__SHRT_MAX__", "32767"),
            ("__INT_MAX__", "2147483647"),
            ("__LONG_MAX__", "9223372036854775807L"),
            ("__LONG_LONG_MAX__", "9223372036854775807LL"),
            ("__INTMAX_MAX__", "9223372036854775807L"),
            ("__UINTMAX_MAX__", "18446744073709551615UL"),
            ("__SIZE_TYPE__", "long unsigned int"),
            ("__PTRDIFF_TYPE__", "long int"),
            ("__WCHAR_TYPE__", "int"),
            ("__INTMAX_TYPE__", "long int"),
            ("__UINTMAX_TYPE__", "long unsigned int"),
            ("__INTPTR_TYPE__", "long int"),
            ("__UINTPTR_TYPE__", "long unsigned int"),
            ("__GNUC__", "4"),
            ("__GNUC_MINOR__", "2"),
            ("__GNUC_PATCHLEVEL__", "1"),
            ("__GNUC_STDC_INLINE__", "1"),
            ("__ORDER_LITTLE_ENDIAN__", "1234"),
            ("__ORDER_BIG_ENDIAN__", "4321"),
            ("__ORDER_PDP_ENDIAN__", "3412"),
            ("__BYTE_ORDER__", "__ORDER_LITTLE_ENDIAN__"),
            ("__USER_LABEL_PREFIX__", ""),
            ("__WCHAR_MAX__", "2147483647"),
            ("__WCHAR_MIN__", "(-2147483647 - 1)"),
            ("__WINT_TYPE__", "unsigned int"),
        ];
        for (name, body) in BUILTINS {
            let mac = self.macro_from_parts(name, None, false, body_tokens(body)?, None)?;
            self.macros
                .insert(mac.name.clone(), MacroEntry::Defined(mac));
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
            CliMacro::Define { name, replacement } => {
                if !is_ident_text(name) || *name == "defined" {
                    return Err(error_at(&at, format!("invalid macro name '{name}'")));
                }
                let body = match replacement {
                    None => body_tokens("1")?,
                    Some(text) => body_tokens(text)?,
                };
                let mac = self.macro_from_parts(name, None, false, body, Some(&at))?;
                self.macros
                    .insert(mac.name.clone(), MacroEntry::Defined(mac));
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
        });
    }

    fn expand_all(&mut self, isolated: bool) -> Result<Vec<PpToken>, Diagnostic> {
        let mut out = Vec::new();
        while self.index < self.tokens.len() {
            if !isolated && self.at_directive() {
                self.handle_directive()?;
                continue;
            }
            if self.tokens[self.index].kind == PpKind::Newline {
                if !isolated {
                    out.push(self.tokens[self.index].clone());
                }
                self.index += 1;
                continue;
            }
            if self.try_expand(isolated)? {
                continue;
            }
            out.push(self.tokens[self.index].clone());
            self.index += 1;
        }
        if !isolated && !self.cond.is_empty() {
            let frame = self.cond.last().unwrap();
            return Err(Diagnostic::new(
                &frame.display,
                frame.line,
                frame.column,
                "unterminated conditional directive",
            ));
        }
        Ok(out)
    }

    fn at_line_start(&self) -> bool {
        self.index == 0 || self.tokens[self.index - 1].kind == PpKind::Newline
    }

    fn at_directive(&self) -> bool {
        self.index < self.tokens.len() && self.at_line_start() && self.tokens[self.index].is_hash()
    }

    fn try_expand(&mut self, isolated: bool) -> Result<bool, Diagnostic> {
        let tok = self.tokens[self.index].clone();
        if tok.kind != PpKind::Ident || tok.hide.contains(&tok.spelling) {
            return Ok(false);
        }
        match self.lookup(&tok.spelling) {
            Found::No => Ok(false),
            Found::Special => {
                self.note_expansion(&tok)?;
                let repl = special_token(&tok);
                self.index += 1;
                self.splice(vec![repl]);
                Ok(true)
            }
            Found::Macro(mac) => {
                if mac.params.is_some() {
                    let Some(lparen) = self.lparen_index(isolated) else {
                        return Ok(false);
                    };
                    self.note_expansion(&tok)?;
                    self.index = lparen;
                    let args = self.collect_args(&mac, &tok)?;
                    let repl = self.substitute(&mac, &args, &tok)?;
                    self.splice(repl);
                    return Ok(true);
                }
                self.note_expansion(&tok)?;
                let repl = self.expand_object(&mac, &tok)?;
                self.index += 1;
                self.splice(repl);
                Ok(true)
            }
        }
    }

    fn note_expansion(&mut self, tok: &PpToken) -> Result<(), Diagnostic> {
        self.expansions += 1;
        if self.expansions > MAX_EXPANSIONS {
            return Err(error_at(tok, "macro expansion is too large"));
        }
        Ok(())
    }

    fn splice(&mut self, repl: Vec<PpToken>) {
        let at = self.index;
        self.tokens.splice(at..at, repl);
        self.index = at;
    }

    fn lookup(&self, name: &str) -> Found {
        match self.macros.get(name) {
            Some(MacroEntry::Defined(mac)) => Found::Macro(mac.clone()),
            Some(MacroEntry::Undefined) => Found::No,
            None if name == "__FILE__" || name == "__LINE__" => Found::Special,
            None => Found::No,
        }
    }

    fn is_defined(&self, name: &str) -> bool {
        match self.macros.get(name) {
            Some(MacroEntry::Defined(_)) => true,
            Some(MacroEntry::Undefined) => false,
            None => name == "__FILE__" || name == "__LINE__",
        }
    }

    fn lparen_index(&self, isolated: bool) -> Option<usize> {
        let mut index = self.index + 1;
        loop {
            if index >= self.tokens.len() {
                return None;
            }
            if self.tokens[index].kind == PpKind::Newline {
                if !isolated {
                    let mut ahead = index + 1;
                    while ahead < self.tokens.len() && self.tokens[ahead].kind == PpKind::Newline {
                        ahead += 1;
                    }
                    if ahead < self.tokens.len() && self.tokens[ahead].is_hash() {
                        return None;
                    }
                }
                index += 1;
                continue;
            }
            if self.tokens[index].is_lparen() {
                if isolated && !paren_closes(&self.tokens[index..]) {
                    return None;
                }
                return Some(index);
            }
            return None;
        }
    }

    fn expand_object(&mut self, mac: &Macro, inv: &PpToken) -> Result<Vec<PpToken>, Diagnostic> {
        let mut body = mac.body.clone();
        // `__LINE__` and `__FILE__` in a replacement list use the invocation,
        // not the line where the macro was defined.
        retarget(&mut body, inv);
        let mut repl = paste_tokens(&body, inv)?;
        repl.retain(|tok| tok.kind != PpKind::Placemarker);
        paint(&mut repl, &inv.hide, &mac.name);
        if let Some(first) = repl.first_mut() {
            first.leading_space |= inv.leading_space;
        }
        Ok(repl)
    }

    fn substitute(
        &mut self,
        mac: &Macro,
        args: &Collected,
        inv: &PpToken,
    ) -> Result<Vec<PpToken>, Diagnostic> {
        let params = mac.params.clone().unwrap_or_default();
        let mut body = mac.body.clone();
        retarget(&mut body, inv);
        let mut raw: HashMap<String, Vec<PpToken>> = HashMap::new();
        for (index, name) in params.iter().enumerate() {
            raw.insert(name.clone(), args.positional[index].clone());
        }
        if mac.variadic {
            raw.insert("__VA_ARGS__".to_string(), args.va.clone());
        }
        let (_need_raw, need_exp) = param_uses(&body, &raw);
        let mut expanded: HashMap<String, Vec<PpToken>> = HashMap::new();
        for name in &need_exp {
            let arg = raw.get(name).cloned().unwrap_or_default();
            expanded.insert(name.clone(), self.expand_list(arg)?);
        }
        let mut subst = Vec::new();
        let mut index = 0;
        while index < body.len() {
            if mac.variadic
                && index + 2 < body.len()
                && body[index].is_comma()
                && body[index + 1].spelling == "##"
                && body[index + 2].spelling == "__VA_ARGS__"
            {
                if args.va_omitted {
                    index += 3;
                    continue;
                }
                subst.push(body[index].clone());
                subst.extend(args.va.clone());
                index += 3;
                continue;
            }
            if body[index].spelling == "#"
                && !hashhash_operand(&body, index)
                && index + 1 < body.len()
                && raw.contains_key(&body[index + 1].spelling)
            {
                let mut string = stringify(&raw[&body[index + 1].spelling], &body[index]);
                string.leading_space = body[index].leading_space;
                subst.push(string);
                index += 2;
                continue;
            }
            if raw.contains_key(&body[index].spelling) {
                let name = &body[index].spelling;
                let beside = hashhash_operand(&body, index);
                let source = if beside {
                    raw.get(name).cloned().unwrap_or_default()
                } else {
                    expanded.get(name).cloned().unwrap_or_default()
                };
                if beside && source.is_empty() {
                    subst.push(placemarker(&body[index]));
                } else if !source.is_empty() {
                    let mut copy = source;
                    if let Some(first) = copy.first_mut() {
                        first.leading_space = body[index].leading_space;
                    }
                    subst.extend(copy);
                }
                index += 1;
                continue;
            }
            subst.push(body[index].clone());
            index += 1;
        }
        let mut repl = paste_tokens(&subst, inv)?;
        repl.retain(|tok| tok.kind != PpKind::Placemarker);
        paint(&mut repl, &inv.hide, &mac.name);
        if let Some(first) = repl.first_mut() {
            first.leading_space |= inv.leading_space;
        }
        Ok(repl)
    }

    fn expand_list(&mut self, list: Vec<PpToken>) -> Result<Vec<PpToken>, Diagnostic> {
        let saved_tokens = std::mem::replace(&mut self.tokens, list);
        let saved_index = self.index;
        self.index = 0;
        let result = self.expand_all(true);
        self.tokens = saved_tokens;
        self.index = saved_index;
        result
    }

    fn collect_args(&mut self, mac: &Macro, inv: &PpToken) -> Result<Collected, Diagnostic> {
        self.index += 1;
        let named = mac.params.as_ref().map(Vec::len).unwrap_or(0);
        let mut parts = vec![Part {
            comma: None,
            tokens: Vec::new(),
        }];
        let mut depth = 0i32;
        loop {
            let tok = self.next_arg_token(inv)?;
            if tok.is_lparen() {
                depth += 1;
                parts.last_mut().unwrap().tokens.push(tok);
            } else if tok.is_rparen() {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                parts.last_mut().unwrap().tokens.push(tok);
            } else if tok.is_comma() && depth == 0 {
                parts.push(Part {
                    comma: Some(tok),
                    tokens: Vec::new(),
                });
            } else {
                parts.last_mut().unwrap().tokens.push(tok);
            }
        }
        self.finish_args(mac, inv, named, parts)
    }

    fn finish_args(
        &self,
        mac: &Macro,
        inv: &PpToken,
        named: usize,
        parts: Vec<Part>,
    ) -> Result<Collected, Diagnostic> {
        let empty_call = parts.len() == 1 && parts[0].tokens.is_empty() && parts[0].comma.is_none();
        if empty_call {
            if named == 0 && !mac.variadic {
                return Ok(Collected {
                    positional: Vec::new(),
                    va: Vec::new(),
                    va_omitted: false,
                });
            }
            if named == 0 && mac.variadic {
                return Ok(Collected {
                    positional: Vec::new(),
                    va: Vec::new(),
                    va_omitted: false,
                });
            }
            if named == 1 && !mac.variadic {
                return Ok(Collected {
                    positional: vec![Vec::new()],
                    va: Vec::new(),
                    va_omitted: false,
                });
            }
            return Err(arity_error(inv, &mac.name, 0, named, mac.variadic));
        }
        if !mac.variadic {
            if parts.len() != named {
                return Err(arity_error(inv, &mac.name, parts.len(), named, false));
            }
            return Ok(Collected {
                positional: parts.into_iter().map(|part| part.tokens).collect(),
                va: Vec::new(),
                va_omitted: false,
            });
        }
        if named == 0 {
            return Ok(Collected {
                positional: Vec::new(),
                va: join_va(&parts, 0),
                va_omitted: false,
            });
        }
        if parts.len() < named {
            return Err(arity_error(inv, &mac.name, parts.len(), named, true));
        }
        let va_omitted = parts.len() == named;
        let positional = parts
            .iter()
            .take(named)
            .map(|part| part.tokens.clone())
            .collect();
        Ok(Collected {
            positional,
            va: join_va(&parts, named),
            va_omitted,
        })
    }

    fn next_arg_token(&mut self, inv: &PpToken) -> Result<PpToken, Diagnostic> {
        let mut saw_space = false;
        loop {
            if self.index >= self.tokens.len() {
                return Err(error_at(
                    inv,
                    format!(
                        "unterminated argument list invoking macro '{}'",
                        inv.spelling
                    ),
                ));
            }
            if self.at_directive() {
                self.handle_directive()?;
                saw_space = true;
                continue;
            }
            if self.tokens[self.index].kind == PpKind::Newline {
                self.index += 1;
                saw_space = true;
                continue;
            }
            let mut tok = self.tokens[self.index].clone();
            self.index += 1;
            if saw_space {
                tok.leading_space = true;
            }
            return Ok(tok);
        }
    }

    fn handle_directive(&mut self) -> Result<(), Diagnostic> {
        let hash = self.tokens[self.index].clone();
        self.index += 1;
        if self.line_done() {
            self.consume_newline();
            return Ok(());
        }
        let name_tok = self.tokens[self.index].clone();
        if name_tok.kind != PpKind::Ident {
            return Err(error_at(&hash, "invalid preprocessing directive"));
        }
        self.index += 1;
        match name_tok.spelling.as_str() {
            "define" => self.do_define(&hash),
            "undef" => self.do_undef(&hash),
            "include" => self.do_include(&hash, false),
            "include_next" => self.do_include(&hash, true),
            "if" => self.do_if(&hash),
            "ifdef" => self.do_ifdef(&hash, true),
            "ifndef" => self.do_ifdef(&hash, false),
            "elif" => self.do_elif(&hash),
            "else" => self.do_else(&hash),
            "endif" => self.do_endif(&hash),
            "error" => self.do_error(&hash),
            "line" => self.do_line(&hash),
            "pragma" => self.do_pragma(&hash),
            other => Err(error_at(
                &name_tok,
                format!("unknown preprocessing directive '#{other}'"),
            )),
        }
    }

    fn do_define(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        if line.is_empty() {
            return Err(error_at(at, "macro name missing"));
        }
        if line[0].kind != PpKind::Ident {
            return Err(error_at(&line[0], "macro name must be an identifier"));
        }
        let name = line[0].spelling.clone();
        if name == "defined" {
            return Err(error_at(
                &line[0],
                "'defined' cannot be used as a macro name",
            ));
        }
        let rest = &line[1..];
        let parsed = if rest
            .first()
            .is_some_and(|tok| tok.is_lparen() && !tok.leading_space)
        {
            parse_function_header(rest)?
        } else {
            ParsedFn {
                params: None,
                variadic: false,
                body: rest.to_vec(),
            }
        };
        let mac =
            self.macro_from_parts(&name, parsed.params, parsed.variadic, parsed.body, Some(at))?;
        self.install(mac, at, false)
    }

    fn do_undef(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        if line.len() != 1 || line[0].kind != PpKind::Ident {
            return Err(error_at(at, "expected one identifier after #undef"));
        }
        if line[0].spelling == "defined" {
            return Err(error_at(
                &line[0],
                "'defined' cannot be used as a macro name",
            ));
        }
        self.macros
            .insert(line[0].spelling.clone(), MacroEntry::Undefined);
        Ok(())
    }

    fn do_include(&mut self, at: &PpToken, include_next: bool) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        let tokens = if starts_header(&line) {
            line
        } else {
            self.expand_list(line)?
        };
        let (name, quote, consumed) = header_from(&tokens, at)?;
        if consumed != tokens.len() {
            return Err(error_at(at, "extra tokens after the file name"));
        }
        if name.is_empty() {
            return Err(error_at(at, "empty file name"));
        }
        let after = if include_next {
            self.files[at.file_id as usize].found_dir.clone()
        } else {
            None
        };
        self.include_file(&name, quote, at, after.as_deref())
    }

    fn do_if(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        let value = self.eval_line(line, at)?;
        self.cond.push(cond_frame(at, value));
        if !value {
            self.skip_false_branch()?;
        }
        Ok(())
    }

    fn do_ifdef(&mut self, at: &PpToken, want_defined: bool) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        if line.len() != 1 || line[0].kind != PpKind::Ident {
            return Err(error_at(at, "expected one identifier"));
        }
        let defined = self.is_defined(&line[0].spelling);
        let value = if want_defined { defined } else { !defined };
        self.cond.push(cond_frame(at, value));
        if !value {
            self.skip_false_branch()?;
        }
        Ok(())
    }

    fn do_elif(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        if self.cond.is_empty() {
            return Err(error_at(at, "#elif without #if"));
        }
        self.reject_after_else("elif", at)?;
        self.skip_rest_of_line();
        self.finish_skip_to_endif()
    }

    fn do_else(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        if self.cond.is_empty() {
            return Err(error_at(at, "#else without #if"));
        }
        self.reject_after_else("else", at)?;
        self.expect_end(at)?;
        self.cond.last_mut().unwrap().else_seen = true;
        self.finish_skip_to_endif()
    }

    fn do_endif(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        if self.cond.is_empty() {
            return Err(error_at(at, "#endif without #if"));
        }
        self.expect_end(at)?;
        self.cond.pop();
        Ok(())
    }

    fn do_error(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        let expanded = self.expand_list(line)?;
        let mut message = String::from("#error");
        for tok in &expanded {
            message.push(' ');
            message.push_str(&tok.spelling);
        }
        Err(error_at(at, message))
    }

    fn do_line(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        let expanded = self.expand_list(line)?;
        if expanded.is_empty() {
            return Err(error_at(at, "expected a line number"));
        }
        let number = line_number(&expanded[0])?;
        let mut index = 1;
        let new_file = if index < expanded.len() && expanded[index].kind == PpKind::String {
            let name = decode_string(&expanded[index])?;
            index += 1;
            Some(name)
        } else {
            None
        };
        if index != expanded.len() {
            return Err(error_at(&expanded[index], "extra tokens in #line"));
        }
        self.apply_line(at.file_id, number, new_file);
        Ok(())
    }

    fn do_pragma(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let line = self.take_line()?;
        if line.first().is_some_and(|tok| tok.spelling == "once") {
            if line.len() != 1 {
                return Err(error_at(&line[1], "extra tokens after #pragma once"));
            }
            let canonical = self.files[at.file_id as usize].canonical.clone();
            self.once.insert(canonical);
        }
        Ok(())
    }

    fn apply_line(&mut self, file_id: u32, number: usize, new_file: Option<String>) {
        if self.index >= self.tokens.len() {
            return;
        }
        let physical = self.tokens[self.index].line as isize;
        let delta = number as isize - physical;
        for tok in &mut self.tokens[self.index..] {
            if tok.file_id != file_id {
                continue;
            }
            let adjusted = tok.line as isize + delta;
            tok.line = if adjusted < 1 { 1 } else { adjusted as usize };
            if let Some(name) = &new_file {
                tok.display = name.clone();
            }
        }
    }

    fn include_file(
        &mut self,
        name: &str,
        quote: bool,
        at: &PpToken,
        after: Option<&Path>,
    ) -> Result<(), Diagnostic> {
        let (path, found_dir) = self.search(name, quote, at, after)?;
        let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if self.once.contains(&canonical) {
            return Ok(());
        }
        let depth = self.files[at.file_id as usize].depth + 1;
        if depth > self.max_depth {
            return Err(error_at(at, "include nested too deeply"));
        }
        let text = read_source(&path, at)?;
        let file_id = self.files.len() as u32;
        let display = path.display().to_string();
        self.files.push(FileInfo {
            real_path: path,
            canonical,
            depth,
            found_dir: Some(found_dir),
        });
        let mut included = lex_source(&text, &display, file_id)?;
        // A header that does not end in a new-line must not continue into the
        // next line of the file that included it. Clang treats that end of
        // file as the end of the line.
        if let Some(last) = included.last() {
            if last.kind != PpKind::Newline {
                let line = last.line;
                let column = last.column.saturating_add(last.spelling.len());
                included.push(PpToken {
                    kind: PpKind::Newline,
                    spelling: "\n".to_string(),
                    display: display.clone(),
                    file_id,
                    line,
                    column,
                    leading_space: false,
                    hide: hide_empty(),
                });
            }
        }
        self.splice(included);
        Ok(())
    }

    fn search(
        &self,
        name: &str,
        quote: bool,
        at: &PpToken,
        after: Option<&Path>,
    ) -> Result<(PathBuf, PathBuf), Diagnostic> {
        let mut dirs = Vec::new();
        if quote {
            if let Some(dir) = self.files[at.file_id as usize].real_path.parent() {
                dirs.push(dir.to_path_buf());
            }
        }
        dirs.extend(self.include_dirs.iter().cloned());
        dirs.extend(self.system_dirs.iter().cloned());
        // `#include_next` resumes after the last copy of the directory that
        // provided the current file. If that directory is not in this search
        // list, the directive behaves like `#include`.
        let start = match after {
            Some(found) => dirs
                .iter()
                .rposition(|dir| dir == found)
                .map_or(0, |index| index + 1),
            None => 0,
        };
        for dir in dirs.into_iter().skip(start) {
            let path = dir.join(name);
            if path.is_file() {
                return Ok((path, dir));
            }
        }
        Err(error_at(at, format!("'{name}' file not found")))
    }

    fn skip_false_branch(&mut self) -> Result<(), Diagnostic> {
        loop {
            match self.skip_until_clause()? {
                Clause::Elif => {
                    let at = self.directive_token();
                    self.reject_after_else("elif", &at)?;
                    let seen = self.cond.last().unwrap().seen_true;
                    if seen {
                        self.skip_rest_of_line();
                        continue;
                    }
                    let line = self.take_line()?;
                    let value = self.eval_line(line, &at)?;
                    let frame = self.cond.last_mut().unwrap();
                    frame.seen_true = value;
                    frame.active = value;
                    if value {
                        return Ok(());
                    }
                }
                Clause::Else => {
                    let at = self.directive_token();
                    self.reject_after_else("else", &at)?;
                    self.expect_end(&at)?;
                    let frame = self.cond.last_mut().unwrap();
                    frame.else_seen = true;
                    if !frame.seen_true {
                        frame.active = true;
                        frame.seen_true = true;
                        return Ok(());
                    }
                }
                Clause::Endif => {
                    let at = cond_token(self.cond.last().unwrap());
                    self.expect_end(&at)?;
                    self.cond.pop();
                    return Ok(());
                }
            }
        }
    }

    fn finish_skip_to_endif(&mut self) -> Result<(), Diagnostic> {
        loop {
            match self.skip_until_clause()? {
                Clause::Elif => {
                    let at = self.directive_token();
                    self.reject_after_else("elif", &at)?;
                    self.skip_rest_of_line();
                }
                Clause::Else => {
                    let at = self.directive_token();
                    self.reject_after_else("else", &at)?;
                    self.cond.last_mut().unwrap().else_seen = true;
                    self.skip_rest_of_line();
                }
                Clause::Endif => {
                    let at = cond_token(self.cond.last().unwrap());
                    self.expect_end(&at)?;
                    self.cond.pop();
                    return Ok(());
                }
            }
        }
    }

    fn skip_until_clause(&mut self) -> Result<Clause, Diagnostic> {
        let mut depth = 0i32;
        let mut nested_else = Vec::new();
        loop {
            self.skip_to_hash()?;
            self.index += 1;
            if self.line_done() {
                self.consume_newline();
                continue;
            }
            let name_tok = self.tokens[self.index].clone();
            if name_tok.kind != PpKind::Ident {
                self.skip_rest_of_line();
                continue;
            }
            let name = name_tok.spelling.clone();
            self.index += 1;
            match name.as_str() {
                "if" | "ifdef" | "ifndef" => {
                    depth += 1;
                    nested_else.push(false);
                    self.skip_rest_of_line();
                }
                "elif" if depth == 0 => return Ok(Clause::Elif),
                "elif" => {
                    if nested_else.last() == Some(&true) {
                        return Err(error_at(&name_tok, "#elif after #else"));
                    }
                    self.skip_rest_of_line();
                }
                "else" if depth == 0 => return Ok(Clause::Else),
                "else" => {
                    if nested_else.last() == Some(&true) {
                        return Err(error_at(&name_tok, "#else after #else"));
                    }
                    if let Some(seen) = nested_else.last_mut() {
                        *seen = true;
                    }
                    self.skip_rest_of_line();
                }
                "endif" if depth == 0 => return Ok(Clause::Endif),
                "endif" => {
                    depth -= 1;
                    nested_else.pop();
                    self.skip_rest_of_line();
                }
                _ => self.skip_rest_of_line(),
            }
        }
    }

    fn directive_token(&self) -> PpToken {
        self.tokens
            .get(self.index.saturating_sub(1))
            .cloned()
            .unwrap_or_else(|| cond_token(self.cond.last().unwrap()))
    }

    fn reject_after_else(&self, kind: &str, at: &PpToken) -> Result<(), Diagnostic> {
        if self.cond.last().is_some_and(|frame| frame.else_seen) {
            return Err(error_at(at, format!("#{kind} after #else")));
        }
        Ok(())
    }

    fn skip_to_hash(&mut self) -> Result<(), Diagnostic> {
        loop {
            if self.index >= self.tokens.len() {
                let frame = self.cond.last().unwrap();
                return Err(Diagnostic::new(
                    &frame.display,
                    frame.line,
                    frame.column,
                    "unterminated conditional directive",
                ));
            }
            if self.at_directive() {
                return Ok(());
            }
            self.index += 1;
        }
    }

    fn eval_line(&mut self, tokens: Vec<PpToken>, at: &PpToken) -> Result<bool, Diagnostic> {
        let replaced = replace_defined(&tokens, |name| self.is_defined(name))?;
        let expanded = self.expand_list(replaced)?;
        let zeroed = replace_idents(expanded);
        if zeroed.is_empty() {
            return Err(error_at(at, "expected an expression"));
        }
        let expr = parse_if_expr(&zeroed)?;
        let value = eval_expr(&expr)?;
        Ok(value.bits != 0)
    }

    fn install(&mut self, mac: Macro, at: &PpToken, force: bool) -> Result<(), Diagnostic> {
        if !force {
            if let Some(MacroEntry::Defined(previous)) = self.macros.get(&mac.name) {
                if !same_macro(previous, &mac) {
                    return Err(error_at(at, format!("macro '{}' redefined", mac.name)));
                }
                return Ok(());
            }
        }
        self.macros
            .insert(mac.name.clone(), MacroEntry::Defined(mac));
        Ok(())
    }

    fn macro_from_parts(
        &self,
        name: &str,
        params: Option<Vec<String>>,
        variadic: bool,
        body: Vec<PpToken>,
        at: Option<&PpToken>,
    ) -> Result<Macro, Diagnostic> {
        let body = normalize_body(body);
        if let Some(at) = at {
            validate_body(&body, params.as_deref(), variadic, at)?;
        } else {
            let marker = command_token();
            validate_body(&body, params.as_deref(), variadic, &marker)?;
        }
        Ok(Macro {
            name: name.to_string(),
            params,
            variadic,
            body,
        })
    }

    fn take_line(&mut self) -> Result<Vec<PpToken>, Diagnostic> {
        let mut tokens = Vec::new();
        while self.index < self.tokens.len() && self.tokens[self.index].kind != PpKind::Newline {
            tokens.push(self.tokens[self.index].clone());
            self.index += 1;
        }
        self.consume_newline();
        Ok(tokens)
    }

    fn skip_rest_of_line(&mut self) {
        while self.index < self.tokens.len() && self.tokens[self.index].kind != PpKind::Newline {
            self.index += 1;
        }
        self.consume_newline();
    }

    fn expect_end(&mut self, at: &PpToken) -> Result<(), Diagnostic> {
        let rest = self.take_line()?;
        if rest.is_empty() {
            Ok(())
        } else {
            Err(error_at(at, "extra tokens at the end of this directive"))
        }
    }

    fn line_done(&self) -> bool {
        self.index >= self.tokens.len() || self.tokens[self.index].kind == PpKind::Newline
    }

    fn consume_newline(&mut self) {
        if self.index < self.tokens.len() && self.tokens[self.index].kind == PpKind::Newline {
            self.index += 1;
        }
    }
}

fn cond_frame(at: &PpToken, value: bool) -> Cond {
    Cond {
        active: value,
        seen_true: value,
        else_seen: false,
        display: at.display.clone(),
        line: at.line,
        column: at.column,
    }
}

fn cond_token(frame: &Cond) -> PpToken {
    PpToken {
        kind: PpKind::Punct,
        spelling: "#".to_string(),
        display: frame.display.clone(),
        file_id: 0,
        line: frame.line,
        column: frame.column,
        leading_space: false,
        hide: hide_empty(),
    }
}

fn command_token() -> PpToken {
    PpToken {
        kind: PpKind::Ident,
        spelling: String::new(),
        display: "<command-line>".to_string(),
        file_id: 0,
        line: 1,
        column: 1,
        leading_space: false,
        hide: hide_empty(),
    }
}

fn special_token(tok: &PpToken) -> PpToken {
    let mut repl = tok.clone();
    repl.hide = hide_empty();
    repl.leading_space = tok.leading_space;
    if tok.spelling == "__LINE__" {
        repl.kind = PpKind::Number;
        repl.spelling = tok.line.to_string();
    } else {
        repl.kind = PpKind::String;
        repl.spelling = format!("\"{}\"", escape_string(&tok.display));
    }
    repl
}

fn escape_string(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if ch == '\\' || ch == '"' {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn retarget(tokens: &mut [PpToken], inv: &PpToken) {
    for tok in tokens {
        tok.display = inv.display.clone();
        tok.file_id = inv.file_id;
        tok.line = inv.line;
        tok.column = inv.column;
    }
}

fn paint(tokens: &mut [PpToken], base: &Hide, name: &str) {
    let extra = hide_with(base, name);
    for tok in tokens {
        tok.hide = hide_union(&tok.hide, &extra);
    }
}

fn paren_closes(tokens: &[PpToken]) -> bool {
    let mut depth = 0i32;
    for tok in tokens {
        if tok.is_lparen() {
            depth += 1;
        } else if tok.is_rparen() {
            depth -= 1;
            if depth == 0 {
                return true;
            }
        }
    }
    false
}

fn param_uses(
    body: &[PpToken],
    raw: &HashMap<String, Vec<PpToken>>,
) -> (HashSet<String>, HashSet<String>) {
    let mut need_raw = HashSet::new();
    let mut need_exp = HashSet::new();
    for (index, tok) in body.iter().enumerate() {
        if tok.kind != PpKind::Ident || !raw.contains_key(&tok.spelling) {
            continue;
        }
        let stringified =
            index > 0 && body[index - 1].spelling == "#" && !hashhash_operand(body, index - 1);
        if hashhash_operand(body, index) || stringified {
            need_raw.insert(tok.spelling.clone());
        } else {
            need_exp.insert(tok.spelling.clone());
        }
    }
    (need_raw, need_exp)
}

fn hashhash_operand(body: &[PpToken], index: usize) -> bool {
    (index + 1 < body.len() && body[index + 1].spelling == "##")
        || (index > 0 && body[index - 1].spelling == "##")
}

fn stringify(arg: &[PpToken], at: &PpToken) -> PpToken {
    let mut text = String::new();
    let mut first = true;
    for tok in arg {
        if tok.kind == PpKind::Placemarker {
            continue;
        }
        if !first && tok.leading_space {
            text.push(' ');
        }
        first = false;
        for ch in tok.spelling.chars() {
            if ch == '\\' || ch == '"' {
                text.push('\\');
            }
            text.push(ch);
        }
    }
    PpToken {
        kind: PpKind::String,
        spelling: format!("\"{text}\""),
        display: at.display.clone(),
        file_id: at.file_id,
        line: at.line,
        column: at.column,
        leading_space: at.leading_space,
        hide: hide_empty(),
    }
}

fn placemarker(at: &PpToken) -> PpToken {
    PpToken {
        kind: PpKind::Placemarker,
        spelling: String::new(),
        display: at.display.clone(),
        file_id: at.file_id,
        line: at.line,
        column: at.column,
        leading_space: at.leading_space,
        hide: hide_empty(),
    }
}

fn paste_tokens(tokens: &[PpToken], inv: &PpToken) -> Result<Vec<PpToken>, Diagnostic> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if index + 1 < tokens.len() && tokens[index + 1].spelling == "##" {
            if index + 2 >= tokens.len() {
                return Err(error_at(inv, "'##' at the end of a macro replacement list"));
            }
            let mut merged = paste_one(&tokens[index], &tokens[index + 2], inv)?;
            index += 3;
            while index < tokens.len() && tokens[index].spelling == "##" {
                if index + 1 >= tokens.len() {
                    return Err(error_at(inv, "'##' at the end of a macro replacement list"));
                }
                merged = paste_one(&merged, &tokens[index + 1], inv)?;
                index += 2;
            }
            out.push(merged);
            continue;
        }
        if tokens[index].spelling == "##" {
            return Err(error_at(
                inv,
                "'##' at the beginning of a macro replacement list",
            ));
        }
        out.push(tokens[index].clone());
        index += 1;
    }
    Ok(out)
}

fn paste_one(left: &PpToken, right: &PpToken, inv: &PpToken) -> Result<PpToken, Diagnostic> {
    if left.kind == PpKind::Placemarker && right.kind == PpKind::Placemarker {
        return Ok(placemarker(left));
    }
    if left.kind == PpKind::Placemarker {
        let mut tok = right.clone();
        tok.leading_space = left.leading_space;
        return Ok(tok);
    }
    if right.kind == PpKind::Placemarker {
        return Ok(left.clone());
    }
    let spelling = format!("{}{}", left.spelling, right.spelling);
    let mut lexed = lex_source(&spelling, &left.display, left.file_id)?;
    lexed.retain(|tok| tok.kind != PpKind::Newline);
    if lexed.len() != 1 {
        return Err(error_at(
            inv,
            format!("pasting '{spelling}' does not produce a valid preprocessing token"),
        ));
    }
    let mut tok = lexed.remove(0);
    tok.line = left.line;
    tok.column = left.column;
    tok.display = left.display.clone();
    tok.file_id = left.file_id;
    tok.leading_space = left.leading_space;
    tok.hide = hide_union(&left.hide, &right.hide);
    Ok(tok)
}

fn join_va(parts: &[Part], named: usize) -> Vec<PpToken> {
    let mut out = Vec::new();
    for (index, part) in parts.iter().enumerate().skip(named) {
        if index > named {
            if let Some(comma) = &part.comma {
                out.push(comma.clone());
            }
        }
        out.extend(part.tokens.iter().cloned());
    }
    out
}

fn arity_error(inv: &PpToken, name: &str, got: usize, need: usize, variadic: bool) -> Diagnostic {
    let message = if variadic {
        format!("macro '{name}' requires at least {need} argument(s) but {got} were provided")
    } else if got < need {
        format!("macro '{name}' requires {need} argument(s) but {got} were provided")
    } else {
        format!("macro '{name}' passed {got} arguments but takes {need}")
    };
    error_at(inv, message)
}

fn replace_defined(
    tokens: &[PpToken],
    defined: impl Fn(&str) -> bool,
) -> Result<Vec<PpToken>, Diagnostic> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index].kind == PpKind::Ident && tokens[index].spelling == "defined" {
            let at = &tokens[index];
            index += 1;
            let name = if index < tokens.len() && tokens[index].is_lparen() {
                if index + 2 >= tokens.len()
                    || tokens[index + 1].kind != PpKind::Ident
                    || !tokens[index + 2].is_rparen()
                {
                    return Err(error_at(at, "operator 'defined' requires an identifier"));
                }
                let name = tokens[index + 1].spelling.clone();
                index += 3;
                name
            } else if index < tokens.len() && tokens[index].kind == PpKind::Ident {
                let name = tokens[index].spelling.clone();
                index += 1;
                name
            } else {
                return Err(error_at(at, "operator 'defined' requires an identifier"));
            };
            out.push(number_token(at, if defined(&name) { "1" } else { "0" }));
        } else {
            out.push(tokens[index].clone());
            index += 1;
        }
    }
    Ok(out)
}

fn number_token(at: &PpToken, spelling: &str) -> PpToken {
    PpToken {
        kind: PpKind::Number,
        spelling: spelling.to_string(),
        display: at.display.clone(),
        file_id: at.file_id,
        line: at.line,
        column: at.column,
        leading_space: at.leading_space,
        hide: hide_empty(),
    }
}

fn replace_idents(tokens: Vec<PpToken>) -> Vec<PpToken> {
    tokens
        .into_iter()
        .map(|mut tok| {
            if tok.kind == PpKind::Ident {
                tok.kind = PpKind::Number;
                tok.spelling = "0".to_string();
                tok.hide = hide_empty();
            }
            tok
        })
        .collect()
}

fn starts_header(tokens: &[PpToken]) -> bool {
    tokens
        .first()
        .is_some_and(|tok| tok.kind == PpKind::String || tok.spelling == "<")
}

fn header_from(tokens: &[PpToken], at: &PpToken) -> Result<(String, bool, usize), Diagnostic> {
    if tokens.is_empty() {
        return Err(error_at(at, "expected a file name"));
    }
    if tokens[0].kind == PpKind::String {
        return Ok((decode_string(&tokens[0])?, true, 1));
    }
    if tokens[0].spelling == "<" {
        let mut name = String::new();
        let mut index = 1;
        while index < tokens.len() {
            if tokens[index].kind == PpKind::Punct && tokens[index].spelling == ">" {
                return Ok((name, false, index + 1));
            }
            if tokens[index].leading_space {
                name.push(' ');
            }
            name.push_str(&tokens[index].spelling);
            index += 1;
        }
        return Err(error_at(at, "missing '>' after the file name"));
    }
    Err(error_at(at, "expected \"filename\" or <filename>"))
}

fn decode_string(tok: &PpToken) -> Result<String, Diagnostic> {
    match crate::lexer::lex(&tok.spelling, &tok.display) {
        Ok(mut tokens) => {
            tokens.pop();
            if tokens.len() != 1 {
                return Err(error_at(tok, "invalid string literal"));
            }
            let bytes = tokens[0].bytes.clone().unwrap_or_default();
            String::from_utf8(bytes).map_err(|_| error_at(tok, "file name is not valid UTF-8"))
        }
        Err(mut err) => {
            err.filename = tok.display.clone();
            err.line = tok.line;
            err.column = tok.column;
            Err(err)
        }
    }
}

fn line_number(tok: &PpToken) -> Result<usize, Diagnostic> {
    let value = literal_val(tok)?;
    if value.unsigned {
        if value.bits == 0 || value.bits > 2_147_483_647 {
            return Err(error_at(tok, "line number is out of range"));
        }
        Ok(value.bits as usize)
    } else {
        let signed = value.bits as i64;
        if !(1..=2_147_483_647).contains(&signed) {
            return Err(error_at(tok, "line number is out of range"));
        }
        Ok(signed as usize)
    }
}

fn read_source(path: &Path, at: &PpToken) -> Result<String, Diagnostic> {
    match std::fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes)
            .map_err(|_| error_at(at, format!("{} is not valid UTF-8", path.display()))),
        Err(err) => Err(error_at(
            at,
            format!("cannot read {}: {err}", path.display()),
        )),
    }
}

struct ParsedFn {
    params: Option<Vec<String>>,
    variadic: bool,
    body: Vec<PpToken>,
}

fn parse_function_header(rest: &[PpToken]) -> Result<ParsedFn, Diagnostic> {
    let mut index = 1;
    let mut params = Vec::new();
    let mut variadic = false;
    if index < rest.len() && rest[index].is_rparen() {
        index += 1;
        return Ok(ParsedFn {
            params: Some(params),
            variadic: false,
            body: rest[index..].to_vec(),
        });
    }
    loop {
        if index >= rest.len() {
            return Err(error_at(
                rest.last().unwrap(),
                "missing ')' in macro parameter list",
            ));
        }
        if rest[index].spelling == "..." && rest[index].kind == PpKind::Punct {
            variadic = true;
            index += 1;
            if index >= rest.len() || !rest[index].is_rparen() {
                return Err(error_at(&rest[index - 1], "expected ')' after '...'"));
            }
            index += 1;
            break;
        }
        if rest[index].kind != PpKind::Ident {
            return Err(error_at(&rest[index], "expected a parameter name"));
        }
        let name = rest[index].spelling.clone();
        if name == "__VA_ARGS__" {
            return Err(error_at(
                &rest[index],
                "'__VA_ARGS__' is not a parameter name",
            ));
        }
        if params.iter().any(|param| param == &name) {
            return Err(error_at(
                &rest[index],
                format!("duplicate macro parameter '{name}'"),
            ));
        }
        params.push(name);
        index += 1;
        if index >= rest.len() {
            return Err(error_at(
                rest.last().unwrap(),
                "missing ')' in macro parameter list",
            ));
        }
        if rest[index].is_rparen() {
            index += 1;
            break;
        }
        if rest[index].is_comma() {
            index += 1;
            if index < rest.len() && rest[index].spelling == "..." {
                continue;
            }
            continue;
        }
        return Err(error_at(
            &rest[index],
            "expected ',' or ')' in macro parameter list",
        ));
    }
    Ok(ParsedFn {
        params: Some(params),
        variadic,
        body: rest[index..].to_vec(),
    })
}

fn normalize_body(mut body: Vec<PpToken>) -> Vec<PpToken> {
    if let Some(first) = body.first_mut() {
        first.leading_space = false;
    }
    for tok in &mut body {
        tok.hide = hide_empty();
    }
    body
}

fn validate_body(
    body: &[PpToken],
    params: Option<&[String]>,
    variadic: bool,
    at: &PpToken,
) -> Result<(), Diagnostic> {
    if body.first().is_some_and(|tok| tok.spelling == "##") {
        return Err(error_at(
            at,
            "'##' at the beginning of a macro replacement list",
        ));
    }
    if body.last().is_some_and(|tok| tok.spelling == "##") {
        return Err(error_at(at, "'##' at the end of a macro replacement list"));
    }
    let Some(params) = params else {
        return Ok(());
    };
    for (index, tok) in body.iter().enumerate() {
        if tok.spelling != "#" || hashhash_operand(body, index) {
            continue;
        }
        let Some(next) = body.get(index + 1) else {
            return Err(error_at(at, "'#' is not followed by a macro parameter"));
        };
        let ok = params.iter().any(|name| name == &next.spelling)
            || (variadic && next.spelling == "__VA_ARGS__");
        if !ok {
            return Err(error_at(at, "'#' is not followed by a macro parameter"));
        }
    }
    Ok(())
}

fn same_macro(left: &Macro, right: &Macro) -> bool {
    left.params == right.params
        && left.variadic == right.variadic
        && same_body(&left.body, &right.body)
}

fn same_body(left: &[PpToken], right: &[PpToken]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    for (index, (a, b)) in left.iter().zip(right).enumerate() {
        if a.kind != b.kind || a.spelling != b.spelling {
            return false;
        }
        if index > 0 && a.leading_space != b.leading_space {
            return false;
        }
    }
    true
}

fn body_tokens(source: &str) -> Result<Vec<PpToken>, Diagnostic> {
    let mut tokens = lex_source(source, "<built-in>", 0)?;
    tokens.retain(|tok| tok.kind != PpKind::Newline);
    Ok(tokens)
}

fn is_ident_text(text: &str) -> bool {
    let mut chars = text.bytes();
    match chars.next() {
        Some(b) if b.is_ascii_alphabetic() || b == b'_' => {}
        _ => return false,
    }
    chars.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn literal_val(tok: &PpToken) -> Result<Val, Diagnostic> {
    // A prefixed character constant has the value and type it has in
    // program text (C11 6.10.1p4): `L'\400'` is the `wchar_t` 256, and
    // `U'\xffffffff'` is an unsigned value.
    match crate::lexer::lex(&tok.spelling, &tok.display) {
        Ok(mut tokens) => {
            tokens.pop();
            if tokens.len() == 1 && tokens[0].kind == TokenKind::Float {
                return Err(error_at(
                    tok,
                    "floating constant in preprocessor expression",
                ));
            }
            if tokens.len() != 1 || tokens[0].value.is_none() || tokens[0].int_ty.is_none() {
                return Err(error_at(
                    tok,
                    format!("invalid integer constant '{}'", tok.spelling),
                ));
            }
            Ok(val_from_compiler(&tokens[0]))
        }
        Err(mut err) => {
            err.filename = tok.display.clone();
            err.line = tok.line;
            err.column = tok.column;
            Err(err)
        }
    }
}

fn val_from_compiler(tok: &Token) -> Val {
    let ty = tok.int_ty.unwrap_or(TypeName::Int);
    let raw = tok.value.unwrap_or(0);
    let bits = if ty.is_signed() {
        sign_extend(raw, ty.width()) as u64
    } else {
        raw & ty.mask()
    };
    // `#if` uses `intmax_t` for every integer constant that fits in it, even
    // when the phase-7 type is a narrower unsigned type (`0xFFFFFFFF` is
    // `unsigned int` and still fits). A `u`/`U` suffix, or a value that does
    // not fit in signed `intmax_t`, stays unsigned. A character constant is
    // not scanned for a suffix.
    let spelling = tok.spelling.as_str();
    let number = spelling.as_bytes().first().is_some_and(u8::is_ascii_digit);
    let u_suffix = number && spelling.bytes().any(|byte| byte == b'u' || byte == b'U');
    let too_big = !ty.is_signed() && ty.width() == 64 && bits > i64::MAX as u64;
    Val {
        bits,
        unsigned: u_suffix || too_big,
    }
}

#[derive(Clone, Copy)]
struct Val {
    bits: u64,
    unsigned: bool,
}

impl Val {
    fn signed(value: i64) -> Self {
        Self {
            bits: value as u64,
            unsigned: false,
        }
    }

    fn zero(unsigned: bool) -> Self {
        Self { bits: 0, unsigned }
    }
}

#[derive(Clone, Copy)]
enum Unary {
    Plus,
    Minus,
    BitNot,
    Not,
}

#[derive(Clone, Copy)]
enum Binary {
    Mul,
    Div,
    Mod,
    Add,
    Sub,
    Shl,
    Shr,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    BitAnd,
    BitXor,
    BitOr,
    And,
    Or,
}

struct IfLoc {
    display: String,
    line: usize,
    column: usize,
}

enum IfExpr {
    Num(Val),
    Unary {
        op: Unary,
        expr: Box<IfExpr>,
        loc: IfLoc,
    },
    Binary {
        op: Binary,
        left: Box<IfExpr>,
        right: Box<IfExpr>,
        loc: IfLoc,
    },
    Cond {
        cond: Box<IfExpr>,
        then_expr: Box<IfExpr>,
        else_expr: Box<IfExpr>,
    },
}

struct IfParser<'a> {
    tokens: &'a [PpToken],
    index: usize,
}

fn parse_if_expr(tokens: &[PpToken]) -> Result<IfExpr, Diagnostic> {
    if tokens.is_empty() {
        return Err(Diagnostic::new("input", 1, 1, "expected an expression"));
    }
    let mut parser = IfParser { tokens, index: 0 };
    let expr = parser.parse_cond()?;
    if parser.index != parser.tokens.len() {
        return Err(parser.error("extra tokens in preprocessor expression"));
    }
    Ok(expr)
}

impl IfParser<'_> {
    fn parse_cond(&mut self) -> Result<IfExpr, Diagnostic> {
        let cond = self.parse_or()?;
        if !self.eat("?") {
            return Ok(cond);
        }
        let then_expr = self.parse_cond()?;
        if !self.eat(":") {
            return Err(self.error("expected ':'"));
        }
        let else_expr = self.parse_cond()?;
        Ok(IfExpr::Cond {
            cond: Box::new(cond),
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
        })
    }

    fn parse_or(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_and, &[("||", Binary::Or)])
    }

    fn parse_and(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_bor, &[("&&", Binary::And)])
    }

    fn parse_bor(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_bxor, &[("|", Binary::BitOr)])
    }

    fn parse_bxor(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_band, &[("^", Binary::BitXor)])
    }

    fn parse_band(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_eq, &[("&", Binary::BitAnd)])
    }

    fn parse_eq(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_rel, &[("==", Binary::Eq), ("!=", Binary::Ne)])
    }

    fn parse_rel(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(
            Self::parse_shift,
            &[
                ("<=", Binary::Le),
                (">=", Binary::Ge),
                ("<", Binary::Lt),
                (">", Binary::Gt),
            ],
        )
    }

    fn parse_shift(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_add, &[("<<", Binary::Shl), (">>", Binary::Shr)])
    }

    fn parse_add(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(Self::parse_mul, &[("+", Binary::Add), ("-", Binary::Sub)])
    }

    fn parse_mul(&mut self) -> Result<IfExpr, Diagnostic> {
        self.parse_binary(
            Self::parse_unary,
            &[("*", Binary::Mul), ("/", Binary::Div), ("%", Binary::Mod)],
        )
    }

    fn parse_binary(
        &mut self,
        mut operand: impl FnMut(&mut Self) -> Result<IfExpr, Diagnostic>,
        ops: &[(&str, Binary)],
    ) -> Result<IfExpr, Diagnostic> {
        let mut left = operand(self)?;
        loop {
            let Some(op) = ops.iter().find(|(spelling, _)| self.peek_punct(spelling)) else {
                break;
            };
            let op = *op;
            self.index += 1;
            let loc = self.prev_loc();
            let right = operand(self)?;
            left = IfExpr::Binary {
                op: op.1,
                left: Box::new(left),
                right: Box::new(right),
                loc,
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<IfExpr, Diagnostic> {
        let Some(tok) = self.current().cloned() else {
            return Err(self.error("expected an expression"));
        };
        let op = match tok.spelling.as_str() {
            "+" if tok.kind == PpKind::Punct => Unary::Plus,
            "-" if tok.kind == PpKind::Punct => Unary::Minus,
            "~" if tok.kind == PpKind::Punct => Unary::BitNot,
            "!" if tok.kind == PpKind::Punct => Unary::Not,
            _ => return self.parse_primary(),
        };
        self.index += 1;
        let expr = self.parse_unary()?;
        Ok(IfExpr::Unary {
            op,
            expr: Box::new(expr),
            loc: loc_of(&tok),
        })
    }

    fn parse_primary(&mut self) -> Result<IfExpr, Diagnostic> {
        let Some(tok) = self.current().cloned() else {
            return Err(self.error("expected an expression"));
        };
        if tok.is_lparen() {
            self.index += 1;
            let inner = self.parse_cond()?;
            if !self.eat(")") {
                return Err(self.error("expected ')'"));
            }
            return Ok(inner);
        }
        if tok.kind == PpKind::Number || tok.kind == PpKind::Char {
            self.index += 1;
            return Ok(IfExpr::Num(literal_val(&tok)?));
        }
        Err(error_at(
            &tok,
            format!(
                "invalid token '{}' in preprocessor expression",
                tok.spelling
            ),
        ))
    }

    fn peek_punct(&self, spelling: &str) -> bool {
        self.current()
            .is_some_and(|tok| tok.kind == PpKind::Punct && tok.spelling == spelling)
    }

    fn eat(&mut self, spelling: &str) -> bool {
        if self.peek_punct(spelling) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn current(&self) -> Option<&PpToken> {
        self.tokens.get(self.index)
    }

    fn prev_loc(&self) -> IfLoc {
        let tok = &self.tokens[self.index - 1];
        loc_of(tok)
    }

    fn error(&self, message: &str) -> Diagnostic {
        if let Some(tok) = self.current() {
            error_at(tok, message)
        } else if let Some(tok) = self.tokens.last() {
            error_at(tok, message)
        } else {
            Diagnostic::new("input", 1, 1, message)
        }
    }
}

fn loc_of(tok: &PpToken) -> IfLoc {
    IfLoc {
        display: tok.display.clone(),
        line: tok.line,
        column: tok.column,
    }
}

fn eval_expr(expr: &IfExpr) -> Result<Val, Diagnostic> {
    match expr {
        IfExpr::Num(value) => Ok(*value),
        IfExpr::Unary { op, expr, loc } => {
            let value = eval_expr(expr)?;
            eval_unary(*op, value, loc)
        }
        IfExpr::Binary {
            op,
            left,
            right,
            loc,
        } => match op {
            Binary::And => {
                let left = eval_expr(left)?;
                if left.bits == 0 {
                    Ok(Val::signed(0))
                } else {
                    let right = eval_expr(right)?;
                    Ok(Val::signed(if right.bits != 0 { 1 } else { 0 }))
                }
            }
            Binary::Or => {
                let left = eval_expr(left)?;
                if left.bits != 0 {
                    Ok(Val::signed(1))
                } else {
                    let right = eval_expr(right)?;
                    Ok(Val::signed(if right.bits != 0 { 1 } else { 0 }))
                }
            }
            _ => {
                let left = eval_expr(left)?;
                let right = eval_expr(right)?;
                eval_binary(*op, left, right, loc)
            }
        },
        IfExpr::Cond {
            cond,
            then_expr,
            else_expr,
        } => {
            let cond = eval_expr(cond)?;
            let unsigned = expr_unsigned(then_expr) || expr_unsigned(else_expr);
            let value = if cond.bits != 0 {
                eval_expr(then_expr)?
            } else {
                eval_expr(else_expr)?
            };
            Ok(Val {
                bits: value.bits,
                unsigned,
            })
        }
    }
}

fn expr_unsigned(expr: &IfExpr) -> bool {
    match expr {
        IfExpr::Num(value) => value.unsigned,
        IfExpr::Unary { op: Unary::Not, .. } => false,
        IfExpr::Unary { expr, .. } => expr_unsigned(expr),
        IfExpr::Binary {
            op:
                Binary::Lt
                | Binary::Gt
                | Binary::Le
                | Binary::Ge
                | Binary::Eq
                | Binary::Ne
                | Binary::And
                | Binary::Or,
            ..
        } => false,
        // The result type of a shift is the promoted left operand (C11 6.5.7).
        IfExpr::Binary {
            op: Binary::Shl | Binary::Shr,
            left,
            ..
        } => expr_unsigned(left),
        IfExpr::Binary { left, right, .. } => expr_unsigned(left) || expr_unsigned(right),
        IfExpr::Cond {
            then_expr,
            else_expr,
            ..
        } => expr_unsigned(then_expr) || expr_unsigned(else_expr),
    }
}

fn eval_unary(op: Unary, value: Val, loc: &IfLoc) -> Result<Val, Diagnostic> {
    let _ = loc;
    Ok(match op {
        Unary::Plus => value,
        Unary::Minus => {
            if value.unsigned {
                Val {
                    bits: value.bits.wrapping_neg(),
                    unsigned: true,
                }
            } else {
                Val::signed((value.bits as i64).wrapping_neg())
            }
        }
        Unary::BitNot => Val {
            bits: !value.bits,
            unsigned: value.unsigned,
        },
        Unary::Not => Val::signed(if value.bits == 0 { 1 } else { 0 }),
    })
}

fn eval_binary(op: Binary, left: Val, right: Val, loc: &IfLoc) -> Result<Val, Diagnostic> {
    if matches!(op, Binary::Shl | Binary::Shr) {
        return eval_shift(op, left, right, loc);
    }
    if matches!(
        op,
        Binary::Lt | Binary::Gt | Binary::Le | Binary::Ge | Binary::Eq | Binary::Ne
    ) {
        return Ok(Val::signed(if compare(op, left, right) { 1 } else { 0 }));
    }
    let unsigned = left.unsigned || right.unsigned;
    if matches!(op, Binary::Div | Binary::Mod) && right.bits == 0 {
        return Err(Diagnostic::new(
            &loc.display,
            loc.line,
            loc.column,
            "division by zero in preprocessor expression",
        ));
    }
    let bits = if unsigned {
        let (x, y) = (left.bits, right.bits);
        match op {
            Binary::Mul => x.wrapping_mul(y),
            Binary::Div => x / y,
            Binary::Mod => x % y,
            Binary::Add => x.wrapping_add(y),
            Binary::Sub => x.wrapping_sub(y),
            Binary::BitAnd => x & y,
            Binary::BitXor => x ^ y,
            Binary::BitOr => x | y,
            _ => 0,
        }
    } else {
        let (x, y) = (left.bits as i64, right.bits as i64);
        let signed = match op {
            Binary::Mul => x.wrapping_mul(y),
            Binary::Div => x.wrapping_div(y),
            Binary::Mod => x.wrapping_rem(y),
            Binary::Add => x.wrapping_add(y),
            Binary::Sub => x.wrapping_sub(y),
            Binary::BitAnd => x & y,
            Binary::BitXor => x ^ y,
            Binary::BitOr => x | y,
            _ => 0,
        };
        signed as u64
    };
    Ok(Val { bits, unsigned })
}

fn compare(op: Binary, left: Val, right: Val) -> bool {
    if left.unsigned || right.unsigned {
        match op {
            Binary::Lt => left.bits < right.bits,
            Binary::Gt => left.bits > right.bits,
            Binary::Le => left.bits <= right.bits,
            Binary::Ge => left.bits >= right.bits,
            Binary::Eq => left.bits == right.bits,
            Binary::Ne => left.bits != right.bits,
            _ => false,
        }
    } else {
        let (x, y) = (left.bits as i64, right.bits as i64);
        match op {
            Binary::Lt => x < y,
            Binary::Gt => x > y,
            Binary::Le => x <= y,
            Binary::Ge => x >= y,
            Binary::Eq => x == y,
            Binary::Ne => x != y,
            _ => false,
        }
    }
}

fn eval_shift(op: Binary, left: Val, right: Val, loc: &IfLoc) -> Result<Val, Diagnostic> {
    let count = if right.unsigned {
        right.bits
    } else {
        let count = right.bits as i64;
        if count < 0 {
            return Err(Diagnostic::new(
                &loc.display,
                loc.line,
                loc.column,
                "negative shift count in preprocessor expression",
            ));
        }
        count as u64
    };
    if count >= 64 {
        return Ok(Val::zero(left.unsigned));
    }
    let bits = if left.unsigned {
        match op {
            Binary::Shl => left.bits.wrapping_shl(count as u32),
            _ => left.bits.wrapping_shr(count as u32),
        }
    } else {
        let value = left.bits as i64;
        match op {
            Binary::Shl => value.wrapping_shl(count as u32) as u64,
            _ => value.wrapping_shr(count as u32) as u64,
        }
    };
    Ok(Val {
        bits,
        unsigned: left.unsigned,
    })
}

struct Scanner<'a> {
    bytes: &'a [u8],
    index: usize,
    line: usize,
    column: usize,
    display: &'a str,
    file_id: u32,
    pending_space: bool,
    tokens: Vec<PpToken>,
}

fn lex_source(source: &str, display: &str, file_id: u32) -> Result<Vec<PpToken>, Diagnostic> {
    let mut scanner = Scanner {
        bytes: source.as_bytes(),
        index: 0,
        line: 1,
        column: 1,
        display,
        file_id,
        pending_space: false,
        tokens: Vec::new(),
    };
    scanner.run()?;
    Ok(scanner.tokens)
}

impl Scanner<'_> {
    fn run(&mut self) -> Result<(), Diagnostic> {
        while self.skip_splices() {
            if self.index >= self.bytes.len() {
                break;
            }
            let line = self.line;
            let column = self.column;
            let byte = self.bytes[self.index];
            if byte >= 128 {
                return self.fail(line, column, "non-ASCII characters are not supported");
            }
            if byte == b' ' || byte == b'\t' || byte == 0x0B || byte == 0x0C {
                self.bump();
                self.pending_space = true;
                continue;
            }
            if byte == b'\n' || byte == b'\r' {
                self.bump();
                self.push_newline(line, column);
                continue;
            }
            if byte == b'/' && self.peek_logical_at(1) == Some(b'/') {
                self.line_comment()?;
                continue;
            }
            if byte == b'/' && self.peek_logical_at(1) == Some(b'*') {
                self.block_comment(line, column)?;
                continue;
            }
            if is_ident_start(byte) {
                self.ident(line, column)?;
                continue;
            }
            if byte.is_ascii_digit()
                || (byte == b'.' && self.peek_logical_at(1).is_some_and(|b| b.is_ascii_digit()))
            {
                self.number(line, column);
                continue;
            }
            if byte == b'"' {
                self.string_or_char(line, column, true)?;
                continue;
            }
            if byte == b'\'' {
                self.string_or_char(line, column, false)?;
                continue;
            }
            if let Some(punct) = self.match_punct() {
                let spelling = canonical_punct(punct);
                let len = punct.len();
                for _ in 0..len {
                    self.next_logical();
                }
                self.push(PpKind::Punct, spelling, line, column);
                continue;
            }
            self.bump();
            self.push(PpKind::Other, &(byte as char).to_string(), line, column);
        }
        Ok(())
    }

    fn line_comment(&mut self) -> Result<(), Diagnostic> {
        self.next_logical();
        self.next_logical();
        while let Some(byte) = self.peek_byte_logical() {
            if byte == b'\n' || byte == b'\r' {
                break;
            }
            self.next_logical();
        }
        self.pending_space = true;
        Ok(())
    }

    fn block_comment(&mut self, line: usize, column: usize) -> Result<(), Diagnostic> {
        self.next_logical();
        self.next_logical();
        loop {
            let Some(byte) = self.peek_byte_logical() else {
                return self.fail(line, column, "unterminated block comment");
            };
            if byte == b'*' && self.peek_logical_at(1) == Some(b'/') {
                self.next_logical();
                self.next_logical();
                self.pending_space = true;
                return Ok(());
            }
            self.next_logical();
        }
    }

    fn ident(&mut self, line: usize, column: usize) -> Result<(), Diagnostic> {
        let mut spelling = String::new();
        while self.peek_byte_logical().is_some_and(is_ident_continue) {
            spelling.push(self.next_logical().unwrap().0 as char);
        }
        if matches!(spelling.as_str(), "L" | "u" | "U" | "u8") {
            if self.peek_byte_logical() == Some(b'"') {
                let mut tok_spelling = self.read_quoted(line, column, true)?;
                tok_spelling.insert_str(0, &spelling);
                self.push(PpKind::String, &tok_spelling, line, column);
                return Ok(());
            }
            if self.peek_byte_logical() == Some(b'\'') {
                let mut tok_spelling = self.read_quoted(line, column, false)?;
                tok_spelling.insert_str(0, &spelling);
                self.push(PpKind::Char, &tok_spelling, line, column);
                return Ok(());
            }
        }
        self.push(PpKind::Ident, &spelling, line, column);
        Ok(())
    }

    fn number(&mut self, line: usize, column: usize) {
        let mut spelling = String::new();
        let (byte, _, _) = self.next_logical().unwrap();
        spelling.push(byte as char);
        loop {
            let Some(byte) = self.peek_byte_logical() else {
                break;
            };
            if byte.is_ascii_digit() || is_ident_continue(byte) || byte == b'.' {
                let (ch, _, _) = self.next_logical().unwrap();
                spelling.push(ch as char);
                if matches!(ch, b'e' | b'E' | b'p' | b'P')
                    && matches!(self.peek_byte_logical(), Some(b'+' | b'-'))
                {
                    spelling.push(self.next_logical().unwrap().0 as char);
                }
                continue;
            }
            break;
        }
        self.push(PpKind::Number, &spelling, line, column);
    }

    fn string_or_char(
        &mut self,
        line: usize,
        column: usize,
        string: bool,
    ) -> Result<(), Diagnostic> {
        let spelling = self.read_quoted(line, column, string)?;
        let kind = if string { PpKind::String } else { PpKind::Char };
        self.push(kind, &spelling, line, column);
        Ok(())
    }

    fn read_quoted(
        &mut self,
        line: usize,
        column: usize,
        string: bool,
    ) -> Result<String, Diagnostic> {
        let closer = if string { b'"' } else { b'\'' };
        // Bytes, not chars: a UTF-8 character inside the literal is copied
        // unchanged and decoded by the phase-7 lexer (C11 5.2.1).
        let mut spelling: Vec<u8> = vec![self.next_logical().unwrap().0];
        loop {
            let Some((byte, _, _)) = self.next_logical() else {
                return self.fail(
                    line,
                    column,
                    if string {
                        "unterminated string literal"
                    } else {
                        "unterminated character constant"
                    },
                );
            };
            if byte == b'\n' || byte == b'\r' {
                return self.fail(
                    line,
                    column,
                    if string {
                        "unterminated string literal"
                    } else {
                        "unterminated character constant"
                    },
                );
            }
            spelling.push(byte);
            if byte == b'\\' {
                let Some((next, _, _)) = self.next_logical() else {
                    return self.fail(
                        line,
                        column,
                        if string {
                            "unterminated string literal"
                        } else {
                            "unterminated character constant"
                        },
                    );
                };
                if next == b'\n' || next == b'\r' {
                    return self.fail(
                        line,
                        column,
                        if string {
                            "unterminated string literal"
                        } else {
                            "unterminated character constant"
                        },
                    );
                }
                spelling.push(next);
                continue;
            }
            if byte == closer {
                break;
            }
        }
        // The source is UTF-8 and only whole characters were copied.
        Ok(String::from_utf8(spelling).expect("UTF-8 literal"))
    }

    fn match_punct(&self) -> Option<&'static str> {
        const PUNCTS: &[&str] = &[
            "%:%:", "<<=", ">>=", "...", "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "++",
            "--", "+=", "-=", "*=", "/=", "%=", "&=", "^=", "|=", "->", "##", "<:", ":>", "<%",
            "%>", "%:", "#", "+", "-", "*", "/", "%", "=", "<", ">", "!", "&", "|", "^", "~", "(",
            ")", "{", "}", "[", "]", ";", ",", "?", ":", ".",
        ];
        PUNCTS
            .iter()
            .find(|punct| self.starts_with(punct.as_bytes()))
            .copied()
    }

    fn starts_with(&self, bytes: &[u8]) -> bool {
        self.peek_logical_n(bytes.len())
            .is_some_and(|peeked| peeked == bytes)
    }

    fn push(&mut self, kind: PpKind, spelling: &str, line: usize, column: usize) {
        let leading_space = self.pending_space;
        self.pending_space = false;
        self.tokens.push(PpToken {
            kind,
            spelling: spelling.to_string(),
            display: self.display.to_string(),
            file_id: self.file_id,
            line,
            column,
            leading_space,
            hide: hide_empty(),
        });
    }

    fn push_newline(&mut self, line: usize, column: usize) {
        self.pending_space = false;
        self.tokens.push(PpToken {
            kind: PpKind::Newline,
            spelling: "\n".to_string(),
            display: self.display.to_string(),
            file_id: self.file_id,
            line,
            column,
            leading_space: false,
            hide: hide_empty(),
        });
    }

    fn fail<T>(&self, line: usize, column: usize, message: &str) -> Result<T, Diagnostic> {
        Err(Diagnostic::new(self.display, line, column, message))
    }

    fn skip_splices(&mut self) -> bool {
        loop {
            if self.index >= self.bytes.len() {
                return false;
            }
            if self.bytes[self.index] == b'\\' && is_newline(self.bytes, self.index + 1) {
                self.bump();
                self.bump();
                continue;
            }
            return true;
        }
    }

    fn next_logical(&mut self) -> Option<(u8, usize, usize)> {
        if !self.skip_splices() {
            return None;
        }
        let line = self.line;
        let column = self.column;
        let byte = self.bump();
        Some((byte, line, column))
    }

    fn peek_byte_logical(&self) -> Option<u8> {
        self.peek_logical_n(1).map(|bytes| bytes[0])
    }

    fn peek_logical_at(&self, offset: usize) -> Option<u8> {
        self.peek_logical_n(offset + 1).map(|bytes| bytes[offset])
    }

    fn peek_logical_n(&self, count: usize) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(count);
        let mut index = self.index;
        while out.len() < count {
            if index >= self.bytes.len() {
                return None;
            }
            if self.bytes[index] == b'\\' && is_newline(self.bytes, index + 1) {
                index += 1;
                index += newline_width(self.bytes, index);
                continue;
            }
            out.push(self.bytes[index]);
            index += 1;
        }
        Some(out)
    }

    fn bump(&mut self) -> u8 {
        let byte = self.bytes[self.index];
        self.index += 1;
        if byte == b'\n' {
            self.line += 1;
            self.column = 1;
        } else if byte == b'\r' {
            if self.index < self.bytes.len() && self.bytes[self.index] == b'\n' {
                self.index += 1;
            }
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        byte
    }
}

fn canonical_punct(punct: &str) -> &str {
    match punct {
        "%:%:" => "##",
        "%:" => "#",
        other => other,
    }
}

fn is_newline(bytes: &[u8], index: usize) -> bool {
    matches!(bytes.get(index), Some(b'\n' | b'\r'))
}

fn newline_width(bytes: &[u8], index: usize) -> usize {
    if bytes.get(index) == Some(&b'\r') && bytes.get(index + 1) == Some(&b'\n') {
        2
    } else {
        1
    }
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pp(source: &str) -> String {
        let mut options = PpOptions::default();
        options.system_dirs.clear();
        preprocess(source, "t.c", &options).unwrap()
    }

    fn pp_err(source: &str) -> String {
        let mut options = PpOptions::default();
        options.system_dirs.clear();
        preprocess(source, "t.c", &options).unwrap_err().to_string()
    }

    fn squash(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn splices_lines_and_drops_comments() {
        let text = pp("int ab\\\nc = 1; // hidden\\\nstill\n/* a \\\nb */ int y;\n");
        assert_eq!(squash(&text), "int abc = 1; int y;");
    }

    #[test]
    fn object_and_function_macros() {
        let text = pp("#define A 2\n#define B A+A\n#define C (A+A)\nint x = B*2;\nint y = C*2;\n#define ID(x) x\n#define ADD(a, b) ((a)+(b))\nint z = ADD(ID(1), 2);\n");
        assert!(text.contains("int x = 2+2*2;"), "{text}");
        assert!(text.contains("int y = (2+2)*2;"), "{text}");
        assert!(text.contains("int z = ((1)+(2));"), "{text}");
    }

    #[test]
    fn hides_recursive_macros_and_rescans() {
        let text = pp("#define x 3\n#define f(a) f(x * (a))\n#undef x\n#define x 2\n#define g f\n#define z z[0]\n#define h g(~\n#define m(a) a(w)\n#define w 0,1\n#define t(a) a\nf(y+1)\nh 5)\nm(f)\n");
        let flat = squash(&text);
        assert!(flat.contains("f(2 * (y+1))"), "{flat}");
        assert!(flat.contains("f(2 * (~ 5))"), "{flat}");
        assert!(flat.contains("f(2 * (0,1))"), "{flat}");
    }

    #[test]
    fn stringifies_and_pastes() {
        let text = pp("#define STR(x) #x\n#define XSTR(x) STR(x)\n#define N 40\nconst char *a = STR(a + b);\nconst char *b = XSTR(N);\nconst char *c = STR();\n#define CAT(a, b) a ## b\n#define CAT3(a, b, c) a ## b ## c\nint CAT(x, y) = CAT3(1, 2, 3);\n");
        assert!(text.contains("\"a + b\""), "{text}");
        assert!(text.contains("\"40\""), "{text}");
        assert!(text.contains("\"\""), "{text}");
        assert!(text.contains("int xy = 123;"), "{text}");
    }

    #[test]
    fn variadic_macros_and_omitted_comma() {
        let text = pp("#define FIRST(a, ...) a\n#define REST(a, ...) __VA_ARGS__\n#define F(a, ...) [a, ## __VA_ARGS__]\nint a = FIRST(3, 4, 5);\nint b = REST(9, 1, 2);\nF(1)\nF(1,)\nF(1, 2, 3)\n");
        assert!(text.contains("int a = 3;"), "{text}");
        assert!(text.contains("int b = 1, 2;"), "{text}");
        assert!(text.contains("[1]"), "{text}");
        assert!(text.contains("[1,]"), "{text}");
        assert!(text.contains("[1, 2, 3]"), "{text}");
    }

    #[test]
    fn conditionals_skip_groups_and_evaluate() {
        let text = pp("#if (1+2)*3 == 9 && 4 < 5\n#define P 1\n#else\n#define P 0\n#endif\n#if -1 < 1u\n#define Q 1\n#else\n#define Q 0\n#endif\n#if 0\n#error no\n#if 1\n#error still no\n#endif\n#elif 0\n#define S 0\n#else\n#define S 1\n#endif\nint x = P+Q+S;\n");
        assert!(text.contains("int x = 1+0+1;"), "{text}");
        assert!(!text.contains("error"), "{text}");
    }

    #[test]
    fn defined_operator_and_undefined_identifiers() {
        let text = pp("#define FOO 3\n#if defined FOO && defined(FOO) && !defined(BAR)\nint a = FOO;\n#endif\n#if unknown\nint b;\n#else\nint c;\n#endif\n");
        assert!(text.contains("int a = 3;"), "{text}");
        assert!(text.contains("int c;"), "{text}");
        assert!(!text.contains("int b;"), "{text}");
    }

    #[test]
    fn line_and_file_macros_follow_the_invocation() {
        let text = pp("int a = __LINE__;\nint b = \\\n__LINE__;\n#define L __LINE__\nint c = L;\n");
        assert!(text.contains("int a = 1;"), "{text}");
        assert!(text.contains("int b = 3;"), "{text}");
        assert!(text.contains("int c = 5;"), "{text}");
        let file = pp("__FILE__\n");
        assert!(file.contains("\"t.c\""), "{file}");
    }

    #[test]
    fn reports_directive_errors() {
        assert!(pp_err("#error boom\n").contains("t.c:1:1: error: #error boom"));
        assert!(pp_err("#if 1\n").contains("unterminated conditional"));
        assert!(pp_err("#else\n").contains("#else without #if"));
        assert!(pp_err("#foo\n").contains("unknown preprocessing directive"));
        assert!(pp_err("#define F(a, b) a\nF(1)\n").contains("requires 2 argument"));
        assert!(pp_err("#define A 1\n#define A 2\n").contains("redefined"));
        assert!(pp("#define A 1\n#define A 1\nint x;\n").contains("int x;"));
        assert!(pp_err("#if 1/0\n#endif\n").contains("division by zero"));
    }

    #[test]
    fn include_guard_and_error_in_header() {
        let dir = std::env::temp_dir().join(format!("cc-pp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(
            dir.join("guard.h"),
            "#ifndef GUARD_H\n#define GUARD_H\nint value;\n#endif\n",
        )
        .unwrap();
        std::fs::write(dir.join("sub").join("bad.h"), "#error from header\n").unwrap();
        let main = dir.join("main.c");
        let source = "#include \"guard.h\"\n#include \"guard.h\"\n#include \"sub/bad.h\"\n";
        let err = preprocess(source, main.to_str().unwrap(), &PpOptions::default())
            .unwrap_err()
            .to_string();
        assert!(err.contains("bad.h"), "{err}");
        assert!(err.contains("#error from header"), "{err}");
        let ok = "#include \"guard.h\"\n#include \"guard.h\"\n";
        let text = preprocess(ok, main.to_str().unwrap(), &PpOptions::default()).unwrap();
        assert_eq!(text.matches("int value;").count(), 1, "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compiler_headers_provide_stdalign_and_max_align_t() {
        let options = PpOptions {
            system_dirs: vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("include")],
            ..PpOptions::default()
        };
        let source = "#include <stdalign.h>\n#include <stddef.h>\n\
            int a = alignof(max_align_t) + __alignas_is_defined + __alignof_is_defined;\n";
        let text = preprocess(source, "t.c", &options).unwrap();
        assert!(text.contains("} max_align_t;"), "{text}");
        assert!(
            squash(&text).contains("int a = _Alignof(max_align_t) + 1 + 1;"),
            "{text}"
        );
    }

    #[test]
    fn command_line_defines_and_angle_include() {
        let dir = std::env::temp_dir().join(format!("cc-pp-inc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("n.h"), "int from_header = NAME;\n").unwrap();
        let mut options = PpOptions {
            system_dirs: vec![dir.clone()],
            ..PpOptions::default()
        };
        options.definitions.push(CliMacro::Define {
            name: "NAME".to_string(),
            replacement: Some("5".to_string()),
        });
        let text = preprocess("#include <n.h>\n", "t.c", &options).unwrap();
        assert!(text.contains("int from_header = 5;"), "{text}");
        options
            .definitions
            .push(CliMacro::Undef("NAME".to_string()));
        let err = preprocess("#include <n.h>\n", "t.c", &options).unwrap();
        assert!(err.contains("int from_header = NAME;"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn predefined_stdc_and_no_recursion_on_line() {
        let text = pp("#if __STDC__ == 1 && __STDC_VERSION__ == 201112L && __x86_64__ && __linux__ && __LP64__\nint ok;\n#endif\n#define ID(x) x\nID(ID)(1)\n");
        assert!(text.contains("int ok;"), "{text}");
        assert!(squash(&text).contains("ID(1)"), "{text}");
    }

    #[test]
    fn review_fixes_match_clang() {
        let glued = pp("#define NEG -1\n1-NEG\n");
        assert!(glued.contains("1- -1"), "{glued}");
        assert!(!glued.contains("1--1"), "{glued}");

        let empty = pp_err("#if\n#endif\n");
        assert!(empty.contains("t.c:1:"), "{empty}");
        assert!(empty.contains("expected an expression"), "{empty}");

        let after_else = pp_err("#if 0\n#else\n#elif 1\n#endif\n");
        assert!(after_else.contains("#elif after #else"), "{after_else}");
        let nested = pp_err("#if 0\n#if 0\n#else\n#else\n#endif\n#endif\n");
        assert!(nested.contains("#else after #else"), "{nested}");

        let hex = pp("#if -1 < 0xFFFFFFFF\nA\n#else\nB\n#endif\n#if -1 < 0x8000000000000000\nG\n#else\nH\n#endif\n#if (1 ? -1 : (1 << 0u)) < 0\nI\n#else\nJ\n#endif\n");
        assert!(hex.contains('A') && !hex.contains('B'), "{hex}");
        assert!(hex.contains('H') && !hex.contains('G'), "{hex}");
        assert!(hex.contains('I') && !hex.contains('J'), "{hex}");

        let dir = std::env::temp_dir().join(format!("cc-pp-nonl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("nonl.h"), "#define X 1").unwrap();
        let main = dir.join("main.c");
        let text = preprocess(
            "#include \"nonl.h\"\nint y = X;\n",
            main.to_str().unwrap(),
            &PpOptions::default(),
        )
        .unwrap();
        assert!(text.contains("int y = 1;"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prefixed_character_constants_in_if() {
        // C11 6.10.1p4: the value and type of `L'\400'` and `U'...'` are those
        // of the constant in program text.
        let text = pp("#if L'\\400' == 256 && u'\\xffff' == 65535 && U'\\xffffffff' > 0\nint ok;\n#endif\n#if L'Ä' == 0xC4 && U'\\U0001F600' == 0x1F600\nint wide;\n#endif\n#if '\\377' < 0\nint neg;\n#endif\n");
        assert_eq!(squash(&text), "int ok; int wide; int neg;");
        assert!(pp_err("#if u'\\x10000'\n#endif\n").contains("out of range"));
        // The character `u` is not an unsigned suffix: `L'u'` is a signed int.
        let text = pp("#if L'u' - 118 < 0 && L'\\u00c4' - 200 < 0\nint signed_wide;\n#endif\n");
        assert_eq!(squash(&text), "int signed_wide;");
        assert!(pp_err("#if 'é'\n#endif\n").contains("character too large"));
    }

    #[test]
    fn utf8_literals_pass_through() {
        let text = pp("const char *s = \"Ä€\"; /* é */ int c = L'é';\n");
        assert_eq!(squash(&text), "const char *s = \"Ä€\"; int c = L'é';");
        assert!(pp_err("int café;\n").contains("non-ASCII"));
    }

    #[test]
    fn conditional_feature_macros() {
        let text = pp("#if __STDC_NO_ATOMICS__ && __STDC_NO_COMPLEX__ && __STDC_NO_THREADS__ && __STDC_NO_VLA__ && __STDC_UTF_16__ && __STDC_UTF_32__\nint ok;\n#endif\n");
        assert_eq!(squash(&text), "int ok;");
    }
}
