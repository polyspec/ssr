"""Write the engine Compose file with absolute host bind mount paths."""

from pathlib import Path
from string import Template
import sys


ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = ROOT / "verification/engine/linux/compose.yaml"
OUTPUT = ROOT / "var/engine-compose.yaml"


def main() -> int:
    cargo = ROOT / "var/engine-cargo"
    target = ROOT / "var/engine-target"
    v8 = Path("/Users/maxkwon/soksakim-project/v8-local")
    if not (v8 / "Cargo.toml").is_file():
        print(f"FAIL engine Compose preparation: missing V8 source {v8}", file=sys.stderr)
        return 1
    cargo.mkdir(parents=True, exist_ok=True)
    target.mkdir(parents=True, exist_ok=True)
    try:
        content = Template(TEMPLATE.read_text()).substitute(
            SSR_SOURCE_DIR=ROOT,
            SSR_V8_DIR=v8,
            SSR_CARGO_DIR=cargo,
            SSR_TARGET_DIR=target,
        )
        OUTPUT.write_text(content)
    except (OSError, KeyError, ValueError) as error:
        print(f"FAIL engine Compose preparation: {error}", file=sys.stderr)
        return 1
    print(f"PASS engine Compose preparation {OUTPUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
