"""Build, lint and test ``ssr-server`` natively on AArch64 Linux in this checkout's container stack.

The stack is the per-checkout engine stack (``tools/prepare_engine_compose.py``): a container
built from ``verification/engine/linux/Dockerfile`` with its Linux gcc toolchain, Rust and
cargo-nextest. The checkout and the declared path dependency checkouts are mounted read only;
only the Cargo cache and the build output are writable. The run holds the checkout lock
``engine-verification`` that ``tools/verify_engine_linux.py`` also holds, so the two never use
the stack at the same time. Every step prints its command, its output as it arrives and its
result with the elapsed time; no step has a time limit. The first failing step ends the steps, and
once the stack was started it is stopped after the steps, also after a failure.

The test targets of ``ssr-server`` that write generated entries into the checkout's build-probe
fixture cannot write to the read-only mount; the target runs the unit tests and the
``development`` and ``process`` targets, which cover the Linux source watch and process code.

    python3 -m tools.check_linux
"""

from pathlib import Path
import sys

from tools.prepare_engine_compose import OUTPUT, names
from tools.verify_engine_linux import LOCK, run_locked, stop


ROOT = Path(__file__).resolve().parents[1]
TARGET = "aarch64-unknown-linux-gnu"


def steps(root=ROOT):
    stack = names(root)
    container = stack["container"]
    offline = ["container", "exec", "-w", "/src", "-e", "CARGO_NET_OFFLINE=true", container]
    return [
        [sys.executable, "tools/verify_archive.py", TARGET],
        [sys.executable, "-m", "tools.prepare_engine_compose"],
        ["container", "build", "--platform", "linux/arm64", "-f", "verification/engine/linux/Dockerfile",
         "-t", stack["image"], "verification/engine/linux"],
        ["containerctl", "-f", str(OUTPUT), "up"],
        [sys.executable, "-m", "tools.verify_engine_status"],
        ["container", "exec", "-w", "/src", container, "python3", "/src/tools/verify_engine_mounts.py"],
        ["container", "exec", "-w", "/src", "-e", "CARGO_NET_OFFLINE=false", container, "cargo", "fetch",
         "--locked"],
        [*offline, "cargo", "clippy", "--locked", "-p", "ssr-server", "--all-targets", "--", "-D", "warnings"],
        [*offline, "cargo", "build", "--locked", "-p", "ssr-server", "--example", "development_process",
         "--example", "socket_process"],
        [*offline, "cargo", "nextest", "run", "--locked", "-p", "ssr-server", "--lib", "--test", "development",
         "--test", "process", "--no-tests", "fail"],
    ]


def main(argv):
    if argv:
        print("usage: python3 -m tools.check_linux", file=sys.stderr)
        return 2
    commands = steps()
    return run_locked(commands, LOCK, stop(), commands[3])


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
