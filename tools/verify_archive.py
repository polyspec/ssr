"""Verify the exact local V8 archive, binding and Cargo features before linking."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
ARCHIVES = {
    "aarch64-apple-darwin": ("5aeffd8d5a0c1b79ac1d70af83d5b19099655fd9c645a794dc43f101f779838c", "ca5adf0cf89c9a70ad460ae73648b2fe89b74aa113b3cb7f757b6a02b758394f"),
    "x86_64-apple-darwin": ("a750271fec6b211457ed0a5cf7d2eab1924b265621a82da86ab959d6ff0823e4", "ca5adf0cf89c9a70ad460ae73648b2fe89b74aa113b3cb7f757b6a02b758394f"),
    "aarch64-unknown-linux-gnu": ("539e283815a396a5796f32858b42e517b858ebaaeaaad05d03290ee8c864a527", "7727826ae479bdb645e807239fb12d1f8e2e23de7a6cf16f5ee592690d1d8506"),
    "x86_64-unknown-linux-gnu": ("f48762ca10d1f1fc605a441c5ae430ec8ce1e9e80f14d78fbc42cb878c30b476", "7727826ae479bdb645e807239fb12d1f8e2e23de7a6cf16f5ee592690d1d8506"),
}


def digest(path):
    sha = hashlib.sha256()
    with path.open("rb") as content:
        for chunk in iter(lambda: content.read(1024 * 1024), b""):
            sha.update(chunk)
    return sha.hexdigest()


def check_disk(root):
    free = shutil.disk_usage(root).free
    if free < 10 * 1024**3:
        raise ValueError(f"at least 10 GiB free disk space is required: available_bytes={free}")


def check_files(archive, binding, archive_hash, binding_hash):
    for path, expected in ((archive, archive_hash), (binding, binding_hash)):
        if not path.is_absolute() or path.resolve() != path or path.is_symlink():
            raise ValueError(f"V8 input must be an absolute canonical regular file: {path}")
        if not path.is_file():
            raise ValueError(f"missing V8 input: {path}")
        actual = digest(path)
        if actual != expected:
            raise ValueError(f"V8 input SHA-256 mismatch: {path}: {actual}")


def check_environment(archive, binding):
    if os.environ.get("V8_FROM_SOURCE", "").lower() in {"1", "true", "yes"}:
        raise ValueError("V8 source builds are not permitted")
    if os.environ.get("V8_FORCE_DEBUG", "").lower() in {"1", "true", "yes"}:
        raise ValueError("the verified V8 archive requires the release engine profile")
    for name in ("DOCS_RS", "DENO_TRYBUILD"):
        if name in os.environ:
            raise ValueError(f"{name} cannot bypass the verified engine build")
    for name, path in (("RUSTY_V8_ARCHIVE", archive), ("RUSTY_V8_SRC_BINDING_PATH", binding)):
        selected = os.environ.get(name)
        if selected is not None and selected != str(path):
            raise ValueError(f"{name} differs from the verified local input: {selected}")


def check_features(metadata):
    packages = [package for package in metadata["packages"] if package["name"] == "v8"]
    if [package["version"] for package in packages] != ["150.4.0"]:
        raise ValueError(f"exactly V8 150.4.0 must be selected; selected versions: "
                         f"{[package['version'] for package in packages]}")
    package_id = packages[0]["id"]
    nodes = [node for node in metadata["resolve"]["nodes"] if node["id"] == package_id]
    features = [sorted(set(node["features"]) - {"default"}) for node in nodes]
    if features != [["simdutf", "use_custom_libcxx"]]:
        raise ValueError(f"V8 Cargo features do not match the simdutf release archive: expected "
                         f"['simdutf', 'use_custom_libcxx'], actual {features}")


def check_metadata(root, target):
    if not root.is_absolute() or root.resolve() != root or not root.is_dir():
        raise ValueError(f"metadata root must be an absolute canonical directory: {root}")
    check_disk(root)
    result = subprocess.run(["cargo", "metadata", "--offline", "--locked", "--format-version", "1", "--filter-platform", target], cwd=root, capture_output=True, text=True, check=False)
    if result.returncode:
        raise ValueError(f"Cargo metadata failed: {result.stderr}")
    check_features(json.loads(result.stdout))


def verify(target, root=ROOT):
    if target not in ARCHIVES:
        raise ValueError(f"unsupported V8 target: {target}")
    archive = root / "var/v8" / f"librusty_v8_simdutf_release_{target}.a.gz"
    binding = root / "var/v8" / f"src_binding_simdutf_release_{target}.rs"
    check_environment(archive, binding)
    check_files(archive, binding, *ARCHIVES[target])
    return archive, binding


def main():
    started = time.monotonic()
    print("START V8 archive verification", flush=True)
    try:
        parser = argparse.ArgumentParser(description=__doc__)
        parser.add_argument("target", nargs="?")
        parser.add_argument("--archive", type=Path)
        parser.add_argument("--binding", type=Path)
        parser.add_argument("--metadata-root", type=Path, default=ROOT)
        parser.add_argument("--files-only", action="store_true")
        args = parser.parse_args()
        system = {"Darwin": "apple-darwin", "Linux": "unknown-linux-gnu"}.get(platform.system())
        machine = {"arm64": "aarch64", "aarch64": "aarch64", "x86_64": "x86_64"}.get(platform.machine())
        target = args.target if args.target is not None else f"{machine}-{system}"
        if target not in ARCHIVES:
            raise ValueError(f"unsupported V8 target: {target}")
        if (args.archive is None) != (args.binding is None):
            raise ValueError("archive and binding must be selected together")
        if args.archive is None:
            archive, binding = verify(target)
        else:
            archive, binding = args.archive, args.binding
            check_environment(archive, binding)
            check_files(archive, binding, *ARCHIVES[target])
        if not args.files_only:
            check_metadata(args.metadata_root, target)
    except (OSError, ValueError, KeyError) as error:
        print(f"FAIL V8 archive verification {time.monotonic() - started:.3f}s: {error}", file=sys.stderr)
        return 1
    print(f"PASS V8 archive verification {time.monotonic() - started:.3f}s target={target} archive={archive} archive_sha256={ARCHIVES[target][0]} binding={binding} binding_sha256={ARCHIVES[target][1]} metadata={not args.files_only}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
