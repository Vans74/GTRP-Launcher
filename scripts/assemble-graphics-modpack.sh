#!/usr/bin/env bash
# Assemble le modpack graphique GTRP : base ReShade + Project2DFX + 7 mods MixMods.
#
# Usage : ./assemble-graphics-modpack.sh [VERSION]   (défaut 1.19.0)
#
# Placement EXPLICITE par mod (leur structure a été inspectée : aucun ne contient
# de dossier "modloader/" — leur dossier racine se dépose DANS modloader/).
# Le script ne publie rien ; il produit le zip, le manifest et un rapport.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MODS_SRC="$ROOT/mods-src"
WORK="$ROOT/modpack-work/build"
STAGING="$WORK/gtrp-assets/enb"
ML="$STAGING/modloader"

BASE_REPO="$ROOT/modpack-work/graphics-base/gtrp-assets/enb"
BASE_TMP="/tmp/reshade/staging/gtrp-assets/enb"
if [[ -d "$BASE_REPO" ]]; then BASE="$BASE_REPO"
elif [[ -d "$BASE_TMP" ]]; then BASE="$BASE_TMP"
else echo "ERREUR: base graphique introuvable." >&2; exit 1; fi

# NOTE (v1.27.0) : ReShade (base) + Real Skybox + SkyGrad + Radar DE (ASI) +
# Absolute Atmosphere UI (menu/polices/loadscreens HD, SANS son radar → conflit)
# + Infernus DE (véhicule) + Vanilla + roads (retextures HD map) + OE Mod
# (visuel océan) + SAVSR (sons véhicules) + Next Gen Weapon Sounds + sbornik-mash
# (véhicules de service ; gyrophares AVS en attente de l'ASI). Exclus des
# derniers : tout CLEO (.cs), le timecyc d'OE, le gta_sa.exe optionnel de SAVSR.
# Retirés plus tôt : végétation, Real Linear Graphics, LOD
# Vegetation (crash 0.3.DL), Project2DFX, Effects Mod (crash 0.3.DL).
# Radar DE et Absolute Atmosphere UI sont fournis en DOSSIERS extraits dans
# mods-src/ (pas en .7z) ; seul Infernus reste une archive .7z.
REQUIRED=(
  "Real_Skybox.7z"
  "Sky_Gradient_Fix.7z"
  "Infernus_DE.7z"
  "Next_Gen_Weapon_Sounds.7z"
)

echo "=== Vérification des archives ==="
miss=0
for f in "${REQUIRED[@]}"; do
  [[ -f "$MODS_SRC/$f" ]] && echo "OK: $f" || { echo "MANQUANT: $f"; miss=1; }
done
[[ $miss -eq 1 ]] && { echo "Dépose les archives manquantes dans $MODS_SRC" >&2; exit 1; }

echo "=== Base + fichiers critiques ==="
for crit in vorbisFileLoader.dll d3d9.dll; do
  [[ -f "$BASE/$crit" ]] || { echo "ERREUR: base incomplète ($crit)" >&2; exit 1; }
done

echo "=== Staging ==="
rm -rf "$WORK"
mkdir -p "$ML"
cp -a "$BASE/." "$STAGING/"
# Project2DFX (SALodLights) retiré à la demande : on purge ses fichiers de la base.
rm -f "$STAGING"/SALodLights.asi "$STAGING"/SALodLights.dat "$STAGING"/SALodLights.ini

EX="$WORK/extract"; mkdir -p "$EX"
x7z() { mkdir -p "$EX/$1"; 7z x -y -o"$EX/$1" "$MODS_SRC/$1.7z" >/dev/null; }

echo "=== Extraction ==="
for f in "${REQUIRED[@]}"; do x7z "${f%.7z}"; done

