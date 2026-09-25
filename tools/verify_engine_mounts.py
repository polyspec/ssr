"""Check the engine build container's source, cache, and output mounts."""

from pathlib import Path
import signal
import sys
import time


EXPECTED = {"/src": "ro", "/cargo": "rw", "/target": "rw"}


def main() -> int:
    start = time.monotonic()
    print("START engine mount verification", flush=True)

    def timeout(_signal: int, _frame: object) -> None:
        print(f"TIMEOUT engine mount verification {time.monotonic() - start:.3f}s")
        raise SystemExit(1)

    signal.signal(signal.SIGALRM, timeout)
    signal.alarm(30)
    mounts = {}
    for line in Path("/proc/self/mountinfo").read_text().splitlines():
        left, right = line.split(" - ", 1)
        fields = left.split()
        mounts[fields[4]] = (set(fields[5].split(",")), right.split()[0])
    for path, access in EXPECTED.items():
        if path not in mounts:
            print(f"FAIL engine mount verification {time.monotonic() - start:.3f}s: {path} is not mounted")
            return 1
        options, filesystem = mounts[path]
        if filesystem != "virtiofs" or access not in options:
            print(
                f"FAIL engine mount verification {time.monotonic() - start:.3f}s: "
                f"{path} is {filesystem} with {sorted(options)}"
            )
            return 1
    print(f"PASS engine mount verification {time.monotonic() - start:.3f}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
