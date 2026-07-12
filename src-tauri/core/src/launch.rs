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
pub fn launch(install: &GameInstall, nickname: &str, host: &str, port: u16) -> Result<()> {
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

    // 1bis) Correctif de compatibilité ENBSeries sur Windows 8/10/11.
    // ENBSeries (0.2xx/0.3xx) plante au démarrage sur Windows récent à cause de
    // clés de registre parasites (mode de compatibilité forcé + entrées
    // MostRecentApplication de DirectDraw/Direct3D dans le VirtualStore). On les
    // purge avant chaque lancement — c'est le même correctif que les scripts
    // « Fix Problems Win 10 » livrés avec les packs ENB, mais automatique.
    apply_enb_compat_fix(&install.gta_exe);

    // 1ter) Patch « 4 Go » (Large Address Aware) sur gta_sa.exe.
    // Le jeu 32 bits est bridé à ~2 Go ; avec le pack graphique + le streaming
    // d'objets textés du serveur (SetObjectMaterialText → polices GDI), les
    // allocations finissent par échouer (« Can't create font … ») puis le jeu
    // plante. Poser le drapeau LAA porte la limite à ~4 Go. Best-effort :
    // idempotent, avec sauvegarde ; un échec (exe verrouillé / droits) ne doit
    // jamais empêcher de jouer.
    let _ = crate::laa::set_large_address_aware(Path::new(&install.gta_exe));

    // 2) Lancement de samp.exe avec l'adresse du serveur.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let work_dir = Path::new(samp_exe).parent();
    let mut cmd = Command::new(samp_exe);
    cmd.arg(format!("{host}:{port}"));
    if let Some(dir) = work_dir {
        cmd.current_dir(dir);
    }
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn()
        .map_err(|e| LauncherError::Io(format!("impossible de lancer samp.exe : {e}")))?;
    Ok(())
}

/// Supprime les clés de registre qui font planter ENBSeries sur Windows 8/10/11.
///
/// Best-effort : toute erreur est ignorée (une clé absente n'est pas un problème,
/// et la clé HKLM peut nécessiter l'élévation). Aucune de ces clés n'est critique
/// pour Windows : elles sont régénérées automatiquement au besoin.
#[cfg(windows)]
fn apply_enb_compat_fix(gta_exe: &str) {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE};
    use winreg::RegKey;

    // Sous-clés supprimées entièrement (sans risque, régénérées par Windows) :
    // entrées « MostRecentApplication » de DirectDraw/Direct3D dans le VirtualStore
    // et cache audio, connus pour corrompre l'initialisation d'ENBSeries.
    const DOOMED_HKCU: [&str; 3] = [
        "Software\\Classes\\VirtualStore\\MACHINE\\SOFTWARE\\Wow6432Node\\Microsoft\\Direct3D\\MostRecentApplication",
        "Software\\Classes\\VirtualStore\\MACHINE\\SOFTWARE\\Wow6432Node\\Microsoft\\DirectDraw\\MostRecentApplication",
        "Software\\Microsoft\\Internet Explorer\\LowRegistry\\Audio\\PolicyConfig\\PropertyStore",
    ];
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for key in DOOMED_HKCU {
        let _ = hkcu.delete_subkey_all(key);
    }

    // AppCompatFlags\Layers : on retire uniquement les entrées du jeu (un mode de
    // compatibilité forcé sur gta_sa.exe/samp.exe est la cause classique du crash).
    // HKCU sans élévation ; HKLM en best-effort (échoue sans droits admin).
    const LAYERS: &str =
        "Software\\Microsoft\\Windows NT\\CurrentVersion\\AppCompatFlags\\Layers";
    let exe_lower = gta_exe.to_lowercase();
    for root in [
        RegKey::predef(HKEY_CURRENT_USER),
        RegKey::predef(HKEY_LOCAL_MACHINE),
    ] {
        let Ok(key) = root.open_subkey_with_flags(LAYERS, KEY_READ | KEY_SET_VALUE) else {
            continue;
        };
        let names: Vec<String> = key
            .enum_values()
            .filter_map(|r| r.ok())
            .map(|(name, _)| name)
            .collect();
        for name in names {
            let n = name.to_lowercase();
            if n == exe_lower || n.ends_with("gta_sa.exe") || n.ends_with("samp.exe") {
                let _ = key.delete_value(&name);
            }
        }
    }
}

#[cfg(not(windows))]
pub fn launch(_install: &GameInstall, _nickname: &str, _host: &str, _port: u16) -> Result<()> {
    Err(LauncherError::Other(
        "Le lancement du jeu n'est disponible que sous Windows.".into(),
    ))
}
