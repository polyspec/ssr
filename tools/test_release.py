"""The release of a tag (tools/release.py, .github/workflows/release.yml).

Each case builds a Git repository in a temporary directory with the manifests of the release, a changelog, a branch
origin/main and the tag, and puts a fake of gh first on PATH. The fake gh answers the check runs of the GitHub API from a
JSON state file and records each call. No case reaches GitHub or a registry.
"""

import json
import os
from pathlib import Path
import re
import subprocess
import sys
from tempfile import TemporaryDirectory
from unittest import TestCase
from unittest.mock import patch

from tools import release


ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "polyspec/ssr"

FAKE_GH = r'''
import json, os, sys
state_path = os.environ["FAKE_STATE"]
state = json.load(open(state_path))
args = sys.argv[1:]
state["calls"].append(args)
if args[:2] == ["api", "--paginate"]:
    for run in state["check_runs"]:
        print(json.dumps([run["id"], run["name"], run["status"], run["conclusion"]]))
elif args[:2] == ["release", "create"]:
    state["notes"] = open(args[args.index("--notes-file") + 1]).read()
else:
    sys.exit(f"fake gh: unexpected arguments {args}")
json.dump(state, open(state_path, "w"))
'''
CHANGELOG = """[Korean](changelog.ko.md)

# Changelog

## Unreleased

- A change after the release.

## 0.0.1

- The first entry of 0.0.1.

- The second entry of 0.0.1.
"""


def git(root, *args):
    return subprocess.run(["git", "-c", "user.name=test", "-c", "user.email=test@example.com", *args], cwd=root,
                          check=True, capture_output=True, text=True).stdout.strip()


class Sandbox:
    """A repository with the manifests of release.MANIFESTS at one version, a changelog, origin/main and fakes."""

    def __init__(self, test, version="0.0.1", changelog=CHANGELOG):
        folder = TemporaryDirectory(prefix="ssr-release-")
        test.addCleanup(folder.cleanup)
        self.root = Path(folder.name) / "repository"
        self.bin = Path(folder.name) / "bin"
        self.state = Path(folder.name) / "state.json"
        self.root.mkdir()
        self.bin.mkdir()
        (self.bin / "gh").write_text(f"#!{sys.executable}\n{FAKE_GH}")
        (self.bin / "gh").chmod(0o755)
        (self.root / "Cargo.toml").write_text(
            '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "%s"\nlicense = "MIT"\n' % version)
        for name in release.CRATES:
            path = f"crates/{name}"
            (self.root / path).mkdir(parents=True)
            dependency = ("" if name == "polyspec-ssr-core" else
                          f'polyspec-ssr-core = {{ path = "../polyspec-ssr-core", version = "={version}" }}\n')
            (self.root / path / "Cargo.toml").write_text(
                f'[package]\nname = "{name}"\nversion.workspace = true\n\n[dependencies]\n{dependency}')
        (self.root / "docs").mkdir()
        (self.root / release.CHANGELOG).write_text(changelog)
        git(self.root, "init", "--quiet", "--initial-branch=main")
        git(self.root, "add", "-A")
        git(self.root, "commit", "--quiet", "-m", "release")
        self.commit = git(self.root, "rev-parse", "HEAD")
        git(self.root, "update-ref", "refs/remotes/origin/main", self.commit)
        self.check_runs([("push-gate", "success"), ("ci-passed", "success")])
        environment = patch.dict(os.environ, {"PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}",
                                              "FAKE_STATE": str(self.state)})
        environment.start()
        test.addCleanup(environment.stop)

    def check_runs(self, runs):
        self.state.write_text(json.dumps({"calls": [], "check_runs": [
            {"id": index, "name": name, "status": "completed" if conclusion else "in_progress", "conclusion": conclusion}
            for index, (name, conclusion) in enumerate(runs, 1)]}))

    def tag(self, tag, commit=None):
        git(self.root, "tag", "-a", tag, "-m", tag, commit or self.commit)
        return tag

    def recorded(self):
        return json.loads(self.state.read_text())


class TagTest(TestCase):
    def test_a_release_tag_is_a_version(self):
        self.assertEqual(release.parse_tag("v0.0.1"), (None, "0.0.1"))
        self.assertEqual(release.parse_tag("v10.20.30"), (None, "10.20.30"))
        for tag in ("v1.0", "1.0.0", "v01.0.0", "v1.0.0-rc.1", "vX.Y.Z"):
            with self.subTest(tag=tag), self.assertRaisesRegex(release.Stop, "a release tag is vX.Y.Z"):
                release.parse_tag(tag)

    def test_a_directory_tag_fails_because_the_repository_has_no_go_module(self):
        with self.assertRaisesRegex(release.Stop, r"^go/v1.0.0: go is not a Go module directory; the Go modules are "
                                                  r"\[\]$"):
            release.parse_tag("go/v1.0.0")


