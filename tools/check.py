import pathlib
import re
import subprocess
import sys
from urllib.parse import unquote


ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from tools import install_packages, owning_tests, push_gate, tool_versions  # noqa: E402
RECORD_WORDS = (
    "envelope", "gate", "orphan", "adopt", "retire", "dead", "first-class", "carries", "speaks", "answers",
)
TERM_WORDS = ("site", "endpoint", "skin", "setting", "사이트", "엔드포인트", "스킨")
LINK = re.compile(r"(?<!!)\[[^]]*\]\(([^)]+)\)")
STATE = re.compile(r"^- \[([ ~o!])\] (S-\d+(?:-\d+)*)\b", re.MULTILINE)
MARKER = re.compile(r"\[[ ~o!xX]\]")


def tracked(root, pattern):
    """The tracked files of the checkout ``root`` that match the Git pathspec ``pattern``. Files
    that the tree does not hold, untracked or ignored, are not read, so the result is the same in
    every checkout of the tree."""
    result = subprocess.run(["git", "ls-files", "-z", "--", pattern], cwd=root, capture_output=True,
                            text=True, check=False)
    if result.returncode:
        raise OSError(f"git ls-files -- {pattern} in {root} exited with {result.returncode}: "
                      f"{result.stderr.strip()}")
    return sorted(root / path for path in result.stdout.split("\0") if path)


def markdown_files(root):
    return tracked(root, "*.md")


def check_markers(root, path):
    """A state marker of the checklist is the state of an item line and nothing else."""
    errors = []
    name = path.relative_to(root).as_posix()
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        state = 2 if STATE.match(line) else None
        for match in MARKER.finditer(line):
            if match.start() != state:
                errors.append(f"{name}:{number}:{match.start() + 1}: state marker {match.group()} outside "
                              "an item state; a checklist marker appears only as the state of an item")
    return errors


def check_text(root, path):
    """The checklist holds blank lines, headings, item lines and the continuation lines of items."""
    errors = []
    name = path.relative_to(root).as_posix()
    in_item = False
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if STATE.match(line):
            in_item = True
        elif in_item and re.match(r"  \S", line):
            pass
        elif not line.strip() or line.startswith("#"):
            in_item = False
        else:
            in_item = False
            column = len(line) - len(line.lstrip()) + 1
            errors.append(f"{name}:{number}:{column}: text outside an item; a checklist holds only items and headings")
    return errors


def check_pairs_and_links(root):
    errors = []
    for path in markdown_files(root):
        counterpart = path.with_name(path.name[:-6] + ".md") if path.name.endswith(".ko.md") else path.with_name(path.stem + ".ko.md")
        if not counterpart.is_file():
            errors.append(f"{path}: missing document pair {counterpart.name}")
        for destination in LINK.findall(path.read_text(encoding="utf-8")):
            destination = destination.split("#", 1)[0]
            if not destination or ":" in destination or destination.startswith("/"):
                continue
            if not (path.parent / unquote(destination)).exists():
                errors.append(f"{path}: broken link {destination}")
    en = root / "docs/checklist.md"
    ko = root / "docs/checklist.ko.md"
    for path in (en, ko):
        if path.is_file():
            errors.extend(check_markers(root, path))
            errors.extend(check_text(root, path))
    if en.is_file() and ko.is_file():
        if STATE.findall(en.read_text(encoding="utf-8")) != STATE.findall(ko.read_text(encoding="utf-8")):
            errors.append("docs/checklist.md: item IDs or states differ from Korean document")
    return errors


