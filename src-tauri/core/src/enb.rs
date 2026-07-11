//! Gestion de l'ENB GTRP (SA_DirectX 2.0 par XMakarusX, base ENBSeries 0.313).
//!
//! Les fichiers ENB sont stockés dans `{gta_root}/gtrp-assets/enb/` (déployés via le
//! modpack du launcher). Avant chaque lancement :
//!   - graphismes activés  → copie récursive vers la racine du jeu ;
//!   - graphismes désactivés → retrait des fichiers précédemment déployés.

use crate::error::{LauncherError, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Dossier relatif (dans le jeu) où le modpack dépose l'ENB en attente.
pub const ENB_STAGING_REL: &str = "gtrp-assets/enb";

/// Fichier témoin : indique que l'ENB GTRP est actuellement actif dans le dossier du jeu.
pub const ENB_MARKER: &str = ".gtrp_enb_active";

#[derive(Debug, Clone, Serialize)]
pub struct EnbPrepareResult {
    pub applied: bool,
    pub message: String,
}

pub fn staging_dir(gta_root: &Path) -> PathBuf {
    gta_root.join(ENB_STAGING_REL)
}

/// Vérifie si le pack ENB a été déployé par le modpack.
pub fn is_pack_installed(gta_root: &Path) -> bool {
    let staging = staging_dir(gta_root);
    if !staging.is_dir() {
        return false;
    }
    fs::read_dir(&staging)
        .map(|mut entries| entries.any(|e| e.is_ok()))
        .unwrap_or(false)
}

/// Active ou désactive l'ENB avant le lancement du jeu.
pub fn prepare(gta_root: &Path, enabled: bool) -> Result<EnbPrepareResult> {
    if enabled {
        activate(gta_root)
    } else {
        deactivate(gta_root)?;
        Ok(EnbPrepareResult {
            applied: false,
            message: "Graphismes améliorés désactivés.".into(),
        })
    }
}

fn activate(gta_root: &Path) -> Result<EnbPrepareResult> {
    let staging = staging_dir(gta_root);
    if !is_pack_installed(gta_root) {
        return Ok(EnbPrepareResult {
            applied: false,
            message: "Pack ENB introuvable — lance d'abord une mise à jour du modpack.".into(),
        });
    }

    // Nettoie un déploiement précédent avant de recopier.
    let _ = deactivate(gta_root);

    copy_dir_recursive(&staging, gta_root, &staging)?;
    fs::write(gta_root.join(ENB_MARKER), b"1")?;

    Ok(EnbPrepareResult {
        applied: true,
        message: "Graphismes améliorés (ENB) activés.".into(),
    })
}

/// Retire proprement un déploiement ENB précédent du dossier du jeu.
/// Utilisé par l'updater avant d'installer un nouveau modpack, pour éviter
/// que d'anciens fichiers (ex. ancien pack ENB) ne subsistent dans le jeu.
pub fn undeploy(gta_root: &Path) -> Result<()> {
    deactivate(gta_root)
}

fn deactivate(gta_root: &Path) -> Result<()> {
    let marker = gta_root.join(ENB_MARKER);
    if !marker.is_file() {
        return Ok(());
    }

    let staging = staging_dir(gta_root);
    if staging.is_dir() {
        remove_staged_files(&staging, gta_root, &staging)?;
    }

    let _ = fs::remove_file(marker);
    Ok(())
}

/// Copie récursivement `src` vers `dst_root`, en conservant les chemins relatifs à `src`.
fn copy_dir_recursive(src: &Path, dst_root: &Path, src_base: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path
            .strip_prefix(src_base)
            .map_err(|_| LauncherError::Io("chemin ENB invalide".into()))?;
        let dest = dst_root.join(rel);

        if path.is_dir() {
            fs::create_dir_all(&dest)?;
            copy_dir_recursive(&path, dst_root, src_base)?;
        } else if path.is_file() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

/// Supprime uniquement les fichiers/dossiers correspondant au contenu du staging ENB.
fn remove_staged_files(staging: &Path, game_root: &Path, staging_base: &Path) -> Result<()> {
    for entry in fs::read_dir(staging)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path
            .strip_prefix(staging_base)
            .map_err(|_| LauncherError::Io("chemin ENB invalide".into()))?;
        let target = game_root.join(rel);

        if path.is_dir() {
            remove_staged_files(&path, game_root, staging_base)?;
            // Supprime le dossier s'il est vide ou ne contient plus que des fichiers ENB.
            if target.is_dir() {
                let _ = fs::remove_dir(&target);
            }
        } else if path.is_file() && target.is_file() {
            let _ = fs::remove_file(&target);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("gtrp_enb_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn activate_and_deactivate_roundtrip() {
        let game = tmp("roundtrip");
        let staging = staging_dir(&game);
        fs::create_dir_all(staging.join("enbseries")).unwrap();
        fs::write(staging.join("d3d9.dll"), b"fake-enb").unwrap();
        fs::write(staging.join("enbseries/test.fx"), b"shader").unwrap();

        let r = prepare(&game, true).unwrap();
        assert!(r.applied);
        assert!(game.join("d3d9.dll").is_file());
        assert!(game.join("enbseries/test.fx").is_file());
        assert!(game.join(ENB_MARKER).is_file());

        let r2 = prepare(&game, false).unwrap();
        assert!(!r2.applied);
        assert!(!game.join("d3d9.dll").exists());
        assert!(!game.join(ENB_MARKER).exists());
        // Le staging reste intact.
        assert!(staging.join("d3d9.dll").is_file());

        let _ = fs::remove_dir_all(&game);
    }

    #[test]
    fn activate_without_pack_is_non_fatal() {
        let game = tmp("nopack");
        let r = prepare(&game, true).unwrap();
        assert!(!r.applied);
        let _ = fs::remove_dir_all(&game);
    }
}
