import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
from tempfile import TemporaryDirectory
from unittest import TestCase
from unittest.mock import patch

from tools import fetch_tools, fetch_v8, tool_versions, verify_archive


ROOT = Path(__file__).resolve().parents[1]
# The workflows of this repository. .github/workflows/release.yml is a copy of the release workflow of kit, which make
# kit-sync writes and make kit-check compares, so its steps are tested in kit.
WORKFLOWS = [ROOT / ".github/workflows/ci.yml", ROOT / ".github/workflows/push-gate.yml"]
# The make files of the targets: the Makefile, the bootstrap of kit that it includes and the kit.mk of the installed package
# that the bootstrap includes.
MAKE_FILES = [ROOT / "Makefile", ROOT / "scripts/kit/kit-bootstrap.mk", ROOT / "node_modules/@polyspec/kit/dist/kit.mk"]
# The steps that install the package of kit before any other make target of a job.
INSTALL_KIT = ["make install-tools TOOLS=npm", "make install-packages"]


# The step of the job ci-passed: the JSON of `needs` reaches make as a variable of its command line.
CI_PASSED_RUN = "make ci-passed RESULTS='${{ toJSON(needs) }}'"
# The jobs that run their checks through make ci-targets of kit and upload its logs and summary. ci-passed prints the
# result of each job it needs in its log.
REPORTING_JOBS = {"ci.yml": ["lint", "test"], "push-gate.yml": ["push-gate"]}
# The make target and the targets of each check job of ci.yml; each CI target runs in exactly one job of each architecture.
CI_JOBS = {"lint": ("ci-lint", "CI_LINT_TARGETS"), "test": ("ci-test", "CI_TEST_TARGETS")}
CI_TARGETS = ["ci-records", "check-fmt", "check-clippy", "check-deny", "check-examples", "ci-nextest"]


def workflow_jobs(text):
    """{job: body} of a workflow, in the order of the file."""
    parts = re.split(r"^  ([a-z-]+):\n", text.split("\njobs:\n", 1)[1], flags=re.MULTILINE)[1:]
    return dict(zip(parts[::2], parts[1::2]))


def job_key(body, key):
    """The value of a key at the level of a job (`if`, `needs`, `runs-on`), as written, or None."""
    found = re.search(rf"(?m)^    {re.escape(key)}: (.*)$", body)
    return found.group(1) if found else None


def makefile_targets():
    return {target for path in MAKE_FILES
            for target in re.findall(r"^([A-Za-z0-9_.-]+):", path.read_text(), re.MULTILINE)}