class VersionsTest(TestCase):
    def test_every_manifest_at_the_version_and_the_changelog_section_pass(self):
        sandbox = Sandbox(self)
        self.assertEqual(release.versions(sandbox.root, "v0.0.1"), "0.0.1")

    def test_a_version_mismatch_names_the_file_and_both_values(self):
        sandbox = Sandbox(self)
        workspace = sandbox.root / "Cargo.toml"
        workspace.write_text(workspace.read_text().replace('version = "0.0.1"', 'version = "0.0.2"'))
        nonce = sandbox.root / "crates/polyspec-ssr-nonce/Cargo.toml"
        nonce.write_text(nonce.read_text().replace("version.workspace = true", 'version = "0.1.0"'))
        with self.assertRaises(release.Stop) as stopped:
            release.versions(sandbox.root, "v0.0.1")
        self.assertEqual(str(stopped.exception), "Cargo.toml: version 0.0.2, the tag v0.0.1 is 0.0.1; "
                                                 "crates/polyspec-ssr-nonce/Cargo.toml: version 0.1.0, the tag v0.0.1 "
                                                 "is 0.0.1")

    def test_a_dependency_on_a_released_crate_requires_the_version_of_the_tag(self):
        sandbox = Sandbox(self)
        server = sandbox.root / "crates/polyspec-ssr-server/Cargo.toml"
        server.write_text(server.read_text().replace('version = "=0.0.1"', 'version = "=0.0.0"'))
        with self.assertRaisesRegex(release.Stop, r"^crates/polyspec-ssr-server/Cargo.toml: the dependency "
                                                  r"polyspec-ssr-core requires =0.0.0, the tag v0.0.1 is =0.0.1$"):
            release.versions(sandbox.root, "v0.0.1")

    def test_a_missing_or_empty_changelog_section_fails(self):
        sandbox = Sandbox(self, version="0.0.2")
        with self.assertRaisesRegex(release.Stop, r"^docs/changelog.md: no section ## 0.0.2 for the tag v0.0.2$"):
            release.versions(sandbox.root, "v0.0.2")
        empty = Sandbox(self, changelog="# Changelog\n\n## Unreleased\n\n## 0.0.1\n\n## 0.0.0\n")
        with self.assertRaisesRegex(release.Stop, r"^docs/changelog.md: the section ## 0.0.1 has no entry for the tag "
                                                  r"v0.0.1$"):
            release.versions(empty.root, "v0.0.1")

    def test_the_section_is_the_release_notes(self):
        sandbox = Sandbox(self)
        self.assertEqual(release.changelog_section(sandbox.root, "0.0.1"),
                         "- The first entry of 0.0.1.\n\n- The second entry of 0.0.1.\n")
        self.assertEqual(release.changelog_section(sandbox.root, "Unreleased"), "- A change after the release.\n")


class VerifyTest(TestCase):
    def test_a_commit_of_main_with_both_checks_passed_is_verified(self):
        sandbox = Sandbox(self)
        self.assertEqual(release.verify(sandbox.root, sandbox.tag("v0.0.1"), REPOSITORY),
                         (sandbox.commit, ["push-gate", "ci-passed"]))
        self.assertEqual(sandbox.recorded()["calls"], [
            ["api", "--paginate", f"repos/{REPOSITORY}/commits/{sandbox.commit}/check-runs?per_page=100",
             "--jq", ".check_runs[] | [.id, .name, .status, .conclusion] | @json"]])

    def test_a_commit_that_is_not_on_main_fails(self):
        sandbox = Sandbox(self)
        git(sandbox.root, "commit", "--quiet", "--allow-empty", "-m", "outside main")
        outside = git(sandbox.root, "rev-parse", "HEAD")
        with self.assertRaisesRegex(release.Stop, f"^v0.0.1: the commit {outside} is not on origin/main; "):
            release.verify(sandbox.root, sandbox.tag("v0.0.1", outside), REPOSITORY)
        self.assertEqual(sandbox.recorded()["calls"], [])

    def test_a_missing_or_failed_check_is_named(self):
        cases = {
            "missing": ([("push-gate", "success")], "the check ci-passed is missing"),
            "failed": ([("push-gate", "failure"), ("ci-passed", "success")],
                       "the check push-gate is completed with the conclusion failure, not success"),
            "running": ([("push-gate", "success"), ("ci-passed", None)],
                        "the check ci-passed is in_progress with the conclusion None, not success"),
            "both": ([], "the check push-gate is missing; the check ci-passed is missing"),
        }
        for case, (runs, message) in cases.items():
            with self.subTest(case=case):
                sandbox = Sandbox(self)
                sandbox.check_runs(runs)
                with self.assertRaises(release.Stop) as stopped:
                    release.verify(sandbox.root, sandbox.tag("v0.0.1"), REPOSITORY)
                self.assertEqual(str(stopped.exception), f"v0.0.1: the commit {sandbox.commit}: {message}")

    def test_the_latest_run_of_a_check_decides(self):
        sandbox = Sandbox(self)
        sandbox.check_runs([("push-gate", "success"), ("ci-passed", "failure"), ("ci-passed", "success")])
        self.assertEqual(release.verify(sandbox.root, sandbox.tag("v0.0.1"), REPOSITORY)[0], sandbox.commit)
        sandbox.check_runs([("push-gate", "success"), ("ci-passed", "success"), ("ci-passed", "failure")])
        with self.assertRaisesRegex(release.Stop, "the check ci-passed is completed with the conclusion failure"):
            release.verify(sandbox.root, "v0.0.1", REPOSITORY)

    def test_without_the_repository_the_step_fails_before_any_request(self):
        sandbox = Sandbox(self)
        with self.assertRaisesRegex(release.Stop, "GITHUB_REPOSITORY is not set"):
            release.verify(sandbox.root, sandbox.tag("v0.0.1"), None)


