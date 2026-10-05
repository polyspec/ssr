"""Validate and execute the tests declared for shared workspace behavior."""

from dataclasses import dataclass
import json
from pathlib import Path
import re
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from tools.cargo_case import REPORT, results, run as run_cargo

DECLARATION = ROOT / "tools/test-ownership.json"
IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")
TOKEN = re.compile(r"[A-Za-z_][A-Za-z0-9_]*|::|[^\s]")


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
    """Read the workspace members. Cargo metadata can resolve the index, so it is a long
    operation: it has no time limit, Cargo progress and errors reach standard error as they
    arrive, and the exit code decides the result."""
    arguments = ["cargo", "metadata", "--no-deps", "--offline", "--format-version", "1"]
    print(f"RUN {' '.join(arguments)}", flush=True)
    started = time.monotonic()
    result = subprocess.run(arguments, cwd=root, stdout=subprocess.PIPE, text=True, check=False)
    elapsed = time.monotonic() - started
    if result.returncode:
        print(f"FAIL cargo metadata exit {result.returncode} {elapsed:.1f}s", flush=True)
        raise OwnershipError(f"workspace metadata failed with exit {result.returncode}")
    print(f"PASS cargo metadata {elapsed:.1f}s", flush=True)
    data = json.loads(result.stdout)
    packages = {package["id"]: package for package in data["packages"]}
    members = {packages[member]["name"]: packages[member] for member in data["workspace_members"]}
    dependencies = {name: {} for name in members}
    for name, package in members.items():
        for dependency in package["dependencies"]:
            target = dependency["name"]
            if target in members and dependency.get("path") is not None:
                alias = (dependency.get("rename") or target).replace("-", "_")
                if alias in dependencies[name]:
                    raise OwnershipError(f"ambiguous Rust dependency name in {name}: {alias}")
                dependencies[name][alias] = target
    return members, dependencies


def rust_tokens(source):
    """Remove Rust comments and literals, then return the remaining tokens."""
    clean = list(source)
    position = 0
    length = len(source)
    while position < length:
        end = position
        if source.startswith("//", position):
            newline = source.find("\n", position + 2)
            end = length if newline < 0 else newline
        elif source.startswith("/*", position):
            depth = 1
            end = position + 2
            while end < length and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            if depth:
                raise OwnershipError("unclosed Rust comment")
        else:
            raw = re.match(r'(?:b|c)?r(#+)?"', source[position:])
            if raw:
                hashes = len(raw.group(1) or "")
                marker = '"' + "#" * hashes
                found = source.find(marker, position + len(raw.group(0)))
                if found < 0:
                    raise OwnershipError("unclosed Rust raw string")
                end = found + len(marker)
            else:
                string_start = position
                if source.startswith(("b\"", "c\""), position):
                    string_start += 1
                if source[string_start:string_start + 1] == '"':
                    end = string_start + 1
                    while end < length:
                        if source[end] == "\\":
                            end += 2
                        elif source[end] == '"':
                            end += 1
                            break
                        else:
                            end += 1
                    if end > length or source[end - 1:end] != '"':
                        raise OwnershipError("unclosed Rust string")
                elif source.startswith("b'", position):
                    string_start = position + 1
                    end = string_start + 1
                    while end < length:
                        if source[end] == "\\":
                            end += 2
                        elif source[end] == "'":
                            end += 1
                            break
                        else:
                            end += 1
                    if end > length or source[end - 1:end] != "'":
                        raise OwnershipError("unclosed Rust byte character")
                elif source[position] == "'":
                    # A short quoted form is a character; otherwise this is a lifetime.
                    candidate = position + 1
                    if candidate < length and source[candidate] == "\\":
                        candidate += 2
                    else:
                        candidate += 1
                    if candidate < length and source[candidate] == "'":
                        end = candidate + 1
        if end > position:
            for index in range(position, min(end, length)):
                if source[index] != "\n":
                    clean[index] = " "
            position = end
        else:
            position += 1
    return TOKEN.findall("".join(clean))


def source_files(crate_root):
    paths = []
    for folder in ("src", "tests", "examples", "benches"):
        directory = crate_root / folder
        if directory.exists():
            paths.extend(directory.rglob("*.rs"))
    build_script = crate_root / "build.rs"
    if build_script.exists():
        paths.append(build_script)
    for path in paths:
        if path.is_symlink():
            raise OwnershipError(f"symbolic link in Rust source: {path}")
    return sorted(paths)


