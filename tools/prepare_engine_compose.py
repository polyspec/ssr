"""Write the engine Compose file of this checkout with absolute host bind mount paths.

Each checkout has its own Compose project, container and image, named from the SHA-256 of the
checkout path, so verifications of two checkouts do not replace each other's stack. Two sessions
in one checkout share the stack through the holder lock of ``tools/verify_engine_linux.py``.
"""

import hashlib
from pathlib import Path
from string import Template
import sys



ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = ROOT / "verification/engine/linux/compose.yaml"
OUTPUT = ROOT / "var/engine-compose.yaml"


def names(root=ROOT):
    """The Compose project, container and image names of the checkout ``root``."""
    digest = hashlib.sha256(str(root).encode()).hexdigest()[:12]
    project = f"ssr-engine-{digest}"
    return {"project": project, "container": f"{project}-engine",
            "image": f"localhost/ssr-engine-verify-{digest}:0.0.1"}


def render(root=ROOT):
    """The Compose file of the checkout ``root``. Every dependency comes from a registry or a
    pinned Git commit, which ``cargo fetch`` reads into the Cargo cache, so no other checkout is
    mounted."""
    stack = names(root)
    return Template(TEMPLATE.read_text()).substitute(
        SSR_ENGINE_PROJECT=stack["project"],
        SSR_ENGINE_IMAGE=stack["image"],
        SSR_SOURCE_DIR=root,
        SSR_CARGO_DIR=root / "var/engine-cargo",
        SSR_TARGET_DIR=root / "var/engine-target",
    )


def main() -> int:
    try:
        content = render(ROOT)
        (ROOT / "var/engine-cargo").mkdir(parents=True, exist_ok=True)
        (ROOT / "var/engine-target").mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text(content)
    except (OSError, KeyError) as error:
        print(f"FAIL engine Compose preparation: {error}", file=sys.stderr)
        return 1
    print(f"PASS engine Compose preparation {OUTPUT} ({names(ROOT)['project']})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
