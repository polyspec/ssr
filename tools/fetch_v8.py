"""Fetch the official V8 archive and binding of a target and verify them.

A checkout without the V8 inputs, such as a CI runner, fetches the official release files of
rusty_v8 150.4.0 for one target into ``var/v8``. Each file is written to a temporary name,
verified against the SHA-256 that ``tools/verify_archive.py`` declares and published by one rename;
a file of another digest fails and is not kept. An existing verified file is kept. No build
script downloads V8: the build reads only these verified files.

    python3 -m tools.fetch_v8 <target>
"""

import hashlib
import os
from pathlib import Path
import sys
import tempfile
import time
import urllib.request

from tools import verify_archive


ROOT = Path(__file__).resolve().parents[1]
RELEASE = "https://github.com/denoland/rusty_v8/releases/download/v150.4.0"


def files(target):
    """The archive and binding names of ``target`` with their declared SHA-256."""
    if target not in verify_archive.ARCHIVES:
        raise ValueError(f"unsupported V8 target: {target}; declared targets: {sorted(verify_archive.ARCHIVES)}")
    archive_hash, binding_hash = verify_archive.ARCHIVES[target]
    return [(f"librusty_v8_simdutf_release_{target}.a.gz", archive_hash),
            (f"src_binding_simdutf_release_{target}.rs", binding_hash)]


def fetch(target, destination=None, release=RELEASE):
    destination = destination or ROOT / "var/v8"
    destination.mkdir(parents=True, exist_ok=True)
    for name, expected in files(target):
        path = destination / name
        if path.is_file() and verify_archive.digest(path) == expected:
            print(f"PASS V8 input {name} exists with SHA-256 {expected}", flush=True)
            continue
        url = f"{release}/{name}"
        print(f"RUN fetch {url}", flush=True)
        started = time.monotonic()
        handle, temporary = tempfile.mkstemp(prefix=f".{name}.", dir=destination)
        try:
            with os.fdopen(handle, "wb") as output, urllib.request.urlopen(url) as response:
                while chunk := response.read(1024 * 1024):
                    output.write(chunk)
            actual = verify_archive.digest(Path(temporary))
            if actual != expected:
                raise ValueError(f"{url}: expected SHA-256 {expected}, actual {actual}")
            os.replace(temporary, path)
        finally:
            if os.path.exists(temporary):
                os.unlink(temporary)
        print(f"PASS fetch {name} {time.monotonic() - started:.1f}s", flush=True)


def main(argv):
    if len(argv) != 1:
        print("usage: python3 -m tools.fetch_v8 <target>", file=sys.stderr)
        return 2
    try:
        fetch(argv[0])
    except (OSError, ValueError) as error:
        print(f"FAIL fetch V8 inputs: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
