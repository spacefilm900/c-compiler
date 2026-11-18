#!/usr/bin/env python3
"""Unit tests for the reducer, on synthetic predicates and stand-in
compilers. Run: python3 -m unittest tools/reduce/test_reduce.py"""
import contextlib
import io
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import textwrap
import time
import unittest
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import reduce  # noqa: E402

SAMPLE = textwrap.dedent('''\
    /* header comment */
    #include <stdio.h>
    #define TWICE(x) \\
        ((x) * 2)
    struct pair { int a, b; };
    static int table[] = { 1, 2, 3, 4 };
    int helper(int x) { return TWICE(x) + 1; }
    int other(void);
    int main(void) {
        // a line comment
        int s = 0;
        for (int i = 0; i < 4; i++) {
            s += table[i];
        }
        if (s > 3) { s = helper(s); } else s = 0;
        printf("%d %s\\n", s, "a \\"quoted\\" string");
        return L'x' == 'x' ? 0 : 1;
    }
    ''')


def write_script(directory, name, body):
    """An executable Python script, used as a stand-in compiler."""
    path = Path(directory) / name
    path.write_text("#!" + sys.executable + "\n" + textwrap.dedent(body))
    path.chmod(path.stat().st_mode | stat.S_IEXEC)
    return path


def is_one_minimal(items, test):
    """Removing any single element breaks the predicate."""
    return all(not test(items[:i] + items[i + 1:]) for i in range(len(items)))


class DdminTests(unittest.TestCase):
    def test_finds_the_two_needed_elements(self):
        test = lambda items: 3 in items and 6 in items
        self.assertEqual(reduce.ddmin(list(range(10)), test), [3, 6])

    def test_result_is_one_minimal_and_keeps_order(self):
        test = lambda items: sum(items) >= 20 and items == sorted(items)
        result = reduce.ddmin(list(range(12)), test)
        self.assertTrue(test(result))
        self.assertTrue(is_one_minimal(result, test))

    def test_adjacent_pairs_need_complements(self):
        # Needs every even element: subsets alone cannot find this.
        needed = set(range(0, 16, 2))
        test = lambda items: needed <= set(items)
        self.assertEqual(reduce.ddmin(list(range(16)), test), sorted(needed))

    def test_empty_result_when_nothing_is_needed(self):
        self.assertEqual(reduce.ddmin([1, 2, 3], lambda items: True), [])

    def test_single_element(self):
        self.assertEqual(reduce.ddmin([7], lambda items: 7 in items), [7])

    def test_split_covers_everything_in_order(self):
        for size in range(0, 20):
            for parts in range(1, 8):
                items = list(range(size))
                chunks = reduce.split(items, parts)
                self.assertEqual([x for chunk in chunks for x in chunk], items)
                self.assertLessEqual(len(chunks), parts)


class TokenizerTests(unittest.TestCase):
    def test_round_trip(self):
        for text in (SAMPLE, "", "\n", "int x;", "a/**/b", "#if 1\n#endif", "x\\\ny",
                     "'\\''", '"unterminated\n', "u8\"s\" U'c' L\"w\"", "1.5e+3f .5 0x1p-3"):
            tokens, tail = reduce.tokenize(text)
            self.assertEqual(reduce.render(tokens, list(range(len(tokens))), tail), text)

    def test_kinds(self):
        tokens, _ = reduce.tokenize('#define A 1\nx += 1.5e-3 + L"s" + \'c\'; a->b ... <<=')
        kinds = [(token.kind, token.text) for token in tokens]
        self.assertEqual(kinds[0], ("pp", "#define A 1"))
        self.assertIn(("punct", "+="), kinds)
        self.assertIn(("number", "1.5e-3"), kinds)
        self.assertIn(("string", 'L"s"'), kinds)
        self.assertIn(("char", "'c'"), kinds)
        self.assertIn(("punct", "->"), kinds)
        self.assertIn(("punct", "..."), kinds)
        self.assertIn(("punct", "<<="), kinds)

    def test_directive_with_continuation_is_one_token(self):
        tokens, _ = reduce.tokenize("#define F(x) \\\n  (x)\nint y;")
        self.assertEqual(tokens[0].text, "#define F(x) \\\n  (x)")
        self.assertEqual(tokens[1].text, "int")

    def test_hash_inside_a_line_is_not_a_directive(self):
        tokens, _ = reduce.tokenize("int a; # define")
        self.assertEqual(tokens[2].kind, "punct")

    def test_render_never_glues_tokens(self):
        tokens, tail = reduce.tokenize("a+b+c")
        # Dropping `b` must not make `++`.
        self.assertEqual(reduce.render(tokens, [0, 1, 3, 4], tail), "a+ +c")

    def test_render_keeps_directives_on_their_own_line(self):
        tokens, tail = reduce.tokenize("int a;\n#define X 1\nint b;\n")
        text = reduce.render(tokens, [0, 1, 3, 4, 5, 6], tail)
        self.assertIn("\n#define X 1\n", text)
        tokens, tail = reduce.tokenize("x\n#if 1\ny\n#endif\n")
        self.assertEqual(reduce.render(tokens, [1, 3], tail), "\n#if 1\n#endif\n")


