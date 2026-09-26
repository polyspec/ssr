from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import TestCase
from unittest.mock import patch

from tools import prepare_crudui_fixture


class CruduiFixtureTest(TestCase):
    def test_checkout_requires_exactly_one_declared_path(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            checkout = root / "library"
            checkout.mkdir()
            declaration = root / "var/checkouts.txt"
            declaration.parent.mkdir()
            with patch.object(prepare_crudui_fixture, "ROOT", root):
                declaration.write_text(f"crudui {checkout}\n")
                self.assertEqual(prepare_crudui_fixture.checkout(), checkout)
                for text in (
                    "",
                    "crudui\n",
                    f"crudui {checkout}\nignored {checkout}\n",
                    f"crudui {checkout}\n\n",
                    f"crudui {checkout}\ncrudui {checkout}\n",
                ):
                    with self.subTest(text=text):
                        declaration.write_text(text)
                        with self.assertRaises(ValueError):
                            prepare_crudui_fixture.checkout()
