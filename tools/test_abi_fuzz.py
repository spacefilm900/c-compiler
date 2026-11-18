"""Tests for tools/abi_fuzz.py that need no compiler."""

import os
import sys
import tempfile
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import abi_fuzz  # noqa: E402


def alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    return True


class RunTimeout(unittest.TestCase):
    def test_timeout_kills_the_whole_process_group(self):
        with tempfile.TemporaryDirectory() as tmp:
            pidfile = Path(tmp) / "pid"
            # The wrapper starts a long-lived descendant in its own process
            # group that keeps stdout and stderr open, then waits for it.
            script = f"sleep 30 & echo $! > {pidfile}; wait"
            started = time.monotonic()
            status, out, err = abi_fuzz.run(["sh", "-c", script], timeout=0.5)
            self.assertEqual(status, "timeout")
            self.assertIn(b"timed out", err)
            self.assertLess(time.monotonic() - started, 10)
            pid = int(pidfile.read_text())
            for _ in range(50):
                if not alive(pid):
                    break
                time.sleep(0.1)
            self.assertFalse(alive(pid), "the descendant outlived the timeout")

    def test_normal_exit_keeps_status_and_output(self):
        status, out, err = abi_fuzz.run(["sh", "-c", "echo out; echo err >&2; exit 3"])
        self.assertEqual((status, out, err), (3, b"out\n", b"err\n"))

    def test_pairs_are_deterministic(self):
        self.assertEqual(abi_fuzz.make_pair(7), abi_fuzz.make_pair(7))


if __name__ == "__main__":
    unittest.main()