class UnitTests(unittest.TestCase):
    def texts(self, units, tokens):
        return [" ".join(token.text for token in tokens[start:end]) for start, end in units]

    def test_top_level_units(self):
        tokens, _ = reduce.tokenize(SAMPLE)
        units = self.texts(reduce.top_level_units(tokens), tokens)
        self.assertEqual(units[0], "#include <stdio.h>")
        self.assertTrue(units[1].startswith("#define TWICE"))
        self.assertEqual(units[2], "struct pair { int a , b ; } ;")
        self.assertEqual(units[3], "static int table [ ] = { 1 , 2 , 3 , 4 } ;")
        self.assertTrue(units[4].startswith("int helper") and units[4].endswith("}"))
        self.assertEqual(units[5], "int other ( void ) ;")
        self.assertTrue(units[6].startswith("int main"))
        self.assertEqual(len(units), 7)

    def test_struct_with_declarator_after_its_body(self):
        tokens, _ = reduce.tokenize("struct s { int a; } v; int f(void) { return 0; }")
        units = self.texts(reduce.top_level_units(tokens), tokens)
        self.assertEqual(units, ["struct s { int a ; } v ;", "int f ( void ) { return 0 ; }"])

    def test_statement_units(self):
        tokens, _ = reduce.tokenize("{ int s = 0; if (s) { s++; } else s--; for (;;) {} }")
        pairs = reduce.match_brackets(tokens)
        units = self.texts(reduce.inner_units(tokens, 0, pairs[0], pairs), tokens)
        self.assertEqual(units, ["int s = 0 ;", "if ( s ) { s ++ ; }", "else s -- ;",
                                 "for ( ; ; ) { }"])

    def test_comma_units(self):
        tokens, _ = reduce.tokenize("f(a, g(b, c), d)")
        pairs = reduce.match_brackets(tokens)
        units = self.texts(reduce.inner_units(tokens, 1, pairs[1], pairs), tokens)
        self.assertEqual(units, ["a ,", "g ( b , c ) ,", "d"])


