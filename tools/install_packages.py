"""Install the packages that the tests build with, once per lock, into an immutable directory.

The tests build the applications of ``tools/build-probe/tests/fixtures`` with the npm packages that
its ``package.json`` and ``package-lock.json`` declare. The packages are installed with ``npm ci``
into ``var/packages/<digest>``, where the digest is the SHA-256 of both files, so a changed lock
selects a new directory and an existing one never changes. An installation runs in a temporary
directory under ``var/packages`` and is published by one rename; a run that finds the directory
of its digest uses it, and the installation of a concurrent run that published first is removed.
Nothing is written into the checkout's fixture directory.

As the nextest setup script ``install-packages`` (``.config/nextest.toml``) the tool writes
``SSR_PACKAGES``, the published directory, and ``SSR_FIXTURES``, the fixture source directory, to the
file that ``NEXTEST_ENV`` names. Without ``NEXTEST_ENV`` it prints both assignments, which
``make verify-build`` and the tool tests use.

    python3 -m tools.install_packages
"""

import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tools/build-probe/tests/fixtures"
PACKAGES = ROOT / "var/packages"
FILES = ("package.json", "package-lock.json")


def digest(fixtures=FIXTURES):
    sha = hashlib.sha256()
    for name in FILES:
        content = (fixtures / name).read_bytes()
        sha.update(f"{name}\0{len(content)}\0".encode())
        sha.update(content)
    return sha.hexdigest()


def check(directory, fixtures=FIXTURES):
    """An installation holds the declared files and a node_modules without symbolic links."""
    for name in FILES:
        if (directory / name).read_bytes() != (fixtures / name).read_bytes():
            raise ValueError(f"{directory / name} differs from {fixtures / name}")
    modules = directory / "node_modules"
    if not modules.is_dir():
        raise ValueError(f"{modules} is missing")
    for path in modules.rglob("*"):
        if path.is_symlink():
            raise ValueError(f"installed package contains a symbolic link: {path}")


def install(fixtures=FIXTURES, packages=PACKAGES):
    """The published directory of the current lock, installed when it does not exist."""
    target = packages / digest(fixtures)
    if target.is_dir():
        check(target, fixtures)
        print(f"PASS packages: {target} exists", file=sys.stderr, flush=True)
        return target
    packages.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=".install-", dir=packages))
    try:
        for name in FILES:
            shutil.copyfile(fixtures / name, temporary / name)
        command = ["npm", "ci", "--install-links", "--no-bin-links", "--ignore-scripts", "--no-audit", "--no-fund"]
        print(f"RUN {' '.join(command)} in {temporary}", file=sys.stderr, flush=True)
        started = time.monotonic()
        result = subprocess.run(command, cwd=temporary, stdout=sys.stderr, check=False)
        if result.returncode:
            raise RuntimeError(f"{' '.join(command)} exited with {result.returncode} in {temporary}")
        check(temporary, fixtures)
        try:
            os.rename(temporary, target)
        except OSError:
            if not target.is_dir():
                raise
            print(f"PASS packages: a concurrent run published {target}", file=sys.stderr, flush=True)
        else:
            print(f"PASS packages: published {target} {time.monotonic() - started:.1f}s", file=sys.stderr,
                  flush=True)
        return target
    finally:
        if temporary.exists():
            shutil.rmtree(temporary)


CONFIG = ROOT / ".config/nextest.toml"
FILTER = re.compile(r"^filter = '([^']*)'\nsetup = \"install-packages\"$", re.MULTILINE)
READERS = re.compile(r"^mod fixture;$|SSR_PACKAGES|SSR_FIXTURES", re.MULTILINE)


def readers(root=ROOT):
    """The nextest binary IDs whose tests read the installation: an integration test target that
    includes the fixture module or names its variables, and a library whose root includes it."""
    tracked = subprocess.run(["git", "ls-files", "-z", "--", "crates"], cwd=root, capture_output=True,
                             text=True, check=True).stdout.split("\0")
    binaries = set()
    for path in tracked:
        parts = Path(path).parts
        if len(parts) != 4 or not path.endswith(".rs") or not READERS.search((root / path).read_text()):
            continue
        if parts[2] == "tests":
            binaries.add(f"{parts[1]}::{Path(parts[3]).stem}")
        elif parts[2:] == ("src", "lib.rs"):
            binaries.add(parts[1])
    return binaries


def filter_errors(root=ROOT):
    """The setup script ``install-packages`` runs before exactly the binaries that read the
    installation; a reader outside its filter, or a listed binary that reads nothing, is an error.
    Its filter is an alternation of ``binary_id(=<id>)``, so the selection is visible and checked."""
    match = FILTER.search((root / ".config/nextest.toml").read_text())
    if match is None:
        return [".config/nextest.toml: no filter of the setup script install-packages"]
    terms = [term.strip() for term in match.group(1).split("|")]
    listed = set()
    errors = []
    for term in terms:
        found = re.fullmatch(r"binary_id\(=([a-z0-9_:-]+)\)", term)
        if found is None:
            errors.append(f".config/nextest.toml: install-packages filter term {term!r} is not binary_id(=<id>)")
        else:
            listed.add(found.group(1))
    needed = readers(root)
    errors += [f"{binary} reads SSR_PACKAGES or SSR_FIXTURES but is outside the install-packages filter"
               for binary in sorted(needed - listed)]
    errors += [f"{binary} is in the install-packages filter but reads no installation"
               for binary in sorted(listed - needed)]
    return errors


def main(argv):
    if argv:
        print("usage: python3 -m tools.install_packages", file=sys.stderr)
        return 2
    try:
        target = install(FIXTURES, PACKAGES)
    except (OSError, ValueError, RuntimeError) as error:
        print(f"FAIL packages: {error}", file=sys.stderr)
        return 1
    assignments = f"SSR_PACKAGES={target}\nSSR_FIXTURES={FIXTURES}\n"
    destination = os.environ.get("NEXTEST_ENV")
    if destination:
        with open(destination, "a", encoding="utf-8") as environment:
            environment.write(assignments)
    else:
        print(assignments, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