# modloader.asi + son dossier .data (plugins std.*.dll) ----------------------
# CRITIQUE : modloader.asi seul ne fait RIEN. Ses plugins (std.stream, std.data,
# std.asi, etc.) vivent dans modloader/.data/plugins et sont indispensables pour
# charger textures/modèles/.dat/.ipl/.asi. On copie donc le dossier modloader/
# complet livré dans l'archive officielle, PUIS on y ajoute les mods.
echo "=== modloader.asi + .data (thelink2012 v0.3.9) ==="
curl -sL -o "$WORK/ml.zip" "https://github.com/thelink2012/modloader/releases/download/v0.3.9/modloader.zip"
unzip -qo "$WORK/ml.zip" -d "$WORK/mlx"
cp "$WORK/mlx/modloader.asi" "$STAGING/"
cp -a "$WORK/mlx/modloader/." "$ML/"
[[ -d "$ML/.data/plugins" ]] || { echo "ERREUR: modloader/.data/plugins absent après copie" >&2; exit 1; }

# 1) Proper Vegetation Retex → RETIRÉ à la demande.
# 2) Improved & Fixed Original Vegetation → RETIRÉ à la demande.
# 3) LOD Vegetation → RETIRÉ (crash SA-MP 0.3.DL).

# 4) Real Skybox (EN) → modloader/ (asi + ini + textures realskybox/) --------
echo "=== Real Skybox (EN) ==="
cp -a "$EX/Real_Skybox/EN/Real Skybox" "$ML/"

# 5) Real Linear Graphics → RETIRÉ à la demande.

# 6) SkyGrad → .asi à la racine (chargé par l'ASI loader vorbisFile.dll) -----
echo "=== SkyGrad ==="
find "$EX/Sky_Gradient_Fix" -iname 'skygrad.asi' -exec cp -a {} "$STAGING/" \;

# 7) Effects Mod → RETIRÉ (crash C++ 0xE06D7363 qq secondes après spawn sur 0.3.DL).

# 8) Infernus DE → modloader/ (véhicule uniquement : dff + txd) --------------
echo "=== Infernus DE ==="
ID_DFF="$(find "$EX/Infernus_DE" -iname 'infernus.dff' | head -1)"
ID_TXD="$(find "$EX/Infernus_DE" -iname 'infernus.txd' | head -1)"
[[ -n "$ID_DFF" && -n "$ID_TXD" ]] || { echo "ERREUR: infernus.dff/txd introuvable" >&2; exit 1; }
mkdir -p "$ML/Infernus DE"
cp -a "$ID_DFF" "$ML/Infernus DE/"
cp -a "$ID_TXD" "$ML/Infernus DE/"

# 9) Radar DE (ASI) → radar-trilogy-sa.asi + dossier radar/ à la racine ------
# Chargé par l'ASI loader (vorbisFile.dll). L'ASI lit ses textures dans
# le dossier "radar/" à la racine du jeu (blip.txd, map.txd).
echo "=== Radar DE ==="
RADAR_SRC="$(find "$MODS_SRC" -maxdepth 1 -type d -iname 'radar-v-stile*' | head -1)"
[[ -n "$RADAR_SRC" ]] || { echo "ERREUR: dossier Radar DE introuvable dans mods-src" >&2; exit 1; }
cp -a "$RADAR_SRC/Release/radar-trilogy-sa.asi" "$STAGING/"
cp -a "$RADAR_SRC/Release/radar" "$STAGING/"

# 10) Absolute Atmosphere UI → modloader/ SANS le dossier "HQ Radar" ---------
# On retire HQ Radar (textures radar HD) : conflit avec le radar DE, dont
# l'auteur recommande de désactiver tout autre mod touchant le radar/HUD.
echo "=== Absolute Atmosphere UI (sans HQ Radar) ==="
cp -a "$MODS_SRC/Absolute Atmosphere UI" "$ML/Absolute Atmosphere UI"
rm -rf "$ML/Absolute Atmosphere UI/HQ Radar"

# 11) Vanilla + roads → modloader/ (retextures HD map + modèles de routes) ---
# Remplacement de txd/dff d'objets EXISTANTS (pas d'IPL custom) → sûr sur 0.3.DL.
echo "=== Vanilla + roads ==="
[[ -d "$MODS_SRC/Vanilla + roads" ]] || { echo "ERREUR: dossier 'Vanilla + roads' introuvable dans mods-src" >&2; exit 1; }
cp -a "$MODS_SRC/Vanilla + roads" "$ML/Vanilla + roads"

