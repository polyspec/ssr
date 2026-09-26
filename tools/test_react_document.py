import pathlib
import tempfile
import unittest

from tools.check import check_react_document


CONTRACT = (
    "`framework_entry` `server_entry` `client_entry` "
    "`Pool::new_react` `Pool::render_stream` `ReactAdapter::stream_parts`"
)
RUNTIME = "`Pool::new_react` `Pool::render_stream` `setTimeout`"


class ReactDocumentContractTests(unittest.TestCase):
    def test_missing_stream_api_and_old_pool_are_errors(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            docs = root / "docs"
            docs.mkdir()
            (docs / "react.md").write_text("`Pool::new`", encoding="utf-8")
            (docs / "react.ko.md").write_text(CONTRACT, encoding="utf-8")
            for name in ("runtime.md", "runtime.ko.md"):
                (docs / name).write_text(RUNTIME, encoding="utf-8")
            errors = check_react_document(root)
            self.assertTrue(any("obsolete React Pool::new" in error for error in errors))
            self.assertTrue(any("omits `Pool::render_stream`" in error for error in errors))

    def test_both_documents_require_the_current_api(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            docs = root / "docs"
            docs.mkdir()
            for name in ("react.md", "react.ko.md"):
                (docs / name).write_text(CONTRACT, encoding="utf-8")
            for name in ("runtime.md", "runtime.ko.md"):
                (docs / name).write_text(RUNTIME, encoding="utf-8")
            self.assertEqual(check_react_document(root), [])


if __name__ == "__main__":
    unittest.main()
