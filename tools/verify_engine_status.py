"""Check that containerctl can read the generated engine Compose file of this checkout."""

import json
from pathlib import Path
import subprocess
import sys
import time

from tools.prepare_engine_compose import names


ROOT = Path(__file__).resolve().parents[1]
COMPOSE = ROOT / "var/engine-compose.yaml"


def main() -> int:
    start = time.monotonic()
    print("START engine Compose status", flush=True)
    try:
        result = subprocess.run(
            ["containerctl", "status", "--json"],
            capture_output=True,
            text=True,
            timeout=30,
            check=True,
        )
        groups = json.loads(result.stdout)["groups"]
        group = next(group for group in groups if group["name"] == names(ROOT)["project"])
        if group.get("error") or group["stackPath"] != str(COMPOSE):
            raise ValueError(f"invalid engine Compose status: {group.get('error')}, {group['stackPath']}")
        if not any(service["name"] == "engine" for service in group["services"]):
            raise ValueError("engine service is missing")
    except subprocess.TimeoutExpired:
        print(f"TIMEOUT engine Compose status {time.monotonic() - start:.3f}s", file=sys.stderr)
        return 1
    except (OSError, subprocess.CalledProcessError, ValueError, KeyError, StopIteration) as error:
        print(f"FAIL engine Compose status {time.monotonic() - start:.3f}s: {error}", file=sys.stderr)
        return 1
    print(f"PASS engine Compose status {time.monotonic() - start:.3f}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