def split_top_level(tokens, delimiter=","):
    chunks = []
    start = 0
    depth = 0
    for index, token in enumerate(tokens):
        if token in ("{", "(", "["):
            depth += 1
        elif token in ("}", ")", "]"):
            depth -= 1
        elif token == delimiter and depth == 0:
            chunks.append(tokens[start:index])
            start = index + 1
    chunks.append(tokens[start:])
    return chunks


def import_paths(tree):
    """Yield public root names selected by a `use` tree."""
    tree = [token for token in tree if token not in {";", "::"}]
    if not tree:
        return
    if "{" in tree:
        opening = tree.index("{")
        depth = 0
        closing = None
        for index in range(opening, len(tree)):
            if tree[index] == "{":
                depth += 1
            elif tree[index] == "}":
                depth -= 1
                if depth == 0:
                    closing = index
                    break
        if closing is None:
            raise OwnershipError("unclosed Rust use tree")
        base = tuple(tree[:opening])
        contents = tree[opening + 1:closing]
        for child in split_top_level(contents):
            if child == ["self"]:
                yield base
            else:
                yield from import_paths(base + tuple(child))
        return
    if "*" in tree:
        yield tuple(tree)
        return
    path = []
    for token in tree:
        if token == "as":
            break
        if token != "::":
            path.append(token)
    yield tuple(path)


def public_use_names(tree, prefix=()):
    tree = [token for token in tree if token not in {";", "::"}]
    if not tree:
        return
    if "*" in tree:
        raise OwnershipError("public root glob import cannot be mapped to one public name")
    if "{" in tree:
        opening = tree.index("{")
        depth = 0
        closing = None
        for index in range(opening, len(tree)):
            if tree[index] == "{":
                depth += 1
            elif tree[index] == "}":
                depth -= 1
                if depth == 0:
                    closing = index
                    break
        if closing is None:
            raise OwnershipError("unclosed public Rust use tree")
        base = prefix + tuple(tree[:opening])
        for child in split_top_level(tree[opening + 1:closing]):
            yield from public_use_names(child, base)
        return
    if "as" in tree:
        alias_index = tree.index("as")
        if alias_index + 1 >= len(tree) or not IDENTIFIER.fullmatch(tree[alias_index + 1]):
            raise OwnershipError("unrecognized public use alias")
        yield tree[alias_index + 1]
        return
    if tree == ["self"]:
        if prefix:
            yield prefix[-1]
        return
    path = prefix + tuple(tree)
    if path:
        yield path[-1]


def use_imports(tokens):
    imports = []
    index = 0
    while index < len(tokens):
        if tokens[index] != "use":
            index += 1
            continue
        if index + 1 < len(tokens) and tokens[index + 1] == "<":
            # Rust precise capture names belong to an impl Trait bound, not a use item.
            index += 1
            continue
        start = index + 1
        depth = 0
        end = start
        while end < len(tokens):
            token = tokens[end]
            if token == "{":
                depth += 1
            elif token == "}":
                depth -= 1
            elif token == ";" and depth == 0:
                break
            end += 1
        if end == len(tokens):
            raise OwnershipError("Rust use statement has no semicolon")
        imports.extend(import_paths(tokens[start:end]))
        index = end + 1
    return imports


def references(tokens, aliases, owner_exports):
    found = set()
    known_aliases = set(aliases)
    imports = use_imports(tokens)
    for path in imports:
        if not path or path[0] not in known_aliases:
            continue
        owner = aliases[path[0]]
        if len(path) == 1:
            raise OwnershipError(f"crate-root import alias is unsupported: {path[0]}")
        if "*" in path:
            raise OwnershipError(f"wildcard import from declared owner is unsupported: {'::'.join(path)}")
        root_name = path[1]
        if root_name not in owner_exports[owner]:
            raise OwnershipError(f"unrecognized public name in import: {owner}::{root_name}")
        found.add((owner, root_name))
    index = 0
    while index < len(tokens):
        start = index
        if tokens[start] == "::":
            start += 1
        if start + 2 < len(tokens) and tokens[start] in known_aliases and tokens[start + 1] == "::":
            owner = aliases[tokens[start]]
            root_name = tokens[start + 2]
            if root_name == "{":
                # The grouped names were resolved by the `use` tree above.
                index += 1
                continue
            if root_name == "*":
                raise OwnershipError(f"wildcard reference to declared owner: {tokens[start]}")
            if root_name not in owner_exports[owner]:
                raise OwnershipError(f"unrecognized public name in reference: {owner}::{root_name}")
            found.add((owner, root_name))
        index += 1
    return found


