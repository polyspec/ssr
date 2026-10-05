from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import patch
import io
import json
import subprocess
from contextlib import redirect_stderr

from tools import verify_engine_status


def status(project, path):
    return json.dumps({"groups": [{"name": project, "stackPath": str(path), "services": [{"name": "engine"}]}]})


class EngineStatusTest(TestCase):
    def run_status(self, result):
        error = io.StringIO()
        with patch.object(verify_engine_status.subprocess, "run", side_effect=[result]), redirect_stderr(error):
            code = verify_engine_status.main()
        return code, error.getvalue()

    def test_a_failed_status_command_names_its_output(self):
        code, error = self.run_status(SimpleNamespace(returncode=3, stdout="", stderr="no daemon"))
        self.assertEqual(code, 1)
        self.assertIn("containerctl status --json exited with 3: no daemon", error)

    def test_a_timeout_names_the_command_and_its_limit(self):
        code, error = self.run_status(subprocess.TimeoutExpired(["containerctl"], 30, output="partial", stderr=""))
        self.assertEqual(code, 1)
        self.assertIn("containerctl status --json did not exit within 30 s", error)
        self.assertIn("'partial'", error)

    def test_the_stack_of_the_checkout_passes(self):
        project = verify_engine_status.names(verify_engine_status.ROOT)["project"]
        code, error = self.run_status(SimpleNamespace(returncode=0, stdout=status(project, verify_engine_status.COMPOSE),
                                                      stderr=""))
        self.assertEqual((code, error), (0, ""))