# 12) OE Mod → modloader/ — VISUEL uniquement (algues, poissons, fond marin,
# particules d'eau). EXCLUS : Timecyc (conflit ReShade/skybox) et le .cs (CLEO
# non installé). modloader détecte les dff/txd et le pseudo-dossier gta3.img/.
echo "=== OE Mod (visuel, sans Timecyc ni CLEO) ==="
OE="$MODS_SRC/OE Mod"
[[ -d "$OE" ]] || { echo "ERREUR: dossier 'OE Mod' introuvable dans mods-src" >&2; exit 1; }
mkdir -p "$ML/OE Mod/gta3.img"
cp -a "$OE/Seaweed/." "$ML/OE Mod/"
cp -a "$OE/Water & Particles/models/." "$ML/OE Mod/"
cp -a "$OE/Fishes/models/gta3.img/." "$ML/OE Mod/gta3.img/"
cp -a "$OE/Seabed/modloader/gta3.img/." "$ML/OE Mod/gta3.img/"

# 13) Sons véhicules (SAVSR) → modloader/ — banques GENRL (sons moteur) -------
# std-bank de modloader charge Bank_XXX/sound_YYY.wav sans l'outil .exe.
# EXCLUS : (Optional)/gta_sa.exe (crash possible) et (Additional sounds).
echo "=== Sons véhicules (SAVSR / GENRL) ==="
SV="$MODS_SRC/Sons véhicules/SAVSR/GENRL"
[[ -d "$SV" ]] || { echo "ERREUR: 'Sons véhicules/SAVSR/GENRL' introuvable" >&2; exit 1; }
mkdir -p "$ML/SAVSR/GENRL"
cp -a "$SV/." "$ML/SAVSR/GENRL/"
find "$ML/SAVSR" -iname 'desktop.ini' -delete

# 14) Next Gen Weapon Sounds → modloader/ — banques de sons d'armes ----------
# EXCLU : "Dynamic Weapon Draw Sounds/cleo/" (CLEO non installé).
echo "=== Next Gen Weapon Sounds ==="
NGW="$(find "$EX/Next_Gen_Weapon_Sounds" -maxdepth 2 -type d -iname 'Next Gen Weapon Sounds' | head -1)"
[[ -n "$NGW" ]] || { echo "ERREUR: dossier 'Next Gen Weapon Sounds' introuvable après extraction" >&2; exit 1; }
cp -a "$NGW" "$ML/Next Gen Weapon Sounds"

