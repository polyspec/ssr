"""Forward Cargo progress while nextest enforces individual test deadlines."""

import os
import signal
import subprocess


def run(command, *, cwd):
    print(f"COMMAND {' '.join(command)}", flush=True)
    process = subprocess.Popen(
        command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, start_new_session=True,
    )
    lines = []
    try:
        for line in process.stdout:
            print(line, end="", flush=True)
            lines.append(line)
        code = process.wait()
    except BaseException:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            # The process group has already exited; wait still reaps the child.
            pass
        process.wait()
        raise
    finally:
        process.stdout.close()
    return subprocess.CompletedProcess(command, code, "".join(lines), "")