def check_words(root):
    errors = []
    pattern = re.compile(r"(?<![\w-])(?:" + "|".join(re.escape(word) for word in RECORD_WORDS) + r")(?![\w-])", re.IGNORECASE)

    def found(text):
        """The record words of a text: the matches of RECORD_WORDS."""
        return [match.group() for match in pattern.finditer(text)]

    for path in markdown_files(root):
        if path.name in ("AGENTS.md", "AGENTS.ko.md"):
            continue
        content = path.read_text(encoding="utf-8")
        for word in found(content):
            errors.append(f"{path}: prohibited record word {word}")
        if path.name in ("AGENTS.md", "AGENTS.ko.md", "checklist.md", "checklist.ko.md"):
            continue
        for word in TERM_WORDS:
            if re.search(r"(?<![\w-])" + re.escape(word) + r"(?![\w-])", content, re.IGNORECASE):
                errors.append(f"{path}: unapproved term {word}")
    for path in tracked(root, "crates/*.rs"):
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if line.lstrip().startswith("//") and found(line):
                errors.append(f"{path}:{number}: prohibited record word")
    if (root / ".git").exists():
        result = subprocess.run(["git", "log", "--format=%B"], cwd=root, capture_output=True, text=True, check=False)
        if result.returncode:
            errors.append(f"git log exited with {result.returncode}: {result.stderr.strip()}")
        elif found(result.stdout):
            errors.append("git log: prohibited record word")
    return errors


def check_react_document(root):
    required = (
        "`framework_entry`",
        "`server_entry`",
        "`client_entry`",
        "`Pool::new_react`",
        "`Pool::render_stream`",
        "`ReactAdapter::stream_parts`",
    )
    errors = []
    for name in ("react.md", "react.ko.md"):
        path = root / "docs" / name
        if not path.is_file():
            errors.append(f"{path}: React adapter document is missing")
            continue
        content = path.read_text(encoding="utf-8")
        for symbol in required:
            if symbol not in content:
                errors.append(f"{path}: React stream contract omits {symbol}")
        if "`Pool::new`" in content:
            errors.append(f"{path}: obsolete React Pool::new contract")
    for name in ("runtime.md", "runtime.ko.md"):
        path = root / "docs" / name
        if not path.is_file():
            errors.append(f"{path}: React runtime document is missing")
            continue
        content = path.read_text(encoding="utf-8")
        for symbol in ("`Pool::new_react`", "`Pool::render_stream`", "`setTimeout`"):
            if symbol not in content:
                errors.append(f"{path}: React runtime contract omits {symbol}")
    return errors


# An absolute path into a home directory names a file of one machine, such as another checkout.
HOME_PATH = re.compile(r"/(?:Users|home)/[A-Za-z0-9._-]+/")


def check_outside_paths(root):
    """Tracked text files name no absolute path into a home directory: such a path, such as a
    dependency on another checkout, exists on one machine only, so the tree would not build or
    give the same result elsewhere."""
    errors = []
    for path in tracked(root, "*"):
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, IsADirectoryError):
            continue
        for number, line in enumerate(text.splitlines(), 1):
            match = HOME_PATH.search(line)
            if match:
                errors.append(f"{path.relative_to(root)}:{number}:{match.start() + 1}: absolute path into a home "
                              f"directory: {match.group()}")
    return errors


def check_untracked(root):
    """Files that are neither tracked nor ignored: the checks read tracked files only, so such a
    file must be added or ignored before the checks can judge it."""
    result = subprocess.run(["git", "ls-files", "-z", "--others", "--exclude-standard"], cwd=root,
                            capture_output=True, text=True, check=False)
    if result.returncode:
        raise OSError(f"git ls-files --others in {root} exited with {result.returncode}: "
                      f"{result.stderr.strip()}")
    return [f"{path}: untracked file; add it to Git or ignore it, the checks read tracked files only"
            for path in result.stdout.split("\0") if path]


def check_hooks(root):
    """The pre-push hook runs in this checkout (tools/push_gate.py hooks-check)."""
    return [f"hooks-check: {error}" for error in push_gate.hooks_check(root)]


def main():
    try:
        errors = (check_untracked(ROOT) + check_outside_paths(ROOT) + check_pairs_and_links(ROOT) + check_words(ROOT)
                  + check_react_document(ROOT) + check_hooks(ROOT))
    except OSError as error:
        print(error, file=sys.stderr)
        return 1
    errors += tool_versions.check()
    errors += owning_tests.check(ROOT)
    errors += install_packages.filter_errors(ROOT)
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1
    print("records, terminology and documents: pass")
    return 0


if __name__ == "__main__":
    sys.exit(main())
