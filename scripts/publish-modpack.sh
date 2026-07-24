#!/usr/bin/env bash
# Publie un modpack complet (bundle + manifest + assets config) sur la release
# GitHub permanente "modpack".
#
# Usage :
#   GTRP_GH_TOKEN=ghp_xxx ./scripts/publish-modpack.sh 1.35.0
#
# Prérequis : ./scripts/assemble-graphics-modpack.sh VERSION déjà exécuté.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="Vans74/GTRP-Launcher"
TAG="modpack"
WORK="$ROOT/modpack-work"

if [[ -z "${GTRP_GH_TOKEN:-}" ]]; then
  echo "ERREUR: definis la variable GTRP_GH_TOKEN avec ton token GitHub." >&2
  exit 1
fi

VERSION="${1:-}"
[[ -n "$VERSION" ]] || { echo "Usage: $0 <version>" >&2; exit 1; }

ZIP="$WORK/gtrp-modpack-$VERSION.zip"
MANIFEST="$WORK/manifest.json"
SIGNING_KEY="${GTRP_MANIFEST_SIGNING_KEY:-/home/afters-projects/.config/gtrp-launcher-signing/manifest-ed25519-private.pem}"
[[ -f "$ZIP" ]] || { echo "ERREUR: $ZIP introuvable — lance assemble-graphics-modpack.sh $VERSION" >&2; exit 1; }
[[ -f "$MANIFEST" ]] || { echo "ERREUR: $MANIFEST introuvable" >&2; exit 1; }

python3 "$ROOT/scripts/sign-manifest.py" "$MANIFEST" --key "$SIGNING_KEY"

api() { curl -s -H "Authorization: token $GTRP_GH_TOKEN" "$@"; }

echo "=== Release '$TAG' ==="
REL_ID=$(api "https://api.github.com/repos/$REPO/releases/tags/$TAG" | python3 -c "import sys,json;print(json.load(sys.stdin)['id'])")
[[ -n "$REL_ID" ]] || { echo "ERREUR: release '$TAG' introuvable" >&2; exit 1; }
echo "release id = $REL_ID"

ASSETS_JSON=$(api "https://api.github.com/repos/$REPO/releases/$REL_ID/assets?per_page=100")

upload_if_missing() {
  local file="$1" name="$2"
  if echo "$ASSETS_JSON" | grep -qF "\"$name\""; then
    echo "  = $name deja present"
    return 0
  fi
  echo "  + upload $name ($(du -h "$file" | awk '{print $1}'))"
  curl -s -X POST -H "Authorization: token $GTRP_GH_TOKEN" -H "Content-Type: application/octet-stream" \
    --data-binary @"$file" \
    "https://uploads.github.com/repos/$REPO/releases/$REL_ID/assets?name=$name" \
    | python3 -c "import sys,json;d=json.load(sys.stdin);print('    ->',d.get('name'),d.get('state'),d.get('size'))"
}

echo "=== Upload bundle + assets config ==="
upload_if_missing "$ZIP" "gtrp-modpack-$VERSION.zip"
while IFS= read -r asset; do
  [[ -n "$asset" ]] || continue
  [[ -f "$WORK/$asset" ]] || { echo "ERREUR: asset config $asset introuvable" >&2; exit 1; }
  upload_if_missing "$WORK/$asset" "$asset"
done < <(python3 -c "import json,urllib.parse; m=json.load(open('$MANIFEST')); print('\\n'.join(urllib.parse.urlparse(f['url']).path.rsplit('/',1)[-1] for f in m.get('files',[])))")

echo "=== Remplacement manifest.json ==="
OLD_MID=$(echo "$ASSETS_JSON" | python3 -c "import sys,json;print(next((a['id'] for a in json.load(sys.stdin) if a['name']=='manifest.json'),''))")
[[ -n "$OLD_MID" ]] && api -X DELETE "https://api.github.com/repos/$REPO/releases/assets/$OLD_MID" >/dev/null
curl -s -X POST -H "Authorization: token $GTRP_GH_TOKEN" -H "Content-Type: application/json" \
  --data-binary @"$MANIFEST" \
  "https://uploads.github.com/repos/$REPO/releases/$REL_ID/assets?name=manifest.json" \
  | python3 -c "import sys,json;d=json.load(sys.stdin);print('  manifest ->',d.get('name'),d.get('state'))"

echo ""
python3 -c "import json;m=json.load(open('$MANIFEST'));print('=== LIVE : version',m['version'],'| bundle_required',m.get('bundle_required'),'| size',m['bundle']['size'])"
echo "=== TERMINÉ ==="
