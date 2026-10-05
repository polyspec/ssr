"""Check the engine build container's source, cache, and output mounts."""

import os
from pathlib import Path
import sys
import time


FIXED = {
    "/src": "ro",
    "/cargo": "rw",
    "/target": "rw",
}
DECLARED = ("SSR_V8_DIR", "SSR_ORDERED_JSON_DIR", "SSR_LIGHTNINGCSS_DIR")


def expected() -> dict[str, str]:
    """Return the required mounts; declared checkouts come from the environment."""
    mounts = dict(FIXED)
    for name in DECLARED:
        value = os.environ.get(name, "")
        if not value.startswith("/"):
            raise ValueError(f"{name} must name an absolute checkout path")
        mounts[value] = "ro"
    return mounts


def check(required: dict[str, str], mountinfo: str) -> list[str]:
    """The errors of the mounts in ``mountinfo`` (the text of ``/proc/self/mountinfo``): each
    required path is mounted with its required access, ``ro`` or ``rw``. The filesystem type is
    not judged; it differs between container runtimes and versions."""
    mounts = {}
    for line in mountinfo.splitlines():
        left, _right = line.split(" - ", 1)
        fields = left.split()
        mounts[fields[4]] = set(fields[5].split(","))
    errors = []
    for path, access in required.items():
        if path not in mounts:
            errors.append(f"{path} is not mounted")
        elif access not in mounts[path]:
            errors.append(f"{path}: expected access {access}, actual mount options {sorted(mounts[path])}")
    return errors


def main() -> int:
    start = time.monotonic()
    print("START engine mount verification", flush=True)
    try:
        errors = check(expected(), Path("/proc/self/mountinfo").read_text())
    except (OSError, ValueError) as error:
        errors = [str(error)]
    for error in errors:
        print(f"FAIL engine mount verification {time.monotonic() - start:.3f}s: {error}")
    if errors:
        return 1
    print(f"PASS engine mount verification {time.monotonic() - start:.3f}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
