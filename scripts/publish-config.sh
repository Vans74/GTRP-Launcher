#!/usr/bin/env bash
# Publie UNIQUEMENT les fichiers de config ReShade en différentiel, sans
# reconstruire ni ré-uploader le bundle complet (320 Mo).
#
# Principe : ces petits fichiers sont référencés dans "files" du manifeste. Le
# launcher compare leur SHA-256 local et ne télécharge QUE ceux qui ont changé.
# Sans changement de version, aucun bundle. Pour un patch avec bump de version,
# utiliser publish-patch.sh (bundle_required=false, launcher 0.1.11+).
# fichier comme asset nommé par son hash (ex. ReShadePreset.<sha8>.ini) pour
# éviter tout cache CDN périmé, puis on patche le manifeste live.
#
# Usage :
#   GTRP_GH_TOKEN=ghp_xxx ./scripts/publish-config.sh
#
# Source de vérité des fichiers : modpack-work/graphics-base/gtrp-assets/enb/
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="Vans74/GTRP-Launcher"
TAG="modpack"
BASE_PUB_URL="https://github.com/$REPO/releases/download/$TAG"
SRC_DIR="$ROOT/modpack-work/graphics-base/gtrp-assets/enb"
CFG_FILES=("ReShade.ini" "ReShadePreset.ini")

if [[ -z "${GTRP_GH_TOKEN:-}" ]]; then
  echo "ERREUR: definis la variable GTRP_GH_TOKEN avec ton token GitHub." >&2
  exit 1
fi

api() { curl -s -H "Authorization: token $GTRP_GH_TOKEN" "$@"; }

echo "=== Récupération de la release '$TAG' ==="
REL_ID=$(api "https://api.github.com/repos/$REPO/releases/tags/$TAG" | python3 -c "import sys,json;print(json.load(sys.stdin)['id'])")
[[ -n "$REL_ID" ]] || { echo "ERREUR: release '$TAG' introuvable" >&2; exit 1; }
echo "release id = $REL_ID"

# Assets existants (nom -> id) pour éviter les doublons / faire du ménage.
ASSETS_JSON=$(api "https://api.github.com/repos/$REPO/releases/$REL_ID/assets?per_page=100")

declare -a NEW_FILES_SPECS=()
for cf in "${CFG_FILES[@]}"; do
  src="$SRC_DIR/$cf"
  [[ -f "$src" ]] || { echo "ERREUR: $src introuvable" >&2; exit 1; }
  sha=$(sha256sum "$src" | awk '{print $1}')
  size=$(stat -c %s "$src")
  ext="${cf##*.}"; base="${cf%.*}"
  asset="${base}.${sha:0:8}.${ext}"

  # Upload seulement si cet asset (ce hash précis) n'existe pas déjà.
  if echo "$ASSETS_JSON" | grep -qF "\"$asset\""; then
    echo "  = $asset deja present (inchange)"
  else
    echo "  + upload $asset"
    curl -s -X POST -H "Authorization: token $GTRP_GH_TOKEN" -H "Content-Type: application/octet-stream" \
      --data-binary @"$src" \
      "https://uploads.github.com/repos/$REPO/releases/$REL_ID/assets?name=$asset" \
      | python3 -c "import sys,json;d=json.load(sys.stdin);print('    ->',d.get('name'),d.get('state'))"
  fi
  NEW_FILES_SPECS+=("gtrp-assets/enb/$cf|$sha|$size|$BASE_PUB_URL/$asset")
done

echo "=== Patch du manifeste live (version + bundle inchangés) ==="
LIVE_MANIFEST=""
for attempt in 1 2 3 4 5; do
  LIVE_MANIFEST=$(curl -s -L "$BASE_PUB_URL/manifest.json?x=$(date +%s%N)")
  if echo "$LIVE_MANIFEST" | python3 -c "import sys,json;json.load(sys.stdin)" 2>/dev/null; then
    break
  fi
  echo "  (tentative $attempt : manifeste indisponible, nouvel essai…)"
  LIVE_MANIFEST=""
  sleep 3
done
[[ -n "$LIVE_MANIFEST" ]] || { echo "ERREUR: impossible de récupérer le manifeste live." >&2; exit 1; }
LIVE_MANIFEST="$LIVE_MANIFEST" SPECS="${NEW_FILES_SPECS[*]}" python3 - > "$ROOT/modpack-work/manifest.json" <<'PY'
import os, sys, json
m = json.loads(os.environ["LIVE_MANIFEST"])
files = []
for spec in os.environ["SPECS"].split():
    path, sha, size, url = spec.split("|")
    files.append({"path": path, "sha256": sha, "size": int(size), "url": url})
m["files"] = files
json.dump(m, sys.stdout, indent=2, ensure_ascii=False)
PY
echo "  nouveau manifeste :"
python3 -c "import json;m=json.load(open('$ROOT/modpack-work/manifest.json'));print('    version',m['version'],'| files:',[f['path'] for f in m['files']])"

# Remplace l'asset manifest.json.
OLD_MID=$(echo "$ASSETS_JSON" | python3 -c "import sys,json;print(next((a['id'] for a in json.load(sys.stdin) if a['name']=='manifest.json'),''))")
[[ -n "$OLD_MID" ]] && api -X DELETE "https://api.github.com/repos/$REPO/releases/assets/$OLD_MID" >/dev/null
curl -s -X POST -H "Authorization: token $GTRP_GH_TOKEN" -H "Content-Type: application/json" \
  --data-binary @"$ROOT/modpack-work/manifest.json" \
  "https://uploads.github.com/repos/$REPO/releases/$REL_ID/assets?name=manifest.json" \
  | python3 -c "import sys,json;d=json.load(sys.stdin);print('  manifest ->',d.get('name'),d.get('state'))"

echo ""
echo "=== TERMINÉ : les joueurs ne re-téléchargeront que les fichiers config modifiés ==="
