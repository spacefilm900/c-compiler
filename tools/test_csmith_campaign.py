#!/usr/bin/env python3
"""Tests for tools/csmith_campaign.py.

These tests use fake toolchains only. They do not compile with the
project compiler, GCC, or Clang, and they do not inspect those compilers'
sources. They check command construction, classification, and that -lm is
present at both -O0 and -O2.
"""

from __future__ import annotations

import json
import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(ROOT / "harness"))

import csmith_campaign as camp  # noqa: E402
import run_csmith as ci_csmith  # noqa: E402


FAKE_TOOLS = r"""#!/usr/bin/env python3
import json, os, sys
from pathlib import Path

role = Path(__file__).name
argv = sys.argv[:]
log_dir = Path(os.environ["FAKE_LOG_DIR"])
(log_dir / f"{role}.argv").write_text(json.dumps(argv), encoding="utf-8")

if role == "csmith":
    out = None
    seed = "0"
    args = argv[1:]
    i = 0
    while i < len(args):
        if args[i] == "-o" and i + 1 < len(args):
            out = Path(args[i + 1])
            i += 2
            continue
        if args[i] == "--seed" and i + 1 < len(args):
            seed = args[i + 1]
            i += 2
            continue
        i += 1
    src = (
        "int main(void) {\n"
        '  puts("checksum = %s");\n'
        "  return 0;\n"
        "}\n"
    ) % seed
    if out is None:
        sys.stdout.write(src)
    else:
        out.write_text(src, encoding="utf-8")
    sys.exit(0)

if "-pedantic-errors" in argv:
    sys.stderr.write("unexpected -pedantic-errors\n")
    sys.exit(99)

# Link or compile to an executable when -o is present.
if "-o" not in argv:
    sys.exit(0)
out = Path(argv[argv.index("-o") + 1])
producing_exe = out.suffix in {".exe", ".mine", ".ref", ""} or "exe" in out.name or out.suffix == ""
# Candidate IR ends with .ll
if out.suffix == ".ll":
    if os.environ.get("FAKE_CANDIDATE_FAIL") == "1":
        sys.stderr.write("invalid operands to '==' (int * and char *)\n")
        sys.exit(1)
    if os.environ.get("FAKE_CANDIDATE_ICE") == "1":
        sys.stderr.write("thread 'main' panicked at 'ice in irgen', src/irgen.rs:1:1\n")
        sys.exit(101)
    out.write_text("; fake ir\n", encoding="utf-8")
    sys.exit(0)

# Linking an executable must pass -lm at every optimisation level.
if "-lm" not in argv:
    sys.stderr.write("missing -lm\n")
    sys.exit(2)

script = "#!/bin/sh\n"
if os.environ.get("FAKE_RUN_TIMEOUT") == role:
    script += "sleep 30\n"
elif os.environ.get("FAKE_WRONG_OUTPUT") == "1" and role == "candidate-link":
    script += 'echo "checksum = WRONG"\n'
else:
    # checksum includes the seed if present in the cwd name; otherwise 1
    script += 'echo "checksum = 1"\n'
script += "exit 0\n"
out.write_text(script, encoding="utf-8")
out.chmod(out.stat().st_mode | 0o111)
sys.exit(0)
"""


def write_fake_tools(dir: Path) -> dict[str, Path]:
    tools = {
        "csmith": dir / "csmith",
        "gcc": dir / "gcc",
        "clang": dir / "clang",
        "candidate": dir / "candidate",
        "candidate-link": dir / "candidate-link",
    }
    for path in tools.values():
        path.write_text(FAKE_TOOLS, encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IEXEC)
    # clang is used both as a reference compiler and to link IR.
    # The linker role is distinguished by a wrapper that re-execs as
    # candidate-link when the input is an IR file.
    clang_wrap = dir / "clang-wrap"
    clang_wrap.write_text(
        "#!/usr/bin/env python3\n"
        "import os, sys\n"
        f"clang = {str(dir / 'clang')!r}\n"
        f"link = {str(dir / 'candidate-link')!r}\n"
        "args = sys.argv[1:]\n"
        "if any(a.endswith('.ll') for a in args):\n"
        "    os.execv(link, [link] + args)\n"
        "os.execv(clang, [clang] + args)\n",
        encoding="utf-8",
    )
    clang_wrap.chmod(clang_wrap.stat().st_mode | stat.S_IEXEC)
    tools["clang-wrap"] = clang_wrap
    return tools


