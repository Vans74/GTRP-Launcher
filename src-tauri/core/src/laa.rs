//! Patch « Large Address Aware » (4 Go) sur gta_sa.exe.
//!
//! gta_sa.exe est un exécutable 32 bits qui, par défaut, ne peut adresser que
//! ~2 Go de mémoire. Avec un pack graphique lourd (ReShade / SkyGFX /
//! ImVehFt + textures HD) ET le streaming d'objets du serveur — notamment
//! `SetObjectMaterialText`, qui fait créer au jeu des polices GDI (Arial,
//! Trebuchet MS, Webdings…) —, on s'approche de cette limite. Les allocations
//! finissent alors par échouer : le jeu logue « SignText: Can't create font … »
//! puis se termine sur un « Microsoft Visual C++ Runtime Error ».
//!
//! Activer le drapeau `IMAGE_FILE_LARGE_ADDRESS_AWARE` de l'en-tête PE porte la
//! limite à ~4 Go (sur Windows 64 bits) et donne la marge nécessaire. C'est le
//! « 4GB patch » standard des jeux 32 bits fortement moddés.
//!
//! Le patch est un unique bit dans l'en-tête PE : idempotent (ne fait rien s'il
//! est déjà posé), réversible (sauvegarde `<exe>.orig-laa` au premier passage),
//! et n'altère ni la taille ni le reste du fichier.

use crate::error::{LauncherError, Result};
use std::path::Path;

const LARGE_ADDRESS_AWARE: u16 = 0x0020;

/// Pose le drapeau LAA sur l'exécutable s'il est absent.
///
/// Renvoie `Ok(true)` si le fichier a été modifié, `Ok(false)` s'il était déjà
/// « large address aware ».
pub fn set_large_address_aware(exe: &Path) -> Result<bool> {
    let data =
        std::fs::read(exe).map_err(|e| LauncherError::Io(format!("lecture {} : {e}", exe.display())))?;

    let chars_off = characteristics_offset(&data)?;
    let chars = u16::from_le_bytes([data[chars_off], data[chars_off + 1]]);
    if chars & LARGE_ADDRESS_AWARE != 0 {
        return Ok(false); // déjà LAA : rien à faire
    }

    // Sauvegarde unique avant la toute première modification.
    let backup = exe.with_extension("exe.orig-laa");
    if !backup.exists() {
        std::fs::write(&backup, &data)
            .map_err(|e| LauncherError::Io(format!("sauvegarde LAA : {e}")))?;
    }

    let mut data = data;
    let new = chars | LARGE_ADDRESS_AWARE;
    data[chars_off..chars_off + 2].copy_from_slice(&new.to_le_bytes());
    std::fs::write(exe, &data)
        .map_err(|e| LauncherError::Io(format!("écriture LAA {} : {e}", exe.display())))?;
    Ok(true)
}

/// Localise l'offset du champ `Characteristics` du COFF File Header d'un PE.
fn characteristics_offset(data: &[u8]) -> Result<usize> {
    if data.len() < 0x40 {
        return Err(LauncherError::Other("exe trop petit (pas un PE)".into()));
    }
    // e_lfanew (offset de l'en-tête PE) est un u32 à 0x3C du DOS header.
    let e_lfanew =
        u32::from_le_bytes([data[0x3C], data[0x3D], data[0x3E], data[0x3F]]) as usize;
    // Signature « PE\0\0 » (4 o) puis COFF File Header ; le champ Characteristics
    // est à +18 du début du COFF File Header.
    if e_lfanew + 4 + 20 > data.len() || &data[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return Err(LauncherError::Other("signature PE introuvable".into()));
    }
    Ok(e_lfanew + 4 + 18)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un PE minimal factice avec un champ Characteristics contrôlé.
    fn fake_pe(characteristics: u16) -> Vec<u8> {
        let e_lfanew: usize = 0x80;
        let mut data = vec![0u8; e_lfanew + 4 + 20];
        data[0] = b'M';
        data[1] = b'Z';
        data[0x3C..0x40].copy_from_slice(&(e_lfanew as u32).to_le_bytes());
        data[e_lfanew..e_lfanew + 4].copy_from_slice(b"PE\0\0");
        data[e_lfanew + 4 + 18..e_lfanew + 4 + 20]
            .copy_from_slice(&characteristics.to_le_bytes());
        data
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("gtrp_laa_{}_{}.exe", std::process::id(), name))
    }

    #[test]
    fn sets_flag_when_absent() {
        let p = tmp("absent");
        std::fs::write(&p, fake_pe(0x010f)).unwrap();
        assert_eq!(set_large_address_aware(&p).unwrap(), true);
        // relire et vérifier le bit + la sauvegarde
        let d = std::fs::read(&p).unwrap();
        let off = characteristics_offset(&d).unwrap();
        let chars = u16::from_le_bytes([d[off], d[off + 1]]);
        assert!(chars & LARGE_ADDRESS_AWARE != 0);
        assert!(p.with_extension("exe.orig-laa").exists());
        let _ = std::fs::remove_file(&p);
        let _ = std::fs::remove_file(p.with_extension("exe.orig-laa"));
    }

    #[test]
    fn idempotent_when_present() {
        let p = tmp("present");
        std::fs::write(&p, fake_pe(0x010f | LARGE_ADDRESS_AWARE)).unwrap();
        assert_eq!(set_large_address_aware(&p).unwrap(), false);
        // pas de sauvegarde créée puisque rien n'a été modifié
        assert!(!p.with_extension("exe.orig-laa").exists());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn rejects_non_pe() {
        let p = tmp("notpe");
        std::fs::write(&p, b"not a pe file at all........................").unwrap();
        assert!(set_large_address_aware(&p).is_err());
        let _ = std::fs::remove_file(&p);
    }
}
