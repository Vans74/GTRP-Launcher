//! Détection de l'installation GTA San Andreas + SA-MP.
//!
//! Sur Windows, on interroge d'abord la clé de registre de SA-MP
//! (`HKCU\Software\SAMP\gta_sa_exe`), puis on tente quelques emplacements
//! d'installation courants. Le code est compilé de façon inerte hors Windows
//! afin que le projet reste testable sur Linux/CI.

use crate::error::{LauncherError, Result};
use serde::Serialize;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct GameInstall {
    /// Chemin vers gta_sa.exe.
    pub gta_exe: String,
    /// Chemin vers samp.exe (dans le même dossier).
    pub samp_exe: Option<String>,
    /// Dossier racine du jeu.
    pub root: String,
}

/// Construit un `GameInstall` à partir d'un chemin gta_sa.exe s'il est valide.
pub fn from_gta_exe(gta_exe: &Path) -> Result<GameInstall> {
    if !gta_exe.is_file() {
        return Err(LauncherError::GameNotFound(format!(
            "{} n'existe pas",
            gta_exe.display()
        )));
    }
    let name = gta_exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name != "gta_sa.exe" {
        return Err(LauncherError::GameNotFound(
            "le fichier sélectionné n'est pas gta_sa.exe".into(),
        ));
    }
    let root = gta_exe
        .parent()
        .ok_or_else(|| LauncherError::GameNotFound("dossier parent introuvable".into()))?;
    let samp = root.join("samp.exe");
    Ok(GameInstall {
        gta_exe: gta_exe.to_string_lossy().to_string(),
        samp_exe: if samp.is_file() {
            Some(samp.to_string_lossy().to_string())
        } else {
            None
        },
        root: root.to_string_lossy().to_string(),
    })
}

/// Tente une détection automatique du jeu.
#[cfg(windows)]
pub fn detect() -> Option<GameInstall> {
    if let Some(path) = registry_gta_path() {
        let p = PathBuf::from(&path);
        if let Ok(install) = from_gta_exe(&p) {
            return Some(install);
        }
    }
    for candidate in common_paths() {
        if let Ok(install) = from_gta_exe(&candidate) {
            return Some(install);
        }
    }
    None
}

#[cfg(not(windows))]
pub fn detect() -> Option<GameInstall> {
    // La détection par registre n'existe que sous Windows.
    None
}

#[cfg(windows)]
fn registry_gta_path() -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let samp = hkcu.open_subkey("Software\\SAMP").ok()?;
    samp.get_value::<String, _>("gta_sa_exe").ok()
}

#[cfg(windows)]
fn common_paths() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let candidates = [
        "C:\\Program Files (x86)\\Rockstar Games\\GTA San Andreas\\gta_sa.exe",
        "C:\\Program Files\\Rockstar Games\\GTA San Andreas\\gta_sa.exe",
        "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Grand Theft Auto San Andreas\\gta_sa.exe",
        "C:\\Games\\GTA San Andreas\\gta_sa.exe",
    ];
    for c in candidates {
        v.push(PathBuf::from(c));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn rejects_non_existing() {
        let p = PathBuf::from("/definitely/not/here/gta_sa.exe");
        assert!(from_gta_exe(&p).is_err());
    }

    #[test]
    fn rejects_wrong_name() {
        // Fichier existant mais mauvais nom.
        let dir = std::env::temp_dir();
        let file = dir.join("notgta.exe");
        std::fs::write(&file, b"x").unwrap();
        assert!(from_gta_exe(&file).is_err());
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn accepts_valid_gta_exe() {
        let dir = std::env::temp_dir().join(format!("gtrp_gta_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let gta = dir.join("gta_sa.exe");
        std::fs::write(&gta, b"x").unwrap();
        let install = from_gta_exe(&gta).unwrap();
        assert!(install.gta_exe.ends_with("gta_sa.exe"));
        assert!(install.samp_exe.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
