"""Build and execute the engine with verified local V8 inputs."""

import os
from pathlib import Path
import platform
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from tools import verify_archive


def verify(target):
    started = time.monotonic()
    print(f"START engine build {target}", flush=True)
    try:
        archive, binding = verify_archive.verify(target, ROOT)
        verify_archive.check_metadata(ROOT / "verification/engine", target)
        target_dir = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "verification/engine/target"))
        if not target_dir.is_absolute() or target_dir.resolve() != target_dir:
            raise ValueError(f"CARGO_TARGET_DIR must be absolute and canonical: {target_dir}")
        native_system = {"Darwin": "apple-darwin", "Linux": "unknown-linux-gnu"}.get(platform.system())
        native_machine = {"arm64": "aarch64", "aarch64": "aarch64", "x86_64": "x86_64"}.get(platform.machine())
        native = target == f"{native_machine}-{native_system}"
        if target == "aarch64-unknown-linux-gnu" and not native:
            raise ValueError("this target requires a native AArch64 Linux build environment")
        env = os.environ.copy()
        env.update({"RUSTY_V8_ARCHIVE": str(archive), "RUSTY_V8_SRC_BINDING_PATH": str(binding), "CARGO_NET_OFFLINE": "true"})
        if target.endswith("unknown-linux-gnu") and not native:
            env[f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"] = str(ROOT / "tools" / f"zig-linker-{target.split('-')[0]}")
        command = ["cargo", "build", "--manifest-path", str(ROOT / "verification/engine/Cargo.toml"), "--offline", "--locked", "--target", target, "--verbose"]
        print(f"COMMAND {' '.join(command)}", flush=True)
        result = subprocess.run(command, cwd=ROOT, env=env, check=False)
        if result.returncode:
            raise ValueError(f"Cargo build exit={result.returncode}")
        print(f"PASS engine build {target} {time.monotonic() - started:.3f}s", flush=True)
        if native:
            executable = target_dir / target / "debug/polyspec-ssr-engine-verification"
            print(f"START engine execution {target} executable={executable}", flush=True)
            execution_start = time.monotonic()
            # The execution normally ends within a second; its limit only ends a hung execution.
            try:
                result = subprocess.run([str(executable)], env=env, timeout=30, check=False)
            except subprocess.TimeoutExpired:
                print(f"HUNG engine execution {target}: {executable} did not exit within its 30 s limit "
                      f"({time.monotonic() - execution_start:.3f}s)", file=sys.stderr)
                return 1
            if result.returncode:
                raise ValueError(f"engine execution exit={result.returncode}")
            print(f"PASS engine execution {target} {time.monotonic() - execution_start:.3f}s", flush=True)
    except (OSError, ValueError) as error:
        print(f"FAIL engine verification {target} {time.monotonic() - started:.3f}s: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("usage: verify_engine.py TARGET", file=sys.stderr)
        sys.exit(2)
    sys.exit(verify(sys.argv[1]))
