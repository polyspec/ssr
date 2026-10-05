from pathlib import Path
import os
import tempfile
import unittest
from unittest import mock

from tools import prepare_engine_compose, verify_engine_mounts


class LocalPathsTest(unittest.TestCase):
    def test_engine_compose_mounts_no_other_checkout(self):
        content = prepare_engine_compose.render(prepare_engine_compose.ROOT)
        mounts = [line.strip() for line in content.splitlines() if line.strip().startswith("- ")]
        self.assertEqual(len(mounts), 3, mounts)
        self.assertTrue(mounts[0].endswith(":/src:ro"), mounts)
        self.assertTrue(mounts[1].endswith(":/cargo") and mounts[2].endswith(":/target"), mounts)

    def test_the_mount_check_requires_the_checkout_cache_and_output(self):
        self.assertEqual(verify_engine_mounts.expected(), {"/src": "ro", "/cargo": "rw", "/target": "rw"})

    def test_mounts_are_judged_by_access_alone(self):
        mountinfo = "\n".join([
            "1 0 0:1 / /src ro,relatime - virtiofs source ro",
            "2 0 0:2 / /cargo rw,relatime - fakeowner cache rw",
            "3 0 0:3 / /target ro,relatime - ext4 output ro",
        ])
        required = {"/src": "ro", "/cargo": "rw", "/target": "rw", "/v8": "ro"}
        self.assertEqual(verify_engine_mounts.check(required, mountinfo), [
            "/target: expected access rw, actual mount options ['relatime', 'ro']",
            "/v8 is not mounted"])

if __name__ == "__main__":
    unittest.main()
