#!/usr/bin/env python3
"""Shared helpers for the quality scripts: strip Rust comments and string
literals (keeping line structure) and find the `#[cfg(test)]` item spans, so
other scripts can tell non-test code from test code. Standard library only."""
import re

def strip(src):
    """Return src with comments and the contents of string and char literals
    replaced by spaces. Newlines are kept, so line numbers do not move."""
    out = []
    i, n = 0, len(src)
    depth = 0  # block comment nesting
    while i < n:
        c = src[i]
        nxt = src[i + 1] if i + 1 < n else ""
        if depth:
            if c == "/" and nxt == "*":
                depth += 1; out.append("  "); i += 2; continue
            if c == "*" and nxt == "/":
                depth -= 1; out.append("  "); i += 2; continue
            out.append("\n" if c == "\n" else " "); i += 1; continue
        if c == "/" and nxt == "/":
            j = src.find("\n", i)
            j = n if j < 0 else j
            out.append(" " * (j - i)); i = j; continue
        if c == "/" and nxt == "*":
            depth = 1; out.append("  "); i += 2; continue
        # raw strings r"..", r#".."#, br".."
        m = re.match(r'b?r(#*)"', src[i:i + 40])
        if m and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            hashes = m.group(1)
            end = src.find('"' + hashes, i + m.end())
            end = n if end < 0 else end + 1 + len(hashes)
            seg = src[i:end]
            out.append('""' + "".join("\n" if ch == "\n" else " " for ch in seg[2:]))
            i = end; continue
        if c == '"' :
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            seg = src[i:j + 1]
            out.append('"' + "".join("\n" if ch == "\n" else " " for ch in seg[1:-1]) + '"')
            i = j + 1; continue
        if c == "'":
            # char literal vs lifetime
            m = re.match(r"'(\\.[^']*|[^'\\])'", src[i:i + 12])
            if m:
                out.append("'" + " " * (m.end() - 2) + "'"); i += m.end(); continue
        out.append(c); i += 1
    return "".join(out)

def _match_brace(clean, k):
    """Index of the `}` matching the `{` at k (or the end of the text)."""
    d = 0
    while k < len(clean):
        if clean[k] == "{": d += 1
        elif clean[k] == "}":
            d -= 1
            if d == 0: return k
        k += 1
    return len(clean) - 1

def test_spans(clean):
    """Line ranges (1-based, inclusive) of items marked #[cfg(test)]."""
    spans = []
    for m in re.finditer(r"#\[cfg\(test\)\]", clean):
        # The item ends at the first `;` or `{` outside (), [] and generic
        # <>; a `;` inside a signature, as in `-> [u8; 1]`, does not end it,
        # and nor does a const-generic block such as `Foo<{ 1 }>`, which is
        # skipped whole. `->` and `=>` are not angle brackets.
        # A `<` opened inside () or [] (for example the comparison in
        # `[u8; (1 < 2) as usize]`) is forgotten when that bracket closes,
        # so it cannot make the body's `{` look like a const-generic block.
        b = semi = -1
        k, depth, angle, saved = m.end(), 0, 0, []
        while k < len(clean):
            ch = clean[k]
            prev = clean[k - 1] if k else ""
            if ch in "([":
                depth += 1
                saved.append(angle)
            elif ch in ")]":
                depth -= 1
                angle = saved.pop() if saved else 0
            elif ch == "<": angle += 1
            elif ch == ">" and prev not in "-=": angle = max(0, angle - 1)
            elif ch == "{" and (depth or angle):
                k = _match_brace(clean, k)
            elif depth == 0 and ch == ";": semi = k; break
            elif depth == 0 and ch == "{": b = k; break
            k += 1
        if semi != -1:
            # `#[cfg(test)] mod foo;` -- the whole file is test code
            spans.append((clean.count("\n", 0, m.start()) + 1,
                          clean.count("\n", 0, semi) + 1, clean[m.end():semi].split()[-1]))
            continue
        d, j = 0, b
        while j < len(clean):
            if clean[j] == "{": d += 1
            elif clean[j] == "}":
                d -= 1
                if d == 0: break
            j += 1
        spans.append((clean.count("\n", 0, m.start()) + 1, clean.count("\n", 0, j) + 1, None))
    return spans

def test_only_modules(lib_clean):
    """Module names declared as `#[cfg(test)] mod name;` in lib.rs (or any module file)."""
    return set(re.findall(r"#\[cfg\(test\)\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;", lib_clean))

def source_files(src_dir, include_tests=False):
    """The .rs files under src_dir, nested module directories included, as
    sorted paths relative to src_dir ("lib.rs", "sema/decl.rs"). A file whose
    module is declared `#[cfg(test)] mod name;` by its parent module (lib.rs
    at the top; `dir.rs` or `dir/mod.rs` for files in `dir/`), or that lies
    inside such a module, is left out unless include_tests is set."""
    import os
    cache = {}

    def declared_test_mods(parent):
        if parent not in cache:
            path = os.path.join(src_dir, parent)
            cache[parent] = (test_only_modules(strip(open(path).read()))
                             if os.path.exists(path) else set())
        return cache[parent]

    def is_test(rel):
        parts = rel[:-3].split("/")
        if parts[-1] == "mod":
            parts = parts[:-1]
        if not parts or parts == ["lib"] or parts == ["main"]:
            return False
        name, dirs = parts[-1], parts[:-1]
        if not dirs:
            parents = ["lib.rs"]
        else:
            d = "/".join(dirs)
            parents = [d + ".rs", d + "/mod.rs"]
        for parent in parents:
            if name in declared_test_mods(parent):
                return True
        return bool(dirs) and is_test("/".join(dirs) + ".rs")

    out = []
    for root, _, names in os.walk(src_dir):
        for n in names:
            if n.endswith(".rs"):
                rel = os.path.relpath(os.path.join(root, n), src_dir).replace(os.sep, "/")
                if include_tests or not is_test(rel):
                    out.append(rel)
    return sorted(out)

def top_module_file(rel):
    """The file name a nested module path belongs to: "sema/decl.rs" -> "sema.rs"."""
    first = rel.split("/")[0]
    return first if first.endswith(".rs") else first + ".rs"

def in_spans(line, spans):
    return any(a <= line <= b for a, b, _ in spans)
