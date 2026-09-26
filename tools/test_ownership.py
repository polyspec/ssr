"""Validate and execute the tests declared for shared workspace behavior."""

from dataclasses import dataclass
import json
from pathlib import Path
import re
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
DECLARATION = ROOT / "tools/test-ownership.json"
IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")


class OwnershipError(Exception):
    pass


@dataclass(frozen=True)
class Case:
    behavior: str
    crate: str
    target: str
    test: str


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise OwnershipError(f"duplicate declaration key: {key}")
        result[key] = value
    return result


def workspace(root):
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--offline", "--format-version", "1"],
        cwd=root, capture_output=True, text=True, timeout=30, check=False,
    )
    if result.returncode:
        raise OwnershipError(f"workspace metadata failed: {result.stderr}")
    data = json.loads(result.stdout)
    packages = {package["id"]: package for package in data["packages"]}
    members = {packages[member]["name"]: packages[member] for member in data["workspace_members"]}
    consumers = {name: set() for name in members}
    for name, package in members.items():
        for dependency in package["dependencies"]:
            if dependency["name"] in members and dependency["path"] is not None:
                consumers[dependency["name"]].add(name)
    return members, consumers


def substantive(source, start):
    position = start
    while position < len(source):
        if source[position].isspace():
            position += 1
        elif source.startswith("//", position):
            end = source.find("\n", position + 2)
            position = len(source) if end < 0 else end + 1
        elif source.startswith("/*", position):
            end = source.find("*/", position + 2)
            if end < 0:
                raise OwnershipError("unclosed comment in test")
            position = end + 2
        else:
            return source[position] != "}"
    return False


def case(root, behavior, value, members):
    if not isinstance(value, dict) or set(value) != {"crate", "target", "test"}:
        raise OwnershipError(f"{behavior}: each case must name crate, target and test")
    crate, target, test = (value[name] for name in ("crate", "target", "test"))
    if crate not in members or any(not isinstance(item, str) or not IDENTIFIER.fullmatch(item)
                                   for item in (target, test)):
        raise OwnershipError(f"{behavior}: invalid crate, target or test")
    path = root / "crates" / crate / "tests" / f"{target}.rs"
    if path.is_symlink() or not path.is_file():
        raise OwnershipError(f"{behavior}: missing test target in {crate}: {path}")
    source = path.read_text()
    pattern = re.compile(
        r"#\[(?:tokio::)?test\]\s*(?:async\s+)?fn\s+" + re.escape(test)
        + r"\s*\(\s*\)\s*\{"
    )
    matches = list(pattern.finditer(source))
    if len(matches) != 1:
        raise OwnershipError(f"{behavior}: missing or repeated test {crate}::{target} {test}")
    if not substantive(source, matches[0].end()):
        raise OwnershipError(f"{behavior}: empty test {crate}::{target} {test}")
    return Case(behavior, crate, target, test)


def validate(root, declaration):
    root = Path(root)
    members, dependents = workspace(root)
    if not isinstance(declaration, dict) or set(declaration) != {"behaviors"}:
        raise OwnershipError("declaration must contain only behaviors")
    behaviors = declaration["behaviors"]
    if not isinstance(behaviors, list) or not behaviors:
        raise OwnershipError("at least one behavior is required")
    cases = []
    seen = set()
    for behavior in behaviors:
        if not isinstance(behavior, dict) or set(behavior) != {"id", "owner", "consumers"}:
            raise OwnershipError("behavior must name id, owner and consumers")
        name = behavior["id"]
        if not isinstance(name, str) or not IDENTIFIER.fullmatch(name) or name in seen:
            raise OwnershipError(f"invalid or repeated behavior: {name}")
        seen.add(name)
        owner = case(root, name, behavior["owner"], members)
        declared = behavior["consumers"]
        if not isinstance(declared, list):
            raise OwnershipError(f"{name}: consumers must be a list")
        consumers = [case(root, name, item, members) for item in declared]
        names = [item.crate for item in consumers]
        expected = dependents[owner.crate]
        if len(names) != len(set(names)) or set(names) != expected:
            raise OwnershipError(
                f"{name}: consumer crates {sorted(names)} differ from direct dependents {sorted(expected)}"
            )
        cases.extend([owner, *consumers])
    return cases


def run(root, cases, command=subprocess.run):
    root = Path(root)
    for item in cases:
        args = [
            "cargo", "nextest", "run", "-p", item.crate, "--test", item.target,
            "--locked", "--no-tests", "fail", "--color", "never", "--", "--exact", item.test,
        ]
        label = f"{item.behavior}: {item.crate}::{item.target} {item.test}"
        print(f"RUN {label}", flush=True)
        started = time.monotonic()
        try:
            result = command(args, cwd=root, capture_output=True, text=True, timeout=180,
                             check=False)
        except subprocess.TimeoutExpired as error:
            raise OwnershipError(f"timeout {label} after {time.monotonic() - started:.1f}s") from error
        output = result.stdout + result.stderr
        passed = any(
            "PASS [" in line and f"{item.crate}::{item.target} {item.test}" in line
            for line in output.splitlines()
        )
        if result.returncode or not passed:
            raise OwnershipError(
                f"{label}: {'failed' if result.returncode else 'did not pass'} "
                f"(exit {result.returncode})\n{output}"
            )
        print(f"PASS {label} {time.monotonic() - started:.1f}s", flush=True)


def main():
    try:
        declaration = json.loads(DECLARATION.read_text(), object_pairs_hook=unique_object)
        cases = validate(ROOT, declaration)
        run(ROOT, cases)
    except (OwnershipError, OSError, json.JSONDecodeError) as error:
        print(f"FAIL test ownership: {error}", file=sys.stderr)
        return 1
    print(f"PASS test ownership: {len(cases)} tests", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
