"""Fetch the release binaries of the pinned test tools of a target and verify them.

A CI runner installs cargo-nextest and cargo-deny from the official release archives that
``tools/tool-versions.json`` declares under ``releases``: each release names a declared tool, a
Rust target, the URL of the archive, its SHA-256 and the path of the executable in the archive.
Each archive is downloaded to a temporary name and verified against its SHA-256; an archive of
another digest fails and nothing of it is installed. The executable is written to a temporary name
in the directory of Cargo's binaries (``$CARGO_HOME/bin``, else ``~/.cargo/bin``) and installed by
one rename. ``make ci`` then checks the version report of each tool against its declaration.

    python3 -m tools.fetch_tools <target>
"""

import hashlib
import json
import os
from pathlib import Path
import sys
import tarfile
import tempfile
import time
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
DECLARATION = ROOT / "tools/tool-versions.json"
KEYS = {"name", "target", "url", "sha256", "executable"}


def releases(target, path=DECLARATION):
    """The declared releases of ``target``; each names a declared tool and every key of KEYS."""
    document = json.loads(path.read_text())
    tools = {tool["name"] for tool in document["tools"]}
    selected = []
    for release in document.get("releases", []):
        if set(release) != KEYS:
            raise ValueError(f"{path}: each release names {', '.join(sorted(KEYS))}: {release}")
        if release["name"] not in tools:
            raise ValueError(f"{path}: the release of {release['name']} names an undeclared tool")
        if release["target"] == target:
            selected.append(release)
    if not selected:
        targets = sorted({release["target"] for release in document.get("releases", [])})
        raise ValueError(f"no tool release for the target {target}; declared targets: {targets}")
    return selected


def digest(path):
    hasher = hashlib.sha256()
    with open(path, "rb") as source:
        while chunk := source.read(1024 * 1024):
            hasher.update(chunk)
    return hasher.hexdigest()


def install(release, destination):
    """Download, verify and install the executable of one release into ``destination``."""
    url = release["url"]
    name = Path(release["executable"]).name
    print(f"RUN fetch {url}", flush=True)
    started = time.monotonic()
    handle, archive = tempfile.mkstemp(prefix=f".{name}.", suffix=".tar.gz", dir=destination)
    executable = None
    try:
        with os.fdopen(handle, "wb") as output, urllib.request.urlopen(url) as response:
            while chunk := response.read(1024 * 1024):
                output.write(chunk)
        actual = digest(archive)
        if actual != release["sha256"]:
            raise ValueError(f"{url}: expected SHA-256 {release['sha256']}, actual {actual}")
        with tarfile.open(archive, "r:gz") as bundle:
            try:
                member = bundle.getmember(release["executable"])
            except KeyError:
                raise ValueError(f"{url}: the archive has no {release['executable']}") from None
            source = bundle.extractfile(member) if member.isfile() else None
            if source is None:
                raise ValueError(f"{url}: {release['executable']} is not a file")
            handle, executable = tempfile.mkstemp(prefix=f".{name}.", dir=destination)
            with os.fdopen(handle, "wb") as output:
                while chunk := source.read(1024 * 1024):
                    output.write(chunk)
        os.chmod(executable, 0o755)
        os.replace(executable, destination / name)
        executable = None
    finally:
        for path in (archive, executable):
            if path and os.path.exists(path):
                os.unlink(path)
    print(f"PASS install {name} from {url} with SHA-256 {release['sha256']} "
          f"{time.monotonic() - started:.1f}s", flush=True)


def fetch(target, destination=None, path=DECLARATION):
    destination = destination or Path(os.environ.get("CARGO_HOME") or Path.home() / ".cargo") / "bin"
    destination.mkdir(parents=True, exist_ok=True)
    for release in releases(target, path):
        install(release, destination)


def main(argv):
    if len(argv) != 1:
        print("usage: python3 -m tools.fetch_tools <target>", file=sys.stderr)
        return 2
    try:
        fetch(argv[0])
    except (OSError, ValueError, KeyError, tarfile.TarError, json.JSONDecodeError) as error:
        print(f"FAIL fetch tools: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
