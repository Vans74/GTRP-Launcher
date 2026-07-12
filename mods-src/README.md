# Mods sources — dépôt des archives MixMods

Dépose ici les **7 fichiers `.7z`** téléchargés depuis MixMods (via sharemods.com dans ton navigateur).

## Fichiers attendus

| Fichier | Mod | Statut |
|---------|-----|--------|
| `Proper_Vegetation_Retex.7z` | Textures HD végétation | **À déposer** |
| `Improved_and_Fixed_Original_Vegetation.7z` | Modèles d'arbres arrondis | **À déposer** |
| `LOD_Vegetation.7z` | Arbres distants (LOD) | **À déposer** |
| `Real_Skybox.7z` | Ciel / nuages réalistes (.asi) | **À déposer** |
| `Real_Linear_Graphics.7z` | Couleurs réalistes (timecyc.dat) | **À déposer** |
| `Sky_Gradient_Fix.7z` | Correction dégradé du ciel (.asi) | ✅ Récupéré (Dropbox) |
| `Effects_Mod_-_by_Ezekiel_-_Junior_Djjr_-_Effects_Loader.7z` | Effets HD (version Effects Loader) | ✅ Récupéré (Google Drive) |

## Liens de téléchargement

1. [Proper Vegetation Retex](https://sharemods.com/wddepz99ep69/Proper_Vegetation_Retex.7z.html)
2. [Improved and Fixed Original Vegetation](https://sharemods.com/5ittq3i4hegc/Improved_and_Fixed_Original_Vegetation.7z.html)
3. [LOD Vegetation](https://sharemods.com/7u9q36epczhn/LOD_Vegetation.7z.html)
4. [Real Skybox](https://sharemods.com/asxvzdjkpa6m/Real_Skybox.7z.html)
5. [SkyGrad](https://sharemods.com/mo5cip3dlyp6/Sky_Gradient_Fix.7z.html)
6. [Real Linear Graphics](https://sharemods.com/dhjea67wrhm1/Real_Linear_Graphics.7z.html)
7. Effects Mod — déjà copié (Google Drive)

## Après dépôt

Préviens-moi quand les **6 archives manquantes** sont dans ce dossier. Le script
`scripts/assemble-graphics-modpack.sh` assemble alors le modpack :

1. repart de la base persistée `modpack-work/graphics-base/` (ReShade + Project2DFX
   + loader `dinput8.dll`) — plus aucune dépendance à `/tmp` ;
2. installe `modloader.asi` et fusionne les sous-arbres `modloader/` de chaque mod ;
3. place les plugins `.asi` (Real Skybox, SkyGrad) à la racine du jeu ;
4. produit un **rapport de contrôle** (`modpack-work/build-report.txt`) listant les
   fichiers placés et les conflits potentiels (doublons `d3d9.dll`, `timecyc.dat`, etc.).

Je relis le rapport **avant** de publier (le script ne publie rien tout seul).

**Note :** le modpack final fera ~180–220 Mo. Les joueurs le téléchargeront
automatiquement via le launcher.
