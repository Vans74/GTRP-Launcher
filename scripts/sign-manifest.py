#!/usr/bin/env python3
"""Canonise et signe manifest.json avec la clé privée Ed25519 hors dépôt."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import subprocess
import tempfile


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", type=pathlib.Path)
    parser.add_argument(
        "--key",
        type=pathlib.Path,
        default=os.environ.get("GTRP_MANIFEST_SIGNING_KEY"),
    )
    args = parser.parse_args()
    if not args.key or not args.key.is_file():
        raise SystemExit(
            "Clé absente : définis GTRP_MANIFEST_SIGNING_KEY ou utilise --key."
        )

    document = json.loads(args.manifest.read_text(encoding="utf-8"))
    document.pop("signature", None)
    canonical = json.dumps(
        document,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode("utf-8")

    with tempfile.TemporaryDirectory(prefix="gtrp-sign-") as directory:
        payload = pathlib.Path(directory) / "payload.json"
        signature = pathlib.Path(directory) / "signature.bin"
        payload.write_bytes(canonical)
        subprocess.run(
            [
                "openssl",
                "pkeyutl",
                "-sign",
                "-rawin",
                "-inkey",
                str(args.key),
                "-in",
                str(payload),
                "-out",
                str(signature),
            ],
            check=True,
        )
        signature_hex = signature.read_bytes().hex()

    if len(signature_hex) != 128:
        raise SystemExit("Signature Ed25519 de longueur invalide.")
    document["signature"] = {
        "algorithm": "ed25519",
        "key_id": "gtrp-manifest-2026-01",
        "value": signature_hex,
    }
    args.manifest.write_text(
        json.dumps(document, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print("Manifest signé avec Ed25519.")


if __name__ == "__main__":
    main()
