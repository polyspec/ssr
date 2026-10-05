"""Holder locks of resources that only one run may use at a time.

A lock is one file under ``var/locks``. Its record names the holder: the checkout of the code
that took it, the holder's pid, the start time of that process, the time the lock was taken, the
command and a random token. The record is written completely to a private file and linked to the
lock path; the link fails when the lock exists, so taking a lock is atomic and a reader never
sees a partial record. A run that finds the lock held is refused with the holder's record. A lock
whose holder process no longer runs is reported and stays in place; ``remove-stopped`` removes it
explicitly. Only the holder releases its lock: the release checks the token first.

    python3 -m tools.holder_lock run <name> [--root <checkout>] -- <command> [arguments...]
    python3 -m tools.holder_lock remove-stopped <lock file>
"""

from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import secrets
import signal
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
FIELDS = (("checkout", str), ("pid", int), ("processStart", str), ("acquired", str),
          ("command", str), ("token", str))


def lock_file(name, root=ROOT):
    """The lock file of a resource of the checkout ``root``."""
    return Path(root) / "var/locks" / f"{name}.lock"


def proc_start(stat, system_stat):
    """The start time from ``/proc/<pid>/stat`` (field 22, clock ticks from boot) and the boot
    time of ``/proc/stat``. The command name in parentheses may hold spaces, so the fields are
    counted after its closing parenthesis."""
    fields = stat[stat.rindex(")") + 2:].split(" ")
    ticks = fields[19] if len(fields) > 19 else ""
    boot = re.search(r"^btime (\d+)$", system_stat, re.MULTILINE)
    if not ticks.isdigit():
        raise ValueError(f"no start time in {stat!r}")
    if not boot:
        raise ValueError("no boot time in /proc/stat")
    booted = datetime.fromtimestamp(int(boot.group(1)), timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.000Z")
    return f"{ticks} clock ticks after the boot at {booted}"


def process_start(pid):
    """The start time of a running process, or None when no such process runs: from ``/proc`` on
    Linux, whose minimal container images have no ``ps``, and as ``ps`` prints it elsewhere."""
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return None
    except PermissionError:
        pass
    if sys.platform == "linux":
        try:
            stat = Path(f"/proc/{pid}/stat").read_text()
        except FileNotFoundError:
            return None
        return proc_start(stat, Path("/proc/stat").read_text())
    result = subprocess.run(["/bin/ps", "-o", "lstart=", "-p", str(pid)], capture_output=True,
                            text=True, env={**os.environ, "LC_ALL": "C"}, check=False)
    if result.returncode == 1:
        return None
    if result.returncode:
        raise OSError(f"ps exited with {result.returncode}: {result.stderr}")
    return result.stdout.strip() or None


def holder_running(record):
    """Whether the process that wrote the record still runs; a reused pid has another start time."""
    return process_start(record["pid"]) == record["processStart"]


def describe(record):
    return (f"pid {record['pid']} (process started {record['processStart']}) of checkout "
            f"{record['checkout']}, held since {record['acquired']} for {json.dumps(record['command'])}")


class HolderLockRefused(Exception):
    def __init__(self, message, record):
        super().__init__(message)
        self.record = record


def read_record(path):
    """Read and check a lock record; a record that cannot be read is an error, never a free lock."""
    text = Path(path).read_text()
    try:
        record = json.loads(text)
    except json.JSONDecodeError as error:
        raise ValueError(f"lock file {path} holds no lock record: {text!r}") from error
    for field, kind in FIELDS:
        if not isinstance(record, dict) or not isinstance(record.get(field), kind):
            raise ValueError(f"lock file {path} has no {field}: {text!r}")
    return record


def refusal(path, record):
    if holder_running(record):
        return HolderLockRefused(f"{path} is held by {describe(record)}; the holder releases it "
                                 "when its run ends", record)
    return HolderLockRefused(f"{path} is held by {describe(record)}, which is not running; remove "
                             f"the lock with: python3 -m tools.holder_lock remove-stopped {path}", record)


class HolderLock:
    """A taken lock; ``release()`` removes it after checking that it still holds this record."""

    def __init__(self, path, record):
        self.path = Path(path)
        self.record = record

    def release(self):
        current = read_record(self.path)
        if current["token"] != self.record["token"]:
            raise ValueError(f"{self.path} was replaced while this run held it; it now names "
                             f"{describe(current)}")
        self.path.unlink()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.release()


def acquire(path, command=None, root=ROOT):
    """Take the lock or raise HolderLockRefused with the holder's record."""
    path = Path(path)
    if not path.is_absolute():
        raise ValueError(f"lock file must be absolute: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    start = process_start(os.getpid())
    if not start:
        raise OSError(f"the start time of process {os.getpid()} is unknown")
    record = {
        "checkout": str(root), "pid": os.getpid(), "processStart": start,
        "acquired": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z",
        "command": command if command is not None else " ".join(sys.argv),
        "token": secrets.token_hex(16),
    }
    written = path.with_name(f"{path.name}.{os.getpid()}.{record['token']}")
    with open(written, "x") as stream:
        stream.write(json.dumps(record) + "\n")
    try:
        os.link(written, path)
    except FileExistsError:
        raise refusal(path, read_record(path)) from None
    finally:
        written.unlink()
    return HolderLock(path, record)


def remove_stopped(path):
    """Remove a lock whose holder no longer runs and return its record; a running holder is
    refused. The lock is renamed aside before its record is checked again, so a lock that a new
    holder took in the meantime is put back instead of removed."""
    path = Path(path)
    record = read_record(path)
    if holder_running(record):
        raise refusal(path, record)
    aside = path.with_name(f"{path.name}.removing.{os.getpid()}.{secrets.token_hex(8)}")
    os.rename(path, aside)
    moved = read_record(aside)
    if moved["token"] != record["token"]:
        try:
            os.link(aside, path)
        except FileExistsError as error:
            raise ValueError(f"{path} was taken by {describe(moved)} during the removal and a third "
                             f"run took it before it was restored; the record of the second holder "
                             f"is in {aside}") from error
        aside.unlink()
        raise refusal(path, moved)
    aside.unlink()
    return record


def run(name, command, root=ROOT):
    """Run a command while holding the checkout lock ``name`` of ``root`` and return its exit
    status. SIGINT, SIGTERM and SIGHUP go to the command, and the lock is released after the
    command has ended."""
    path = lock_file(name, root)
    try:
        lock = acquire(path, " ".join(command), root)
    except HolderLockRefused as refused:
        print(f"FAIL lock: {refused}", file=sys.stderr, flush=True)
        return 1
    print(f"PASS lock: acquired {path} (pid {os.getpid()})", flush=True)
    try:
        child = subprocess.Popen(command)
        forward = lambda number, _frame: child.send_signal(number)
        previous = {number: signal.signal(number, forward)
                    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)}
        try:
            status = child.wait()
        finally:
            for number, handler in previous.items():
                signal.signal(number, handler)
        return status if status >= 0 else 128 - status
    finally:
        lock.release()
        print(f"PASS lock: released {path}", flush=True)


def main(argv):
    if len(argv) >= 3 and argv[0] == "run" and "--" in argv:
        separator = argv.index("--")
        options, command = argv[2:separator], argv[separator + 1:]
        if command and options in ([], ["--root", *options[1:2]]) and len(options) in (0, 2):
            root = Path(options[1]) if options else ROOT
            return run(argv[1], command, root)
    if len(argv) == 2 and argv[0] == "remove-stopped":
        try:
            record = remove_stopped(Path(argv[1]))
        except (HolderLockRefused, OSError, ValueError) as error:
            print(f"FAIL lock: {error}", file=sys.stderr)
            return 1
        print(f"PASS lock: removed {argv[1]} of {describe(record)}, which is not running")
        return 0
    print("usage: python3 -m tools.holder_lock run <name> [--root <checkout>] -- <command> ...\n"
          "       python3 -m tools.holder_lock remove-stopped <lock file>", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
