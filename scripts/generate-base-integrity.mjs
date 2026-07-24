#!/usr/bin/env node
/**
 * Génère la référence des fichiers GTA/SA-MP qui ne proviennent pas du
 * modpack GTRP. À exécuter uniquement sur une installation propre, déjà
 * préparée une fois par le launcher.
 *
 * Usage:
 *   node scripts/generate-base-integrity.mjs "C:/GTRP JEU" assets/base-game-integrity.json
 */

import { createHash } from "node:crypto";
import { createReadStream, existsSync } from "node:fs";
import { readdir, readFile, stat, writeFile } from "node:fs/promises";
import path from "node:path";

const [, , rootArg, outputArg] = process.argv;
if (!rootArg || !outputArg) {
  console.error(
    'Usage: node scripts/generate-base-integrity.mjs "<jeu propre>" <sortie.json>',
  );
  process.exit(2);
}

const root = path.resolve(rootArg);
const output = path.resolve(outputArg);
const markerPath = path.join(root, ".gtrp_enb_active");
if (!existsSync(path.join(root, "gta_sa.exe")) || !existsSync(markerPath)) {
  throw new Error(
    "Installation invalide : gta_sa.exe ou inventaire .gtrp_enb_active absent.",
  );
}

const marker = await readFile(markerPath, "utf8");
const deployed = new Set(
  marker
    .split(/\r?\n/)
    .filter((line) => line.startsWith("file="))
    .map((line) => line.slice(5).replaceAll("\\", "/").toLowerCase()),
);

// Ces fichiers sont reconstruits ou contrôlés séparément par le générateur de
// politique. Les journaux et captures n'ont aucune influence sur le jeu.
const exactExclusions = new Set(
  [
    ".gtrp_enb_active",
    "vorbisFile.dll",
    "modloader/modloader.ini",
    "modloader/.data/config.ini",
    "modloader/.data/plugins/plugins.ini",
  ].map((value) => value.toLowerCase()),
);

function isExcluded(rel) {
  const key = rel.toLowerCase();
  return (
    key.startsWith("gtrp-assets/") ||
    key.startsWith("reshade-screenshots/") ||
    key.endsWith(".log") ||
    deployed.has(key) ||
    exactExclusions.has(key)
  );
}

async function walk(directory, result = []) {
  const entries = await readdir(directory, { withFileTypes: true });
  entries.sort((a, b) => a.name.localeCompare(b.name, "en"));
  for (const entry of entries) {
    const absolute = path.join(directory, entry.name);
    if (entry.isSymbolicLink()) {
      throw new Error(`Lien symbolique/jonction refusé dans la référence : ${absolute}`);
    }
    if (entry.isDirectory()) {
      await walk(absolute, result);
    } else if (entry.isFile()) {
      result.push(absolute);
    }
  }
  return result;
}

function sha256File(file) {
  return new Promise((resolve, reject) => {
    const hash = createHash("sha256");
    const stream = createReadStream(file);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("error", reject);
    stream.on("end", () => resolve(hash.digest("hex")));
  });
}

const files = [];
for (const absolute of await walk(root)) {
  const rel = path.relative(root, absolute).replaceAll("\\", "/");
  if (isExcluded(rel)) continue;
  const metadata = await stat(absolute);
  files.push({
    path: rel,
    sha256: await sha256File(absolute),
    size: metadata.size,
    profile: "always",
    ...(rel.toLowerCase() === "gta_sa.exe.orig-laa" ? { optional: true } : {}),
  });
}
files.sort((a, b) => a.path.toLowerCase().localeCompare(b.path.toLowerCase(), "en"));

const document = {
  schema: 1,
  description:
    "Référence GTA San Andreas 1.0 + SA-MP 0.3.DL-R1 autorisée pour GTRP.",
  files,
};
await writeFile(output, `${JSON.stringify(document, null, 2)}\n`, "utf8");
console.log(`Référence écrite : ${files.length} fichier(s) -> ${output}`);
