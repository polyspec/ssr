import json
import os
import pathlib
import subprocess
import sys
import time


def main():
    started = time.monotonic()
    print("RUN build render benchmark", flush=True)
    command = ["cargo", "build", "--offline", "--locked", "-p", "ssr-runtime", "--example", "bench", "--verbose"]
    print(f"COMMAND {' '.join(command)}", flush=True)
    build = subprocess.run(command, check=False)
    if build.returncode:
        print(f"FAIL build render benchmark {time.monotonic() - started:.3f}s", file=sys.stderr)
        return build.returncode
    metadata = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
        capture_output=True,
        text=True,
        check=True,
        timeout=30,
    )
    target = pathlib.Path(os.environ.get("CARGO_TARGET_DIR", json.loads(metadata.stdout)["target_directory"]))
    if not target.is_absolute():
        print(f"FAIL Cargo target directory must be absolute: {target}", file=sys.stderr)
        return 1
    executable = target / "debug" / "examples" / ("bench.exe" if os.name == "nt" else "bench")
    if not executable.is_file():
        print(f"FAIL missing render benchmark executable: {executable}", file=sys.stderr)
        return 1
    print(f"PASS build render benchmark {time.monotonic() - started:.3f}s", flush=True)
    started = time.monotonic()
    print("RUN render benchmark", flush=True)
    try:
        result = subprocess.run([str(executable)], timeout=120, check=False)
    except subprocess.TimeoutExpired:
        print(f"TIMEOUT render benchmark {time.monotonic() - started:.3f}s", file=sys.stderr)
        return 1
    print(f"{'PASS' if result.returncode == 0 else 'FAIL'} render benchmark {time.monotonic() - started:.3f}s", flush=True)
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
