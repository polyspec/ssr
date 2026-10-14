import json
import pathlib
import re
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from tools import install_packages  # noqa: E402
RECORD_WORDS = (
    "envelope", "gate", "orphan", "adopt", "retire", "dead", "first-class", "carries", "speaks", "answers",
)
TERM_WORDS = ("site", "endpoint", "skin", "setting", "사이트", "엔드포인트", "스킨")


def kit_copies(root):
    """The paths of the copies of kit in the checkout ``root``: the keys of ``kit.copies`` of the
    installed package @polyspec/kit when ``package.json`` declares it, else none. make kit-check of
    kit compares each copy with the package, and the copies are records of kit, not of ssr."""
    manifest = root / "package.json"
    if not manifest.is_file() or "@polyspec/kit" not in json.loads(manifest.read_text()).get("devDependencies", {}):
        return set()
    package = root / "node_modules/@polyspec/kit/package.json"
    if not package.is_file():
        raise OSError("node_modules/@polyspec/kit/package.json is missing; run make install-packages")
    return set(json.loads(package.read_text())["kit"]["copies"])


def tracked(root, pattern):
    """The tracked files of the checkout ``root`` that match the Git pathspec ``pattern``, without
    the copies of kit. Files that the tree does not hold, untracked or ignored, are not read, so the
    result is the same in every checkout of the tree."""
    result = subprocess.run(["git", "ls-files", "-z", "--", pattern], cwd=root, capture_output=True,
                            text=True, check=False)
    if result.returncode:
        raise OSError(f"git ls-files -- {pattern} in {root} exited with {result.returncode}: "
                      f"{result.stderr.strip()}")
    copies = kit_copies(root)
    return sorted(root / path for path in result.stdout.split("\0") if path and path not in copies)


def markdown_files(root):
    return tracked(root, "*.md")


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
        if path.name in ("AGENTS.md", "AGENTS.ko.md", "execution-checklist.md", "execution-checklist.ko.md"):
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


# A measured time: an elapsed duration or a clock reading.
MEASURED_TIME = re.compile(r"\belapsed\b|monotonic\(|perf_counter|performance\.now|Date\.now|time\.time\(|Instant::now")
ASSERTION = re.compile(r"\b(?:debug_)?assert(?:_eq|_ne)?!\s*\(|\bself\.assert\w*\s*\(|\bassert\.\w+\s*\(|"
                       r"\bexpect\s*\(|^[ \t]*assert\s", re.MULTILINE)
SOURCES = ("*.rs", "*.py", "*.js", "*.mjs", "*.ts", "*.tsx")


def assertion_text(text, match, quotes):
    """The text an assertion judges: the balanced arguments of an assertion call, whose string
    literals (delimited by ``quotes``) take no part in the balance, or the rest of the line of an
    ``assert`` statement."""
    if not match.group().endswith("("):
        end = text.find("\n", match.end())
        return text[match.end():len(text) if end < 0 else end]
    depth, index, quote = 1, match.end(), None
    while index < len(text):
        character = text[index]
        if quote:
            if character == "\\":
                index += 1
            elif character == quote:
                quote = None
        elif character in quotes:
            quote = character
        else:
            depth += {"(": 1, ")": -1}.get(character, 0)
            if depth == 0:
                return text[match.end():index]
        index += 1
    return text[match.end():]


def check_timed_assertions(root):
    """No test judges a measured time. Performance is measured and printed, and never fails a test:
    a slower host fails a correct test by the clock and not by its result, so a test asserts the
    behaviour (the work finished, the event happened) and prints the time it took."""
    errors = []
    for pattern in SOURCES:
        for path in tracked(root, pattern):
            text = path.read_text(encoding="utf-8")
            for match in ASSERTION.finditer(text):
                # A Rust apostrophe also starts a lifetime, so only double quotes delimit its strings.
                quotes = '"' if path.suffix == ".rs" else "\"'`"
                if MEASURED_TIME.search(assertion_text(text, match, quotes)):
                    number = text.count("\n", 0, match.start()) + 1
                    errors.append(f"{path.relative_to(root)}:{number}: an assertion judges a measured time; "
                                  "assert the behaviour and print the time")
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


def check_records(root):
    """The checks that read only tracked files: the words of the records, the React document, the
    paths and the timed assertions. The job push-gate runs them, so a commit whose records fail
    never reaches main. The document pairs, links and checklists are checked by make
    documents-check of kit."""
    return check_outside_paths(root) + check_words(root) + check_react_document(root) + check_timed_assertions(root)


# The modes: ``records`` runs only the checks of the tracked files; without a mode every check runs.
MODES = ("records", None)


def main(argv=()):
    mode = argv[0] if argv else None
    if len(argv) > 1 or mode not in MODES:
        print("usage: python3 tools/check.py [records]", file=sys.stderr)
        return 2
    try:
        errors = check_records(ROOT)
        if mode != "records":
            errors = check_untracked(ROOT) + errors
    except OSError as error:
        print(error, file=sys.stderr)
        return 1
    if mode != "records":
        errors += install_packages.filter_errors(ROOT)
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1
    print(f"{'records' if mode == 'records' else 'records, terminology and fixture filter'}: pass")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
