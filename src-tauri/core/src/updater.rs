//! Auto-updater du modpack + vérification d'intégrité (anti-triche léger).
//!
//! Principe : un `manifest.json` distant décrit chaque fichier attendu dans le
//! dossier du jeu (chemin relatif, SHA-256, taille). Le launcher compare avec
//! l'état local et ne télécharge que ce qui manque ou diffère, en vérifiant le
//! hash de chaque téléchargement. Il peut aussi signaler des fichiers interdits.

use crate::error::{LauncherError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    /// Version du modpack (affichée dans l'UI).
    #[serde(default)]
    pub version: String,
    /// URL de base pour construire les URLs de fichiers si `url` est absent.
    #[serde(default)]
    pub base_url: String,
    /// Fichiers attendus.
    #[serde(default)]
    pub files: Vec<ManifestFile>,
    /// Motifs de fichiers interdits (anti-triche), relatifs au dossier du jeu.
    #[serde(default)]
    pub forbidden: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManifestFile {
    /// Chemin relatif au dossier racine du jeu (ex: "modloader/skins/skin1.txd").
    pub path: String,
    /// SHA-256 attendu (hex minuscule).
    pub sha256: String,
    /// Taille en octets (indicatif, pour la barre de progression).
    #[serde(default)]
    pub size: u64,
    /// URL explicite ; si absente, on utilise `{base_url}/{path}`.
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedFile {
    pub path: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    /// Raison : "manquant" ou "obsolète".
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdatePlan {
    pub up_to_date: bool,
    pub files: Vec<PlannedFile>,
    pub total_bytes: u64,
    pub manifest_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub current_file: String,
    pub files_done: usize,
    pub files_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntegrityReport {
    pub ok: bool,
    /// Fichiers attendus mais manquants ou corrompus.
    pub invalid: Vec<String>,
    /// Fichiers interdits détectés.
    pub forbidden_found: Vec<String>,
}

/// Empêche toute évasion du dossier du jeu (path traversal) via un manifest malveillant.
fn safe_join(root: &Path, rel: &str) -> Result<PathBuf> {
    let rel = rel.replace('\\', "/");
    if rel.starts_with('/') || rel.contains("..") || rel.contains(':') {
        return Err(LauncherError::Integrity(format!(
            "chemin de fichier non autorisé : {rel}"
        )));
    }
    Ok(root.join(rel))
}

/// Calcule le SHA-256 (hex) d'un fichier, en streaming (aucune limite de taille RAM).
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn resolve_url(base_url: &str, file: &ManifestFile) -> String {
    if let Some(u) = &file.url {
        return u.clone();
    }
    let base = base_url.trim_end_matches('/');
    format!("{base}/{}", file.path.trim_start_matches('/'))
}

/// Télécharge et parse le manifest distant.
pub fn fetch_manifest(url: &str) -> Result<Manifest> {
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|e| LauncherError::Network(format!("manifest : {e}")))?;
    let text = resp
        .into_string()
        .map_err(|e| LauncherError::Network(format!("lecture manifest : {e}")))?;
    let manifest: Manifest = serde_json::from_str(&text)?;
    Ok(manifest)
}

/// Compare le manifest avec l'état local et calcule la liste des fichiers à mettre à jour.
pub fn plan_updates(manifest: &Manifest, gta_root: &Path) -> Result<UpdatePlan> {
    let mut files = Vec::new();
    let mut total = 0u64;

    for f in &manifest.files {
        let dest = safe_join(gta_root, &f.path)?;
        let expected = f.sha256.to_ascii_lowercase();

        let needs = if !dest.is_file() {
            Some("manquant")
        } else {
            match sha256_file(&dest) {
                Ok(local) if local.eq_ignore_ascii_case(&expected) => None,
                _ => Some("obsolète"),
            }
        };

        if let Some(reason) = needs {
            total += f.size;
            files.push(PlannedFile {
                path: f.path.clone(),
                url: resolve_url(&manifest.base_url, f),
                sha256: expected,
                size: f.size,
                reason: reason.to_string(),
            });
        }
    }

    Ok(UpdatePlan {
        up_to_date: files.is_empty(),
        total_bytes: total,
        files,
        manifest_version: manifest.version.clone(),
    })
}

/// Télécharge un fichier, vérifie son SHA-256, puis l'installe atomiquement.
/// `on_bytes` est appelé à chaque bloc reçu avec le nombre d'octets du bloc.
fn download_verify<F: FnMut(u64)>(
    url: &str,
    dest: &Path,
    expected_sha: &str,
    mut on_bytes: F,
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(60))
        .call()
        .map_err(|e| LauncherError::Network(format!("{url} : {e}")))?;

    let tmp = dest.with_extension("gtrp_part");
    {
        let mut reader = resp.into_reader();
        let mut out = std::fs::File::create(&tmp)?;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 65536];
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| LauncherError::Network(format!("téléchargement : {e}")))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n])?;
            on_bytes(n as u64);
        }
        out.flush()?;
        let got = hex::encode(hasher.finalize());
        if !got.eq_ignore_ascii_case(expected_sha) {
            let _ = std::fs::remove_file(&tmp);
            return Err(LauncherError::Integrity(format!(
                "hash invalide pour {} (attendu {}, obtenu {})",
                dest.display(),
                expected_sha,
                got
            )));
        }
    }

    // Installation atomique.
    std::fs::rename(&tmp, dest).or_else(|_| {
        // rename peut échouer entre volumes : fallback copie.
        std::fs::copy(&tmp, dest).map(|_| ()).and_then(|_| {
            let _ = std::fs::remove_file(&tmp);
            Ok(())
        })
    })?;
    Ok(())
}