class CommandConstructionTests(unittest.TestCase):
    def test_reference_link_includes_lm_at_o0_and_o2(self) -> None:
        for opt in ("O0", "O2"):
            cmd = camp.reference_compile_cmd(
                "gcc", "s.c", "gcc.exe", opt, "/inc", "gnu11"
            )
            self.assertIn("-lm", cmd, msg=opt)
            self.assertIn(f"-{opt}", cmd)
            self.assertNotIn("-pedantic-errors", cmd)
            self.assertNotIn("-std=c11", cmd)

    def test_candidate_link_includes_lm_at_o0_and_o2(self) -> None:
        for opt in ("O0", "O2"):
            cmd = camp.candidate_link_cmd("clang", "x.ll", "cand.exe", opt)
            self.assertIn("-lm", cmd, msg=opt)
            self.assertIn(f"-{opt}", cmd)

    def test_gnu11_mode_flags(self) -> None:
        cand = camp.candidate_compile_cmd("cc", "s.c", "s.ll", "/inc", "gnu11")
        self.assertIn("-std=gnu11", cand)
        self.assertNotIn("-std=c11", cand)
        ref = camp.reference_compile_cmd("gcc", "s.c", "a.exe", "O0", "/inc", "gnu11")
        self.assertNotIn("-std=c11", ref)
        self.assertNotIn("-pedantic-errors", ref)

    def test_strict_mode_uses_c11(self) -> None:
        cand = camp.candidate_compile_cmd("cc", "s.c", "s.ll", "/inc", "strict")
        self.assertNotIn("-std=gnu11", cand)
        ref = camp.reference_compile_cmd("gcc", "s.c", "a.exe", "O2", "/inc", "strict")
        self.assertIn("-std=c11", ref)
        self.assertIn("-lm", ref)
        self.assertNotIn("-pedantic-errors", ref)

    def test_default_csmith_options_cap_size_and_disable_packed(self) -> None:
        opts = camp.DEFAULT_CSMITH_OPTIONS
        self.assertIn("--no-packed-struct", opts)
        self.assertIn("--no-argc", opts)
        self.assertIn("--max-funcs", opts)
        self.assertIn("--max-block-depth", opts)
        self.assertIn("--max-expr-complexity", opts)


