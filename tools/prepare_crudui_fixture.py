import base64
import hashlib
import json
import pathlib
import subprocess
import sys
import time


ROOT = pathlib.Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tools/build-probe/tests/fixtures"
DESTINATION = ROOT / "var/crudui-fixture"
PACKAGES = {
    "@crudui/validator": "validator-ts",
    "@crudui/generator-core": "generator-core",
    "@crudui/generator-react": "generator-react",
}


def run(command, cwd, json_output=False):
    print(f"RUN {' '.join(command)}", flush=True)
    started = time.monotonic()
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, timeout=60)
    if result.returncode:
        raise RuntimeError(
            f"command failed ({result.returncode}): {' '.join(command)}\n"
            f"{result.stdout}{result.stderr}"
        )
    if result.stderr:
        sys.stderr.write(result.stderr)
    if result.stdout and not json_output:
        sys.stdout.write(result.stdout)
    print(f"PASS {' '.join(command)} {time.monotonic() - started:.3f}s", flush=True)
    return result.stdout


def checkout():
    path = ROOT / "var/checkouts.txt"
    if path.resolve(strict=True) != path:
        raise ValueError("checkout declaration must not contain symbolic links")
    lines = path.read_text().splitlines()
    if len(lines) != 1 or not lines[0].startswith("crudui "):
        raise ValueError("var/checkouts.txt must declare exactly one crudui checkout")
    root = pathlib.Path(lines[0].removeprefix("crudui "))
    if not root.is_absolute() or root.resolve(strict=True) != root:
        raise ValueError("crudui checkout must be an absolute path without symbolic links")
    return root


def prepare():
    source_root = checkout()
    DESTINATION.mkdir(parents=True, exist_ok=True)
    if DESTINATION.resolve(strict=True) != DESTINATION:
        raise ValueError("fixture directory must not contain symbolic links")
    archives = DESTINATION / "archives"
    archives.mkdir(exist_ok=True)
    if archives.resolve(strict=True) != archives:
        raise ValueError("archive directory must not contain symbolic links")
    dependencies = {"react": "19.1.1", "react-dom": "19.1.1", "web-streams-polyfill": "4.2.0"}
    for name, folder in PACKAGES.items():
        package = source_root / "packages" / folder
        if package.resolve(strict=True) != package:
            raise ValueError(f"package path must not contain symbolic links: {package}")
        manifest = json.loads((package / "package.json").read_text())
        if manifest.get("name") != name or manifest.get("version") != "0.0.1":
            raise ValueError(f"unexpected package identity: {package}")
        if not (package / "dist").is_dir() or (package / "dist").resolve() != package / "dist":
            raise ValueError(f"package build output missing: {package}")
        output = run(
            ["npm", "pack", "--json", "--ignore-scripts", "--pack-destination", str(archives)],
            package,
            json_output=True,
        )
        packed = json.loads(output)
        if len(packed) != 1 or packed[0]["name"] != name or packed[0]["version"] != "0.0.1":
            raise ValueError(f"unexpected npm archive result: {package}")
        archive = archives / packed[0]["filename"]
        if not archive.is_file():
            raise ValueError(f"npm archive missing: {archive}")
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        archive_with_digest = archives / f"{archive.stem}-{digest}.tgz"
        archive.replace(archive_with_digest)
        dependencies[name] = f"file:{archive_with_digest}"

    manifest = {"private": True, "name": "ssr-crudui-verification", "version": "0.0.1", "dependencies": dependencies}
    (DESTINATION / "package.json").write_text(json.dumps(manifest, indent=2) + "\n")
    for name in ("CruduiApp.tsx", "CruduiApp.css"):
        (DESTINATION / name).write_bytes((SOURCE / name).read_bytes())
    (DESTINATION / "package-lock.json").unlink(missing_ok=True)
    run(["npm", "install", "--package-lock-only", "--install-links", "--ignore-scripts", "--no-audit", "--no-fund"], DESTINATION)
    run(["npm", "ci", "--install-links", "--ignore-scripts", "--no-audit", "--no-fund"], DESTINATION)

    lock = json.loads((DESTINATION / "package-lock.json").read_text())
    if lock["packages"][""]["dependencies"] != dependencies:
        raise ValueError("npm lock dependencies differ from declared inputs")
    for path in (DESTINATION / "node_modules").rglob("*"):
        if path.is_symlink():
            raise ValueError(f"installed package contains a symbolic link: {path}")
    for name in PACKAGES:
        location = DESTINATION / "node_modules" / name
        if location.is_symlink() or not location.is_dir():
            raise ValueError(f"installed package must be a directory: {name}")
        installed = json.loads((location / "package.json").read_text())
        if installed.get("name") != name or installed.get("version") != "0.0.1":
            raise ValueError(f"installed package identity differs: {name}")
        record = lock["packages"][f"node_modules/{name}"]
        archive = pathlib.Path(dependencies[name].removeprefix("file:"))
        integrity = "sha512-" + base64.b64encode(hashlib.sha512(archive.read_bytes()).digest()).decode()
        resolved = record.get("resolved", "")
        if not resolved.startswith("file:"):
            raise ValueError(f"installed package lock has no file source: {name}")
        locked_archive = pathlib.Path(resolved.removeprefix("file:"))
        if not locked_archive.is_absolute():
            locked_archive = DESTINATION / locked_archive
        if locked_archive.resolve(strict=True) != archive or record.get("integrity") != integrity:
            raise ValueError(f"installed package lock differs: {name}")
    print("PASS CRUDUI package installation matches the lock", flush=True)


if __name__ == "__main__":
    try:
        prepare()
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired, RuntimeError) as error:
        print(f"FAIL CRUDUI fixture preparation: {error}", file=sys.stderr)
        sys.exit(1)
