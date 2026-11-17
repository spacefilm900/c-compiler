#!/usr/bin/env python3
"""Unit tests for the quality scripts. Run: python3 -m unittest tools/quality/test_quality_tools.py"""
import os, sys, tempfile, unittest
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rust_scan, complexity, crash_points

SAMPLE = '''fn a(x: Option<u8>) -> u8 {
    // x.unwrap() in a comment
    let s = "panic!(\\"no\\")";
    if x.is_some() && true { x.unwrap() } else { 0 }
}

fn b(v: u8) -> u8 {
    match v {
        0 => 1,
        1 => 2,
        _ => unreachable!("v"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn t() { None::<u8>.expect("in a test"); }
}
'''

class QualityTools(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        src = os.path.join(self.dir.name, "src")
        os.mkdir(src)
        with open(os.path.join(src, "lib.rs"), "w") as f:
            f.write("pub mod sample;\n#[cfg(test)]\nmod only_tests;\n")
        with open(os.path.join(src, "sample.rs"), "w") as f:
            f.write(SAMPLE)
        with open(os.path.join(src, "only_tests.rs"), "w") as f:
            f.write("fn z() { None::<u8>.unwrap(); }\n")
        self.src = src

    def tearDown(self):
        self.dir.cleanup()

    def test_strip_keeps_lines_and_blanks_comments_and_strings(self):
        clean = rust_scan.strip(SAMPLE)
        self.assertEqual(clean.count("\n"), SAMPLE.count("\n"))
        self.assertNotIn("in a comment", clean)
        self.assertNotIn("panic!", clean)

    def test_test_spans_and_modules(self):
        clean = rust_scan.strip(SAMPLE)
        spans = rust_scan.test_spans(clean)
        self.assertEqual([(s[0], s[1]) for s in spans], [(15, 19)])
        arr = "fn a() {}\n#[cfg(test)]\nfn fixture() -> [u8; 1] {\n    [0]\n}\nfn b() {}\n"
        self.assertEqual([(s[0], s[1]) for s in rust_scan.test_spans(rust_scan.strip(arr))], [(2, 5)])
        cg = "#[cfg(test)]\nfn fixture() -> Foo<{\n    let n = 1;\n    n\n}> {\n    Foo\n}\nfn after() {}\n"
        self.assertEqual([(s[0], s[1]) for s in rust_scan.test_spans(rust_scan.strip(cg))], [(1, 7)])
        gen = "#[cfg(test)]\nfn g<T: Into<u8>>(x: T) -> Vec<u8> where T: Copy {\n    vec![x.into()]\n}\n"
        self.assertEqual([(s[0], s[1]) for s in rust_scan.test_spans(rust_scan.strip(gen))], [(1, 4)])
        cmp = "#[cfg(test)]\nfn fixture() -> [u8; (1 < 2) as usize] {\n    [0]\n}\nfn after() {}\n"
        self.assertEqual([(s[0], s[1]) for s in rust_scan.test_spans(rust_scan.strip(cmp))], [(1, 4)])
        self.assertEqual(rust_scan.test_only_modules("#[cfg(test)]\nmod only_tests;"), {"only_tests"})

    def test_source_files_follow_module_directories(self):
        nest = os.path.join(self.src, "nest")
        os.mkdir(nest)
        with open(os.path.join(self.src, "lib.rs"), "a") as f:
            f.write("pub mod nest;\n")
        with open(os.path.join(nest, "mod.rs"), "w") as f:
            f.write("mod inner;\n#[cfg(test)]\nmod checks;\n")
        for name in ("inner.rs", "checks.rs"):
            with open(os.path.join(nest, name), "w") as f:
                f.write("fn c(x: Option<u8>) -> u8 { x.unwrap() }\n")
        self.assertEqual(rust_scan.source_files(self.src),
                         ["lib.rs", "nest/inner.rs", "nest/mod.rs", "sample.rs"])
        self.assertIn("nest/checks.rs", rust_scan.source_files(self.src, include_tests=True))
        self.assertEqual(rust_scan.top_module_file("nest/inner.rs"), "nest.rs")
        self.assertEqual(rust_scan.top_module_file("lib.rs"), "lib.rs")
        rows = crash_points.scan(self.src)
        self.assertEqual(sorted(r["file"] for r in rows if r["file"].startswith("src/nest")), ["src/nest/inner.rs"])
        funcs, _ = complexity.scan(self.src)
        self.assertIn(("src/nest/inner.rs", "c"), {(f["file"], f["name"]) for f in funcs})

    def test_complexity(self):
        funcs, files = complexity.scan(self.src)
        by = {f["name"]: f for f in funcs}
        self.assertEqual(set(by), {"a", "b"})
        self.assertEqual(by["a"]["lines"], 5)
        self.assertEqual(by["a"]["cyclomatic"], 3)   # 1 + if + &&
        self.assertEqual(by["b"]["cyclomatic"], 3)   # 1 + (3 arms - 1)

    def test_crash_points_skip_comments_strings_and_tests(self):
        rows = crash_points.scan(self.src)
        self.assertEqual(sorted((r["line"], r["kind"]) for r in rows), [(4, "unwrap"), (11, "unreachable")])

if __name__ == "__main__":
    unittest.main()