class WorkflowRuleTest(TestCase):
    def test_every_step_runs_a_make_target(self):
        targets = makefile_targets()
        for workflow in WORKFLOWS:
            commands = re.findall(r"^\s+run: (.+)$", workflow.read_text(), re.MULTILINE)
            self.assertTrue(commands, workflow)
            for command in commands:
                found = re.fullmatch(r"make ([a-z0-9-]+)(?: [A-Z_]+='\$\{\{ [^}]+ \}\}'| TOOLS=[a-z]+)*", command)
                self.assertIsNotNone(found, f"{workflow.name}: {command} is not a make target")
                self.assertIn(found.group(1), targets, f"{workflow.name}: {command}")

    def test_every_job_uploads_its_logs_and_summary_also_after_a_failure(self):
        for workflow in WORKFLOWS:
            jobs = workflow_jobs(workflow.read_text())
            reporting = REPORTING_JOBS.get(workflow.name, [])
            self.assertEqual(sorted(set(jobs) - set(reporting)), {"ci.yml": ["ci-passed"]}.get(workflow.name, []),
                             workflow.name)
            for name in reporting:
                body = jobs[name]
                self.assertRegex(body, r"if: \$\{\{ !cancelled\(\) \}\}\n\s+uses: actions/upload-artifact@",
                                 f"{workflow.name}: {name}")
                self.assertIn("path: var/report/ci-targets/", body, f"{workflow.name}: {name}")
                if "matrix:" in body:
                    self.assertIn("fail-fast: false", body, f"{workflow.name}: {name}")

    def test_every_action_is_pinned_to_a_commit(self):
        for workflow in WORKFLOWS:
            for line in workflow.read_text().splitlines():
                if "uses: " in line:
                    self.assertRegex(line, r"uses: [a-z-]+/[a-z-]+@[0-9a-f]{40} # v\d+$", workflow.name)

    def test_a_new_push_cancels_the_previous_pull_request_run(self):
        text = (ROOT / ".github/workflows/ci.yml").read_text()
        self.assertIn("\nconcurrency:\n  group: ${{ github.workflow }}-${{ github.ref }}\n"
                      "  cancel-in-progress: ${{ github.event_name == 'pull_request' }}\n", text)
        self.assertNotIn("concurrency", (ROOT / ".github/workflows/push-gate.yml").read_text())

    def test_each_workflow_declares_exactly_its_triggers(self):
        # ci.yml runs the checks on every pull request, merge group and manual run; push-gate.yml also runs on every
        # push outside the branches of the merge queue. The only other workflow is the release workflow of kit.
        triggers = {
            "ci.yml": "on:\n  pull_request:\n  merge_group:\n  workflow_dispatch:\n",
            "push-gate.yml": "on:\n  push:\n    branches-ignore: ['gh-readonly-queue/**']\n  pull_request:\n  merge_group:\n",
        }
        workflows = ROOT / ".github/workflows"
        self.assertEqual(sorted(path.name for path in workflows.glob("*.y*ml")), sorted([*triggers, "release.yml"]))
        for name, block in triggers.items():
            declared = re.search(r"(?m)^on:\n(?:  .*\n)+", (workflows / name).read_text())
            self.assertEqual(declared.group(0) if declared else "", block, name)

    def test_the_workflows_run_on_pull_requests_and_merge_groups(self):
        for name in ("ci.yml", "push-gate.yml"):
            trigger = re.search(r"(?ms)^on:\n(.*?)^\S", (ROOT / ".github/workflows" / name).read_text()).group(1)
            events = re.findall(r"(?m)^  ([\w-]+):", trigger)
            self.assertIn("pull_request", events, name)
            self.assertIn("merge_group", events, name)
            if name == "ci.yml":
                self.assertNotIn("push", events, "the merge group runs CI on the commit that main receives")
            else:
                self.assertIn("    branches-ignore: ['gh-readonly-queue/**']\n", trigger)

    def test_the_push_check_runs_the_record_checks_and_ci_runs_every_check(self):
        makefile = (ROOT / "Makefile").read_text()
        push = re.search(r"^CI_PUSH_TARGETS = (.*)$", makefile, re.MULTILINE).group(1).split()
        ci = re.search(r"^CI_LINT_TARGETS = (.*)$", makefile, re.MULTILINE).group(1).split()
        self.assertIn("push-records", push)
        self.assertIn("ci-records", ci)
        self.assertRegex(makefile, r"(?m)^push-records:\n\tpython3 tools/check.py records$")
        self.assertRegex(makefile, r"(?m)^ci-records:\n\tpython3 tools/check.py$")
        self.assertIn("run: make ci-push", (ROOT / ".github/workflows/push-gate.yml").read_text())

    def test_the_record_mode_fails_a_tree_whose_records_fail(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "docs").mkdir()
            (root / "docs/record.md").write_text("An orphan process.\n")
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(["git", "add", "-A"], cwd=root, check=True)
            from tools import check
            errors = check.check_records(root)
        self.assertIn(f"{root / 'docs/record.md'}: prohibited record word orphan", errors)
        result = subprocess.run([sys.executable, "tools/check.py", "records"], cwd=ROOT, capture_output=True, text=True)
        self.assertEqual((result.returncode, result.stdout), (0, "records: pass\n"), result.stderr)
        usage = subprocess.run([sys.executable, "tools/check.py", "other"], cwd=ROOT, capture_output=True, text=True)
        self.assertEqual(usage.returncode, 2)

    def test_ci_passed_is_the_last_job_and_needs_every_other_job(self):
        jobs = workflow_jobs((ROOT / ".github/workflows/ci.yml").read_text())
        self.assertEqual(list(jobs)[-1], "ci-passed")
        body = jobs["ci-passed"]
        self.assertEqual(job_key(body, "if"), "${{ always() }}")
        needs = [job.strip() for job in job_key(body, "needs").strip("[]").split(",")]
        self.assertEqual(sorted(needs), sorted(job for job in jobs if job != "ci-passed"))
        # The runner of the job push-gate; make ci-passed needs only Node.js and the package of kit.
        gate = workflow_jobs((ROOT / ".github/workflows/push-gate.yml").read_text())["push-gate"]
        self.assertEqual(job_key(body, "runs-on"), job_key(gate, "runs-on"))
        self.assertEqual(re.findall(r"(?m)^\s+run: (.+)$", body), [*INSTALL_KIT, CI_PASSED_RUN])
        self.assertTrue(body.rstrip().endswith(f"run: {CI_PASSED_RUN}"))
        self.assertNotRegex(body, r"(?m)^    name:")

    def test_each_architecture_runs_every_ci_target_in_exactly_one_job(self):
        jobs = workflow_jobs((ROOT / ".github/workflows/ci.yml").read_text())
        makefile = (ROOT / "Makefile").read_text()
        self.assertEqual(sorted(jobs), sorted([*CI_JOBS, "ci-passed"]))
        targets = []
        for job, (target, variable) in CI_JOBS.items():
            body = jobs[job]
            self.assertIn("\n        runner: [ubuntu-24.04-arm, ubuntu-24.04]\n", body, job)
            self.assertEqual(job_key(body, "runs-on"), "${{ matrix.runner }}", job)
            self.assertEqual(re.findall(r"(?m)^\s+run: (.+)$", body), [*INSTALL_KIT, "make ci-setup", f"make {target}"], job)
            self.assertIn(f"name: ci-{job}-${{{{ matrix.runner }}}}\n", body, job)
            self.assertEqual(recipe(target)[-1], ci_targets(variable), job)
            targets += re.search(rf"(?m)^{variable} = (.*)$", makefile).group(1).split()
        self.assertEqual(sorted(targets), sorted(CI_TARGETS))
        self.assertNotRegex(makefile, r"(?m)^ci:")


