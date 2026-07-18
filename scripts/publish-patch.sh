#!/usr/bin/env bash
# Publie un patch léger : quelques fichiers individuels + bump de version,
# SANS re-télécharger le bundle complet (~260 Mo).
#
# Le manifeste live reçoit "bundle_required": false et la liste "files" est
# remplacée par les fichiers du patch. Le launcher 0.1.11+ ne télécharge que
# ceux dont le SHA-256 local diffère.
#
# Usage :
#   GTRP_GH_TOKEN=ghp_xxx ./scripts/publish-patch.sh 1.34.4 \
#     modloader/.../copcarsf.dff \
#     gtrp-assets/enb/GTRP-HD.ini
#
# Chaque chemin est relatif au dossier racine GTA (comme dans le manifeste).
# Le fichier source est cherché dans modpack-work/graphics-base/<chemin>.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="Vans74/GTRP-Launcher"
TAG="modpack"
BASE_PUB_URL="https://github.com/$REPO/releases/download/$TAG"
SRC_ROOT="$ROOT/modpack-work/graphics-base"

if [[ -z "${GTRP_GH_TOKEN:-}" ]]; then
  echo "ERREUR: definis la variable GTRP_GH_TOKEN avec ton token GitHub." >&2
  exit 1
fi

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <nouvelle_version> <chemin_relatif_gta> [...]" >&2
  exit 1
fi

NEW_VERSION="$1"
shift
PATCH_PATHS=("$@")

api() { curl -s -H "Authorization: token $GTRP_GH_TOKEN" "$@"; }

echo "=== Récupération de la release '$TAG' ==="
REL_ID=$(api "https://api.github.com/repos/$REPO/releases/tags/$TAG" | python3 -c "import sys,json;print(json.load(sys.stdin)['id'])")
[[ -n "$REL_ID" ]] || { echo "ERREUR: release '$TAG' introuvable" >&2; exit 1; }
echo "release id = $REL_ID"

ASSETS_JSON=$(api "https://api.github.com/repos/$REPO/releases/$REL_ID/assets?per_page=100")

declare -a NEW_FILES_SPECS=()
for rel in "${PATCH_PATHS[@]}"; do
  src="$SRC_ROOT/$rel"
  [[ -f "$src" ]] || { echo "ERREUR: $src introuvable" >&2; exit 1; }
  sha=$(sha256sum "$src" | awk '{print $1}')
  size=$(stat -c %s "$src")
  base_name=$(basename "$rel")
  ext="${base_name##*.}"
  stem="${base_name%.*}"
  if [[ "$base_name" == "$stem" ]]; then
    asset="${stem}.${sha:0:8}"
  else
    asset="${stem}.${sha:0:8}.${ext}"
  fi

  if echo "$ASSETS_JSON" | grep -qF "\"$asset\""; then
    echo "  = $asset deja present (inchange)"
  else
    echo "  + upload $asset ($rel)"
    curl -s -X POST -H "Authorization: token $GTRP_GH_TOKEN" -H "Content-Type: application/octet-stream" \
      --data-binary @"$src" \
      "https://uploads.github.com/repos/$REPO/releases/$REL_ID/assets?name=$asset" \
      | python3 -c "import sys,json;d=json.load(sys.stdin);print('    ->',d.get('name'),d.get('state'))"
  fi
  NEW_FILES_SPECS+=("$rel|$sha|$size|$BASE_PUB_URL/$asset")
done

echo "=== Patch du manifeste live -> version $NEW_VERSION (bundle_required=false) ==="
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

LIVE_MANIFEST="$LIVE_MANIFEST" NEW_VERSION="$NEW_VERSION" SPECS="$(printf '%s\n' "${NEW_FILES_SPECS[@]}")" python3 - > "$ROOT/modpack-work/manifest.json" <<'PY'
import os, sys, json
m = json.loads(os.environ["LIVE_MANIFEST"])
m["version"] = os.environ["NEW_VERSION"]
m["bundle_required"] = False
files = []
for spec in os.environ["SPECS"].splitlines():
    if not spec.strip():
        continue
    path, sha, size, url = spec.split("|")
    files.append({"path": path, "sha256": sha, "size": int(size), "url": url})
m["files"] = files
json.dump(m, sys.stdout, indent=2, ensure_ascii=False)
PY

echo "  nouveau manifeste :"
python3 -c "import json;m=json.load(open('$ROOT/modpack-work/manifest.json'));print('    version',m['version'],'| bundle_required',m.get('bundle_required'),'| files:',[f['path'] for f in m['files']])"

OLD_MID=$(echo "$ASSETS_JSON" | python3 -c "import sys,json;print(next((a['id'] for a in json.load(sys.stdin) if a['name']=='manifest.json'),''))")
[[ -n "$OLD_MID" ]] && api -X DELETE "https://api.github.com/repos/$REPO/releases/assets/$OLD_MID" >/dev/null
curl -s -X POST -H "Authorization: token $GTRP_GH_TOKEN" -H "Content-Type: application/json" \
  --data-binary @"$ROOT/modpack-work/manifest.json" \
  "https://uploads.github.com/repos/$REPO/releases/$REL_ID/assets?name=manifest.json" \
  | python3 -c "import sys,json;d=json.load(sys.stdin);print('  manifest ->',d.get('name'),d.get('state'))"

echo ""
echo "=== TERMINÉ : les joueurs (launcher 0.1.11+) ne téléchargeront que les fichiers du patch ==="
