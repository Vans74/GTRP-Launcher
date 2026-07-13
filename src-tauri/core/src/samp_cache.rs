//! Préchargement fiable du cache artwork SA-MP 0.3.DL.

use crate::config;
use crate::error::{LauncherError, Result};
use crate::updater::{self, Progress};
use crc32fast::Hasher as Crc32;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
struct CacheBundle {
    name: String,
    sha256: String,
    size: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct CacheFile {
    name: String,
    sha256: String,
    size: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct CacheManifest {
    schema: u32,
    version: String,
    server_host: String,
    server_port: u16,
    files_base_url: String,
    bundles_base_url: String,
    bundle: CacheBundle,
    files: Vec<CacheFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheSyncResult {
    pub version: String,
    pub total_files: usize,
    pub downloaded_files: usize,
    pub reused_files: usize,
    pub bytes_downloaded: u64,
    pub used_bundle: bool,
    pub cache_dir: String,
}

pub fn cache_dir(documents_dir: &Path, host: &str, port: u16) -> PathBuf {
    documents_dir
        .join("GTA San Andreas User Files")
        .join("SAMP")
        .join("cache")
        .join(format!("{host}.{port}"))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn expected_crc(name: &str) -> Option<u32> {
    let (stem, extension) = name.rsplit_once('.')?;
    if extension != "dff" && extension != "txd" {
        return None;
    }
    let hex = stem.strip_prefix("0x")?;
    if hex.is_empty() || hex.len() > 8 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let crc = u32::from_str_radix(hex, 16).ok()?;
    if format!("0x{crc:X}") != stem {
        return None;
    }
    Some(crc)
}

fn validate_manifest(manifest: &CacheManifest) -> Result<()> {
    if manifest.schema != 1
        || manifest.version.is_empty()
        || manifest.server_host != config::SERVER_HOST
        || manifest.server_port != config::SERVER_PORT
        || manifest.files_base_url.trim_end_matches('/')
            != config::ARTWORK_FILES_BASE_URL.trim_end_matches('/')
        || manifest.bundles_base_url.trim_end_matches('/')
            != config::ARTWORK_BUNDLES_BASE_URL.trim_end_matches('/')
        || manifest.files.is_empty()
        || !valid_sha256(&manifest.bundle.sha256)
        || manifest.bundle.name != format!("samp-cache-{}.zip", manifest.version)
    {
        return Err(LauncherError::Integrity(
            "manifeste du cache SA-MP invalide ou destiné à un autre serveur".into(),
        ));
    }

    let mut names = HashSet::new();
    for file in &manifest.files {
        if expected_crc(&file.name).is_none()
            || !valid_sha256(&file.sha256)
            || !names.insert(file.name.clone())
        {
            return Err(LauncherError::Integrity(format!(
                "entrée artwork invalide : {}",
                file.name
            )));
        }
    }
    Ok(())
}

fn fetch_manifest() -> Result<CacheManifest> {
    let url = updater::cache_busted(config::ARTWORK_MANIFEST_URL);
    let response = ureq::get(&url)
        .timeout(Duration::from_secs(30))
        .set("Cache-Control", "no-cache")
        .set("Pragma", "no-cache")
        .call()
        .map_err(|e| LauncherError::Network(format!("catalogue artwork : {e}")))?;
    let text = response
        .into_string()
        .map_err(|e| LauncherError::Network(format!("lecture catalogue artwork : {e}")))?;
    let manifest: CacheManifest = serde_json::from_str(&text)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn crc32_file(path: &Path) -> Result<u32> {
    let mut stream = File::open(path)?;
    let mut hasher = Crc32::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize())
}

fn cache_file_is_current(path: &Path, expected: &CacheFile) -> bool {
    path.is_file()
        && path.metadata().map(|m| m.len()).ok() == Some(expected.size)
        && updater::sha256_file(path)
            .map(|sha| sha.eq_ignore_ascii_case(&expected.sha256))
            .unwrap_or(false)
}

fn temporary_path(destination: &Path, suffix: &str) -> PathBuf {
    destination.with_file_name(format!(
        ".{}.{}.{}",
        destination
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("artwork"),
        std::process::id(),
        suffix
    ))
}

fn atomic_install(temporary: &Path, destination: &Path) -> Result<()> {
    let backup = temporary_path(destination, "gtrp_old");
    let _ = fs::remove_file(&backup);
    if destination.exists() {
        fs::rename(destination, &backup)?;
    }
    match fs::rename(temporary, destination) {
        Ok(()) => {
            let _ = fs::remove_file(&backup);
            Ok(())
        }
        Err(error) => {
            if backup.exists() && !destination.exists() {
                let _ = fs::rename(&backup, destination);
            }
            Err(error.into())
        }
    }
}

fn verify_download(path: &Path, expected: &CacheFile) -> Result<()> {
    let sha = updater::sha256_file(path)?;
    if !sha.eq_ignore_ascii_case(&expected.sha256) {
        return Err(LauncherError::Integrity(format!(
            "SHA-256 invalide pour {}",
            expected.name
        )));
    }
    let crc = crc32_file(path)?;
    if Some(crc) != expected_crc(&expected.name) {
        return Err(LauncherError::Integrity(format!(
            "CRC SA-MP invalide pour {}",
            expected.name
        )));
    }
    Ok(())
}

fn download_to<F: FnMut(u64)>(url: &str, destination: &Path, mut on_bytes: F) -> Result<()> {
    let response = ureq::get(url)
        .timeout(Duration::from_secs(1800))
        .call()
        .map_err(|e| LauncherError::Network(format!("{url} : {e}")))?;
    let mut input = response.into_reader();
    let mut output = File::create(destination)?;
    let mut buffer = [0u8; 65536];
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|e| LauncherError::Network(format!("téléchargement artwork : {e}")))?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count])?;
        on_bytes(count as u64);
    }
    output.flush()?;
    output.sync_all()?;
    Ok(())
}

