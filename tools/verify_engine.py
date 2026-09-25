"""Build the pinned engine combination for one target with a verified V8 archive."""

import hashlib
import os
from pathlib import Path
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


def verify(target: str, fetch: bool) -> int:
    if target not in ARCHIVES:
        print(f"FAIL unsupported target {target}", file=sys.stderr)
        return 2
    archive = ROOT / "var/v8" / f"librusty_v8_simdutf_release_{target}.a.gz"
    print(f"START engine build {target}", flush=True)
    start = time.monotonic()

    def fail(message: str) -> int:
        print(f"FAIL engine build {target} {time.monotonic() - start:.1f}s: {message}", file=sys.stderr)
        return 1

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
    env = os.environ.copy()
    env["RUSTY_V8_ARCHIVE"] = str(archive)
    if target.endswith("unknown-linux-gnu"):
        env[f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"] = str(
            ROOT / "tools" / f"zig-linker-{target.split('-')[0]}"
        )
        env["CXXSTDLIB"] = "c++"
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
    return 0


if __name__ == "__main__":
    if len(sys.argv) not in (2, 3) or (len(sys.argv) == 3 and sys.argv[2] != "--fetch"):
        print("usage: python3 tools/verify_engine.py TARGET [--fetch]", file=sys.stderr)
        sys.exit(2)
    sys.exit(verify(sys.argv[1], len(sys.argv) == 3))
