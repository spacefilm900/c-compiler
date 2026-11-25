#!/usr/bin/env python3
"""Check that what the documentation says about the language matches hcc.

Each claim below names a feature, a small program, and whether hcc must
accept it in strict C11 (`-std=c11`, the default) and under `-std=gnu11`.
The program is compiled with build/hcc in both modes and the result must
match. Each claim also lists phrases that must appear in the documents
that describe it, and STALE lists phrases that used to be true and must
not come back. Run from the repository root:

    python3 tools/check_docs.py

Exit status 1 if a claim or a phrase does not hold. CC_UNDER_TEST
overrides build/hcc.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HCC = os.environ.get("CC_UNDER_TEST", str(ROOT / "build" / "hcc"))

# (feature, program, accepted in c11, accepted in gnu11, [(document, phrase)])
CLAIMS = [
    ("range designators", "int a[4] = { [0 ... 3] = 1 };\n", False, True,
     [("docs/LANGUAGE.md", "a GNU range designator (`[0 ... 3]`) in strict C11 (it needs `-std=gnu11`)"),
      ("docs/USER_GUIDE.md", "`[0 ... 3] = x`")]),
    ("case ranges", "int f(int x) { switch (x) { case 1 ... 3: return 1; } return 0; }\n", False, True,
     [("docs/LANGUAGE.md", "Case ranges `case low ... high:`")]),
    ("labels as values", "void f(void) { void *p = &&l; goto *p; l:; }\n", False, True,
     [("docs/LANGUAGE.md", "computed `goto` and `&&label` in strict C11 (they need `-std=gnu11`)")]),
    ("multi-character constants", "int x = 'ab';\n", True, True,
     [("docs/LANGUAGE.md", "A multi-character character constant (`'ab'`, C11 6.4.4.4p10) is an `int`"),
      ("DESIGN.md", "A multi-character character constant such as `'ab'`")]),
    ("multi-character wide constants", "int x = L'ab';\n", False, False,
     [("docs/LANGUAGE.md", "A multi-character `L`, `u` or `U` constant is a diagnostic")]),
    ("universal character names in identifiers", "int \\u00e9t\\u00e9 = 1;\n", True, True, []),
    ("_Complex", "double _Complex z;\n", False, False,
     [("docs/LANGUAGE.md", "`__STDC_NO_COMPLEX__` is predefined")]),
    ("variable-length arrays", "void f(int n) { int a[n]; a[0] = 0; }\n", True, True,
     [("docs/LANGUAGE.md", "variable-length arrays (including variably modified types)")]),
    ("_Atomic and threads", "_Atomic int a; _Thread_local int t;\n", True, True, []),
    ("a label before a declaration", "void f(void) { l: int x; (void)x; }\n", False, False,
     [("docs/LANGUAGE.md", "a label followed by a declaration (`case 1: int x;`)")]),
    ("tentative array definitions", "int t[];\nint main(void) { return t[0]; }\n", True, True, []),
]

# Phrases that described earlier versions and must not reappear.
STALE = [
    ("docs/LANGUAGE.md", "in both modes)"),
    ("docs/LANGUAGE.md", "Multi-character character constants and hex"),
    ("DESIGN.md", "This subset rejects it."),
    ("STATUS.md", "Multi-character character constants (`'ab'`) are rejected"),
    ("STATUS.md", "No variable-length arrays."),
    ("README.md", "actions/checkout` v4"),
]


def accepted(source: str, std: str, tmp: Path) -> bool:
    src = tmp / "claim.c"
    src.write_text(source)
    proc = subprocess.run([HCC, f"-std={std}", str(src), "-o", str(tmp / "claim.ll")],
                          capture_output=True, text=True, timeout=60)
    if "panicked" in proc.stderr:
        raise SystemExit(f"hcc panicked on:\n{source}")
    return proc.returncode == 0


def main() -> int:
    problems = []
    texts: dict[str, str] = {}

    def text(doc: str) -> str:
        if doc not in texts:
            texts[doc] = (ROOT / doc).read_text()
        return texts[doc]

    with tempfile.TemporaryDirectory(prefix="hcc-docs-") as td:
        tmp = Path(td)
        for feature, source, c11, gnu11, phrases in CLAIMS:
            for std, want in (("c11", c11), ("gnu11", gnu11)):
                got = accepted(source, std, tmp)
                if got != want:
                    problems.append(f"{feature}: hcc -std={std} {'accepts' if got else 'rejects'} it, "
                                    f"the documentation says it {'is accepted' if want else 'is rejected'}")
            for doc, phrase in phrases:
                if phrase not in " ".join(text(doc).split()):
                    problems.append(f"{feature}: {doc} no longer says {phrase!r}")
    for doc, phrase in STALE:
        if phrase in " ".join(text(doc).split()):
            problems.append(f"{doc} still says {phrase!r}, which is out of date")
    for problem in problems:
        print("FAIL", problem)
    print(f"docs: {len(CLAIMS)} claims, {len(STALE)} stale phrases, {len(problems)} problems")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
