#!/usr/bin/env python3
"""Vérificateur indépendant pour valider une politique sur une installation test."""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import pathlib


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", type=pathlib.Path)
    parser.add_argument("root", type=pathlib.Path)
    parser.add_argument("--hd", action="store_true")
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    policy = manifest["integrity"]
    expected = {
        item["path"].replace("\\", "/").lower(): item
        for item in policy["files"]
        if item.get("profile", "always") == "always"
        or (args.hd and item.get("profile") == "hd")
    }
    mutable = [item.replace("\\", "/").lower() for item in policy["mutable_paths"]]

    def is_mutable(rel: str) -> bool:
        rel = rel.lower()
        for pattern in mutable:
            if pattern.endswith("/**"):
                prefix = pattern[:-3]
                if rel == prefix or rel.startswith(prefix + "/"):
                    return True
            elif pattern.startswith("*") and rel.endswith(pattern[1:]):
                return True
            elif fnmatch.fnmatchcase(rel, pattern):
                return True
        return False

    invalid = []
    for rel, item in expected.items():
        path = args.root / pathlib.PurePosixPath(rel)
        if not path.exists() and item.get("optional"):
            continue
        if not path.is_file() or path.stat().st_size != item["size"]:
            invalid.append(rel)
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest.lower() != item["sha256"].lower():
            invalid.append(rel)

    unexpected = []
    reparse = []
    for path in args.root.rglob("*"):
        rel = path.relative_to(args.root).as_posix()
        if path.is_symlink():
            reparse.append(rel)
        elif path.is_file() and rel.lower() not in expected and not is_mutable(rel):
            unexpected.append(rel)

    print(
        f"Contrôlés={len(expected)} invalides={len(invalid)} "
        f"inattendus={len(unexpected)} liens={len(reparse)}"
    )
    for label, values in (
        ("INVALID", invalid),
        ("UNEXPECTED", unexpected),
        ("REPARSE", reparse),
    ):
        for value in values[:30]:
            print(f"{label}: {value}")
    if invalid or unexpected or reparse:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
