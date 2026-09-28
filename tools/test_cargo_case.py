import io
from pathlib import Path
import signal
from unittest import TestCase
from unittest.mock import Mock, patch

from tools import cargo_case


class CargoCaseTest(TestCase):
    def test_forwards_each_line_before_process_exit_and_preserves_failure(self):
        destination = io.StringIO()
        process = Mock(stdout=io.StringIO("Compiling crate\nFAIL case\n"), pid=345)
        def finish():
            self.assertIn("Compiling crate\nFAIL case\n", destination.getvalue())
            return 7
        process.wait.side_effect = finish
        with patch.object(cargo_case.subprocess, "Popen", return_value=process) as start:
            with patch("sys.stdout", destination):
                result = cargo_case.run(["cargo", "nextest", "run"], cwd=Path("/project"))
        self.assertEqual(result.returncode, 7)
        self.assertEqual(result.stdout, "Compiling crate\nFAIL case\n")
        self.assertTrue(process.stdout.closed)
        self.assertNotIn("timeout", start.call_args.kwargs)
        self.assertTrue(start.call_args.kwargs["start_new_session"])

    def test_interruption_terminates_group_and_reaps_child(self):
        class Interrupted(io.StringIO):
            def __next__(self):
                raise KeyboardInterrupt
        process = Mock(stdout=Interrupted(), pid=345)
        with patch.object(cargo_case.subprocess, "Popen", return_value=process):
            with patch.object(cargo_case.os, "killpg") as kill:
                with self.assertRaises(KeyboardInterrupt):
                    cargo_case.run(["cargo"], cwd=Path("/project"))
        kill.assert_called_once_with(345, signal.SIGKILL)
        process.wait.assert_called_once_with()
        self.assertTrue(process.stdout.closed)