def public_root_exports(crate_root):
    exports = set()
    for path in (crate_root / "src").rglob("*.rs") if (crate_root / "src").exists() else []:
        if path.parent != crate_root / "src" or path.name != "lib.rs":
            continue
        tokens = rust_tokens(path.read_text())
        depth = 0
        index = 0
        while index < len(tokens):
            if tokens[index] == "{" and depth == 0:
                depth += 1
                index += 1
                continue
            if tokens[index] == "{" and depth > 0:
                depth += 1
            elif tokens[index] == "}" and depth:
                depth -= 1
            if depth == 0 and tokens[index] == "pub":
                next_index = index + 1
                public = True
                if next_index < len(tokens) and tokens[next_index] == "(":
                    closing = next_index + 1
                    while closing < len(tokens) and tokens[closing] != ")":
                        closing += 1
                    if tokens[next_index + 1:closing] != []:
                        public = False
                    next_index = closing + 1
                if not public:
                    index = next_index
                    continue
                if next_index < len(tokens) and tokens[next_index] == "async":
                    next_index += 1
                if next_index < len(tokens) and tokens[next_index] == "unsafe":
                    next_index += 1
                if next_index < len(tokens) and tokens[next_index] == "extern":
                    next_index += 1
                    if next_index < len(tokens) and tokens[next_index].startswith('"'):
                        next_index += 1
                if next_index >= len(tokens):
                    raise OwnershipError(f"incomplete public item in {path}")
                kind = tokens[next_index]
                if kind == "use":
                    end = next_index + 1
                    brace_depth = 0
                    while end < len(tokens):
                        if tokens[end] == "{":
                            brace_depth += 1
                        elif tokens[end] == "}":
                            brace_depth -= 1
                        elif tokens[end] == ";" and brace_depth == 0:
                            break
                        end += 1
                    if end == len(tokens):
                        raise OwnershipError(f"public use has no semicolon in {path}")
                    exports.update(public_use_names(tokens[next_index + 1:end]))
                    index = end
                elif kind in {"struct", "enum", "union", "const", "static", "type", "trait", "mod", "fn"}:
                    if next_index + 1 >= len(tokens) or not IDENTIFIER.fullmatch(tokens[next_index + 1]):
                        raise OwnershipError(f"unrecognized public root item in {path}: {kind}")
                    exports.add(tokens[next_index + 1])
                    index = next_index + 1
                else:
                    raise OwnershipError(f"unrecognized public root item in {path}: {kind}")
            index += 1
    if not exports:
        raise OwnershipError(f"no public root exports found in {crate_root}")
    return exports


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
    """Check the declaration against the workspace and return its cases. Every declaration error
    is collected and reported together in one OwnershipError, so one run names them all."""
    root = Path(root)
    members, dependencies = workspace(root)
    if not isinstance(declaration, dict) or set(declaration) != {"behaviors"}:
        raise OwnershipError("declaration must contain only behaviors")
    behaviors = declaration["behaviors"]
    if not isinstance(behaviors, list) or not behaviors:
        raise OwnershipError("at least one behavior is required")
    errors = []
    cases = []
    seen = set()
    valid = []
    for behavior in behaviors:
        try:
            if not isinstance(behavior, dict) or set(behavior) != {"id", "owner", "consumers", "exports"}:
                raise OwnershipError("behavior must name id, owner, exports and consumers")
            name = behavior["id"]
            if not isinstance(name, str) or not IDENTIFIER.fullmatch(name) or name in seen:
                raise OwnershipError(f"invalid or repeated behavior: {name}")
            seen.add(name)
            owner = case(root, name, behavior["owner"], members)
            exported = behavior["exports"]
            if (not isinstance(exported, list) or not exported
                    or any(not isinstance(item, str) or not IDENTIFIER.fullmatch(item) for item in exported)
                    or len(exported) != len(set(exported))):
                raise OwnershipError(f"{name}: exports must be a nonempty list of unique public names")
            declared = behavior["consumers"]
            if not isinstance(declared, list):
                raise OwnershipError(f"{name}: consumers must be a list")
            consumers = []
            for item in declared:
                try:
                    consumers.append(case(root, name, item, members))
                except OwnershipError as error:
                    errors.append(str(error))
            names = [item["crate"] for item in declared if isinstance(item, dict) and "crate" in item]
            if len(names) != len(set(names)):
                raise OwnershipError(f"{name}: repeated consumer crate")
            cases.extend([owner, *consumers])
            valid.append(behavior)
        except OwnershipError as error:
            errors.append(str(error))

    owner_exports = {}
    behavior_for_export = {}
    for behavior in valid:
        owner_name = behavior["owner"]["crate"]
        try:
            owner_exports.setdefault(owner_name, public_root_exports(root / "crates" / owner_name))
        except OwnershipError as error:
            errors.append(str(error))
            continue
        for exported in behavior["exports"]:
            if exported not in owner_exports[owner_name]:
                errors.append(f"{behavior['id']}: owner does not export {owner_name}::{exported}")
                continue
            key = (owner_name, exported)
            if key in behavior_for_export:
                errors.append(f"public root name is assigned more than once: {owner_name}::{exported}")
                continue
            behavior_for_export[key] = behavior["id"]
    for owner_name, exports in owner_exports.items():
        declared_exports = {export for crate, export in behavior_for_export if crate == owner_name}
        if exports != declared_exports:
            errors.append(
                f"{owner_name}: declared public names {sorted(declared_exports)} differ from root exports {sorted(exports)}"
            )

    actual_consumers = {behavior["id"]: set() for behavior in valid}
    for crate_name, aliases in dependencies.items():
        aliases = {alias: target for alias, target in aliases.items() if target in owner_exports}
        if not aliases:
            continue
        for path in source_files(root / "crates" / crate_name):
            try:
                found = references(rust_tokens(path.read_text()), aliases, owner_exports)
            except OwnershipError as error:
                errors.append(f"{path}: {error}")
                continue
            for owner_name, exported in found:
                behavior_name = behavior_for_export.get((owner_name, exported))
                if behavior_name is None:
                    errors.append(f"unmapped public name: {owner_name}::{exported}")
                    continue
                actual_consumers[behavior_name].add(crate_name)

    for behavior in valid:
        name = behavior["id"]
        declared = {item["crate"] for item in behavior["consumers"]}
        actual = actual_consumers[name]
        if actual != declared:
            errors.append(
                f"{name}: declared consumer crates {sorted(declared)} differ from Rust source consumers {sorted(actual)}"
            )
    if errors:
        unique = list(dict.fromkeys(errors))
        raise OwnershipError(f"{len(unique)} declaration errors:\n" + "\n".join(f"  {error}" for error in unique))
    return cases