def ci_targets(variable):
    """The recipe line of a CI job target: make ci-targets of kit with the targets of ``variable`` and one Cargo build job
    per processor of the runner."""
    return f'BUILD_JOBS=$$(nproc) $(MAKE) --no-print-directory ci-targets TARGETS="$({variable})"'


def recipe(target):
    """The commands of a target of the Makefile, as written."""
    lines = (ROOT / "Makefile").read_text().splitlines()
    start = lines.index(f"{target}:") + 1
    commands = []
    for line in lines[start:]:
        if not line.startswith("\t"):
            break
        commands.append(line.strip())
    return commands


class BuildJobsTest(TestCase):
    """Cargo builds with one job by default and with one job per processor of the runner in CI."""

    def build_jobs(self, environment):
        with TemporaryDirectory() as directory:
            cargo = Path(directory) / "cargo"
            cargo.write_text('#!/bin/sh\necho "CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS"\n')
            cargo.chmod(0o755)
            inherited = {name: value for name, value in os.environ.items()
                         if name not in ("MAKEFLAGS", "MFLAGS", "MAKELEVEL", "GNUMAKEFLAGS", "MAKEFILES", "BUILD_JOBS",
                                         "CARGO_BUILD_JOBS")}
            inherited["PATH"] = f"{directory}{os.pathsep}{inherited['PATH']}"
            ran = subprocess.run(["make", "--no-print-directory", "ci-nextest"], cwd=ROOT,
                                 env={**inherited, **environment}, capture_output=True, text=True)
        self.assertEqual(ran.returncode, 0, ran.stdout + ran.stderr)
        return re.findall(r"(?m)^CARGO_BUILD_JOBS=(.*)$", ran.stdout)

    def test_a_local_build_runs_one_job_and_build_jobs_sets_the_count(self):
        self.assertEqual(self.build_jobs({}), ["1"])
        self.assertEqual(self.build_jobs({"BUILD_JOBS": "4"}), ["4"])

    def test_the_ci_targets_build_with_the_processor_count_of_the_runner(self):
        for target, variable in CI_JOBS.values():
            self.assertEqual(recipe(target)[-1], ci_targets(variable))


class FetchV8Test(TestCase):
    def test_the_files_are_verified_and_published_and_another_digest_fails(self):
        with TemporaryDirectory() as directory:
            release = Path(directory) / "release"
            release.mkdir()
            target = "x86_64-unknown-linux-gnu"
            names = [name for name, _ in fetch_v8.files(target)]
            for name in names:
                (release / name).write_bytes(f"content of {name}".encode())
            digests = tuple(hashlib.sha256(f"content of {name}".encode()).hexdigest() for name in names)
            destination = Path(directory) / "v8"
            with patch.dict(verify_archive.ARCHIVES, {target: digests}):
                fetch_v8.fetch(target, destination, release.as_uri())
                self.assertEqual(sorted(path.name for path in destination.iterdir()), sorted(names))
                (release / names[0]).write_bytes(b"changed")
                (destination / names[0]).unlink()
                with self.assertRaisesRegex(ValueError, f"expected SHA-256 {digests[0]}, actual "):
                    fetch_v8.fetch(target, destination, release.as_uri())
            self.assertEqual(sorted(path.name for path in destination.iterdir()), [names[1]])

    def test_an_undeclared_target_fails(self):
        with self.assertRaisesRegex(ValueError, "unsupported V8 target: riscv64"):
            fetch_v8.files("riscv64")