fn install_bundle<F: FnMut(Progress)>(
    manifest: &CacheManifest,
    missing: &[&CacheFile],
    destination: &Path,
    mut progress: F,
) -> Result<u64> {
    let bundle_path = temporary_path(destination, "gtrp_cache_bundle");
    let _ = fs::remove_file(&bundle_path);
    let bundle_url = format!(
        "{}/{}",
        config::ARTWORK_BUNDLES_BASE_URL.trim_end_matches('/'),
        manifest.bundle.name
    );
    let mut downloaded = 0u64;
    download_to(&bundle_url, &bundle_path, |count| {
        downloaded += count;
        progress(Progress {
            current_file: "Cache SA-MP complet".into(),
            files_done: 0,
            files_total: 1,
            bytes_done: downloaded,
            bytes_total: manifest.bundle.size.max(1),
        });
    })?;
    let bundle_sha = updater::sha256_file(&bundle_path)?;
    if !bundle_sha.eq_ignore_ascii_case(&manifest.bundle.sha256) {
        let _ = fs::remove_file(&bundle_path);
        return Err(LauncherError::Integrity(
            "SHA-256 invalide pour le bundle du cache SA-MP".into(),
        ));
    }

    let bundle_file = File::open(&bundle_path)?;
    let mut archive = zip::ZipArchive::new(bundle_file)
        .map_err(|e| LauncherError::Integrity(format!("bundle artwork invalide : {e}")))?;
    for (index, expected) in missing.iter().enumerate() {
        let mut entry = archive
            .by_name(&expected.name)
            .map_err(|e| LauncherError::Integrity(format!("{} absent du bundle : {e}", expected.name)))?;
        if entry.size() != expected.size {
            return Err(LauncherError::Integrity(format!(
                "taille invalide dans le bundle : {}",
                expected.name
            )));
        }
        let final_path = destination.join(&expected.name);
        let temporary = temporary_path(&final_path, "gtrp_extract");
        let _ = fs::remove_file(&temporary);
        {
            let mut output = File::create(&temporary)?;
            std::io::copy(&mut entry, &mut output)?;
            output.flush()?;
            output.sync_all()?;
        }
        if let Err(error) = verify_download(&temporary, expected) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        atomic_install(&temporary, &final_path)?;
        progress(Progress {
            current_file: format!("Cache SA-MP : {}", expected.name),
            files_done: index + 1,
            files_total: missing.len(),
            bytes_done: manifest.bundle.size,
            bytes_total: manifest.bundle.size.max(1),
        });
    }
    let _ = fs::remove_file(&bundle_path);
    Ok(downloaded)
}