/// Applique le plan de mise à jour. `progress` est appelé régulièrement.
pub fn apply_updates<F: FnMut(Progress)>(
    plan: &UpdatePlan,
    gta_root: &Path,
    mut progress: F,
) -> Result<()> {
    let files_total = plan.files.len();
    let bytes_total = plan.total_bytes.max(1);
    let mut bytes_done = 0u64;

    for (i, f) in plan.files.iter().enumerate() {
        let dest = safe_join(gta_root, &f.path)?;
        progress(Progress {
            current_file: f.path.clone(),
            files_done: i,
            files_total,
            bytes_done,
            bytes_total,
        });

        download_verify(&f.url, &dest, &f.sha256, |n| {
            bytes_done += n;
        })?;

        progress(Progress {
            current_file: f.path.clone(),
            files_done: i + 1,
            files_total,
            bytes_done,
            bytes_total,
        });
    }
    Ok(())
}

/// Vérifie l'intégrité complète : chaque fichier du manifest doit être présent
/// et valide, et aucun fichier interdit ne doit être présent.
pub fn verify_integrity(manifest: &Manifest, gta_root: &Path) -> Result<IntegrityReport> {
    let mut invalid = Vec::new();
    for f in &manifest.files {
        let dest = safe_join(gta_root, &f.path)?;
        let ok = dest.is_file()
            && sha256_file(&dest)
                .map(|h| h.eq_ignore_ascii_case(&f.sha256))
                .unwrap_or(false);
        if !ok {
            invalid.push(f.path.clone());
        }
    }

    let forbidden_found = scan_forbidden(gta_root, &manifest.forbidden);

    Ok(IntegrityReport {
        ok: invalid.is_empty() && forbidden_found.is_empty(),
        invalid,
        forbidden_found,
    })
}

/// Recherche des fichiers interdits. Un motif peut être un chemin relatif exact
/// ou se terminer par un joker `*` sur l'extension/nom (ex: "cleo/*", "*.asi").
pub fn scan_forbidden(gta_root: &Path, patterns: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    if patterns.is_empty() {
        return found;
    }
    for pat in patterns {
        // Motif exact d'un chemin.
        if !pat.contains('*') {
            if safe_join(gta_root, pat).map(|p| p.is_file()).unwrap_or(false) {
                found.push(pat.clone());
            }
            continue;
        }
        // Motifs à joker : on parcourt récursivement.
        collect_matches(gta_root, gta_root, pat, &mut found);
    }
    found.sort();
    found.dedup();
    found
}

