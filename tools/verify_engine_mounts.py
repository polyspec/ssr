"""Check the engine build container's source, cache, and output mounts."""

import os
from pathlib import Path
import signal
import sys
import time


FIXED = {
    "/src": "ro",
    "/cargo": "rw",
    "/target": "rw",
}
DECLARED = ("SSR_V8_DIR", "SSR_ORDERED_JSON_DIR")


def expected() -> dict[str, str]:
    """Return the required mounts; declared checkouts come from the environment."""
    mounts = dict(FIXED)
    for name in DECLARED:
        value = os.environ.get(name, "")
        if not value.startswith("/"):
            raise ValueError(f"{name} must name an absolute checkout path")
        mounts[value] = "ro"
    return mounts


def main() -> int:
    start = time.monotonic()
    print("START engine mount verification", flush=True)

    def timeout(_signal: int, _frame: object) -> None:
        print(f"TIMEOUT engine mount verification {time.monotonic() - start:.3f}s")
        raise SystemExit(1)

    signal.signal(signal.SIGALRM, timeout)
    signal.alarm(30)
    try:
        required = expected()
    except ValueError as error:
        print(f"FAIL engine mount verification {time.monotonic() - start:.3f}s: {error}")
        return 1
    mounts = {}
    for line in Path("/proc/self/mountinfo").read_text().splitlines():
        left, right = line.split(" - ", 1)
        fields = left.split()
        mounts[fields[4]] = (set(fields[5].split(",")), right.split()[0])
    for path, access in required.items():
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