fn install_files<F: FnMut(Progress)>(
    manifest: &CacheManifest,
    missing: &[&CacheFile],
    destination: &Path,
    mut progress: F,
) -> Result<u64> {
    let total_bytes = missing.iter().map(|file| file.size).sum::<u64>().max(1);
    let mut downloaded = 0u64;
    for (index, expected) in missing.iter().enumerate() {
        let final_path = destination.join(&expected.name);
        let temporary = temporary_path(&final_path, "gtrp_download");
        let _ = fs::remove_file(&temporary);
        let url = format!(
            "{}/{}",
            manifest.files_base_url.trim_end_matches('/'),
            expected.name
        );
        if let Err(error) = download_to(&url, &temporary, |count| {
            downloaded += count;
            progress(Progress {
                current_file: format!("Cache SA-MP : {}", expected.name),
                files_done: index,
                files_total: missing.len(),
                bytes_done: downloaded,
                bytes_total: total_bytes,
            });
        }) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if let Err(error) = verify_download(&temporary, expected) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        atomic_install(&temporary, &final_path)?;
        progress(Progress {
            current_file: format!("Cache SA-MP : {}", expected.name),
            files_done: index + 1,
            files_total: missing.len(),
            bytes_done: downloaded,
            bytes_total: total_bytes,
        });
    }
    Ok(downloaded)
}

pub fn sync_cache<F: FnMut(Progress)>(
    documents_dir: &Path,
    mut progress: F,
) -> Result<CacheSyncResult> {
    let manifest = fetch_manifest()?;
    let destination = cache_dir(documents_dir, config::SERVER_HOST, config::SERVER_PORT);
    fs::create_dir_all(&destination)?;

    progress(Progress {
        current_file: "Vérification du cache SA-MP".into(),
        files_done: 0,
        files_total: manifest.files.len(),
        bytes_done: 0,
        bytes_total: 1,
    });
    let missing: Vec<&CacheFile> = manifest
        .files
        .iter()
        .filter(|file| !cache_file_is_current(&destination.join(&file.name), file))
        .collect();
    let missing_bytes = missing.iter().map(|file| file.size).sum::<u64>();
    let use_bundle = missing.len() >= 32 || missing_bytes > manifest.bundle.size / 3;

    let bytes_downloaded = if missing.is_empty() {
        0
    } else if use_bundle {
        install_bundle(&manifest, &missing, &destination, &mut progress)?
    } else {
        install_files(&manifest, &missing, &destination, &mut progress)?
    };

    Ok(CacheSyncResult {
        version: manifest.version,
        total_files: manifest.files.len(),
        downloaded_files: missing.len(),
        reused_files: manifest.files.len() - missing.len(),
        bytes_downloaded,
        used_bundle: use_bundle && !missing.is_empty(),
        cache_dir: destination.to_string_lossy().into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gtrp_cache_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn cache_names_are_strict_and_match_crc() {
        assert_eq!(expected_crc("0xBA694AF2.dff"), Some(0xBA694AF2));
        assert_eq!(expected_crc("0x4CFA046.txd"), Some(0x04CFA046));
        assert_eq!(expected_crc("0x04CFA046.txd"), None);
        assert_eq!(expected_crc("../evil.dff"), None);
        assert_eq!(expected_crc("0x1234.exe"), None);
    }

    #[test]
    fn cache_path_supports_redirected_documents() {
        let documents = Path::new("C:/Users/Test/OneDrive/Documents");
        let path = cache_dir(documents, "51.255.92.237", 3400);
        assert!(path.ends_with("GTA San Andreas User Files/SAMP/cache/51.255.92.237.3400"));
    }

    #[test]
    fn crc32_matches_samp_filename() {
        let dir = temp_dir("crc");
        let file = dir.join("sample.dff");
        fs::write(&file, b"hello").unwrap();
        assert_eq!(crc32_file(&file).unwrap(), 0x3610A686);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn current_file_requires_size_and_sha() {
        let dir = temp_dir("current");
        let file = dir.join("0x3610A686.dff");
        fs::write(&file, b"hello").unwrap();
        let expected = CacheFile {
            name: "0x3610A686.dff".into(),
            sha256: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".into(),
            size: 5,
        };
        assert!(cache_file_is_current(&file, &expected));
        fs::write(&file, b"HELLO").unwrap();
        assert!(!cache_file_is_current(&file, &expected));
        let _ = fs::remove_dir_all(dir);
    }
}