def run(root, cases, command=run_cargo):
    """Build every workspace test once, then run the declared cases on those builds.

    A separate `-p` run for each case resolves features for that package alone, so shared
    crates are compiled again for each feature set. One workspace build matches the later
    workspace test run, and one nextest run executes every case with its own nextest timeout.
    """
    root = Path(root)
    build = ["cargo", "nextest", "run", "--workspace", "--locked", "--no-run", "--color", "never"]
    print("RUN build workspace tests", flush=True)
    started = time.monotonic()
    try:
        result = command(build, cwd=root)
    except subprocess.TimeoutExpired as error:
        raise OwnershipError(f"timeout build workspace tests after {time.monotonic() - started:.1f}s") from error
    if result.returncode:
        raise OwnershipError(
            f"build workspace tests: failed (exit {result.returncode})\n{result.stdout + result.stderr}"
        )
    print(f"PASS build workspace tests {time.monotonic() - started:.1f}s", flush=True)
    expression = " | ".join(
        f"(package(={item.crate}) & binary(={item.target}) & test(={item.test}))" for item in cases
    )
    args = [
        "cargo", "nextest", "run", "--workspace", "--locked", "--no-tests", "fail",
        "--color", "never", *REPORT, "-E", expression,
    ]
    print(f"RUN {len(cases)} ownership cases", flush=True)
    started = time.monotonic()
    try:
        result = command(args, cwd=root)
    except subprocess.TimeoutExpired as error:
        raise OwnershipError(f"timeout ownership cases after {time.monotonic() - started:.1f}s") from error
    output = result.stdout + result.stderr
    events = results(output)
    missing = []
    for item in cases:
        label = f"{item.behavior}: {item.crate}::{item.target} {item.test}"
        event = events.get(f"{item.crate}::{item.target}${item.test}", "not run")
        passed = event == "ok"
        print(f"{'PASS' if passed else 'FAIL'} {label}{'' if passed else f' ({event})'}", flush=True)
        if not passed:
            missing.append(label)
    if result.returncode or missing:
        raise OwnershipError(
            f"ownership cases {'failed' if result.returncode else 'did not pass'} "
            f"(exit {result.returncode}): {', '.join(missing)}\n{output}"
        )
    print(f"PASS {len(cases)} ownership cases {time.monotonic() - started:.1f}s", flush=True)


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