class ReducerTests(unittest.TestCase):
    def test_reduces_to_what_the_predicate_needs(self):
        def predicate(text):
            return "helper(s)" in text.replace(" ", "") and text.count("{") == text.count("}")
        reducer = reduce.Reducer(predicate)
        result = reducer.reduce(SAMPLE)
        self.assertTrue(predicate(result))
        self.assertLess(len(result), len(SAMPLE) // 5)

    def test_every_candidate_is_tested_once(self):
        seen = []

        def predicate(text):
            seen.append(text)
            return "table" in text
        reducer = reduce.Reducer(predicate)
        reducer.reduce(SAMPLE)
        self.assertEqual(len(seen), len(set(seen)))
        self.assertEqual(reducer.tests, len(seen))
        self.assertGreater(reducer.hits, 0)

    def test_uninteresting_input_is_an_error(self):
        with self.assertRaises(ValueError):
            reduce.Reducer(lambda text: False).reduce(SAMPLE)

    def test_result_is_a_fixpoint_of_the_passes(self):
        predicate = lambda text: "printf" in text and "return" in text
        reducer = reduce.Reducer(predicate)
        result = reducer.reduce(SAMPLE)
        self.assertEqual(reducer.reduce(result), result)

    def test_max_tests_stops_early(self):
        reducer = reduce.Reducer(lambda text: "main" in text, max_tests=5)
        result = reducer.reduce(SAMPLE)
        self.assertLessEqual(reducer.tests, 5)
        self.assertIn("main", result)


class CommandLineTests(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.input = Path(self.dir) / "input.c"
        self.input.write_text(SAMPLE)
        self.output = Path(self.dir) / "out.c"

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def main(self, *args):
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return reduce.main([str(self.input), "-o", str(self.output), "-q", *args])

    def test_cmd_mode_with_placeholder(self):
        status = self.main("--mode", "cmd", "--cmd", "grep -q quoted {}")
        self.assertEqual(status, 0)
        result = self.output.read_text()
        self.assertIn("quoted", result)
        self.assertLessEqual(len(result.splitlines()), 2)
        self.assertEqual(self.input.read_text(), SAMPLE, "the input is not modified")

    def test_cmd_mode_appends_the_path_and_sets_reduce_file(self):
        status = self.main("--mode", "cmd", "--cmd", 'grep -q helper "$REDUCE_FILE" && grep -q TWICE')
        self.assertEqual(status, 0)
        result = self.output.read_text()
        self.assertIn("helper", result)
        self.assertIn("TWICE", result)

    def test_timeout_is_not_interesting(self):
        status = self.main("--mode", "cmd", "--cmd", "sleep 5", "--timeout", "0.2")
        self.assertEqual(status, 2)
        self.assertFalse(self.output.exists())

    def test_ice_mode_with_a_stand_in_compiler(self):
        fake = write_script(self.dir, "fakecc", '''\
            import sys
            text = open([a for a in sys.argv[1:] if a.endswith(".c")][0]).read()
            if "table[i]" in text:
                sys.stderr.write("x.c:1:1: internal compiler error: lost (in irgen)\\n")
                sys.exit(3)
            if "helper" in text:
                sys.stderr.write("x.c:1:1: internal compiler error: other (in irgen)\\n")
                sys.exit(3)
            ''')
        status = self.main("--mode", "ice", "--message", "lost", "--compiler", str(fake))
        self.assertEqual(status, 0)
        result = self.output.read_text()
        self.assertIn("table[i]", result.replace(" ", ""))
        self.assertNotIn("helper", result)

    def test_crash_mode_needs_a_panic_or_signal(self):
        fake = write_script(self.dir, "fakecc", '''\
            import os, signal, sys
            text = open([a for a in sys.argv[1:] if a.endswith(".c")][0]).read()
            if "for" in text:
                os.kill(os.getpid(), signal.SIGSEGV)
            sys.exit(1)
            ''')
        self.assertEqual(self.main("--mode", "crash", "--compiler", str(fake)), 0)
        self.assertEqual(self.output.read_text().split(), ["for"])
        self.output.unlink()
        # An ordinary error (exit 1) is not a crash.
        quiet = write_script(self.dir, "quietcc", "import sys\nsys.exit(1)\n")
        self.assertEqual(self.main("--mode", "crash", "--compiler", str(quiet)), 2)

    def test_output_limit_makes_a_candidate_uninteresting(self):
        # The command succeeds, but floods stdout; past the limit it is not
        # interesting, so the input cannot be reduced.
        flood = "head -c 100000 /dev/zero; grep -q helper {}"
        self.assertEqual(self.main("--mode", "cmd", "--cmd", flood, "--output-limit", "1000"), 2)
        self.assertEqual(self.main("--mode", "cmd", "--cmd", flood), 0)

    def test_timeout_kills_the_whole_process_group(self):
        marker = Path(self.dir) / "survivor"
        # The shell starts a child that would write the marker after the
        # timeout; killing the group must stop it.
        command = f"(sleep 1; touch {marker}) & wait"
        self.assertEqual(self.main("--mode", "cmd", "--cmd", command, "--timeout", "0.2"), 2)
        time.sleep(1.5)
        self.assertFalse(marker.exists())

    def test_missing_compiler_is_reported_before_reducing(self):
        missing = str(Path(self.dir) / "no-such-compiler")
        self.assertEqual(self.main("--mode", "ice", "--compiler", missing), 2)
        self.assertEqual(self.main("--mode", "accept", "--compiler", sys.executable,
                                   "--clang", missing), 2)

    @unittest.skipUnless(shutil.which(os.environ.get("CLANG", "clang")), "needs clang")
    def test_a_sanitizer_report_rejects_the_candidate(self):
        clang = os.environ.get("CLANG", "clang")
        fake = write_script(self.dir, "fakecc", f'''\
            import subprocess, sys
            args = sys.argv[1:]
            out = args[args.index("-o") + 1]
            src = [a for a in args if a.endswith(".c")][0]
            sys.exit(subprocess.run([{clang!r}, "-w", "-S", "-emit-llvm", "-DOURS=1", src,
                                     "-o", out]).returncode)
            ''')
        # Stand-in sanitizer compilers: the "sanitized" program is the real
        # program, run through a wrapper that may add a sanitizer report.
        def ub_cc(name, report):
            return write_script(self.dir, name, f'''\
                import os, subprocess, sys
                args = sys.argv[1:]
                out = args[args.index("-o") + 1]
                src = [a for a in args if a.endswith(".c")][0]
                real = out + ".real"
                plain = [a for a in args if not a.startswith("-fsanitize") and a != "-g"]
                plain[plain.index("-o") + 1] = real
                status = subprocess.run([{clang!r}, "-w", *plain]).returncode
                with open(out, "w") as f:
                    f.write("#!/bin/sh\\n")
                    f.write({report!r})
                    f.write("exec " + real + "\\n")
                os.chmod(out, 0o755)
                sys.exit(status)
                ''')
        self.input.write_text("int putchar(int);\nint main(void) {\n#ifdef OURS\n"
                              "    putchar('x');\n#endif\n    return 0;\n}\n")
        noisy = ub_cc("noisycc", "echo 'x.c:1:1: runtime error: overflow' >&2\n")
        self.assertEqual(self.main("--mode", "miscompile", "--compiler", str(fake),
                                   "--ub-cc", str(noisy), "--passes", "lines"), 2)
        quiet = ub_cc("quietcc", "")
        self.assertEqual(self.main("--mode", "miscompile", "--compiler", str(fake),
                                   "--ub-cc", str(quiet), "--passes", "lines"), 0)

    @unittest.skipUnless(shutil.which(os.environ.get("CLANG", "clang")), "needs clang")
    def test_miscompile_mode_with_a_stand_in_compiler(self):
        # The stand-in compiles with Clang, but defines OURS, so the
        # program's output differs only where the source tests OURS.
        clang = os.environ.get("CLANG", "clang")
        fake = write_script(self.dir, "fakecc", f'''\
            import subprocess, sys
            args = sys.argv[1:]
            out = args[args.index("-o") + 1]
            src = [a for a in args if a.endswith(".c")][0]
            sys.exit(subprocess.run([{clang!r}, "-w", "-S", "-emit-llvm", "-DOURS=1", src,
                                     "-o", out]).returncode)
            ''')
        self.input.write_text(textwrap.dedent('''\
            int putchar(int);
            int square(int x) { return x * x; }
            int twice(int x) { return x + x; }
            int main(void) {
                int total = square(3);
                total += twice(4) - 8;
            #ifdef OURS
                total++;
            #endif
                putchar('0' + total % 10);
                return 0;
            }
            '''))
        status = self.main("--mode", "miscompile", "--compiler", str(fake), "--no-ub-check",
                           "--passes", "lines,decls")
        self.assertEqual(status, 0)
        result = self.output.read_text()
        self.assertIn("#ifdef OURS", result)
        self.assertIn("putchar", result)
        self.assertNotIn("twice", result)

    @unittest.skipUnless(shutil.which(os.environ.get("CLANG", "clang")), "needs clang")
    def test_reject_mode_keeps_clang_accepting(self):
        fake = write_script(self.dir, "fakecc", '''\
            import sys
            text = open([a for a in sys.argv[1:] if a.endswith(".c")][0]).read()
            if "struct pair" in text:
                sys.stderr.write("x.c:1:1: error: no pairs\\n")
                sys.exit(1)
            ''')
        status = self.main("--mode", "reject", "--message", "no pairs", "--compiler", str(fake))
        self.assertEqual(status, 0)
        result = self.output.read_text()
        self.assertIn("struct pair", result)
        # Clang accepts the result.
        check = subprocess.run([os.environ.get("CLANG", "clang"), "-std=c11", "-pedantic-errors",
                                "-fsyntax-only", str(self.output)], capture_output=True)
        self.assertEqual(check.returncode, 0, check.stderr)


if __name__ == "__main__":
    unittest.main()
