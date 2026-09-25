import pathlib
import re
import subprocess
import sys
from urllib.parse import unquote


ROOT = pathlib.Path(__file__).resolve().parents[1]
RECORD_WORDS = (
    "envelope", "gate", "orphan", "adopt", "retire", "dead", "first-class", "carries", "speaks", "answers",
)
TERM_WORDS = ("site", "endpoint", "skin", "setting", "사이트", "엔드포인트", "스킨")
LINK = re.compile(r"(?<!!)\[[^]]*\]\(([^)]+)\)")
STATE = re.compile(r"^- \[([ ~o])\] (S-\d+(?:-\d+)?)\b", re.MULTILINE)


def markdown_files(root):
    return sorted(path for path in root.rglob("*.md") if "var" not in path.relative_to(root).parts and "target" not in path.relative_to(root).parts)


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
    for path in sorted((root / "crates").rglob("*.rs")) if (root / "crates").exists() else []:
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if line.lstrip().startswith("//") and found(line):
                errors.append(f"{path}:{number}: prohibited record word")
    if (root / ".git").exists():
        result = subprocess.run(["git", "log", "--format=%B"], cwd=root, capture_output=True, text=True, check=False)
        if result.returncode:
            errors.append("git log: failed")
        elif found(result.stdout):
            errors.append("git log: prohibited record word")
    return errors


def main():
    errors = check_pairs_and_links(ROOT) + check_words(ROOT)
    result = subprocess.run(["cargo", "nextest", "--version"], capture_output=True, text=True, check=False)
    if result.returncode or not result.stdout.startswith("cargo-nextest 0.9.146 "):
        errors.append("cargo-nextest 0.9.146 is required")
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1
    print("records, terminology and documents: pass")
    return 0


if __name__ == "__main__":
    sys.exit(main())