class FetchToolsTest(TestCase):
    """make ci-setup installs the release binaries of the test tools, each archive verified by its SHA-256."""

    def declaration(self, directory, content, member="tool-1.0/tool"):
        archive = Path(directory) / "tool.tar.gz"
        source = Path(directory) / "tool"
        source.write_bytes(content)
        with tarfile.open(archive, "w:gz") as bundle:
            bundle.add(source, arcname=member)
        path = Path(directory) / "tool-versions.json"
        path.write_text(json.dumps({
            "tools": [{"name": "tool", "command": ["tool", "--version"], "version": "tool 1.0"}],
            "releases": [{"name": "tool", "target": "x86_64-unknown-linux-gnu", "url": archive.as_uri(),
                          "sha256": hashlib.sha256(archive.read_bytes()).hexdigest(), "executable": "tool-1.0/tool"}]}))
        return path

    def test_the_executable_is_verified_and_installed_and_another_digest_installs_nothing(self):
        with TemporaryDirectory() as directory:
            declaration = self.declaration(directory, b"#!/bin/sh\necho tool 1.0\n")
            destination = Path(directory) / "bin"
            fetch_tools.fetch("x86_64-unknown-linux-gnu", destination, declaration)
            installed = destination / "tool"
            self.assertEqual(installed.read_bytes(), b"#!/bin/sh\necho tool 1.0\n")
            self.assertEqual(installed.stat().st_mode & 0o777, 0o755)
            self.assertEqual([path.name for path in destination.iterdir()], ["tool"])
            installed.unlink()
            expected = json.loads(declaration.read_text())["releases"][0]["sha256"]
            self.declaration(directory, b"changed")
            document = json.loads(declaration.read_text())
            document["releases"][0]["sha256"] = expected
            declaration.write_text(json.dumps(document))
            with self.assertRaisesRegex(ValueError, f"expected SHA-256 {expected}, actual "):
                fetch_tools.fetch("x86_64-unknown-linux-gnu", destination, declaration)
            self.assertEqual(list(destination.iterdir()), [])

    def test_a_missing_executable_an_undeclared_target_or_tool_and_a_missing_key_fail(self):
        with TemporaryDirectory() as directory:
            declaration = self.declaration(directory, b"tool", member="other/tool")
            destination = Path(directory) / "bin"
            with self.assertRaisesRegex(ValueError, "the archive has no tool-1.0/tool"):
                fetch_tools.fetch("x86_64-unknown-linux-gnu", destination, declaration)
            self.assertEqual(list(destination.iterdir()), [])
            with self.assertRaisesRegex(ValueError, r"no tool release for the target riscv64; declared targets: "
                                                    r"\['x86_64-unknown-linux-gnu'\]"):
                fetch_tools.releases("riscv64", declaration)
            document = json.loads(declaration.read_text())
            document["releases"][0]["name"] = "other"
            declaration.write_text(json.dumps(document))
            with self.assertRaisesRegex(ValueError, "the release of other names an undeclared tool"):
                fetch_tools.releases("x86_64-unknown-linux-gnu", declaration)
            del document["releases"][0]["sha256"]
            declaration.write_text(json.dumps(document))
            with self.assertRaisesRegex(ValueError, "each release names executable, name, sha256, target, url"):
                fetch_tools.releases("x86_64-unknown-linux-gnu", declaration)

    def test_each_runner_installs_the_declared_versions_of_the_test_tools_from_their_releases(self):
        tools = tool_versions.declared()
        for target in ("aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"):
            releases = {release["name"]: release for release in fetch_tools.releases(target)}
            self.assertEqual(sorted(releases), ["cargo-deny", "cargo-nextest"], target)
            for name, release in releases.items():
                version = tools[name]["version"].split()[1]
                self.assertRegex(release["url"], rf"^https://github\.com/[\w-]+/[\w-]+/releases/download/\S*{re.escape(version)}"
                                                 rf"/{name}-{re.escape(version)}-{target.split('-')[0]}-\S+\.tar\.gz$")
                self.assertRegex(release["sha256"], r"^[0-9a-f]{64}$")
        setup = recipe("ci-setup")
        self.assertFalse([command for command in setup if "cargo install" in command], setup)
        self.assertIn("python3 -m tools.fetch_tools \"$$(RUSTUP_AUTO_INSTALL=0 rustc -vV | sed -n 's/^host: //p')\"", setup)
        for target, _ in CI_JOBS.values():
            self.assertEqual(recipe(target)[:2], ["$(MAKE) --no-print-directory toolchain-check",
                                                  "python3 -m tools.tool_versions check python3 cargo-nextest cargo-deny"])
