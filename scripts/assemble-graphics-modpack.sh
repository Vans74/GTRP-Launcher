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

REQUIRED=(
  "Proper_Vegetation_Retex.7z"
  "Improved_and_Fixed_Original_Vegetation.7z"
  "LOD_Vegetation.7z"
  "Real_Skybox.7z"
  "Sky_Gradient_Fix.7z"
  "Real_Linear_Graphics.7z"
  "Effects_Mod_-_by_Ezekiel_-_Junior_Djjr_-_Effects_Loader.7z"
)

echo "=== Vérification des archives ==="
miss=0
for f in "${REQUIRED[@]}"; do
  [[ -f "$MODS_SRC/$f" ]] && echo "OK: $f" || { echo "MANQUANT: $f"; miss=1; }
done
[[ $miss -eq 1 ]] && { echo "Dépose les archives manquantes dans $MODS_SRC" >&2; exit 1; }

echo "=== Base + fichiers critiques ==="
for crit in dinput8.dll d3d9.dll; do
  [[ -f "$BASE/$crit" ]] || { echo "ERREUR: base incomplète ($crit)" >&2; exit 1; }
done

echo "=== Staging ==="
rm -rf "$WORK"
mkdir -p "$ML"
cp -a "$BASE/." "$STAGING/"
# Correctif crash 0x006FF35B (LOD + Project2DFX).
[[ -f "$STAGING/SALodLights.ini" ]] && sed -i 's/^LoadAllBinaryIPLs *= *1/LoadAllBinaryIPLs = 0/' "$STAGING/SALodLights.ini"

EX="$WORK/extract"; mkdir -p "$EX"
x7z() { mkdir -p "$EX/$1"; 7z x -y -o"$EX/$1" "$MODS_SRC/$1.7z" >/dev/null; }

echo "=== Extraction ==="
for f in "${REQUIRED[@]}"; do x7z "${f%.7z}"; done

# modloader.asi (loader de mods) --------------------------------------------
echo "=== modloader.asi (thelink2012 v0.3.9) ==="
curl -sL -o "$WORK/ml.zip" "https://github.com/thelink2012/modloader/releases/download/v0.3.9/modloader.zip"
unzip -qo "$WORK/ml.zip" -d "$WORK/mlx"
cp "$WORK/mlx/modloader.asi" "$STAGING/"

# 1) Proper Vegetation Retex → modloader/ (textures .txd) --------------------
echo "=== Proper Vegetation Retex ==="
cp -a "$EX/Proper_Vegetation_Retex/Proper Vegetation Retex" "$ML/"

# 2) Improved & Fixed Original Vegetation → modloader/ (modèles .dff) ---------
echo "=== Improved & Fixed Original Vegetation ==="
cp -a "$EX/Improved_and_Fixed_Original_Vegetation/Improved and Fixed Original Vegetation" "$ML/"

# 3) LOD Vegetation → modloader/ (Loader.txt gère COL+IDE, y compris SA-MP) ---
echo "=== LOD Vegetation ==="
cp -a "$EX/LOD_Vegetation/LOD Vegetation" "$ML/"

# 4) Real Skybox (EN) → modloader/ (asi + ini + textures realskybox/) --------
echo "=== Real Skybox (EN) ==="
cp -a "$EX/Real_Skybox/EN/Real Skybox" "$ML/"

# 5) Real Linear Graphics → variante LITE (sans dépendance 24h TimeCycle) -----
echo "=== Real Linear Graphics (Lite, sans 24h) ==="
cp -a "$EX/Real_Linear_Graphics/(alt)/(lite - no 24h timecycle)/Real Linear Graphics Lite" "$ML/Real Linear Graphics"

# 6) SkyGrad → .asi à la racine (chargé par dinput8.dll) ---------------------
echo "=== SkyGrad ==="
find "$EX/Sky_Gradient_Fix" -iname 'skygrad.asi' -exec cp -a {} "$STAGING/" \;

# 7) Effects Mod → modloader/ (Effects, Effects Loader, FxsFuncs) + models/ --
echo "=== Effects Mod ==="
EM="$EX/Effects_Mod_-_by_Ezekiel_-_Junior_Djjr_-_Effects_Loader"
if [[ -d "$EM/modloader" ]]; then cp -a "$EM/modloader/." "$ML/"; fi
if [[ -d "$EM/models" ]]; then cp -a "$EM/models" "$STAGING/"; fi

# --- Rapport de contrôle ----------------------------------------------------
REPORT="$ROOT/modpack-work/build-report.txt"
{
  echo "=== RAPPORT D'ASSEMBLAGE GTRP ==="
  echo "date: $(date -Is)"
  echo ""
  echo "--- .asi à la racine (chargés par dinput8.dll) ---"
  find "$STAGING" -maxdepth 1 -iname '*.asi' -printf '  %f\n' | sort
  echo ""
  echo "--- Dossiers modloader/ ---"
  find "$ML" -maxdepth 1 -mindepth 1 -type d -printf '  %f\n' | sort
  echo ""
  echo "--- Contrôles ---"
  echo "  d3d9.dll racine : $(find "$STAGING" -maxdepth 1 -iname 'd3d9.dll' | wc -l) (attendu 1)"
  echo "  dinput8.dll racine : $(find "$STAGING" -maxdepth 1 -iname 'dinput8.dll' | wc -l) (attendu 1)"
  echo "  modloader.asi racine : $(find "$STAGING" -maxdepth 1 -iname 'modloader.asi' | wc -l) (attendu 1)"
  echo "  timecyc trouvés :"
  find "$ML" -iname 'timecyc*.dat' -printf '    %P\n' || true
  echo "  .asi supplémentaires dans modloader/ :"
  find "$ML" -iname '*.asi' -printf '    %P\n' || echo "    (aucun)"
  echo "  LoadAllBinaryIPLs :"
  grep -n 'LoadAllBinaryIPLs' "$STAGING/SALodLights.ini" | sed 's/^/    /' || true
  echo ""
  echo "--- Comptage fichiers jeu (txd/dff) ---"
  echo "  .txd : $(find "$ML" -iname '*.txd' | wc -l)   .dff : $(find "$ML" -iname '*.dff' | wc -l)"
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
