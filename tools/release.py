"""Release a tag of a commit of main: the steps of ``.github/workflows/release.yml``.

    python3 -m tools.release verify TAG     the tagged commit is on main and passed the checks push-gate and ci-passed
    python3 -m tools.release versions TAG   every manifest of the tag has its version and docs/changelog.md its section
    python3 -m tools.release assets TAG     build the archives of the tag into var/release/assets: none
    python3 -m tools.release publish TAG    create the GitHub Release of the tag with its notes and archives

Every change reaches main through the merge queue with the required checks, so every commit of main passed the full
checks; the maintainer releases by tagging a commit of main after a version-bump pull request, and a tag push runs
these steps in order. A tag ``vX.Y.Z`` releases the crates of CRATES at version X.Y.Z. A tag
``<directory>/vX.Y.Z`` releases the Go module of that directory; the repository has no Go module (GO_MODULES), so
such a tag fails. No step reruns the tests.

``verify`` resolves the tag to its commit, requires that commit to be an ancestor of origin/main (``git merge-base
--is-ancestor``) and reads the check runs of the commit from the GitHub API (``gh api
repos/<repository>/commits/<sha>/check-runs``, the repository of GITHUB_REPOSITORY): the latest run of each of
push-gate and ci-passed must be completed with the conclusion success. ``versions`` compares X.Y.Z with the version of
the workspace manifest (``[workspace.package]``), with the version of every crate manifest that declares its own (a
crate with ``version.workspace = true`` takes the workspace version) and with the requirement ``=X.Y.Z`` of every
dependency on a crate of CRATES, and requires the section ``## X.Y.Z`` in docs/changelog.md. The release assets are
npm tarballs and Composer zips only, and the repository has neither, so ``assets`` builds no archive. A crate is not
released as an archive; it is consumed by git tag, because ``cargo package`` rewrites git dependencies, such as
polyspec-ordered-json, into crates.io requirements that do not resolve. ``publish`` runs ``gh release create TAG
--verify-tag --title TAG --notes-file <the notes>`` without archives; the notes are the section X.Y.Z when it has at
most 125000 characters, the limit of a GitHub release body, and otherwise one line that links the section ``## X.Y.Z``
of docs/changelog.md at the tag. Each failure names the tag, the file or
check and both values, and exits with status 1.
"""

import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
from urllib.parse import quote


ROOT = Path(__file__).resolve().parents[1]
MAIN = "origin/main"
CHECKS = ("push-gate", "ci-passed")
CHANGELOG = "docs/changelog.md"
REPOSITORY_URL = "https://github.com/polyspec/ssr"
# GitHub refuses a release body over 125000 characters.
NOTES_LIMIT = 125000
ASSETS = "var/release/assets"
WORKSPACE = "Cargo.toml"
# The crates of the workspace that a tag vX.Y.Z releases.
CRATES = ("polyspec-ssr-core", "polyspec-ssr-build", "polyspec-ssr-runtime", "polyspec-ssr-adapter-react",
          "polyspec-ssr-adapter-vue", "polyspec-ssr-adapter-svelte", "polyspec-ssr-adapter-vanilla",
          "polyspec-ssr-nonce", "polyspec-ssr-server")
WORKSPACE_VERSION = "the workspace version of every crate"
GIT_TAG = "not released as an archive; consumed by git tag"
# The manifests whose version a tag vX.Y.Z sets, each with how the tag releases it: the workspace manifest, which sets
# the version of the crates, and the manifest of every crate, which is consumed by git tag because `cargo package`
# rewrites git dependencies into crates.io requirements that do not resolve.
MANIFESTS = {WORKSPACE: WORKSPACE_VERSION, **{f"crates/{name}/Cargo.toml": GIT_TAG for name in CRATES}}
# The tracked manifests that no tag releases, with the reason.
NOT_RELEASED = {
    "tools/build-probe/Cargo.toml": "the build verification program of make verify-build",
    "tools/build-probe/tests/fixtures/package.json": "the npm packages of the build cases",
    "tools/license-fixtures/xxhash-0.8.17/Cargo.toml": "a license fixture of tools/test_workspace.py",
    "tools/license-fixtures/xxhash-0.8.18/Cargo.toml": "a license fixture of tools/test_workspace.py",
    "verification/engine/Cargo.toml": "the engine verification program of tools/verify_engine.py",
}
# The Go modules: a tag <directory>/vX.Y.Z releases the module of that directory.
GO_MODULES = {}
TAG = re.compile(r"(?:(?P<directory>[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)*)/)?"
                 r"v(?P<version>(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))")


