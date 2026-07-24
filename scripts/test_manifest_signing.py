from __future__ import annotations

import importlib.util
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).with_name("sign-manifest.py")
SPEC = importlib.util.spec_from_file_location("sign_manifest", MODULE_PATH)
assert SPEC and SPEC.loader
SIGN_MANIFEST = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SIGN_MANIFEST)


def manifest(update_sha: str = "a" * 64, integrity_sha: str = "a" * 64) -> dict:
    return {
        "files": [
            {
                "path": "gtrp-assets/enb/GTRP-HD.ini",
                "sha256": update_sha,
                "size": 1874,
            }
        ],
        "integrity": {
            "enforce": True,
            "files": [
                {
                    "path": "gtrp-assets/enb/GTRP-HD.ini",
                    "sha256": integrity_sha,
                    "size": 1874,
                    "profile": "always",
                }
            ],
        },
    }


class ManifestConsistencyTests(unittest.TestCase):
    def test_accepts_matching_update_and_inventory(self) -> None:
        SIGN_MANIFEST.validate_update_integrity_consistency(manifest())

    def test_rejects_hash_mismatch(self) -> None:
        with self.assertRaisesRegex(ValueError, "contradiction"):
            SIGN_MANIFEST.validate_update_integrity_consistency(
                manifest(update_sha="b" * 64)
            )

    def test_rejects_missing_inventory_entry(self) -> None:
        document = manifest()
        document["integrity"]["files"] = []
        with self.assertRaisesRegex(ValueError, "vide"):
            SIGN_MANIFEST.validate_update_integrity_consistency(document)

    def test_rejects_disabled_policy(self) -> None:
        document = manifest()
        document["integrity"]["enforce"] = False
        with self.assertRaisesRegex(ValueError, "désactivée"):
            SIGN_MANIFEST.validate_update_integrity_consistency(document)


if __name__ == "__main__":
    unittest.main()
