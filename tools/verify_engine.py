"""Build the pinned engine combination for one target with a verified V8 archive."""

import hashlib
import os
from pathlib import Path
import platform
import subprocess
import sys
import time
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
ARCHIVES = {
    "aarch64-apple-darwin": "5aeffd8d5a0c1b79ac1d70af83d5b19099655fd9c645a794dc43f101f779838c",
    "x86_64-apple-darwin": "a750271fec6b211457ed0a5cf7d2eab1924b265621a82da86ab959d6ff0823e4",
    "aarch64-unknown-linux-gnu": "539e283815a396a5796f32858b42e517b858ebaaeaaad05d03290ee8c864a527",
    "x86_64-unknown-linux-gnu": "f48762ca10d1f1fc605a441c5ae430ec8ce1e9e80f14d78fbc42cb878c30b476",
}


def verify(target: str, fetch: bool, archive_only: bool) -> int:
    if target not in ARCHIVES:
        print(f"FAIL unsupported target {target}", file=sys.stderr)
        return 2
    archive = ROOT / "var/v8" / f"librusty_v8_simdutf_release_{target}.a.gz"
    operation = "engine archive" if archive_only else "engine build"
    print(f"START {operation} {target}", flush=True)
    start = time.monotonic()

    def fail(message: str) -> int:
        print(f"FAIL {operation} {target} {time.monotonic() - start:.1f}s: {message}", file=sys.stderr)
        return 1

    native_linux = False
    if not archive_only:
        native_linux = (
            platform.system() == "Linux"
            and target == f"{platform.machine()}-unknown-linux-gnu"
        )
        if target == "aarch64-unknown-linux-gnu" and not native_linux:
            return fail("this target requires a native AArch64 Linux build environment")
        target_dir = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "verification/engine/target"))
        if not target_dir.is_absolute():
            return fail(f"CARGO_TARGET_DIR must be absolute: {target_dir}")

    if fetch and not archive.is_file():
        archive.parent.mkdir(parents=True, exist_ok=True)
        url = (
            "https://github.com/denoland/rusty_v8/releases/download/v150.4.0/"
            f"{archive.name}"
        )
        temporary = archive.with_suffix(archive.suffix + ".part")
        try:
            with urllib.request.urlopen(url, timeout=120) as response, temporary.open("wb") as output:
                while chunk := response.read(1024 * 1024):
                    output.write(chunk)
            if hashlib.sha256(temporary.read_bytes()).hexdigest() != ARCHIVES[target]:
                raise ValueError(f"archive SHA-256 mismatch: {url}")
            temporary.replace(archive)
        except (OSError, ValueError) as error:
            temporary.unlink(missing_ok=True)
            return fail(f"archive download: {error}")
    if not archive.is_file():
        return fail(f"missing archive {archive}")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    if digest != ARCHIVES[target]:
        return fail(f"archive SHA-256: {digest}")
    if archive_only:
        print(f"PASS engine archive {target} {time.monotonic() - start:.1f}s SHA-256={digest}")
        return 0
    env = os.environ.copy()
    env["RUSTY_V8_ARCHIVE"] = str(archive)
    if target.endswith("unknown-linux-gnu") and not native_linux:
        env[f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"] = str(
            ROOT / "tools" / f"zig-linker-{target.split('-')[0]}"
        )
    command = [
        "cargo", "build", "--manifest-path", str(ROOT / "verification/engine/Cargo.toml"),
        "--locked", "--target", target,
    ]
    try:
        result = subprocess.run(command, cwd=ROOT, env=env, timeout=900, check=False)
    except subprocess.TimeoutExpired:
        print(f"TIMEOUT engine build {target} {time.monotonic() - start:.1f}s")
        return 1
    elapsed = time.monotonic() - start
    if result.returncode:
        return fail(f"exit={result.returncode}")
    print(f"PASS engine build {target} {elapsed:.1f}s SHA-256={digest}")
    if native_linux:
        executable = target_dir / target / "debug/ssr-engine-verification"
        print(f"START engine execution {target}", flush=True)
        execution_start = time.monotonic()
        try:
            execution = subprocess.run([str(executable)], env=env, timeout=30, check=False)
        except (OSError, subprocess.TimeoutExpired) as error:
            print(
                f"FAIL engine execution {target} {time.monotonic() - execution_start:.1f}s: {error}",
                file=sys.stderr,
            )
            return 1
        if execution.returncode:
            print(
                f"FAIL engine execution {target} {time.monotonic() - execution_start:.1f}s: "
                f"exit={execution.returncode}",
                file=sys.stderr,
            )
            return 1
        print(f"PASS engine execution {target} {time.monotonic() - execution_start:.1f}s")
    return 0


if __name__ == "__main__":
    if len(sys.argv) not in (2, 3) or (len(sys.argv) == 3 and sys.argv[2] not in ("--fetch", "--fetch-only")):
        print("usage: python3 tools/verify_engine.py TARGET [--fetch|--fetch-only]", file=sys.stderr)
        sys.exit(2)
    option = sys.argv[2] if len(sys.argv) == 3 else ""
    sys.exit(verify(sys.argv[1], bool(option), option == "--fetch-only"))
