//! Gestion des graphismes améliorés GTRP (ReShade + mods via modloader/ASI).
//!
//! Les fichiers sont stockés dans `{gta_root}/gtrp-assets/enb/` (déployés via le
//! modpack du launcher). Avant chaque lancement :
//!   - graphismes activés  → copie récursive vers la racine du jeu ;
//!   - graphismes désactivés → retrait des fichiers précédemment déployés.
//!
//! Cas particulier de l'ASI loader (Ultimate ASI Loader) : sur GTA SA / SA-MP
//! 0.3.DL, les `.asi` (modloader, radar, skygrad, Real Skybox…) ne se chargent de
//! façon fiable que via un proxy `vorbisFile.dll`, PAS via `dinput8.dll`. Le
//! modpack livre donc le loader sous un nom neutre (`vorbisFileLoader.dll`) et le
//! launcher l'installe lui-même en `vorbisFile.dll`, en sauvegardant le
//! `vorbisFile.dll` d'origine du jeu en `vorbisFileHooked.dll` (vers lequel le
//! loader relaie les appels audio OGG). Ainsi chaque joueur obtient les ASI sans
//! aucune manipulation manuelle. Le nom neutre évite qu'un ancien launcher (sans
//! cette logique) n'écrase le `vorbisFile.dll` d'origine lors d'une simple copie.

use crate::error::{LauncherError, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Dossier relatif (dans le jeu) où le modpack dépose l'ENB en attente.
pub const ENB_STAGING_REL: &str = "gtrp-assets/enb";

/// Fichier témoin : indique que l'ENB GTRP est actuellement actif dans le dossier du jeu.
pub const ENB_MARKER: &str = ".gtrp_enb_active";

/// Nom neutre sous lequel le modpack livre l'Ultimate ASI Loader (non chargé
/// automatiquement par le jeu ; c'est le launcher qui l'installe).
pub const LOADER_SRC_NAME: &str = "vorbisFileLoader.dll";

/// Nom sous lequel le loader doit être installé pour charger les ASI de façon
/// fiable sur GTA SA / SA-MP 0.3.DL.
pub const LOADER_TARGET_NAME: &str = "vorbisFile.dll";

/// Sauvegarde du `vorbisFile.dll` d'origine du jeu (le loader y relaie l'audio OGG).
pub const LOADER_BACKUP_NAME: &str = "vorbisFileHooked.dll";

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

    // Installe l'ASI loader en vorbisFile.dll (avec sauvegarde de l'original)
    // pour que modloader et les autres .asi se chargent chez tous les joueurs.
    install_asi_loader(gta_root, &staging)?;

    fs::write(gta_root.join(ENB_MARKER), b"1")?;

    Ok(EnbPrepareResult {
        applied: true,
        message: "Graphismes améliorés activés.".into(),
    })
}

/// Compare deux fichiers octet par octet (taille puis contenu).
fn files_equal(a: &Path, b: &Path) -> bool {
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(ma), Ok(mb)) if ma.len() == mb.len() => match (fs::read(a), fs::read(b)) {
            (Ok(da), Ok(db)) => da == db,
            _ => false,
        },
        _ => false,
    }
}

/// Installe l'Ultimate ASI Loader en `vorbisFile.dll`, en préservant le
/// `vorbisFile.dll` d'origine du jeu sous `vorbisFileHooked.dll`.
fn install_asi_loader(gta_root: &Path, staging: &Path) -> Result<()> {
    let src = staging.join(LOADER_SRC_NAME);
    if !src.is_file() {
        // Le modpack ne fournit pas de loader : rien à faire.
        return Ok(());
    }

    let target = gta_root.join(LOADER_TARGET_NAME);
    let backup = gta_root.join(LOADER_BACKUP_NAME);

    // Sauvegarde unique du vorbisFile.dll d'origine (jamais écraser une sauvegarde
    // existante, et ne pas sauvegarder notre propre loader).
    if !backup.exists() && target.is_file() && !files_equal(&target, &src) {
        fs::rename(&target, &backup)?;
    }

    fs::copy(&src, &target)?;
    Ok(())
}

/// Retire notre ASI loader et restaure le `vorbisFile.dll` d'origine du jeu.
fn uninstall_asi_loader(gta_root: &Path, staging: &Path) {
    let src = staging.join(LOADER_SRC_NAME);
    let target = gta_root.join(LOADER_TARGET_NAME);
    let backup = gta_root.join(LOADER_BACKUP_NAME);

    // Ne supprime vorbisFile.dll que si c'est bien notre loader (ou si une
    // sauvegarde de l'original existe, prête à être restaurée).
    if target.is_file() {
        let is_ours = src.is_file() && files_equal(&target, &src);
        if is_ours || backup.is_file() {
            let _ = fs::remove_file(&target);
        }
    }

    // Restaure l'original s'il avait été sauvegardé.
    if backup.is_file() && !target.exists() {
        let _ = fs::rename(&backup, &target);
    }
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
        // Restaure le vorbisFile.dll d'origine (retire notre loader).
        uninstall_asi_loader(gta_root, &staging);
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

    #[test]
    fn asi_loader_backup_and_restore_roundtrip() {
        let game = tmp("loader");
        let staging = staging_dir(&game);
        fs::create_dir_all(&staging).unwrap();
        // Loader livré par le modpack (nom neutre) + un fichier ENB quelconque.
        fs::write(staging.join(LOADER_SRC_NAME), b"ULTIMATE-ASI-LOADER").unwrap();
        fs::write(staging.join("d3d9.dll"), b"reshade").unwrap();
        // vorbisFile.dll d'origine du jeu (contenu différent du loader).
        fs::write(game.join(LOADER_TARGET_NAME), b"ORIGINAL-VORBIS-AUDIO").unwrap();

        // Activation : l'original est sauvegardé, le loader prend sa place.
        let r = prepare(&game, true).unwrap();
        assert!(r.applied);
        assert!(game.join(LOADER_BACKUP_NAME).is_file());
        assert_eq!(
            fs::read(game.join(LOADER_TARGET_NAME)).unwrap(),
            b"ULTIMATE-ASI-LOADER"
        );
        assert_eq!(
            fs::read(game.join(LOADER_BACKUP_NAME)).unwrap(),
            b"ORIGINAL-VORBIS-AUDIO"
        );

        // Ré-activation : ne doit PAS sauvegarder le loader par-dessus la sauvegarde.
        let r2 = prepare(&game, true).unwrap();
        assert!(r2.applied);
        assert_eq!(
            fs::read(game.join(LOADER_BACKUP_NAME)).unwrap(),
            b"ORIGINAL-VORBIS-AUDIO"
        );

        // Désactivation : l'original est restauré, le loader retiré.
        let r3 = prepare(&game, false).unwrap();
        assert!(!r3.applied);
        assert!(!game.join(LOADER_BACKUP_NAME).exists());
        assert_eq!(
            fs::read(game.join(LOADER_TARGET_NAME)).unwrap(),
            b"ORIGINAL-VORBIS-AUDIO"
        );

        let _ = fs::remove_dir_all(&game);
    }
}
