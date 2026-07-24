//! Lancement du jeu vers le serveur GTRP.
//!
//! Méthode standard et éprouvée côté SA-MP :
//!   1. Écrire le pseudo et le chemin de gta_sa.exe dans `HKCU\Software\SAMP`.
//!   2. Démarrer `samp.exe` avec l'adresse `IP:PORT` en argument.
//!
//! Cette approche évite l'injection manuelle de DLL (fragile) et s'appuie sur
//! le launcher officiel SA-MP déjà installé chez le joueur.
//!
//! NB : cette partie est spécifique à Windows ; hors Windows elle renvoie une
//! erreur explicite (le jeu ne tourne pas nativement sous Linux).

use crate::error::{LauncherError, Result};
use crate::gta::GameInstall;

/// Prépare le registre puis lance le jeu.
#[cfg(windows)]
pub fn launch(install: &GameInstall, nickname: &str, host: &str, port: u16) -> Result<u32> {
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::Command;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_WRITE};
    use winreg::RegKey;

    let samp_exe = install.samp_exe.as_ref().ok_or_else(|| {
        LauncherError::GameNotFound(
            "samp.exe introuvable dans le dossier du jeu. SA-MP est-il installé ?".into(),
        )
    })?;

    // 1) Écriture des clés de registre attendues par SA-MP.
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (samp_key, _) = hkcu
        .create_subkey_with_flags("Software\\SAMP", KEY_WRITE)
        .map_err(|e| LauncherError::Config(format!("registre SAMP : {e}")))?;
    samp_key
        .set_value("PlayerName", &nickname)
        .map_err(|e| LauncherError::Config(format!("écriture PlayerName : {e}")))?;
    samp_key
        .set_value("gta_sa_exe", &install.gta_exe)
        .map_err(|e| LauncherError::Config(format!("écriture gta_sa_exe : {e}")))?;

    // 2) Lancement de samp.exe avec l'adresse du serveur.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let work_dir = Path::new(samp_exe).parent();
    let mut cmd = Command::new(samp_exe);
    cmd.arg(format!("{host}:{port}"));
    if let Some(dir) = work_dir {
        cmd.current_dir(dir);
    }
    cmd.creation_flags(CREATE_NO_WINDOW);
    let child = cmd
        .spawn()
        .map_err(|e| LauncherError::Io(format!("impossible de lancer samp.exe : {e}")))?;
    Ok(child.id())
}

#[cfg(not(windows))]
pub fn launch(_install: &GameInstall, _nickname: &str, _host: &str, _port: u16) -> Result<u32> {
    Err(LauncherError::Other(
        "Le lancement du jeu n'est disponible que sous Windows.".into(),
    ))
}