class AssetsTest(TestCase):
    def test_a_tag_builds_no_archive_because_every_crate_is_consumed_by_git_tag(self):
        sandbox = Sandbox(self)
        stale = sandbox.root / release.ASSETS / "polyspec-ssr-core-0.0.1.crate"
        stale.parent.mkdir(parents=True)
        stale.write_text("crate")
        self.assertEqual(release.asset_names("v0.0.1"), [])
        self.assertEqual(release.assets(sandbox.root, sandbox.tag("v0.0.1")), [])
        self.assertEqual(list((sandbox.root / release.ASSETS).iterdir()), [])
        source = (ROOT / "tools/release.py").read_text()
        self.assertNotIn('"cargo", "package"', source)
        self.assertNotIn("--exclude-lockfile", source)


class PublishTest(TestCase):
    def test_publish_creates_the_release_with_the_notes_and_no_archive(self):
        sandbox = Sandbox(self)
        tag = sandbox.tag("v0.0.1")
        release.assets(sandbox.root, tag)
        self.assertEqual(release.publish(sandbox.root, tag), release.asset_names(tag))
        recorded = sandbox.recorded()
        call = recorded["calls"][-1]
        notes = call[call.index("--notes-file") + 1]
        self.assertEqual(call, ["release", "create", "v0.0.1", "--verify-tag", "--title", "v0.0.1", "--notes-file", notes])
        self.assertEqual(recorded["notes"], "- The first entry of 0.0.1.\n\n- The second entry of 0.0.1.\n")

    def test_publish_without_the_changelog_section_fails_before_any_request(self):
        sandbox = Sandbox(self, version="0.0.2")
        with self.assertRaisesRegex(release.Stop, r"^docs/changelog.md: no section ## 0.0.2$"):
            release.publish(sandbox.root, sandbox.tag("v0.0.2"))
        self.assertEqual(sandbox.recorded()["calls"], [])


class RepositoryTest(TestCase):
    def test_every_tracked_manifest_is_released_or_declared_as_not_released(self):
        tracked = subprocess.run(["git", "ls-files"], cwd=ROOT, check=True, capture_output=True, text=True).stdout.split()
        manifests = sorted(path for path in tracked if Path(path).name in (
            "package.json", "composer.json", "Cargo.toml", "VERSION", "pyproject.toml", "go.mod"))
        self.assertEqual(manifests, sorted([*release.MANIFESTS, *release.NOT_RELEASED]))
        self.assertEqual(release.GO_MODULES, {})
        members = re.findall(r'"crates/([^"]+)"', release.section((ROOT / "Cargo.toml").read_text(), "workspace"))
        self.assertEqual(sorted(members), sorted(release.CRATES))
        for name in release.CRATES:
            self.assertRegex((ROOT / "crates" / name / "Cargo.toml").read_text(), rf'(?m)^name = "{re.escape(name)}"$')

    def test_every_crate_is_not_released_as_an_archive_and_consumed_by_git_tag(self):
        for name, how in release.MANIFESTS.items():
            with self.subTest(manifest=name):
                expected = (release.WORKSPACE_VERSION if name == release.WORKSPACE
                            else "not released as an archive; consumed by git tag")
                self.assertEqual(how, expected)

    def test_the_manifests_of_the_tree_pass_the_version_check_of_their_version(self):
        # No version is released, so the changelog has no section of the workspace version and that is the only failure.
        version = release.manifest_version(ROOT / "Cargo.toml")
        with self.assertRaisesRegex(release.Stop, rf"^docs/changelog.md: no section ## {re.escape(version)} for the tag v{version}$"):
            release.versions(ROOT, f"v{version}")

    def test_make_runs_each_step_with_the_tag_of_the_environment(self):
        makefile = (ROOT / "Makefile").read_text()
        environment = {name: value for name, value in os.environ.items()
                       if name not in ("MAKEFLAGS", "MFLAGS", "MAKELEVEL", "GNUMAKEFLAGS", "MAKEFILES", "TAG")}
        for step in ("verify", "versions", "assets", "publish"):
            with self.subTest(step=step):
                self.assertRegex(makefile, rf'(?m)^release-{step}:\n\t\$\(if \$\(TAG\),,\$\(error make \$@ needs '
                                           rf'TAG=<tag>[^)]*\)\)\n\tpython3 -m tools.release {step} "\$\$TAG"$')
                missing = subprocess.run(["make", "--no-print-directory", f"release-{step}"], cwd=ROOT, env=environment,
                                         capture_output=True, text=True)
                self.assertNotEqual(missing.returncode, 0)
                self.assertIn(f"make release-{step} needs TAG=<tag>", missing.stderr)
