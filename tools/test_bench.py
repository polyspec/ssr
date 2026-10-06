from pathlib import Path
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
        command = run.call_args.args[0]
        self.assertEqual(command[command.index("--features") + 1], "bench")

    def test_the_benchmark_run_has_no_time_limit(self):
        messages = ('{"reason":"compiler-artifact","target":{"name":"bench","kind":["example"]},'
                    '"executable":"/target/debug/examples/bench"}')
        results = [SimpleNamespace(returncode=0, stdout=messages), SimpleNamespace(returncode=0)]
        with patch.object(bench.subprocess, "run", side_effect=results) as run:
            self.assertEqual(bench.main(), 0)
        self.assertEqual(run.call_args.args[0], ["/target/debug/examples/bench"])
        self.assertNotIn("timeout", run.call_args.kwargs)

    def test_the_benchmark_runs_the_executable_that_cargo_reports(self):
        messages = "\n".join([
            '{"reason":"compiler-artifact","target":{"name":"polyspec-ssr-runtime","kind":["lib"]},"executable":null}',
            '{"reason":"compiler-artifact","target":{"name":"bench","kind":["example"]},'
            '"executable":"/elsewhere/target/debug/examples/bench"}',
        ])
        self.assertEqual(bench.executable(messages), "/elsewhere/target/debug/examples/bench")
        with self.assertRaisesRegex(ValueError, "no executable for the bench example"):
            bench.executable(messages.splitlines()[0])
        self.assertNotIn("cargo\", \"metadata", Path(bench.__file__).read_text())
