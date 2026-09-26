"""Verify the local V8 archive used by isolate-group builds."""

import hashlib
import os
from pathlib import Path
import platform
import shutil
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
ARCHIVES = {
    "aarch64-apple-darwin": (
        "b18bcc65f8cb5f3249bea722614caf8ac1a5f5cef35ee9e01a91b6ec245cb9a0",
        "e71f32a0f97f99e13566c5da28804ad09421a0ecd600fbb6b79f301b9759ee2a",
    ),
}


def digest(path):
    sha = hashlib.sha256()
    with path.open("rb") as content:
        for chunk in iter(lambda: content.read(1024 * 1024), b""):
            sha.update(chunk)
    return sha.hexdigest()


def verify(archive, source, expected, binding, binding_source, binding_expected, install):
    start = time.monotonic()
    operation = "V8 archive install" if install else "V8 archive verification"
    print(f"START {operation}", flush=True)

    def fail(message):
        print(f"FAIL {operation} {time.monotonic() - start:.1f}s: {message}", file=sys.stderr)
        return 1

    files = ((archive, source, expected), (binding, binding_source, binding_expected))
    if any(not path.is_absolute() for pair in files for path in pair[:2]):
        return fail("archive, binding and Cargo output paths must be absolute")
    destinations = {archive.resolve(), binding.resolve()}
    sources = {source.resolve(), binding_source.resolve()}
    if len(destinations) != 2 or len(sources) != 2 or destinations & sources:
        return fail("archive and binding paths must be distinct from Cargo outputs")
    if not install and os.environ.get("V8_FROM_SOURCE", "").lower() in {"1", "true", "yes"}:
        return fail("routine checks cannot select a V8 source build")
    try:
        if install:
            for _, origin, expected_digest in files:
                if not origin.is_file():
                    return fail(f"missing V8 source file: {origin}")
                actual = digest(origin)
                if actual != expected_digest:
                    return fail(f"V8 source SHA-256 mismatch: {origin}: {actual}")
            for destination, origin, expected_digest in files:
                destination.parent.mkdir(parents=True, exist_ok=True)
                temporary = None
                try:
                    with tempfile.NamedTemporaryFile(dir=destination.parent, prefix=".v8-", delete=False) as output:
                        temporary = Path(output.name)
                        with origin.open("rb") as content:
                            shutil.copyfileobj(content, output)
                    copied_digest = digest(temporary)
                    if copied_digest != expected_digest:
                        return fail(f"copied V8 file SHA-256 mismatch: {temporary}: {copied_digest}")
                    temporary.replace(destination)
                finally:
                    if temporary is not None:
                        temporary.unlink(missing_ok=True)
        for destination, _, expected_digest in files:
            if not destination.is_file():
                return fail(f"missing V8 file: {destination}")
            actual = digest(destination)
            if actual != expected_digest:
                return fail(f"V8 file SHA-256 mismatch: {destination}: {actual}")
    except OSError as error:
        return fail(str(error))
    print(f"PASS {operation} {time.monotonic() - start:.1f}s archive={expected} binding={binding_expected}", flush=True)
    return 0


def main():
    if len(sys.argv) != 2 or sys.argv[1] not in {"verify", "install"}:
        print("usage: verify_group_archive.py verify|install", file=sys.stderr)
        return 2
    machine = platform.machine()
    target = "aarch64-apple-darwin" if platform.system() == "Darwin" and machine == "arm64" else None
    if target not in ARCHIVES:
        print(f"FAIL unsupported V8 source archive target: {platform.system()} {machine}", file=sys.stderr)
        return 1
    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target_dir.is_absolute():
        print(f"FAIL CARGO_TARGET_DIR must be absolute: {target_dir}", file=sys.stderr)
        return 1
    archive = ROOT / "var/v8" / f"librusty_v8_source_{target}.a"
    source = target_dir / "debug/gn_out/obj/librusty_v8.a"
    binding = ROOT / "var/v8" / f"src_binding_source_{target}.rs"
    binding_source = target_dir / "debug/gn_out/src_binding.rs"
    selected = os.environ.get("RUSTY_V8_ARCHIVE")
    if selected is not None and selected != str(archive):
        print(f"FAIL RUSTY_V8_ARCHIVE differs from verified archive: {selected}", file=sys.stderr)
        return 1
    selected_binding = os.environ.get("RUSTY_V8_SRC_BINDING_PATH")
    if selected_binding is not None and selected_binding != str(binding):
        print(f"FAIL RUSTY_V8_SRC_BINDING_PATH differs from verified binding: {selected_binding}", file=sys.stderr)
        return 1
    archive_hash, binding_hash = ARCHIVES[target]
    return verify(archive, source, archive_hash, binding, binding_source, binding_hash, sys.argv[1] == "install")


if __name__ == "__main__":
    sys.exit(main())
