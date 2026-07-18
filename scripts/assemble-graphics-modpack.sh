#!/usr/bin/env bash
# Assemble le modpack GTRP : contenus permanents + graphismes HD optionnels.
#
# Usage : ./assemble-graphics-modpack.sh [VERSION]   (défaut 1.19.0)
#
# Placement EXPLICITE par mod (leur structure a été inspectée : aucun ne contient
# de dossier "modloader/" — leur dossier racine se dépose DANS modloader/).
# Le script ne publie rien ; il produit le zip, le manifest et un rapport.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPTS="$(cd "$(dirname "$0")" && pwd)"
MODS_SRC="$ROOT/mods-src"
WORK="$ROOT/modpack-work/build"
STAGING="$WORK/gtrp-assets/enb"
ML="$STAGING/modloader"

BASE_REPO="$ROOT/modpack-work/graphics-base/gtrp-assets/enb"
BASE_TMP="/tmp/reshade/staging/gtrp-assets/enb"
if [[ -d "$BASE_REPO" ]]; then BASE="$BASE_REPO"
elif [[ -d "$BASE_TMP" ]]; then BASE="$BASE_TMP"
else echo "ERREUR: base graphique introuvable." >&2; exit 1; fi

# NOTE (v1.44.0) : le bouton ne contrôle plus le modpack entier. Véhicules,
# skins, armes, sons, interface, radar, modloader et ImVehFt sont permanents.
# ReShade, les routes HD et OE Mod sont les seuls contenus conditionnels.
# Le moteur et les shaders ne sont jamais réhébergés : le launcher les télécharge
# depuis ReShade et le dépôt officiel crosire, puis vérifie chaque SHA-256. Seuls
# les réglages neutres/stables GTRP sont livrés ici.
# 597 rabaissé PROPREMENT (toute la carrosserie -0.055, roues
# exclues → plus aucune pièce ni gyrophare « en suspend »). Gyrophares 597 =
# éclairage universel ImVehFt sur les frames light_em (EML custom retiré).
# ImVehFt 2.1.1 (clignotants, feux).
# + Torrence Police LV, Stanier Unmarked, HVY APC, New Weapons Pack, Stanier LED (597).
# ENBSeries, SkyGFX, Proper Shaders et Project2DFX retirés (remplacés par ReShade).
# Atmosphere UI + Infernus DE + Vanilla + roads + OE Mod + Next Gen Weapon Sounds.
# Real Skybox RETIRÉ (chaîne graphique volontairement minimale et stable).
# Radar DE et Absolute Atmosphere UI sont fournis en DOSSIERS extraits dans
# mods-src/ (pas en .7z) ; seul Infernus reste une archive .7z.
REQUIRED=(
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
for crit in vorbisFileLoader.dll; do
  [[ -f "$BASE/$crit" ]] || { echo "ERREUR: base incomplète ($crit)" >&2; exit 1; }
done

echo "=== Staging ==="
rm -rf "$WORK"
mkdir -p "$ML"
cp -a "$BASE/." "$STAGING/"
# Retire intégralement les anciens moteurs/effets graphiques. Ils ne doivent ni
# cohabiter avec ReShade, ni rester actifs lorsque le bouton HD est coupé.
rm -f "$STAGING"/SALodLights.asi "$STAGING"/SALodLights.dat "$STAGING"/SALodLights.ini
rm -f "$STAGING"/d3d9.dll "$STAGING"/d3d9.dll.orig-splash
rm -f "$STAGING"/ReShade*
rm -f "$STAGING"/skygfx.asi "$STAGING"/skygfx.ini "$STAGING"/skygfx1.ini "$STAGING"/skygfx2.ini "$STAGING"/skygfx3.ini
rm -f "$STAGING"/skygrad.asi
rm -rf "$STAGING"/reshade-shaders "$STAGING"/neo "$STAGING"/models
rm -f "$STAGING"/enbseries.asi "$STAGING"/enblocal.ini "$STAGING"/enbseries.ini
rm -f "$STAGING"/enb*.fx "$STAGING"/enb*.fx.ini "$STAGING"/enbseries.log
rm -rf "$STAGING"/enbseries "$ML/Proper Shaders"
rm -f "$STAGING"/data/colorcycle.dat
find "$STAGING/data" -type d -empty -delete 2>/dev/null || true
echo "=== Project2DFX (SALodLights) : retiré ==="
echo "=== Ancien ReShade / ENB / SkyGFX / SkyGrad : retirés ==="
echo "=== Proper Shaders : retiré ==="

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

# 4) Real Skybox → RETIRÉ (chaîne graphique volontairement minimale).

# 5) Real Linear Graphics → RETIRÉ à la demande.

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

# 13) Sons véhicules (SAVSR) → RETIRÉ à la demande.

# 14) Next Gen Weapon Sounds → modloader/ — banques de sons d'armes ----------
# EXCLU : "Dynamic Weapon Draw Sounds/cleo/" (CLEO non installé).
echo "=== Next Gen Weapon Sounds ==="
NGW="$(find "$EX/Next_Gen_Weapon_Sounds" -maxdepth 2 -type d -iname 'Next Gen Weapon Sounds' | head -1)"
[[ -n "$NGW" ]] || { echo "ERREUR: dossier 'Next Gen Weapon Sounds' introuvable après extraction" >&2; exit 1; }
cp -a "$NGW" "$ML/Next Gen Weapon Sounds"

# 15) sbornik-mash → RETIRÉ à la demande (véhicules police/FBI/DOT/pompiers).

# 16) Vapid Stanier Police Cruiser LED (copcarsf / Police SF, id 597) ---------
# Source : https://www.gtaall.com/gta-san-andreas/cars/205991-vapid-stanier-police-cruiser-led-lights.html
# Remplace le modèle 597. Patch SA-MP : noms de frames >23 caractères → crash 0.3.DL.
echo "=== Vapid Stanier Police LED (copcarsf) ==="
VS_SRC="$(find "$MODS_SRC" -maxdepth 1 -type d \( -iname 'vapid*stanier*police*led*' -o -iname 'vapid stanier police led' \) | head -1)"
[[ -n "$VS_SRC" ]] || { echo "ERREUR: dossier 'Vapid Stanier Police LED' introuvable dans mods-src" >&2; exit 1; }
[[ -f "$VS_SRC/copcarsf.dff" && -f "$VS_SRC/copcarsf.txd" ]] || { echo "ERREUR: copcarsf.dff/txd introuvable dans $VS_SRC" >&2; exit 1; }
mkdir -p "$ML/Vapid Stanier Police LED"
TMP_DFF="$WORK/copcarsf-patched.dff"
TMP_DFF2="$WORK/copcarsf-lowered.dff"
python3 "$SCRIPTS/patch-dff-samp.py" "$VS_SRC/copcarsf.dff" "$TMP_DFF"
# Rabaissement propre : on descend TOUTE la carrosserie (portes, capot, pare-chocs,
# phares, gyrophares light_em… tout ce qui est enfant direct de la racine) de -0.055,
# roues exclues. Décaler seulement chassis_dummy laissait ces pièces « en suspend ».
python3 "$SCRIPTS/lower-vehicle-body.py" "$TMP_DFF" "$TMP_DFF2" -0.055
# Calage des coronas : ImVehFt dessine la corona d'urgence AU-DESSUS du point
# light_em. On abaisse les ancres de la rampe de toit (light_em9-14) pour poser
# la lumière pile sur la rampe (le reste de la carrosserie ne bouge pas).
python3 "$SCRIPTS/lower-frames-z.py" "$TMP_DFF2" "$ML/Vapid Stanier Police LED/copcarsf.dff" -0.13 \
  light_em9 light_em10 light_em11 light_em12 light_em13 light_em14
cp -f "$VS_SRC/copcarsf.txd" "$ML/Vapid Stanier Police LED/"
echo "  -> copcarsf.txd ($(du -h "$VS_SRC/copcarsf.txd" | awk '{print $1}'))"

# 16b) Vapid Torrence Police Las Venturas v2 (copcarvg / Police LV) ----------
# Source : https://www.gtaall.com/gta-san-andreas/cars/156936-vapid-torrence-police-las-venturas-v2.html
echo "=== Vapid Torrence Police LV (copcarvg) ==="
TLV_SRC="$(find "$MODS_SRC" -maxdepth 1 -type d \( -iname 'vapid*torrence*police*lv*' -o -iname 'vapid torrence police lv' \) | head -1)"
[[ -n "$TLV_SRC" ]] || { echo "ERREUR: dossier 'Vapid Torrence Police LV' introuvable dans mods-src" >&2; exit 1; }
[[ -f "$TLV_SRC/copcarvg.dff" && -f "$TLV_SRC/copcarvg.txd" ]] || { echo "ERREUR: copcarvg.dff/txd introuvable dans $TLV_SRC" >&2; exit 1; }
mkdir -p "$ML/Vapid Torrence Police LV"
python3 "$SCRIPTS/patch-dff-samp.py" "$TLV_SRC/copcarvg.dff" "$ML/Vapid Torrence Police LV/copcarvg.dff"
cp -f "$TLV_SRC/copcarvg.txd" "$ML/Vapid Torrence Police LV/"
echo "  -> copcarvg.txd ($(du -h "$TLV_SRC/copcarvg.txd" | awk '{print $1}'))"

# 16c) Vapid Stanier Unmarked Cruiser (copcarla / Police LS) ------------------
# Source : https://www.gtaall.com/gta-san-andreas/cars/205992-vapid-stanier-unmarked-cruiser.html
echo "=== Vapid Stanier Unmarked (copcarla) ==="
SU_SRC="$(find "$MODS_SRC" -maxdepth 1 -type d \( -iname 'vapid*stanier*unmarked*' -o -iname 'vapid stanier unmarked' \) | head -1)"
[[ -n "$SU_SRC" ]] || { echo "ERREUR: dossier 'Vapid Stanier Unmarked' introuvable dans mods-src" >&2; exit 1; }
[[ -f "$SU_SRC/copcarla.dff" && -f "$SU_SRC/copcarla.txd" ]] || { echo "ERREUR: copcarla.dff/txd introuvable dans $SU_SRC" >&2; exit 1; }
mkdir -p "$ML/Vapid Stanier Unmarked"
python3 "$SCRIPTS/patch-dff-samp.py" "$SU_SRC/copcarla.dff" "$ML/Vapid Stanier Unmarked/copcarla.dff"
cp -f "$SU_SRC/copcarla.txd" "$ML/Vapid Stanier Unmarked/"
echo "  -> copcarla.txd ($(du -h "$SU_SRC/copcarla.txd" | awk '{print $1}'))"

# 16d) GTA V HVY APC (swatvan / SWAT) -----------------------------------------
# Source : https://www.gtaall.com/gta-san-andreas/cars/272540-gta-v-hvy-apc.html
echo "=== HVY APC (swatvan) ==="
APC_SRC="$(find "$MODS_SRC" -maxdepth 1 -type d \( -iname 'hvy*apc*' -o -iname 'hvy apc' \) | head -1)"
[[ -n "$APC_SRC" ]] || { echo "ERREUR: dossier 'HVY APC' introuvable dans mods-src" >&2; exit 1; }
[[ -f "$APC_SRC/swatvan.dff" && -f "$APC_SRC/swatvan.txd" ]] || { echo "ERREUR: swatvan.dff/txd introuvable dans $APC_SRC" >&2; exit 1; }
mkdir -p "$ML/HVY APC"
python3 "$SCRIPTS/patch-dff-samp.py" "$APC_SRC/swatvan.dff" "$ML/HVY APC/swatvan.dff"
cp -f "$APC_SRC/swatvan.txd" "$ML/HVY APC/"
echo "  -> swatvan.txd ($(du -h "$APC_SRC/swatvan.txd" | awk '{print $1}'))"

# 16e) New Weapons Pack (modèles/textures armes) ------------------------------
# Source : https://www.gtaall.com/gta-san-andreas/weapons/15673-new-weapons-pack.html
# Compatible avec Next Gen Weapon Sounds (sons uniquement, pas de conflit DFF/TXD).
echo "=== New Weapons Pack ==="
NWP_SRC="$(find "$MODS_SRC" -maxdepth 1 -type d \( -iname 'new*weapons*pack*' -o -iname 'new weapons pack' \) | head -1)"
[[ -n "$NWP_SRC" ]] || { echo "ERREUR: dossier 'New Weapons Pack' introuvable dans mods-src" >&2; exit 1; }
NWP_COUNT="$(find "$NWP_SRC" -maxdepth 1 \( -iname '*.dff' -o -iname '*.txd' \) | wc -l)"
[[ "$NWP_COUNT" -ge 10 ]] || { echo "ERREUR: New Weapons Pack incomplet ($NWP_COUNT fichiers)" >&2; exit 1; }
mkdir -p "$ML/New Weapons Pack"
cp -a "$NWP_SRC/." "$ML/New Weapons Pack/"
echo "  -> $NWP_COUNT fichier(s) dff/txd"

# 17) ReShade autonome + profil GTRP -----------------------------------------
# Le moteur et les shaders ne sont PAS inclus dans ce modpack. Le launcher
# télécharge l'installateur officiel et deux archives de shaders épinglées,
# vérifie chaque hash, extrait une liste blanche puis applique le profil GTRP.
echo "=== ReShade 6.7.3 (sources vérifiées + profil GTRP) ==="
ASSETS="$ROOT/assets"
for asset in hd-paths.txt reshade-source.json ReShade-GTRP/ReShade.ini ReShade-GTRP/GTRP-HD.ini; do
  [[ -f "$ASSETS/$asset" ]] || { echo "ERREUR: asset $asset absent" >&2; exit 1; }
done
cp -a "$ASSETS/ReShade-GTRP/." "$STAGING/"
cp -f "$ASSETS/hd-paths.txt" "$STAGING/.gtrp-hd-paths"
cp -f "$ASSETS/reshade-source.json" "$STAGING/.gtrp-hd-component.json"
echo "  -> binaires/shaders non réhébergés ; téléchargements épinglés ; profil sans profondeur"

# 18) ImVehFt 2.1.1 → racine (gyrophares, clignotants, feux, saleté) ------------
# Source : https://www.gtaall.com/gta-san-andreas/cleo/119689-improved-vehicle-features-211.html
# .asi chargé par le loader vorbisFileLoader.dll. Données dans ImVehFt/ à la racine.
# SAMP_fix=1 (compat SA-MP). Gyrophares 597 : le modèle est « IVF adapted » et
# possède ses 14 frames light_em intégrées ; ImVehFt les anime automatiquement via
# son éclairage d'urgence UNIVERSEL. AUCUN EML custom (un EML mal formé plaçait des
# coronas parasites « en suspend »).
echo "=== ImVehFt 2.1.1 ==="
IVF_SRC="$(find "$MODS_SRC" -maxdepth 1 -type d -iname 'ImVehFt' | head -1)"
[[ -n "$IVF_SRC" ]] || { echo "ERREUR: dossier 'ImVehFt' introuvable dans mods-src" >&2; exit 1; }
[[ -f "$IVF_SRC/ImVehFt.asi" ]] || { echo "ERREUR: ImVehFt.asi introuvable dans $IVF_SRC" >&2; exit 1; }
[[ -d "$IVF_SRC/ImVehFt" ]] || { echo "ERREUR: dossier données ImVehFt/ introuvable dans $IVF_SRC" >&2; exit 1; }
cp -f "$IVF_SRC/ImVehFt.asi" "$STAGING/"
cp -a "$IVF_SRC/ImVehFt" "$STAGING/"
# Purge tout EML custom 597 (on s'appuie sur l'éclairage universel des light_em).
rm -f "$STAGING/ImVehFt/eml/597.eml"
grep -q '^SAMP_fix=1' "$STAGING/ImVehFt/ImVehFt.ini" || { echo "ERREUR: SAMP_fix=1 absent de ImVehFt.ini" >&2; exit 1; }
echo "  -> ImVehFt.asi + ImVehFt/ (SAMP_fix=1, gyrophares 597 = light_em universels)"

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
  echo "  anciens d3d9/ReShade/SkyGFX : $(find "$STAGING" -maxdepth 1 \( -iname 'd3d9.dll' -o -iname 'skygfx.asi' \) | wc -l) (attendu 0)"
  echo "  vorbisFileLoader.dll racine : $(find "$STAGING" -maxdepth 1 -iname 'vorbisFileLoader.dll' | wc -l) (attendu 1)"
  echo "  modloader.asi racine : $(find "$STAGING" -maxdepth 1 -iname 'modloader.asi' | wc -l) (attendu 1)"
  echo "  modloader/.data/plugins : $(find "$ML/.data/plugins" -iname 'std.*.dll' 2>/dev/null | wc -l) plugins (attendu >0)"
  echo "  timecyc trouvés :"
  find "$ML" -iname 'timecyc*.dat' -printf '    %P\n' || true
  echo "  .asi supplémentaires dans modloader/ :"
  find "$ML" -iname '*.asi' -printf '    %P\n' || echo "    (aucun)"
  echo "  sources ReShade : $(test -f "$STAGING/.gtrp-hd-component.json" && echo présentes || echo ABSENTES)"
  echo "  profil ReShade GTRP : $(test -f "$STAGING/GTRP-HD.ini" && echo présent || echo ABSENT)"
  echo "  binaire d3d9.dll réhébergé : $(find "$STAGING" -iname 'd3d9.dll' | wc -l) (attendu 0)"
  echo "  shaders .fx réhébergés : $(find "$STAGING" -iname '*.fx' | wc -l) (attendu 0)"
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

# --- Fichiers "config" différentiels ----------------------------------------
# Ces petits fichiers texte (séparation HD, source officielle et preset) sont
# référencés individuellement
# dans "files". Le launcher compare leur SHA-256 local et ne re-télécharge QUE
# ceux qui ont changé, SANS reprendre le bundle complet — à condition de ne pas
# changer "version" (sinon le bundle prime). Chaque asset est nommé par son hash
# (ex. GTRP-HD.<sha8>.ini) pour éviter tout cache CDN périmé au download.
BASE_PUB_URL="https://github.com/Vans74/GTRP-Launcher/releases/download/modpack"
CFG_FILES=(
  ".gtrp-hd-paths"
  ".gtrp-hd-component.json"
  "ReShade.ini"
  "GTRP-HD.ini"
)

# Nettoie les anciennes copies d'assets config avant régénération.
rm -f "$ROOT"/modpack-work/hd-paths.*.txt "$ROOT"/modpack-work/reshade-source.*.json "$ROOT"/modpack-work/ReShade.*.ini "$ROOT"/modpack-work/GTRP-HD.*.ini 2>/dev/null || true

CFG_SPECS=()   # "path_in_game|asset_name" pour l'étape d'upload
for cf in "${CFG_FILES[@]}"; do
  src="$STAGING/$cf"
  [[ -f "$src" ]] || { echo "ERREUR: config $cf absente du staging" >&2; exit 1; }
  csha=$(sha256sum "$src" | awk '{print $1}')
  cname="$(basename "$cf")"
  case "$cname" in
    .gtrp-hd-paths) cbase="hd-paths"; cext="txt" ;;
    .gtrp-hd-component.json) cbase="reshade-source"; cext="json" ;;
    *) cext="${cname##*.}"; cbase="${cname%.*}" ;;
  esac
  asset="${cbase}.${csha:0:8}.${cext}"
  cp -f "$src" "$ROOT/modpack-work/$asset"
  CFG_SPECS+=("gtrp-assets/enb/$cf|$asset")
