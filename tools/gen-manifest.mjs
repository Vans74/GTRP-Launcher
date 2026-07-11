#!/usr/bin/env node
// Génère le manifest.json du modpack GTRP à partir d'un dossier.
//
// Utilisation :
//   node gen-manifest.mjs <dossier_modpack> <base_url> [version] > manifest.json
//
// Exemple :
//   node gen-manifest.mjs ./modpack https://gtrp.fr/launcher/files 1.4.0 > manifest.json
//
// Le manifest liste chaque fichier avec son chemin relatif, son SHA-256 et sa
// taille. Le launcher télécharge {base_url}/{chemin} pour les fichiers modifiés.

import { createHash } from "node:crypto";
import { readdirSync, statSync, readFileSync } from "node:fs";
import { join, relative, sep } from "node:path";

const [, , dir, baseUrl, version = "1.0.0"] = process.argv;

if (!dir || !baseUrl) {
  console.error("Usage : node gen-manifest.mjs <dossier_modpack> <base_url> [version]");
  process.exit(1);
}

function walk(root, current, out) {
  for (const entry of readdirSync(current)) {
    const full = join(current, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      walk(root, full, out);
    } else if (st.isFile()) {
      const rel = relative(root, full).split(sep).join("/");
      const buf = readFileSync(full);
      const sha256 = createHash("sha256").update(buf).digest("hex");
      out.push({ path: rel, sha256, size: st.size });
    }
  }
}

const files = [];
walk(dir, dir, files);
files.sort((a, b) => a.path.localeCompare(b.path));

const manifest = {
  version,
  base_url: baseUrl,
  files,
  // Motifs de fichiers interdits (anti-triche). Ex : "*.asi", "cleo/*".
  forbidden: [],
};

process.stdout.write(JSON.stringify(manifest, null, 2) + "\n");
console.error(`OK : ${files.length} fichier(s), version ${version}.`);
