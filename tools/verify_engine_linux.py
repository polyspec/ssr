"""Verify the engine on Linux ARM64 in this checkout's container stack, under its holder lock.

The stack (Compose project, container, image, Cargo cache and build output) belongs to this
checkout (``tools/prepare_engine_compose.py``). Two sessions in one checkout share it, so the whole
verification and the removal of the stack run under the checkout lock ``engine-verification``: a
second run is refused with the holder's checkout, pid and process start time. Every step prints its
command, its output as it arrives and its result with the elapsed time; no step has a time limit.

    python3 -m tools.verify_engine_linux verify
    python3 -m tools.verify_engine_linux down
"""

from pathlib import Path
import subprocess
import sys
import time

from tools import holder_lock
from tools.prepare_engine_compose import OUTPUT, names


ROOT = Path(__file__).resolve().parents[1]
TARGET = "aarch64-unknown-linux-gnu"
LOCK = holder_lock.lock_file("engine-verification", ROOT)


def verify_steps(root=ROOT):
    stack = names(root)
    container = stack["container"]
    return [
        [sys.executable, "tools/verify_archive.py", TARGET],
        [sys.executable, "-m", "tools.prepare_engine_compose"],
        ["container", "build", "--platform", "linux/arm64", "-f", "verification/engine/linux/Dockerfile",
         "-t", stack["image"], "verification/engine/linux"],
        ["containerctl", "-f", str(OUTPUT), "up"],
        [sys.executable, "-m", "tools.verify_engine_status"],
        ["container", "exec", "-w", "/src", container, "python3", "/src/tools/verify_engine_mounts.py"],
        ["container", "exec", "-w", "/src", "-e", "CARGO_NET_OFFLINE=false", container, "cargo", "fetch",
         "--locked", "--manifest-path", "/src/verification/engine/Cargo.toml"],
        ["container", "exec", "-w", "/src", container, "python3", "/src/tools/verify_engine.py", TARGET],
        ["containerctl", "-f", str(OUTPUT), "down"],
    ]


def down_steps():
    return [["containerctl", "-f", str(OUTPUT), "down"]]


def run_step(command):
    """Run one step with its output passed through and return its exit status."""
    line = " ".join(command)
    print(f"RUN {line}", flush=True)
    start = time.monotonic()
    status = subprocess.run(command, cwd=ROOT, check=False).returncode
    result = "PASS" if status == 0 else f"FAIL exit {status}"
    print(f"{result} {line} {time.monotonic() - start:.3f}s", flush=True)
    return status


def run_locked(steps, lock_path=LOCK):
    """Run the steps in order under the lock; the first failing step ends the run."""
    try:
        lock = holder_lock.acquire(lock_path)
    except holder_lock.HolderLockRefused as refused:
        print(f"FAIL lock: {refused}", file=sys.stderr, flush=True)
        return 1
    print(f"PASS lock: acquired {lock_path} (pid {lock.record['pid']})", flush=True)
    try:
        for command in steps:
            status = run_step(command)
            if status:
                return status
        return 0
    finally:
        lock.release()
        print(f"PASS lock: released {lock_path}", flush=True)


def main(argv):
    if argv == ["verify"]:
        return run_locked(verify_steps())
    if argv == ["down"]:
        return run_locked(down_steps())
    print("usage: python3 -m tools.verify_engine_linux verify|down", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