# 15) sbornik-mash → modloader/ — VÉHICULES de service (police/FBI/DOT/pompiers)
# Remplacements de véhicules existants (dff/txd) → sûr sur 0.3.DL. Les gyrophares
# sirens/*.json (format Advanced Vehicle Sirens) sont TENUS EN ATTENTE : ils
# nécessitent l'ASI AVS (fourni séparément). On place le dossier sirens/ à la
# racine du jeu SEULEMENT si l'ASI AVS est présent dans la base.
echo "=== sbornik-mash (véhicules de service) ==="
SB="$MODS_SRC/sbornik-mash"
[[ -d "$SB/Cars" ]] || { echo "ERREUR: 'sbornik-mash/Cars' introuvable" >&2; exit 1; }
mkdir -p "$ML/sbornik-mash"
cp -a "$SB/Cars/." "$ML/sbornik-mash/"
# Advanced Vehicle Sirens : activé uniquement si l'ASI officiel est dans la base.
if compgen -G "$BASE"/*[Ss]iren*.asi > /dev/null || [[ -f "$BASE/AdvancedVehicleSirens.asi" ]]; then
  echo "  -> AVS détecté : déploiement asi + sirens/"
  cp -a "$BASE"/*[Ss]iren*.asi "$STAGING/" 2>/dev/null || true
  [[ -f "$BASE/AdvancedVehicleSirens.asi" ]] && cp -a "$BASE/AdvancedVehicleSirens.asi" "$STAGING/"
  cp -a "$SB/sirens" "$STAGING/sirens"
else
  echo "  -> AVS absent : gyrophares (sirens/*.json) NON déployés (véhicules seuls)."
fi

# --- Rapport de contrôle ----------------------------------------------------
REPORT="$ROOT/modpack-work/build-report.txt"
{
  echo "=== RAPPORT D'ASSEMBLAGE GTRP ==="
  echo "date: $(date -Is)"
  echo ""
  echo "--- .asi à la racine (chargés par le loader vorbisFile.dll) ---"
  find "$STAGING" -maxdepth 1 -iname '*.asi' -printf '  %f\n' | sort
  echo ""
  echo "--- Dossiers modloader/ ---"
  find "$ML" -maxdepth 1 -mindepth 1 -type d -printf '  %f\n' | sort
  echo ""
  echo "--- Contrôles ---"
  echo "  d3d9.dll racine : $(find "$STAGING" -maxdepth 1 -iname 'd3d9.dll' | wc -l) (attendu 1)"
  echo "  vorbisFileLoader.dll racine : $(find "$STAGING" -maxdepth 1 -iname 'vorbisFileLoader.dll' | wc -l) (attendu 1)"
  echo "  modloader.asi racine : $(find "$STAGING" -maxdepth 1 -iname 'modloader.asi' | wc -l) (attendu 1)"
  echo "  modloader/.data/plugins : $(find "$ML/.data/plugins" -iname 'std.*.dll' 2>/dev/null | wc -l) plugins (attendu >0)"
  echo "  timecyc trouvés :"
  find "$ML" -iname 'timecyc*.dat' -printf '    %P\n' || true
  echo "  .asi supplémentaires dans modloader/ :"
  find "$ML" -iname '*.asi' -printf '    %P\n' || echo "    (aucun)"
  echo "  LoadAllBinaryIPLs :"
  grep -n 'LoadAllBinaryIPLs' "$STAGING/SALodLights.ini" | sed 's/^/    /' || true
  echo ""
  echo "--- Comptage fichiers jeu (txd/dff/wav) ---"
  echo "  .txd : $(find "$ML" -iname '*.txd' | wc -l)   .dff : $(find "$ML" -iname '*.dff' | wc -l)   .wav : $(find "$ML" -iname '*.wav' | wc -l)"
  echo "  banques audio (Bank_*) : $(find "$ML" -type d -iname 'Bank_*' | wc -l)"
  echo "  CLEO résiduel (.cs) : $(find "$ML" -iname '*.cs' | wc -l) (attendu 0)"
  echo "  .exe résiduel : $(find "$ML" -iname '*.exe' | wc -l) (attendu 0)"
  echo ""
  echo "--- Taille staging ---"
  du -sh "$STAGING"
} | tee "$REPORT"

echo ""
echo "=== Archive ==="
VERSION="${1:-1.19.0}"
OUT="$ROOT/modpack-work/gtrp-modpack-$VERSION.zip"
rm -f "$OUT"
(cd "$WORK" && zip -r -q -X "$OUT" gtrp-assets)
SIZE=$(stat -c %s "$OUT"); SHA=$(sha256sum "$OUT" | awk '{print $1}')
echo "version=$VERSION size=$SIZE sha256=$SHA"

cat > "$ROOT/modpack-work/manifest.json" <<EOF
{
  "version": "$VERSION",
  "base_url": "https://github.com/Vans74/GTRP-Launcher/releases/download/modpack",
  "bundle": {
    "url": "https://github.com/Vans74/GTRP-Launcher/releases/download/modpack/gtrp-modpack-$VERSION.zip",
    "sha256": "$SHA",
    "size": $SIZE
  },
  "files": [],
  "forbidden": []
}
EOF

echo ""
echo "=== TERMINÉ (rien publié) ==="
echo "Zip      : $OUT"
echo "Manifest : $ROOT/modpack-work/manifest.json"
echo "Rapport  : $REPORT"
