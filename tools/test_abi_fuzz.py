"""Tests for tools/abi_fuzz.py that need no compiler."""

import os
import shutil
import signal
import sys
import tempfile
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import abi_fuzz  # noqa: E402


def alive(pid: int) -> bool:
    """True while pid runs. A zombie (exited, not yet reaped by whoever
    adopted it) counts as exited."""
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    try:
        stat = Path(f"/proc/{pid}/stat").read_text()
    except OSError:
        return True
    return stat.rsplit(")", 1)[1].split()[0] != "Z"


def kill_quietly(pid: int) -> None:
    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


class RunTimeout(unittest.TestCase):
    def test_timeout_kills_the_whole_process_group(self):
        with tempfile.TemporaryDirectory() as tmp:
            pidfile = Path(tmp) / "pid"
            # The wrapper starts a long-lived descendant in its own process
            # group that keeps stdout and stderr open, then waits for it.
            script = 'sleep 30 & echo $! > "$1"; wait'
            started = time.monotonic()
            try:
                status, out, err = abi_fuzz.run(["sh", "-c", script, "sh", str(pidfile)], timeout=0.5)
                self.assertEqual(status, "timeout")
                self.assertIn(b"timed out", err)
                self.assertLess(time.monotonic() - started, 10)
                pid = int(pidfile.read_text())
                for _ in range(50):
                    if not alive(pid):
                        break
                    time.sleep(0.1)
                self.assertFalse(alive(pid), "the descendant outlived the timeout")
            finally:
                if pidfile.exists() and pidfile.read_text().strip():
                    kill_quietly(int(pidfile.read_text()))

    @unittest.skipUnless(shutil.which("setsid"), "needs setsid")
    def test_timeout_does_not_wait_for_a_descendant_outside_the_group(self):
        with tempfile.TemporaryDirectory() as tmp:
            pidfile = Path(tmp) / "pid"
            # The descendant moves to its own session and records its PID
            # from there, so the group kill misses it; it keeps the captured
            # pipes open for 20 s. The wrapper waits until it has moved.
            script = (
                'setsid sh -c \'echo $$ > "$1"; exec sleep 20\' sh "$1" & '
                'while [ ! -s "$1" ]; do sleep 0.01; done; wait'
            )
            started = time.monotonic()
            try:
                status, _, _ = abi_fuzz.run(["sh", "-c", script, "sh", str(pidfile)], timeout=2)
                self.assertEqual(status, "timeout")
                self.assertLess(time.monotonic() - started, 10)
                pid = int(pidfile.read_text())
                self.assertEqual(os.getsid(pid), pid, "descendant did not start its own session")
                self.assertTrue(alive(pid), "the descendant should have escaped the group kill")
            finally:
                if pidfile.exists() and pidfile.read_text().strip():
                    kill_quietly(int(pidfile.read_text()))

    def test_normal_exit_keeps_status_and_output(self):
        status, out, err = abi_fuzz.run(["sh", "-c", "echo out; echo err >&2; exit 3"])
        self.assertEqual((status, out, err), (3, b"out\n", b"err\n"))

    def test_pairs_are_deterministic(self):
        self.assertEqual(abi_fuzz.make_pair(7), abi_fuzz.make_pair(7))


if __name__ == "__main__":
    unittest.main()


class Int128Option(unittest.TestCase):
    def tearDown(self):
        abi_fuzz.OPTIONS["int128"] = False

    def test_default_programs_do_not_change(self):
        plain = abi_fuzz.make_pair(7)
        self.assertNotIn("__int128", plain[0] + plain[1])
        self.assertNotIn("I128", plain[0])

    def test_int128_adds_the_type_and_its_printer(self):
        abi_fuzz.OPTIONS["int128"] = True
        found = False
        for seed in range(1, 20):
            d, u = abi_fuzz.make_pair(seed)
            self.assertIn(abi_fuzz.I128_MACRO, d)
            self.assertIn(abi_fuzz.I128_MACRO, u)
            found = found or "__int128" in d.replace(abi_fuzz.I128_MACRO, "")
        self.assertTrue(found, "no seed used __int128")
