"""Run the CI targets, each to its end, and report every result.

Every target is a make target of the checkout. Each one runs whether an earlier one failed; its
output goes to the job log as it arrives and to ``var/ci/<target>.log``. After the last target,
``var/ci/summary.md`` lists each target with its result, exit status and elapsed time, and the first
lines of each failure: the lines that name an error or a failure, else the last lines of its log.
The summary is also appended to the file that ``GITHUB_STEP_SUMMARY`` names. The run fails when a
target failed.

    python3 -m tools.ci_run <target>...
"""

import os
from pathlib import Path
import re
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
FAILURE = re.compile(r"error|FAIL|panicked|Error|failed|TIMEOUT")
FIRST_LINES = 20


def first_lines(log):
    """The first lines of a failure in ``log``: the lines that name an error or a failure, or the
    last lines when none does."""
    lines = log.splitlines()
    named = [line for line in lines if FAILURE.search(line)]
    return (named or lines[-FIRST_LINES:])[:FIRST_LINES]


def run_target(root, target, logs):
    print(f"RUN {target}", flush=True)
    started = time.monotonic()
    log_path = logs / f"{target}.log"
    with open(log_path, "w", encoding="utf-8") as log:
        process = subprocess.Popen(["make", "--no-print-directory", target], cwd=root, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, text=True, errors="replace")
        for line in process.stdout:
            sys.stdout.write(line)
            log.write(line)
        status = process.wait()
    elapsed = time.monotonic() - started
    print(f"{'PASS' if status == 0 else 'FAIL'} {target} exit {status} {elapsed:.1f}s", flush=True)
    return {"target": target, "status": status, "elapsed": elapsed, "log": log_path}


def summary(results):
    lines = ["# CI result", "", "| Target | Result | Exit | Time |", "| --- | --- | --- | --- |"]
    for result in results:
        lines.append(f"| `{result['target']}` | {'PASS' if result['status'] == 0 else 'FAIL'} | "
                     f"{result['status']} | {result['elapsed']:.1f} s |")
    for result in results:
        if result["status"]:
            lines += ["", f"## `{result['target']}`", "", "```"]
            lines += first_lines(result["log"].read_text(encoding="utf-8", errors="replace"))
            lines += ["```"]
    return "\n".join(lines) + "\n"


def run(targets, root=ROOT):
    logs = Path(root) / "var/ci"
    logs.mkdir(parents=True, exist_ok=True)
    results = [run_target(root, target, logs) for target in targets]
    text = summary(results)
    (logs / "summary.md").write_text(text, encoding="utf-8")
    destination = os.environ.get("GITHUB_STEP_SUMMARY")
    if destination:
        with open(destination, "a", encoding="utf-8") as output:
            output.write(text)
    print(text, flush=True)
    return 1 if any(result["status"] for result in results) else 0


def main(argv):
    if not argv:
        print("usage: python3 -m tools.ci_run <target>...", file=sys.stderr)
        return 2
    return run(argv)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