done

CFG_SPECS_JOINED="$(printf '%s;;' "${CFG_SPECS[@]}")"
VERSION="$VERSION" SHA="$SHA" SIZE="$SIZE" BASE_PUB_URL="$BASE_PUB_URL" STAGING="$STAGING" \
CFG_SPECS="$CFG_SPECS_JOINED" python3 - "$ROOT/modpack-work/manifest.json" <<'PY'
import os, sys, json, hashlib
out = sys.argv[1]
base = os.environ["BASE_PUB_URL"]
staging = os.environ["STAGING"]
files = []
for spec in filter(None, os.environ["CFG_SPECS"].split(";;")):
    path_in_game, asset = spec.split("|")
    local = os.path.join(staging, path_in_game.split("gtrp-assets/enb/", 1)[1])
    data = open(local, "rb").read()
    files.append({
        "path": path_in_game,
        "sha256": hashlib.sha256(data).hexdigest(),
        "size": len(data),
        "url": f"{base}/{asset}",
    })
manifest = {
    "version": os.environ["VERSION"],
    "base_url": base,
    "bundle": {
        "url": f'{base}/gtrp-modpack-{os.environ["VERSION"]}.zip',
        "sha256": os.environ["SHA"],
        "size": int(os.environ["SIZE"]),
    },
    "bundle_required": True,
    "files": files,
    "forbidden": [],
}
json.dump(manifest, open(out, "w"), indent=2, ensure_ascii=False)
print("manifest écrit avec", len(files), "fichier(s) config différentiel(s)")
PY

echo ""
echo "=== TERMINÉ (rien publié) ==="
echo "Zip      : $OUT"
echo "Manifest : $ROOT/modpack-work/manifest.json"
echo "Rapport  : $REPORT"
echo ""
echo "Assets config à uploader (en plus du bundle + manifest) :"
for spec in "${CFG_SPECS[@]}"; do echo "  ${spec#*|}"; done
