import os
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
from unittest import TestCase
from unittest.mock import patch

from tools import install_packages


FAKE_NPM = """#!/bin/sh
echo "$*" >> "$NPM_LOG"
mkdir -p node_modules/sample node_modules/playwright-core
echo '{}' > node_modules/sample/package.json
cat > node_modules/playwright-core/cli.js <<'SCRIPT'
const fs = require("node:fs");
if (process.argv.slice(2).join(" ") !== "install chromium") process.exit(3);
fs.mkdirSync(process.env.PLAYWRIGHT_BROWSERS_PATH + "/chromium", { recursive: true });
fs.writeFileSync(process.env.PLAYWRIGHT_BROWSERS_PATH + "/chromium/chrome", "browser");
SCRIPT
cat > node_modules/playwright-core/index.mjs <<'SCRIPT'
export const chromium = { executablePath: () => process.env.PLAYWRIGHT_BROWSERS_PATH + "/chromium/chrome" };
SCRIPT
if [ -n "$NPM_LINK" ]; then ln -s sample node_modules/linked; fi
"""


class InstallPackagesTest(TestCase):
    def setUp(self):
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.fixtures = self.root / "fixtures"
        self.fixtures.mkdir()
        (self.fixtures / "package.json").write_text('{"name":"sample"}\n')
        (self.fixtures / "package-lock.json").write_text('{"lockfileVersion":3}\n')
        self.packages = self.root / "packages"
        bin_directory = self.root / "bin"
        bin_directory.mkdir()
        (bin_directory / "npm").write_text(FAKE_NPM)
        (bin_directory / "npm").chmod(0o755)
        self.log = self.root / "npm.log"
        self.environment = patch.dict(os.environ, {"PATH": f"{bin_directory}:{os.environ['PATH']}",
                                                   "NPM_LOG": str(self.log)})
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def test_the_lock_digest_names_an_installation_that_is_published_once(self):
        target = install_packages.install(self.fixtures, self.packages)
        self.assertEqual(target, self.packages / install_packages.digest(self.fixtures))
        self.assertTrue((target / "node_modules/sample/package.json").is_file())
        self.assertEqual(install_packages.install(self.fixtures, self.packages), target)
        self.assertEqual(self.log.read_text().splitlines(),
                         ["ci --install-links --no-bin-links --ignore-scripts --no-audit --no-fund"])
        self.assertEqual(sorted(path.name for path in self.packages.iterdir()), [target.name])

    def test_a_changed_lock_selects_a_new_installation(self):
        first = install_packages.install(self.fixtures, self.packages)
        (self.fixtures / "package-lock.json").write_text('{"lockfileVersion":3,"changed":true}\n')
        second = install_packages.install(self.fixtures, self.packages)
        self.assertNotEqual(first, second)
        self.assertTrue(first.is_dir() and second.is_dir())

    def test_a_symbolic_link_fails_and_publishes_nothing(self):
        with patch.dict(os.environ, {"NPM_LINK": "1"}):
            with self.assertRaisesRegex(ValueError, "installed package contains a symbolic link: .*linked"):
                install_packages.install(self.fixtures, self.packages)
        self.assertEqual(list(self.packages.iterdir()), [])

    def test_a_concurrent_publication_keeps_the_first_installation(self):
        target = self.packages / install_packages.digest(self.fixtures)
        real_rename = os.rename

        def publish_first(source, destination):
            # Another run publishes the same digest between this run's install and its rename.
            (target / "node_modules/sample").mkdir(parents=True)
            for name in install_packages.FILES:
                (target / name).write_bytes((self.fixtures / name).read_bytes())
            (target / "node_modules/sample/package.json").write_text("{}\n")
            real_rename(source, destination)

        with patch.object(install_packages.os, "rename", side_effect=publish_first):
            self.assertEqual(install_packages.install(self.fixtures, self.packages), target)
        self.assertEqual(sorted(path.name for path in self.packages.iterdir()), [target.name])

    def test_the_setup_script_names_the_installation_and_the_sources(self):
        destination = self.root / "nextest-env"
        destination.write_text("")
        with patch.object(install_packages, "FIXTURES", self.fixtures), \
                patch.object(install_packages, "PACKAGES", self.packages), \
                patch.dict(os.environ, {"NEXTEST_ENV": str(destination)}):
            self.assertEqual(install_packages.main([]), 0)
        target = self.packages / install_packages.digest(self.fixtures)
        self.assertEqual(destination.read_text(), f"SSR_PACKAGES={target}\nSSR_FIXTURES={self.fixtures}\n"
                                                  f"SSR_BROWSER={target}/browsers/chromium/chrome\n")

    def test_the_installation_holds_the_pinned_browser(self):
        target = install_packages.install(self.fixtures, self.packages)
        self.assertEqual(install_packages.browser(target), target / "browsers/chromium/chrome")
        (target / "browsers/chromium/chrome").unlink()
        with self.assertRaisesRegex(ValueError, "no chromium executable in .*browsers"):
            install_packages.install(self.fixtures, self.packages)


class FilterTest(TestCase):
    def checkout(self, directory, binaries):
        root = Path(directory)
        (root / ".config").mkdir()
        terms = " | ".join(f"binary_id(={binary})" for binary in binaries)
        (root / ".config/nextest.toml").write_text(
            f"[[profile.default.scripts]]\nfilter = '{terms}'\nsetup = \"install-packages\"\n")
        for path, text in (("crates/a/tests/reads.rs", "mod fixture;\nfn x() {}\n"),
                           ("crates/a/tests/plain.rs", "fn x() {}\n"),
                           ("crates/a/src/lib.rs", "#[cfg(test)]\nmod fixture;\n"),
                           ("crates/b/tests/env.rs", "fn x() { std::env::var(\"SSR_PACKAGES\"); }\n")):
            (root / path).parent.mkdir(parents=True, exist_ok=True)
            (root / path).write_text(text)
        subprocess.run(["git", "init", "-q"], cwd=root, check=True)
        subprocess.run(["git", "add", "-A"], cwd=root, check=True)
        return root

    def test_the_filter_names_exactly_the_readers(self):
        with TemporaryDirectory() as directory:
            root = self.checkout(directory, ["a", "a::reads", "b::env"])
            self.assertEqual(install_packages.filter_errors(root), [])

    def test_a_reader_outside_the_filter_and_a_stale_entry_fail(self):
        with TemporaryDirectory() as directory:
            root = self.checkout(directory, ["a", "a::plain", "b::env"])
            self.assertEqual(install_packages.filter_errors(root), [
                "a::reads reads SSR_PACKAGES or SSR_FIXTURES but is outside the install-packages filter",
                "a::plain is in the install-packages filter but reads no installation"])

    def test_a_filter_that_is_not_a_list_of_binaries_fails(self):
        with TemporaryDirectory() as directory:
            root = self.checkout(directory, ["a"])
            config = root / ".config/nextest.toml"
            config.write_text(config.read_text().replace("binary_id(=a)", "all()"))
            self.assertIn(".config/nextest.toml: install-packages filter term 'all()' is not binary_id(=<id>)",
                          install_packages.filter_errors(root))

    def test_the_repository_filter_names_its_readers(self):
        self.assertEqual(install_packages.filter_errors(), [])
