#!/usr/bin/env node
// Prépare le pack ENB GTRP pour le modpack (SA DirectX 3.0 - Inari Lú).
//
// 1. Télécharge / extrais le mod depuis GTAinside :
//    https://www.gtainside.com/en/sanandreas/mods/213892-sa-directx-3-0-unofficial-update-by-inari-l/
// 2. Lance ce script pour copier les fichiers dans la structure attendue par le launcher :
//
//    node tools/prepare-enb-pack.mjs <dossier_mod_extrait> <dossier_modpack>
//
// Résultat : les fichiers seront placés dans <modpack>/gtrp-assets/enb/
// Ensuite, génère le manifest :
//    node tools/gen-manifest.mjs <dossier_modpack> https://gtrp.fr/launcher/files 1.0.0 > manifest.json

import { cpSync, mkdirSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, relative, sep } from "node:path";

const [, , srcDir, modpackDir] = process.argv;

if (!srcDir || !modpackDir) {
  console.error("Usage : node tools/prepare-enb-pack.mjs <dossier_mod_extrait> <dossier_modpack>");
  process.exit(1);
}

if (!existsSync(srcDir)) {
  console.error(`Source introuvable : ${srcDir}`);
  process.exit(1);
}

const dest = join(modpackDir, "gtrp-assets", "enb");
mkdirSync(dest, { recursive: true });

function copyRecursive(from, to) {
  for (const entry of readdirSync(from)) {
    const src = join(from, entry);
    const dst = join(to, entry);
    const st = statSync(src);
    if (st.isDirectory()) {
      mkdirSync(dst, { recursive: true });
      copyRecursive(src, dst);
    } else if (st.isFile()) {
      cpSync(src, dst);
    }
  }
}

copyRecursive(srcDir, dest);

let count = 0;
function countFiles(dir) {
  for (const entry of readdirSync(dir)) {
    const p = join(dir, entry);
    if (statSync(p).isDirectory()) countFiles(p);
    else count++;
  }
}
countFiles(dest);

console.error(`OK : ${count} fichier(s) copiés vers ${dest}`);
console.error("Prochaine étape : node tools/gen-manifest.mjs", modpackDir, "https://gtrp.fr/launcher/files X.Y.Z");
