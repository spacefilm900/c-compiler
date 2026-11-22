"""Unit tests for tools/platform/challenge_native.py (no compiler needed)."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "platform"))
import challenge_native as cn  # noqa: E402


class InstrumentTest(unittest.TestCase):
    def instrument(self, text, sanitize="thread"):
        with tempfile.TemporaryDirectory() as d:
            ll = Path(d) / "x.ll"
            ll.write_text(text)
            runner = cn.Runner.__new__(cn.Runner)
            runner.sanitize = sanitize
            count = runner.instrument(ll)
            return count, ll.read_text()

    def test_every_definition_gets_the_attribute(self):
        text = ("declare i32 @puts(ptr)\n"
                "define i32 @main() {\nentry:\n  ret i32 0\n}\n"
                "define internal void @f(ptr %p0) #0 {\nentry:\n  ret void\n}\n"
                'define i32 @g() "target-features"="+cx16" {\nentry:\n  ret i32 1\n}\n')
        count, out = self.instrument(text)
        self.assertEqual(count, 3)
        self.assertIn("define i32 @main() sanitize_thread {", out)
        self.assertIn("define internal void @f(ptr %p0) sanitize_thread #0 {", out)
        self.assertIn('define i32 @g() sanitize_thread "target-features"="+cx16" {', out)
        self.assertIn("declare i32 @puts(ptr)\n", out)

    def test_a_definition_it_cannot_parse_is_reported(self):
        count, _ = self.instrument("define i32 @main()\n{\n  ret i32 0\n}\n", "address")
        self.assertEqual(count, -1)

    def test_no_sanitizer_leaves_the_file(self):
        count, out = self.instrument("define i32 @main() {\n}\n", None)
        self.assertEqual(count, 0)
        self.assertEqual(out, "define i32 @main() {\n}\n")


class KnownFailuresTest(unittest.TestCase):
    def test_levels_and_reasons(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / "known.txt"
            path.write_text("# comment\n\nHCC09-068 O2  # LLP64 long\nHCC08-038  # no libatomic\n")
            known = cn.load_known(path)
        self.assertEqual(known[("HCC09-068", "O2")], "LLP64 long")
        self.assertNotIn(("HCC09-068", "O0"), known)
        self.assertEqual(known[("HCC08-038", "O0")], "no libatomic")
        self.assertEqual(known[("HCC08-038", "O2")], "no libatomic")


class ReportTest(unittest.TestCase):
    def test_sanitizer_reports_are_recognised(self):
        self.assertTrue(cn.SANITIZER_REPORT.search(b"==1==WARNING: ThreadSanitizer: data race"))
        self.assertTrue(cn.SANITIZER_REPORT.search(b"==1==ERROR: AddressSanitizer: stack-buffer-overflow"))
        self.assertTrue(cn.SANITIZER_REPORT.search(b"t.c:3:5: runtime error: signed integer overflow"))
        self.assertFalse(cn.SANITIZER_REPORT.search(b"ok\n"))

    def test_a_sanitizer_crash_is_not_a_finding(self):
        # What ThreadSanitizer printed for a thread from glibc's thrd_create
        # in the first weekly run.
        crash = (b"ThreadSanitizerThreadSanitizer:DEADLYSIGNAL\n"
                 b"==157092==ERROR: ThreadSanitizer: SEGV on unknown address 0x000000000018\n"
                 b"ThreadSanitizer: nested bug in the same thread, aborting.\n")
        self.assertTrue(cn.SANITIZER_CRASH.search(crash))
        self.assertFalse(cn.SANITIZER_CRASH.search(b"==1==WARNING: ThreadSanitizer: data race\n"))
        self.assertFalse(cn.SANITIZER_CRASH.search(b"==1==ERROR: AddressSanitizer: heap-use-after-free\n"))

    def test_windows_line_ends_match_the_kit(self):
        self.assertEqual(cn.normal(b"ok\r\n"), "ok\n")


if __name__ == "__main__":
    unittest.main()