class Stop(Exception):
    """A step fails; the message names the cause."""


def parse_tag(tag):
    """(Go module directory or None, version) of a release tag."""
    found = TAG.fullmatch(tag)
    if not found:
        raise Stop(f"{tag}: a release tag is vX.Y.Z or <Go module directory>/vX.Y.Z")
    directory = found.group("directory")
    if directory is not None and directory not in GO_MODULES:
        raise Stop(f"{tag}: {directory} is not a Go module directory; the Go modules are {sorted(GO_MODULES)}")
    return directory, found.group("version")


def run(command, cwd, env=None):
    """The standard output of a command; Stop with the command, its exit status and its standard error."""
    try:
        result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True)
    except OSError as error:
        raise Stop(f"{' '.join(command)} could not start: {error}")
    if result.returncode:
        raise Stop(f"{' '.join(command)} exited with {result.returncode}: "
                   f"{result.stderr.strip() or result.stdout.strip()}")
    return result.stdout


def tagged_commit(root, tag):
    return run(["git", "rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}"], root).strip()


def verify(root, tag, repository):
    """The tagged commit is on main and the latest run of every check of CHECKS concluded success."""
    parse_tag(tag)
    if not repository:
        raise Stop("GITHUB_REPOSITORY is not set; it names the repository <owner>/<name> whose check runs are read")
    commit = tagged_commit(root, tag)
    ancestry = subprocess.run(["git", "merge-base", "--is-ancestor", commit, MAIN], cwd=root, capture_output=True,
                              text=True)
    if ancestry.returncode == 1:
        raise Stop(f"{tag}: the commit {commit} is not on {MAIN}; a release tags a commit of main")
    if ancestry.returncode:
        raise Stop(f"git merge-base --is-ancestor {commit} {MAIN} exited with {ancestry.returncode}: "
                   f"{ancestry.stderr.strip()}")
    listed = run(["gh", "api", "--paginate", f"repos/{repository}/commits/{commit}/check-runs?per_page=100",
                  "--jq", ".check_runs[] | [.id, .name, .status, .conclusion] | @json"], root)
    runs = [json.loads(line) for line in listed.splitlines() if line.strip()]
    problems = []
    for name in CHECKS:
        named = [entry for entry in runs if entry[1] == name]
        if not named:
            problems.append(f"the check {name} is missing")
            continue
        _, _, status, conclusion = max(named, key=lambda entry: entry[0])
        if status != "completed" or conclusion != "success":
            problems.append(f"the check {name} is {status} with the conclusion {conclusion}, not success")
    if problems:
        raise Stop(f"{tag}: the commit {commit}: " + "; ".join(problems))
    return commit, list(CHECKS)


def section(text, name):
    """The body of the TOML table ``[name]`` of a manifest, or None."""
    found = re.search(rf"(?ms)^\[{re.escape(name)}\]\s*$(.*?)(?=^\[|\Z)", text)
    return found.group(1) if found else None


def manifest_version(path):
    """The version that a manifest declares, or None for a crate with ``version.workspace = true``."""
    text = path.read_text(encoding="utf-8")
    table = section(text, "workspace.package" if path.name == WORKSPACE and section(text, "workspace") is not None
                    else "package")
    if table is None:
        raise Stop(f"{path}: no [package] or [workspace.package] table")
    if re.search(r"(?m)^version\.workspace\s*=\s*true\s*$", table):
        return None
    found = re.search(r'(?m)^version\s*=\s*"([^"]*)"', table)
    if not found:
        raise Stop(f"{path}: no version")
    return found.group(1)


