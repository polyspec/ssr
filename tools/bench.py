"""Build and run the render benchmark.

The build and the benchmark run are long operations: they report their commands, progress and
elapsed time and have no time limit. The executable is the one that
Cargo reports for the ``bench`` example in its JSON messages, so the benchmark runs exactly the
program that this build made, whatever the target directory.
"""

import json
import subprocess
import sys
import time


def executable(messages):
    """The executable of the ``bench`` example from Cargo's JSON messages."""
    for line in messages.splitlines():
        message = json.loads(line)
        target = message.get("target", {})
        if (message.get("reason") == "compiler-artifact" and target.get("name") == "bench"
                and target.get("kind") == ["example"]):
            return message["executable"]
    raise ValueError("Cargo reported no executable for the bench example")


def main():
    started = time.monotonic()
    print("RUN build render benchmark", flush=True)
    command = ["cargo", "build", "--offline", "--locked", "-p", "ssr-runtime", "--example", "bench", "--features",
               "bench", "--verbose", "--message-format", "json-render-diagnostics"]
    print(f"COMMAND {' '.join(command)}", flush=True)
    build = subprocess.run(command, stdout=subprocess.PIPE, text=True, check=False)
    if build.returncode:
        print(f"FAIL build render benchmark exit {build.returncode} {time.monotonic() - started:.3f}s",
              file=sys.stderr)
        return build.returncode
    try:
        program = executable(build.stdout)
    except (ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"FAIL build render benchmark: {error}", file=sys.stderr)
        return 1
    print(f"PASS build render benchmark {time.monotonic() - started:.3f}s: {program}", flush=True)
    started = time.monotonic()
    print("RUN render benchmark", flush=True)
    result = subprocess.run([program], check=False)
    print(f"{'PASS' if result.returncode == 0 else 'FAIL'} render benchmark exit {result.returncode} "
          f"{time.monotonic() - started:.3f}s", flush=True)
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
