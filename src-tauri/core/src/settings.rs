//! Persistance des réglages joueur (pseudo, chemin du jeu) dans un fichier JSON
//! situé dans le dossier de configuration de l'application.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    /// Pseudo utilisé pour se connecter en jeu.
    #[serde(default)]
    pub nickname: String,
    /// Chemin absolu vers gta_sa.exe (Windows).
    #[serde(default)]
    pub gta_path: Option<String>,
    /// Chemin absolu vers samp.exe (déduit si absent).
    #[serde(default)]
    pub samp_path: Option<String>,
}

fn settings_file(config_dir: &Path) -> PathBuf {
    config_dir.join("settings.json")
}

/// Charge les réglages ; renvoie des réglages par défaut si le fichier n'existe pas
/// ou est illisible (jamais d'erreur bloquante).
pub fn load(config_dir: &Path) -> Settings {
    let path = settings_file(config_dir);
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

/// Sauvegarde les réglages (crée le dossier si nécessaire).
pub fn save(config_dir: &Path, settings: &Settings) -> Result<()> {
    std::fs::create_dir_all(config_dir)?;
    let path = settings_file(config_dir);
    let json = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Validation d'un pseudo SA-MP : 3-24 caractères, alphanumérique + [];()_$=@.
pub fn is_valid_nickname(nick: &str) -> bool {
    let len = nick.chars().count();
    if !(3..=24).contains(&len) {
        return false;
    }
    nick.chars().all(|c| {
        c.is_ascii_alphanumeric() || "[]();$=@._-".contains(c)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nickname_validation() {
        assert!(is_valid_nickname("Evan_Dupont"));
        assert!(is_valid_nickname("John[GTRP]"));
        assert!(!is_valid_nickname("ab")); // trop court
        assert!(!is_valid_nickname("nom avec espaces"));
        assert!(!is_valid_nickname("émoji_é")); // non-ascii
        assert!(!is_valid_nickname(&"x".repeat(25))); // trop long
    }

    #[test]
    fn load_missing_returns_default() {
        let dir = std::env::temp_dir().join("gtrp_test_missing_dir_xyz");
        let s = load(&dir);
        assert_eq!(s.nickname, "");
        assert!(s.gta_path.is_none());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("gtrp_test_{}", std::process::id()));
        let mut s = Settings::default();
        s.nickname = "Tester".into();
        s.gta_path = Some("C:/Games/gta_sa.exe".into());
        save(&dir, &s).unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.nickname, "Tester");
        assert_eq!(loaded.gta_path.as_deref(), Some("C:/Games/gta_sa.exe"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