class ClassificationTests(unittest.TestCase):
    def _run(self, code: int | str = 0, stdout: str = "checksum = 1", **extra):
        rec = {"code": code, "stdout": stdout, "stderr": ""}
        rec.update(extra)
        return rec

    def test_agreement(self) -> None:
        status = camp.classify_seed(
            gcc_build={"code": 0},
            clang_build={"code": 0},
            candidate_build={"code": 0},
            candidate_link={"code": 0},
            gcc_run=self._run(),
            clang_run=self._run(),
            candidate_run=self._run(),
        )
        self.assertEqual(status, "agreement")

    def test_wrong_output(self) -> None:
        status = camp.classify_seed(
            gcc_build={"code": 0},
            clang_build={"code": 0},
            candidate_build={"code": 0},
            candidate_link={"code": 0},
            gcc_run=self._run(),
            clang_run=self._run(),
            candidate_run=self._run(stdout="checksum = 2"),
        )
        self.assertEqual(status, "candidate-wrong-output")

    def test_crash_or_ice(self) -> None:
        status = camp.classify_seed(
            gcc_build={"code": 0},
            clang_build={"code": 0},
            candidate_build={
                "code": 101,
                "stderr": "thread 'main' panicked at 'ice', src/irgen.rs:1:1",
            },
            candidate_link=None,
            gcc_run=self._run(),
            clang_run=self._run(),
            candidate_run=None,
        )
        self.assertEqual(status, "candidate-crash-or-ice")

    def test_rejects_valid_gnu_c(self) -> None:
        status = camp.classify_seed(
            gcc_build={"code": 0},
            clang_build={"code": 0},
            candidate_build={"code": 1, "stderr": "invalid operands to '=='"},
            candidate_link=None,
            gcc_run=self._run(),
            clang_run=self._run(),
            candidate_run=None,
        )
        self.assertEqual(status, "candidate-rejects-valid-gnu-c")

    def test_invalid_program(self) -> None:
        status = camp.classify_seed(
            gcc_build={"code": 1, "stderr": "error"},
            clang_build={"code": 1, "stderr": "error"},
            candidate_build={"code": 1, "stderr": "error"},
            candidate_link=None,
            gcc_run=None,
            clang_run=None,
            candidate_run=None,
        )
        self.assertEqual(status, "invalid-program")

    def test_reference_timeout(self) -> None:
        status = camp.classify_seed(
            gcc_build={"code": 0},
            clang_build={"code": 0},
            candidate_build={"code": 0},
            candidate_link={"code": 0},
            gcc_run=self._run(code="timeout"),
            clang_run=self._run(code="timeout"),
            candidate_run=self._run(code="timeout"),
        )
        self.assertEqual(status, "reference-timeout")

    def test_reference_disagreement(self) -> None:
        status = camp.classify_seed(
            gcc_build={"code": 0},
            clang_build={"code": 0},
            candidate_build={"code": 0},
            candidate_link={"code": 0},
            gcc_run=self._run(stdout="checksum = 1"),
            clang_run=self._run(stdout="checksum = 2"),
            candidate_run=self._run(stdout="checksum = 1"),
        )
        self.assertEqual(status, "reference-disagreement")

    def test_same_signal_is_not_agreement(self) -> None:
        status = camp.classify_seed(
            gcc_build={"code": 0},
            clang_build={"code": 0},
            candidate_build={"code": 0},
            candidate_link={"code": 0},
            gcc_run=self._run(code=-9, stdout=""),
            clang_run=self._run(code=-9, stdout=""),
            candidate_run=self._run(code=-9, stdout=""),
        )
        self.assertEqual(status, "reference-crash")
        self.assertFalse(camp.is_valid_comparison(status))

    def test_candidate_signal_is_crash(self) -> None:
        status = camp.classify_seed(
            gcc_build={"code": 0},
            clang_build={"code": 0},
            candidate_build={"code": 0},
            candidate_link={"code": 0},
            gcc_run=self._run(),
            clang_run=self._run(),
            candidate_run=self._run(code=-11, stdout=""),
        )
        self.assertEqual(status, "candidate-crash-or-ice")

    def test_candidate_compile_timeout_is_timeout(self) -> None:
        status = camp.classify_seed(
            gcc_build={"code": 0},
            clang_build={"code": 0},
            candidate_build={"code": "timeout", "stderr": ""},
            candidate_link=None,
            gcc_run=self._run(),
            clang_run=self._run(),
            candidate_run=None,
        )
        self.assertEqual(status, "candidate-timeout")

    def test_candidate_link_timeout_is_timeout(self) -> None:
        status = camp.classify_seed(
            gcc_build={"code": 0},
            clang_build={"code": 0},
            candidate_build={"code": 0},
            candidate_link={"code": "timeout", "stderr": ""},
            gcc_run=self._run(),
            clang_run=self._run(),
            candidate_run=None,
        )
        self.assertEqual(status, "candidate-timeout")

    def test_stdout_digest_detects_difference_beyond_truncation(self) -> None:
        status = camp.classify_seed(
            gcc_build={"code": 0},
            clang_build={"code": 0},
            candidate_build={"code": 0},
            candidate_link={"code": 0},
            gcc_run=self._run(stdout="TAIL", **{"stdout_sha256": "aaa"}),
            clang_run=self._run(stdout="TAIL", **{"stdout_sha256": "aaa"}),
            candidate_run=self._run(stdout="TAIL", **{"stdout_sha256": "bbb"}),
        )
        self.assertEqual(status, "candidate-wrong-output")

    def test_valid_comparison_only_when_all_ran(self) -> None:
        self.assertTrue(camp.is_valid_comparison("agreement"))
        self.assertTrue(camp.is_valid_comparison("candidate-wrong-output"))
        self.assertFalse(camp.is_valid_comparison("candidate-rejects-valid-gnu-c"))
        self.assertFalse(camp.is_valid_comparison("invalid-program"))
        self.assertFalse(camp.is_valid_comparison("reference-timeout"))


class EndToEndFakeToolchainTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory(prefix="csmith-camp-test-")
        self.dir = Path(self.tmp.name)
        self.logs = self.dir / "logs"
        self.logs.mkdir()
        self.tools = write_fake_tools(self.dir)
        os.environ["FAKE_LOG_DIR"] = str(self.logs)
        for key in (
            "FAKE_CANDIDATE_FAIL",
            "FAKE_CANDIDATE_ICE",
            "FAKE_WRONG_OUTPUT",
            "FAKE_RUN_TIMEOUT",
        ):
            os.environ.pop(key, None)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def _run(self, out: Path, opt: str = "O0", count: int = 1, mode: str = "gnu11"):
        return camp.run_campaign(
            camp.CampaignConfig(
                compiler=str(self.tools["candidate"]),
                csmith=str(self.tools["csmith"]),
                gcc=str(self.tools["gcc"]),
                clang=str(self.tools["clang-wrap"]),
                csmith_include=str(self.dir / "inc"),
                seed_start=1,
                count=count,
                opts=[opt],
                timeout=2,
                out=out,
                mode=mode,
            )
        )

    def test_end_to_end_agreement_records_lm_and_gnu_flags(self) -> None:
        out = self.dir / "results"
        summary = self._run(out, opt="O0")
        self.assertEqual(summary["totals"]["O0"]["agreement"], 1)
        self.assertEqual(summary["valid_comparisons"]["O0"], 1)
        source = out / "seeds" / "1" / "source.c"
        self.assertTrue(source.is_file())
        result = json.loads((out / "seeds" / "1" / "O0" / "result.json").read_text())
        self.assertEqual(result["status"], "agreement")
        gcc_cmd = result["runs"]["gcc"]["build"]["cmd"]
        clang_cmd = result["runs"]["clang"]["build"]["cmd"]
        link_cmd = result["runs"]["candidate"]["link"]["cmd"]
        cand_cmd = result["runs"]["candidate"]["build"]["cmd"]
        self.assertIn("-lm", gcc_cmd)
        self.assertIn("-lm", clang_cmd)
        self.assertIn("-lm", link_cmd)
        self.assertIn("-O0", gcc_cmd)
        self.assertIn("-std=gnu11", cand_cmd)
        self.assertNotIn("-pedantic-errors", gcc_cmd)
        self.assertNotIn("-std=c11", gcc_cmd)

    def test_lm_present_at_o2_as_well(self) -> None:
        out = self.dir / "results-o2"
        summary = self._run(out, opt="O2")
        self.assertEqual(summary["totals"]["O2"]["agreement"], 1)
        result = json.loads((out / "seeds" / "1" / "O2" / "result.json").read_text())
        for name in ("gcc", "clang"):
            self.assertIn("-lm", result["runs"][name]["build"]["cmd"])
            self.assertIn("-O2", result["runs"][name]["build"]["cmd"])
        self.assertIn("-lm", result["runs"]["candidate"]["link"]["cmd"])
        self.assertIn("-O2", result["runs"]["candidate"]["link"]["cmd"])

    def test_candidate_reject_is_classified(self) -> None:
        os.environ["FAKE_CANDIDATE_FAIL"] = "1"
        out = self.dir / "results-rej"
        summary = self._run(out)
        self.assertEqual(summary["totals"]["O0"]["candidate-rejects-valid-gnu-c"], 1)
        self.assertEqual(summary["valid_comparisons"]["O0"], 0)

    def test_resume_skips_complete_seed(self) -> None:
        out = self.dir / "results-resume"
        self._run(out, count=1)
        stamp = (out / "seeds" / "1" / "source.c").stat().st_mtime
        self._run(out, count=1)
        self.assertEqual((out / "seeds" / "1" / "source.c").stat().st_mtime, stamp)


