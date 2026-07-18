# GTRP Launcher

Launcher officiel du serveur **Grand Theft RolePlay** (SA-MP 0.3.7).
Application de bureau **Windows** construite avec **Tauri** (noyau Rust + interface web),
légère (~5–10 Mo) et aux couleurs du serveur.

## Fonctionnalités

- **Jouer en 1 clic** vers le serveur (pseudo mémorisé, aucune manip côté joueur).
- **Statut serveur en direct** (en ligne/hors ligne, joueurs, ping) via le protocole query SA-MP.
- **Auto-updater du modpack** : téléchargement différentiel basé sur un `manifest.json`
  distant, avec vérification d'intégrité **SHA-256** et installation atomique.
- **Graphismes HD optionnels** : le bouton, activé par défaut, ne contrôle que
  ENBSeries 0.430 et les retouches de décor. Les véhicules, skins, armes, sons,
  radar, interface et leur ASI loader restent toujours actifs. Le pack ENB
  source est téléchargé directement, vérifié par SHA-256 puis extrait sur liste
  blanche ; seuls les réglages neutres GTRP sont livrés avec le modpack.
- **Préchargement artwork SA-MP** : synchronisation des DFF/TXD déclarés par le
  serveur dans le cache 0.3.DL (CRC + SHA-256, bundle initial puis différentiel).
- **Vérification d'intégrité / anti-triche léger** : contrôle des fichiers attendus
  et détection de fichiers interdits (`*.asi`, `cleo/*`, …).
- **Actualités / changelog** intégrés (`news.json` distant) et liens Discord / site.
- **Auto-update du launcher** : à chaque ouverture, le launcher vérifie une nouvelle version,
  télécharge le patch signé, s'installe et redémarre — **sans réinstallation manuelle**.

## Auto-update du launcher (pour les joueurs)

À partir de la **v0.1.2**, le launcher se met à jour tout seul :

1. Au démarrage, il interroge `latest.json` sur GitHub Releases.
2. Si une version plus récente existe, il télécharge le patch (barre de progression).
3. Il installe en mode silencieux et redémarre automatiquement.

**Important :** les joueurs sur v0.1.0 ou v0.1.1 doivent installer **une dernière fois** la v0.1.2.
Ensuite, toutes les futures mises à jour sont automatiques.

Pour publier une nouvelle version côté staff :

```bash
# 1. Bumper la version dans package.json, src-tauri/Cargo.toml, tauri.conf.json
# 2. Commit + tag
git tag v0.1.3
git push origin main --tags
# 3. La CI build, signe et publie automatiquement la GitHub Release avec latest.json
```

## Architecture

```
launcher/
├─ index.html, src/            # Interface (HTML/CSS/JS, Vite)
├─ public/assets/              # Logo + bannière (DA GTRP)
├─ src-tauri/
│  ├─ src/lib.rs               # Couche Tauri : commandes exposées au frontend
│  └─ core/                    # Crate "gtrp-core" : logique métier (SANS GUI, testable partout)
│     └─ src/{query,updater,enb,samp_cache,gta,launch,settings,news,config,error}.rs
├─ assets/                     # Preset HD GTRP + descripteurs de déploiement
├─ tools/gen-manifest.mjs      # Générateur de manifest du modpack
└─ .github/workflows/build.yml # CI : build de l'installeur Windows
```

La logique sensible (query, updater, hachage, détection du jeu) vit dans `gtrp-core`,
**sans dépendance graphique**, ce qui la rend testable sur n'importe quelle plateforme
(`cargo test` dans `src-tauri/core`).

## Configuration

Tout est centralisé dans `src-tauri/core/src/config.rs` :

| Constante        | Rôle                                             |
| ---------------- | ------------------------------------------------ |
| `SERVER_HOST`    | Domaine/IP public du serveur (défaut `gtrp.fr`)  |
| `SERVER_PORT`    | Port SA-MP (défaut `3400`)                       |
| `ASSET_BASE_URL` | URL de base : `{ASSET_BASE_URL}/manifest.json` et `/news.json` |
| `DISCORD_URL`    | Invitation Discord                               |
| `WEB_URL`        | Site web                                         |

## Modpack : héberger et publier une mise à jour

1. Placer les fichiers du modpack dans un dossier (arborescence **relative au dossier du jeu**).
2. Générer le manifest :
   ```bash
   node tools/gen-manifest.mjs ./modpack https://gtrp.fr/launcher/files 1.4.0 > manifest.json
   ```
3. Mettre en ligne, sous `ASSET_BASE_URL` :
   - `manifest.json`
   - `news.json` (optionnel)
   - le dossier `files/` contenant les fichiers du modpack
4. Au prochain lancement, les joueurs reçoivent automatiquement les fichiers modifiés.

### Format `news.json`

```json
{
  "items": [
    { "title": "Mise à jour 1.4", "date": "2026-07-11", "tag": "Update", "body": "..." }
  ]
}
```

## Développement

```bash
cd launcher
npm install
npm run dev          # interface seule (aperçu navigateur, backend simulé)
npm run tauri dev    # application complète (nécessite les libs système Windows/Linux)
```

Tests de la logique métier :

```bash
cd src-tauri/core && cargo test
```

## Produire l'installeur Windows (sans PC Windows)

Le build se fait via **GitHub Actions** :

1. Pousser ce dossier sur un dépôt **GitHub**.
2. Onglet **Actions** → workflow *Build GTRP Launcher* → **Run workflow**
   (ou pousser un tag `vX.Y.Z`).
3. Récupérer l'installeur dans les **Artifacts** (`gtrp-launcher-windows`).

## Notes techniques

- Le lancement du jeu écrit `PlayerName` et `gta_sa_exe` dans `HKCU\Software\SAMP`
  puis démarre `samp.exe <host>:<port>` (méthode standard SA-MP, sans injection de DLL).
- Le cache artwork est écrit dans le dossier Documents résolu par Windows, y compris
  lorsque celui-ci est redirigé vers OneDrive. Le téléchargement natif SA-MP reste
  disponible si le catalogue HTTPS est momentanément inaccessible.
- Le décodage des chaînes du serveur est tolérant (Latin-1) pour éviter tout souci d'accents.
- Les téléchargements sont vérifiés par SHA-256 ; un chemin de manifest malveillant
  (`..`, chemin absolu) est refusé.
