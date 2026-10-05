import pathlib
import tempfile
import unittest

from tools.check import check_pairs_and_links


STRAY = (
    "States: `[ ]` waiting, `[~]` in progress.\n"
    "\n"
    "| Item | State |\n"
    "| --- | --- |\n"
    "| S-1 | [o] |\n"
    "\n"
    "- [o] S-1 Mark an item `[o]` when it is complete.\n"
    "  The continuation names [!] in prose.\n"
    "- [~] S-1-1 Keep this item in progress. - [ ] S-1-2\n"
    "- [x] S-1-3 Close the item in the task list form.\n"
    "- [o] S-1-4 Name `[X]` in the text.\n"
)
CLEAN = (
    "# Checklist\n"
    "\n"
    "- [o] S-1 Mark an item complete when it is done.\n"
    "  The continuation names a bypass in words.\n"
    "- [~] S-1-1 Keep this item in progress.\n"
    "- [ ] S-1-2 Wait for S-1-1.\n"
    "- [!] S-1-3 Bypass this item.\n"
)


def checklist_root(directory, content):
    root = pathlib.Path(directory)
    docs = root / "docs"
    docs.mkdir()
    for name in ("checklist.md", "checklist.ko.md"):
        (docs / name).write_text(content, encoding="utf-8")
    return root


class ChecklistMarkerTests(unittest.TestCase):
    def test_marker_outside_an_item_state_names_its_location(self):
        with tempfile.TemporaryDirectory() as directory:
            errors = check_pairs_and_links(checklist_root(directory, STRAY))
        expected = []
        for name in ("docs/checklist.md", "docs/checklist.ko.md"):
            for location, marker in (("1:10", "[ ]"), ("1:25", "[~]"), ("5:9", "[o]"),
                                     ("7:25", "[o]"), ("8:26", "[!]"), ("9:43", "[ ]"),
                                     ("10:3", "[x]"), ("11:19", "[X]")):
                expected.append(f"{name}:{location}: state marker {marker} outside an item state; "
                                "a checklist marker appears only as the state of an item")
        self.assertEqual(errors, expected)

    def test_markers_only_as_item_states_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(check_pairs_and_links(checklist_root(directory, CLEAN)), [])

    def test_repository_checklists_pass(self):
        root = pathlib.Path(__file__).resolve().parents[1]
        errors = [error for error in check_pairs_and_links(root) if "state marker" in error]
        self.assertEqual(errors, [])


if __name__ == "__main__":
    unittest.main()
