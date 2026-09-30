"""Read the absolute local checkout paths declared by Cargo manifests."""

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
MANIFESTS = (
    ROOT / "Cargo.toml",
    ROOT / "tools/build-probe/Cargo.toml",
    ROOT / "verification/engine/Cargo.toml",
)
ENTRY = re.compile(r'^([A-Za-z0-9_-]+)\s*=\s*\{[^{}]*\bpath\s*=\s*"([^"]+)"[^{}]*\}\s*$')


def path_dependencies(manifest: Path) -> dict[str, Path]:
    """Return every absolute path dependency of one manifest by crate name.

    Only single-line inline tables are accepted; any other line that names a
    path is an error, so no declaration is skipped.
    """
    found = {}
    for number, line in enumerate(manifest.read_text().splitlines(), 1):
        if not re.search(r"\bpath\s*=", line):
            continue
        entry = ENTRY.match(line.strip())
        if entry is None:
            raise ValueError(f"{manifest}:{number}: unsupported path declaration")
        name, value = entry.groups()
        path = Path(value)
        if not path.is_absolute():
            continue
        if name in found and found[name] != path:
            raise ValueError(f"{manifest}:{number}: {name} names two paths")
        found[name] = path
    return found


def declared(root: Path = ROOT) -> dict[str, Path]:
    """Return the root manifest paths of the V8, CSS and JSON checkouts."""
    paths = path_dependencies(root / "Cargo.toml")
    missing = sorted({"v8", "lightningcss", "ordered-json"} - set(paths))
    if missing:
        raise ValueError(f"{root / 'Cargo.toml'}: missing path dependencies {', '.join(missing)}")
    return paths


def checkout(path: Path) -> Path:
    """Return the Git checkout root that contains a declared path."""
    for candidate in (path, *path.parents):
        if (candidate / ".git").exists():
            return candidate
    raise ValueError(f"{path} is not inside a Git checkout")
