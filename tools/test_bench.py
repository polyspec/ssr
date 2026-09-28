from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import patch

from tools import bench


class BenchmarkBuildTest(TestCase):
    def test_build_reports_failure_without_total_duration_limit(self):
        with patch.object(bench.subprocess, "run", return_value=SimpleNamespace(returncode=7)) as run:
            self.assertEqual(bench.main(), 7)
        self.assertNotIn("timeout", run.call_args.kwargs)
        self.assertIn("--verbose", run.call_args.args[0])