class CiHarnessCommandTests(unittest.TestCase):
    def test_ci_runner_uses_gnu11_and_lm(self) -> None:
        cand = ci_csmith.candidate_ir_cmd("cc", "s.c", "/inc", "s.ll")
        link = ci_csmith.link_ir_cmd("clang", "s.ll", "a.out")
        ref = ci_csmith.reference_cmd("clang", "s.c", "/inc", "a.out")
        self.assertIn("-std=gnu11", cand)
        self.assertIn("-lm", link)
        self.assertIn("-lm", ref)
        self.assertIn("-std=gnu11", ref)
        self.assertNotIn("-pedantic-errors", ref)
        self.assertNotIn("-std=c11", cand)
        self.assertNotIn("-std=c11", ref)


class SummarizeAndToolTests(unittest.TestCase):
    def test_summarize_does_not_double_count_generation_failed(self) -> None:
        with tempfile.TemporaryDirectory(prefix="csmith-sum-") as tmp:
            out = Path(tmp)
            seed = out / "seeds" / "1"
            (seed / "O0").mkdir(parents=True)
            (seed / "O2").mkdir(parents=True)
            camp.write_json(
                seed / "generation.json",
                {"code": 1, "stderr": "csmith failed"},
            )
            for opt in ("O0", "O2"):
                camp.write_json(
                    seed / opt / "result.json",
                    {
                        "seed": 1,
                        "opt": opt,
                        "status": "generation-failed",
                        "valid_comparison": False,
                    },
                )
            summary = camp.summarize_tree(out, ["O0", "O2"])
            self.assertEqual(summary["totals"]["O0"]["generation-failed"], 1)
            self.assertEqual(summary["totals"]["O2"]["generation-failed"], 1)
            self.assertEqual(summary["seed_count"], 1)

    def test_resolve_tool_expands_a_relative_file_path(self) -> None:
        with tempfile.TemporaryDirectory(prefix="csmith-tool-") as tmp:
            tmp_path = Path(tmp)
            tool = tmp_path / "gcc"
            tool.write_text("#!/bin/sh\n", encoding="utf-8")
            tool.chmod(tool.stat().st_mode | 0o111)
            resolved = camp.resolve_tool(str(Path(tmp) / "gcc"))
            self.assertTrue(Path(resolved).is_absolute())
            self.assertEqual(Path(resolved).resolve(), tool.resolve())

    def test_stale_generation_failed_is_replaced_after_success(self) -> None:
        with tempfile.TemporaryDirectory(prefix="csmith-stale-") as tmp:
            tmp_path = Path(tmp)
            out = tmp_path / "results"
            tools_dir = tmp_path / "tools"
            tools_dir.mkdir()
            logs = tmp_path / "logs"
            logs.mkdir()
            os.environ["FAKE_LOG_DIR"] = str(logs)
            tools = write_fake_tools(tools_dir)
            seed = out / "seeds" / "1"
            (seed / "O0").mkdir(parents=True)
            camp.write_json(seed / "generation.json", {"code": 1})
            camp.write_json(
                seed / "O0" / "result.json",
                {
                    "seed": 1,
                    "opt": "O0",
                    "status": "generation-failed",
                    "valid_comparison": False,
                },
            )
            summary = camp.run_campaign(
                camp.CampaignConfig(
                    compiler=str(tools["candidate"]),
                    csmith=str(tools["csmith"]),
                    gcc=str(tools["gcc"]),
                    clang=str(tools["clang-wrap"]),
                    csmith_include=str(tmp_path / "inc"),
                    seed_start=1,
                    count=1,
                    opts=["O0"],
                    timeout=2,
                    out=out,
                    mode="gnu11",
                )
            )
            self.assertTrue((seed / "source.c").is_file())
            result = json.loads((seed / "O0" / "result.json").read_text())
            self.assertEqual(result["status"], "agreement")
            self.assertEqual(summary["totals"]["O0"]["agreement"], 1)
            self.assertEqual(summary["totals"]["O0"]["generation-failed"], 0)


if __name__ == "__main__":
    unittest.main()