fn pattern_matches(rel: &str, pattern: &str) -> bool {
    let rel = rel.replace('\\', "/").to_ascii_lowercase();
    let pattern = pattern.replace('\\', "/").to_ascii_lowercase();
    if let Some(suffix) = pattern.strip_prefix('*') {
        // "*.asi" -> se termine par ".asi"
        return rel.ends_with(&suffix);
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        // "cleo/*" -> commence par "cleo/"
        return rel.starts_with(&prefix);
    }
    rel == pattern
}

fn collect_matches(root: &Path, dir: &Path, pattern: &str, out: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_matches(root, &path, pattern, out);
        } else if let Ok(rel) = path.strip_prefix(root) {
            let rel_str = rel.to_string_lossy().to_string();
            if pattern_matches(&rel_str, pattern) {
                out.push(rel_str);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("gtrp_upd_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn sha256_of_known_content() {
        let dir = tmp_dir("sha");
        let f = dir.join("a.txt");
        std::fs::write(&f, b"hello").unwrap();
        // sha256("hello")
        assert_eq!(
            sha256_file(&f).unwrap(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn safe_join_rejects_traversal() {
        let root = Path::new("/game");
        assert!(safe_join(root, "../evil").is_err());
        assert!(safe_join(root, "/etc/passwd").is_err());
        assert!(safe_join(root, "C:/win").is_err());
        assert!(safe_join(root, "modloader/ok.txd").is_ok());
    }

    #[test]
    fn plan_detects_missing_and_valid() {
        let dir = tmp_dir("plan");
        // fichier valide
        let good = dir.join("good.txt");
        std::fs::write(&good, b"hello").unwrap();
        let good_sha = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

        let manifest = Manifest {
            version: "1.0.0".into(),
            base_url: "https://x/files".into(),
            files: vec![
                ManifestFile {
                    path: "good.txt".into(),
                    sha256: good_sha.into(),
                    size: 5,
                    url: None,
                },
                ManifestFile {
                    path: "missing.txt".into(),
                    sha256: "deadbeef".into(),
                    size: 10,
                    url: None,
                },
            ],
            forbidden: vec![],
        };

        let plan = plan_updates(&manifest, &dir).unwrap();
        assert!(!plan.up_to_date);
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].path, "missing.txt");
        assert_eq!(plan.files[0].url, "https://x/files/missing.txt");
        assert_eq!(plan.total_bytes, 10);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn forbidden_scan_matches_patterns() {
        let dir = tmp_dir("forb");
        std::fs::create_dir_all(dir.join("cleo")).unwrap();
        std::fs::write(dir.join("cleo/cheat.cs"), b"x").unwrap();
        std::fs::write(dir.join("trainer.asi"), b"x").unwrap();
        std::fs::write(dir.join("clean.txt"), b"x").unwrap();

        let found = scan_forbidden(&dir, &["*.asi".into(), "cleo/*".into()]);
        assert!(found.iter().any(|f| f.ends_with("trainer.asi")));
        assert!(found.iter().any(|f| f.replace('\\', "/").ends_with("cleo/cheat.cs")));
        assert!(!found.iter().any(|f| f.ends_with("clean.txt")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn integrity_report_flags_issues() {
        let dir = tmp_dir("integ");
        std::fs::write(dir.join("present.txt"), b"hello").unwrap();
        let manifest = Manifest {
            version: "1".into(),
            base_url: "x".into(),
            files: vec![ManifestFile {
                path: "present.txt".into(),
                sha256: "wronghash".into(),
                size: 5,
                url: None,
            }],
            forbidden: vec![],
        };
        let report = verify_integrity(&manifest, &dir).unwrap();
        assert!(!report.ok);
        assert_eq!(report.invalid, vec!["present.txt".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