def changelog_section(root, version):
    """The body of the section ``## version`` of docs/changelog.md, without the anchor of the next section."""
    lines = (Path(root) / CHANGELOG).read_text(encoding="utf-8").splitlines()
    if f"## {version}" not in lines:
        raise Stop(f"{CHANGELOG}: no section ## {version}")
    start = lines.index(f"## {version}") + 1
    end = next((index for index in range(start, len(lines)) if lines[index].startswith("## ")), len(lines))
    body = lines[start:end]
    while body and (not body[-1].strip() or re.fullmatch(r'<a id="[^"]*"></a>', body[-1].strip())):
        body.pop()
    while body and not body[0].strip():
        body.pop(0)
    if not body:
        raise Stop(f"{CHANGELOG}: the section ## {version} has no entry")
    return "\n".join(body) + "\n"


def release_notes(tag, version, section):
    """The notes of the release: the section when it has at most NOTES_LIMIT characters, otherwise one line that links
    the section ``## version`` of docs/changelog.md at the tag, whose anchor is the version without dots."""
    if len(section) <= NOTES_LIMIT:
        return section
    url = f"{REPOSITORY_URL}/blob/{quote(tag, safe='/')}/{CHANGELOG}#{version.replace('.', '')}"
    return f"The changes of {version} are listed in [CHANGELOG.md]({url}).\n"


def versions(root, tag):
    """Every manifest of the tag declares its version, and docs/changelog.md has the section of the version."""
    root = Path(root)
    _, version = parse_tag(tag)
    problems = []
    crates = set(CRATES)
    for name in MANIFESTS:
        path = root / name
        declared = manifest_version(path)
        if declared is not None and declared != version:
            problems.append(f"{name}: version {declared}, the tag {tag} is {version}")
        for crate, requirement in re.findall(r'(?m)^([\w-]+)\s*=\s*\{[^}\n]*\bversion\s*=\s*"([^"]*)"',
                                             path.read_text(encoding="utf-8")):
            if crate in crates and requirement != f"={version}":
                problems.append(f"{name}: the dependency {crate} requires {requirement}, the tag {tag} is ={version}")
    try:
        changelog_section(root, version)
    except Stop as error:
        problems.append(f"{error} for the tag {tag}")
    if problems:
        raise Stop("; ".join(problems))
    return version


def asset_names(tag):
    """The archives of the tag. The release assets are npm tarballs and Composer zips only, and the repository has
    neither, so a tag has none."""
    parse_tag(tag)
    return []


def assets(root, tag):
    """Build the archives of the tag into an empty ASSETS; the names of the archives, none for this repository."""
    root = Path(root)
    names = asset_names(tag)
    target = root / ASSETS
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    return names


def publish(root, tag):
    """Create the GitHub Release of the tag with the section of docs/changelog.md as notes and the archives."""
    root = Path(root)
    _, version = parse_tag(tag)
    notes = release_notes(tag, version, changelog_section(root, version))
    names = asset_names(tag)
    target = root / ASSETS
    missing = [name for name in names if not (target / name).is_file()]
    if missing:
        raise Stop(f"{ASSETS} lacks {missing}; make release-assets builds them")
    with tempfile.TemporaryDirectory(prefix="release-notes-") as folder:
        notes_file = Path(folder) / "notes.md"
        notes_file.write_text(notes, encoding="utf-8")
        run(["gh", "release", "create", tag, "--verify-tag", "--title", tag, "--notes-file", str(notes_file),
             *[str(target / name) for name in names]], root)
    return names


def main(argv, root=ROOT, environ=os.environ):
    if len(argv) != 2 or argv[0] not in ("verify", "versions", "assets", "publish") or not argv[1]:
        print(__doc__, file=sys.stderr)
        return 2
    mode, tag = argv
    try:
        if mode == "verify":
            commit, checks = verify(root, tag, environ.get("GITHUB_REPOSITORY"))
            print(f"[release] {tag}: the commit {commit} is on {MAIN} and passed {', '.join(checks)}")
        elif mode == "versions":
            version = versions(root, tag)
            print(f"[release] {tag}: every manifest of the tag declares {version} and {CHANGELOG} has ## {version}")
        elif mode == "assets":
            names = assets(root, tag)
            print(f"[release] {tag}: built {', '.join(names) if names else 'no archive'} in {ASSETS}")
        else:
            names = publish(root, tag)
            print(f"[release] {tag}: created the GitHub Release with {', '.join(names) if names else 'no archive'}")
    except (Stop, OSError, ValueError) as error:
        print(f"[release] {mode} {tag} failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
