#!/usr/bin/env python3
"""Canonise et signe manifest.json avec la clé privée Ed25519 hors dépôt."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import subprocess
import tempfile


def _normalize_path(value: object) -> str:
    path = str(value).replace("\\", "/").strip("/")
    if (
        not path
        or ":" in path
        or ".." in pathlib.PurePosixPath(path).parts
    ):
        raise ValueError(f"chemin non autorisé dans le manifeste : {value}")
    return path.lower()


def validate_update_integrity_consistency(document: dict) -> None:
    """Refuse de signer un fichier diffusé avec une empreinte différente."""

    policy = document.get("integrity")
    if not isinstance(policy, dict) or not policy.get("enforce"):
        raise ValueError("politique d'intégrité stricte absente ou désactivée")

    inventory: dict[str, dict] = {}
    for item in policy.get("files", []):
        key = _normalize_path(item.get("path", ""))
        if key in inventory:
            raise ValueError(f"chemin dupliqué dans l'inventaire : {item['path']}")
        inventory[key] = item

    if not inventory:
        raise ValueError("inventaire d'intégrité vide")

    for update in document.get("files", []):
        key = _normalize_path(update.get("path", ""))
        expected = inventory.get(key)
        if expected is None:
            raise ValueError(
                f"fichier diffusé absent de l'inventaire signé : {update['path']}"
            )

        update_sha = str(update.get("sha256", "")).lower()
        expected_sha = str(expected.get("sha256", "")).lower()
        update_size = int(update.get("size", 0))
        expected_size = int(expected.get("size", 0))
        if update_sha != expected_sha or update_size != expected_size:
            raise ValueError(
                "contradiction mise à jour/intégrité pour "
                f"{update['path']} : diffusé={update_sha}/{update_size}, "
                f"autorisé={expected_sha}/{expected_size}"
            )


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
    try:
        validate_update_integrity_consistency(document)
    except (TypeError, ValueError) as error:
        raise SystemExit(f"Manifeste incohérent, signature refusée : {error}") from error

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
