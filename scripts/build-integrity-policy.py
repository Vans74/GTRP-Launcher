#!/usr/bin/env python3
"""Injecte l'inventaire exhaustif dans un manifest GTRP déjà assemblé."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import pathlib
import urllib.request
import zipfile


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def normalized(path: str) -> str:
    value = path.replace("\\", "/").strip("/")
    if not value or ":" in value or ".." in pathlib.PurePosixPath(value).parts:
        raise ValueError(f"chemin non autorisé : {path}")
    return value


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", required=True, type=pathlib.Path)
    parser.add_argument("--staging", required=True, type=pathlib.Path)
    parser.add_argument("--base", required=True, type=pathlib.Path)
    parser.add_argument("--components", type=pathlib.Path)
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    baseline = json.loads(args.base.read_text(encoding="utf-8"))
    if baseline.get("schema") != 1:
        raise ValueError("version de référence du jeu non prise en charge")

    entries: dict[str, dict] = {}

    def add(entry: dict) -> None:
        item = dict(entry)
        item["path"] = normalized(item["path"])
        item["sha256"] = item["sha256"].lower()
        item.setdefault("size", 0)
        item.setdefault("profile", "always")
        if len(item["sha256"]) != 64:
            raise ValueError(f"SHA-256 invalide : {item['path']}")
        key = item["path"].lower()
        previous = entries.get(key)
        if previous is not None and previous != item:
            raise ValueError(f"inventaires contradictoires : {item['path']}")
        entries[key] = item

    for entry in baseline["files"]:
        add(entry)

    hd_rules = []
    rules_path = args.staging / ".gtrp-hd-paths"
    for line in rules_path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            hd_rules.append(line.replace("\\", "/").lower())

    def is_hd(rel: str) -> bool:
        key = rel.lower()
        return any(
            key.startswith(rule) if rule.endswith("/") else key == rule
            for rule in hd_rules
        )

    # Le staging complet est toujours contrôlé. Ses fichiers déployés sont
    # ajoutés une seconde fois avec leur chemin final et leur profil.
    for source in sorted(path for path in args.staging.rglob("*") if path.is_file()):
        rel = source.relative_to(args.staging).as_posix()
        data = source.read_bytes()
        common = {"sha256": sha256(data), "size": len(data)}
        add(
            {
                "path": f"gtrp-assets/enb/{rel}",
                **common,
                "profile": "always",
            }
        )
        if rel not in {".gtrp-hd-paths", ".gtrp-hd-component.json"}:
            add(
                {
                    "path": rel,
                    **common,
                    "profile": "hd" if is_hd(rel) else "always",
                }
            )

    # Ultimate ASI Loader remplace vorbisFile.dll à partir de cette source.
    loader = args.staging / "vorbisFileLoader.dll"
    if loader.is_file():
        data = loader.read_bytes()
        add(
            {
                "path": "vorbisFile.dll",
                "sha256": sha256(data),
                "size": len(data),
                "profile": "always",
            }
        )

    # ModLoader crée ces copies au premier lancement. Elles peuvent être
    # absentes, mais jamais avoir un contenu différent.
    generated = {
        "modloader/.data/modloader.ini.0": "modloader/modloader.ini",
        "modloader/.data/config.ini.0": "modloader/.data/config.ini",
        "modloader/.data/plugins.ini.0": "modloader/.data/plugins/plugins.ini",
    }
    for source_rel, target_rel in generated.items():
        source = args.staging / source_rel
        if source.is_file():
            data = source.read_bytes()
            add(
                {
                    "path": target_rel,
                    "sha256": sha256(data),
                    "size": len(data),
                    "profile": "always",
                    "optional": True,
                }
            )

    component_path = args.components or (args.staging / ".gtrp-hd-component.json")
    component_file = json.loads(component_path.read_text(encoding="utf-8"))
    components = component_file.get("components", [component_file])
    for component in components:
        kind = component.get("kind", "archive")
        destination = component.get("destination", ".").strip("./")
        if kind == "installer":
            for output in component.get("outputs", []):
                if "size" not in output:
                    raise ValueError(
                        f"taille de sortie absente pour {output.get('path')}"
                    )
                add(
                    {
                        "path": output["path"],
                        "sha256": output["sha256"],
                        "size": output["size"],
                        "profile": "hd",
                    }
                )
            for output in component.get("generated_outputs", []):
                add({**output, "profile": "hd"})
            continue

        request = urllib.request.Request(
            component["url"], headers={"User-Agent": "GTRP integrity builder"}
        )
        archive_data = urllib.request.urlopen(request, timeout=90).read()
        if sha256(archive_data).lower() != component["sha256"].lower():
            raise ValueError(f"archive composant altérée : {component['name']}")
        prefix = component["archive_prefix"].strip("/") + "/"
        includes = [item.replace("\\", "/").strip("/") for item in component["include"]]
        with zipfile.ZipFile(io.BytesIO(archive_data)) as archive:
            names = {name.replace("\\", "/"): name for name in archive.namelist()}
            for include in includes:
                archive_name = prefix + include
                if archive_name not in names:
                    raise ValueError(
                        f"{include} absent du composant {component['name']}"
                    )
                data = archive.read(names[archive_name])
                final_path = "/".join(
                    part for part in (destination, include) if part
                )
                add(
                    {
                        "path": final_path,
                        "sha256": sha256(data),
                        "size": len(data),
                        "profile": "hd",
                    }
                )

    manifest["integrity"] = {
        "generation": 1,
        "enforce": True,
        "files": sorted(entries.values(), key=lambda item: item["path"].lower()),
        "mutable_paths": [
            ".gtrp_enb_active",
            "gtrp-assets/.modpack_version",
            "gtrp-assets/components/**",
            "reshade-screenshots/**",
            "*.log",
        ],
    }
    # Toute ancienne signature est invalidée par cette reconstruction.
    manifest.pop("signature", None)
    args.manifest.write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print(f"Inventaire exhaustif : {len(entries)} fichier(s)")


if __name__ == "__main__":
    main()
