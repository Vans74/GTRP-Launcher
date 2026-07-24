//! Surveillance de l'installation et des modules chargés pendant la session.
//!
//! Le contrôle pré-lancement ferme la fenêtre principale, mais il faut encore
//! empêcher l'ajout d'un ASI/DLL ou le remplacement d'une texture après ce
//! contrôle. Le garde combine ReadDirectoryChangesW (via `notify`) et, sous
//! Windows, l'inventaire des modules réellement chargés par SA-MP/GTA.

use crate::error::{LauncherError, Result};
use crate::updater::{sha256_file, IntegrityFile, IntegrityPolicy, IntegrityProfile, Manifest};
use notify::{RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeViolation {
    pub kind: String,
    pub message: String,
    pub path: Option<String>,
}

fn rel_key(root: &Path, path: &Path) -> Option<(String, String)> {
    let rel = path.strip_prefix(root).ok()?;
    let display = rel.to_string_lossy().replace('\\', "/");
    Some((display.to_ascii_lowercase(), display))
}

fn mutable_path_matches(rel: &str, patterns: &[String]) -> bool {
    let rel = rel.replace('\\', "/").to_ascii_lowercase();
    patterns.iter().any(|pattern| {
        let pattern = pattern.replace('\\', "/").to_ascii_lowercase();
        if let Some(prefix) = pattern.strip_suffix("/**") {
            rel == prefix || rel.starts_with(&format!("{prefix}/"))
        } else if let Some(suffix) = pattern.strip_prefix('*') {
            rel.ends_with(suffix)
        } else {
            rel == pattern
        }
    })
}

fn expected_files(policy: &IntegrityPolicy, hd_enabled: bool) -> HashMap<String, IntegrityFile> {
    policy
        .files
        .iter()
        .filter(|file| {
            file.profile == IntegrityProfile::Always
                || (file.profile == IntegrityProfile::Hd && hd_enabled)
        })
        .map(|file| {
            (
                file.path.replace('\\', "/").to_ascii_lowercase(),
                file.clone(),
            )
        })
        .collect()
}

fn violation(kind: &str, message: String, path: Option<String>) -> RuntimeViolation {
    RuntimeViolation {
        kind: kind.into(),
        message,
        path,
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn validate_changed_path(
    root: &Path,
    path: &Path,
    expected: &HashMap<String, IntegrityFile>,
    mutable_paths: &[String],
) -> Option<RuntimeViolation> {
    let (key, display) = rel_key(root, path)?;
    if key.is_empty() {
        return None;
    }

    // Même un dossier mutable ne peut pas être remplacé par une jonction.
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if is_reparse_point(&metadata) => {
            return Some(violation(
                "reparse_point",
                format!("Lien ou jonction injecté pendant la session : {display}"),
                Some(display),
            ));
        }
        Ok(metadata) if metadata.is_dir() => return None,
        Ok(metadata) if metadata.is_file() => {
            if mutable_path_matches(&key, mutable_paths) {
                return None;
            }
            let Some(file) = expected.get(&key) else {
                return Some(violation(
                    "unexpected_file",
                    format!("Fichier non autorisé ajouté pendant la session : {display}"),
                    Some(display),
                ));
            };
            let valid = metadata.len() == file.size
                && sha256_file(path)
                    .map(|hash| hash.eq_ignore_ascii_case(&file.sha256))
                    .unwrap_or(false);
            if !valid {
                return Some(violation(
                    "modified_file",
                    format!("Fichier GTRP modifié pendant la session : {display}"),
                    Some(display),
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(file) = expected.get(&key) {
                if !file.optional {
                    return Some(violation(
                        "removed_file",
                        format!("Fichier GTRP supprimé pendant la session : {display}"),
                        Some(display),
                    ));
                }
            }
        }
        Err(error) => {
            return Some(violation(
                "filesystem_error",
                format!("Contrôle impossible pour {display} : {error}"),
                Some(display),
            ));
        }
        _ => {}
    }
    None
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
        MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, TerminateProcess,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    };

    fn wide_string(value: &[u16]) -> String {
        let length = value
            .iter()
            .position(|item| *item == 0)
            .unwrap_or(value.len());
        String::from_utf16_lossy(&value[..length])
    }

    fn process_path(pid: u32) -> Option<PathBuf> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle == 0 {
                return None;
            }
            let mut buffer = vec![0u16; 32768];
            let mut length = buffer.len() as u32;
            let ok = QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length);
            CloseHandle(handle);
            (ok != 0).then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize])))
        }
    }

    pub(super) fn game_processes(root: &Path) -> Vec<u32> {
        let root = root
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        let mut result = Vec::new();
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return result;
            }
            let mut entry: PROCESSENTRY32W = zeroed();
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            let mut ok = Process32FirstW(snapshot, &mut entry);
            while ok != 0 {
                let name = wide_string(&entry.szExeFile).to_ascii_lowercase();
                if matches!(name.as_str(), "gta_sa.exe" | "samp.exe") {
                    if let Some(path) = process_path(entry.th32ProcessID) {
                        let path = path
                            .to_string_lossy()
                            .replace('\\', "/")
                            .to_ascii_lowercase();
                        if path.starts_with(&format!("{root}/")) {
                            result.push(entry.th32ProcessID);
                        }
                    }
                }
                ok = Process32NextW(snapshot, &mut entry);
            }
            CloseHandle(snapshot);
        }
        result
    }

    fn module_paths(pid: u32) -> Option<Vec<PathBuf>> {
        let mut result = Vec::new();
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid);
            if snapshot == INVALID_HANDLE_VALUE {
                return None;
            }
            let mut entry: MODULEENTRY32W = zeroed();
            entry.dwSize = size_of::<MODULEENTRY32W>() as u32;
            let mut ok = Module32FirstW(snapshot, &mut entry);
            while ok != 0 {
                result.push(PathBuf::from(wide_string(&entry.szExePath)));
                ok = Module32NextW(snapshot, &mut entry);
            }
            CloseHandle(snapshot);
        }
        Some(result)
    }

    fn windows_root() -> String {
        std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase()
    }

    pub(super) fn validate_modules(
        pid: u32,
        root: &Path,
        expected: &HashMap<String, IntegrityFile>,
        verified: &mut HashSet<(u32, String)>,
        failures: &mut HashMap<u32, u8>,
    ) -> Option<RuntimeViolation> {
        let Some(modules) = module_paths(pid) else {
            let attempts = failures.entry(pid).or_default();
            *attempts = attempts.saturating_add(1);
            if *attempts >= 3 {
                return Some(violation(
                    "module_scan_error",
                    format!(
                        "Impossible de contrôler les modules chargés par le processus {pid}."
                    ),
                    None,
                ));
            }
            return None;
        };
        failures.remove(&pid);
        let windows = windows_root();
        for module in modules {
            let module_display = module.to_string_lossy().replace('\\', "/");
            let module_key = module_display.to_ascii_lowercase();
            if module_key == windows || module_key.starts_with(&format!("{windows}/")) {
                continue;
            }
            let Some((rel, display)) = rel_key(root, &module) else {
                return Some(violation(
                    "foreign_module",
                    format!("Module externe injecté dans GTA/SA-MP : {module_display}"),
                    Some(module_display),
                ));
            };
            let Some(file) = expected.get(&rel) else {
                return Some(violation(
                    "unexpected_module",
                    format!("Module local non autorisé chargé : {display}"),
                    Some(display),
                ));
            };
            if verified.insert((pid, rel.clone())) {
                let valid = std::fs::metadata(&module)
                    .map(|metadata| metadata.len() == file.size)
                    .unwrap_or(false)
                    && sha256_file(&module)
                        .map(|hash| hash.eq_ignore_ascii_case(&file.sha256))
                        .unwrap_or(false);
                if !valid {
                    return Some(violation(
                        "modified_module",
                        format!("Module local altéré chargé : {display}"),
                        Some(display),
                    ));
                }
            }
        }
        None
    }

    pub(super) fn terminate(pids: &[u32]) {
        for pid in pids {
            unsafe {
                let handle = OpenProcess(PROCESS_TERMINATE, 0, *pid);
                if handle != 0 {
                    let _ = TerminateProcess(handle, 0x47545250);
                    CloseHandle(handle);
                }
            }
        }
    }
}

/// Arme la surveillance avant de lancer la session.
///
/// Le watcher est installé, puis un second contrôle exhaustif est effectué
/// avant d'appeler `launch_game`. Ainsi aucun fichier ne peut être remplacé
/// entre le dernier hash et le démarrage sans générer un événement déjà mis en
/// file. En cas de violation, tous les processus GTA/SA-MP de cette installation
/// sont terminés puis le callback informe l'interface.
#[cfg(windows)]
pub fn launch_guarded<L, F>(
    gta_root: PathBuf,
    manifest: Manifest,
    hd_enabled: bool,
    launch_game: L,
    on_violation: F,
) -> Result<u32>
where
    L: FnOnce() -> Result<u32>,
    F: Fn(RuntimeViolation) + Send + 'static,
{
    let policy = manifest.integrity.as_ref().ok_or_else(|| {
        LauncherError::Integrity("Surveillance impossible : politique d'intégrité absente.".into())
    })?;
    let expected = expected_files(policy, hd_enabled);
    let mutable = policy.mutable_paths.clone();
    let (sender, receiver) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    })
    .map_err(|error| {
        LauncherError::Integrity(format!("Surveillance du dossier impossible : {error}"))
    })?;
    watcher
        .watch(&gta_root, RecursiveMode::Recursive)
        .map_err(|error| {
            LauncherError::Integrity(format!("Surveillance du dossier impossible : {error}"))
        })?;

    let report =
        crate::updater::verify_integrity_for_profile(&manifest, &gta_root, hd_enabled)?;
    if !report.ok {
        return Err(LauncherError::Integrity(format!(
            "L'installation a changé pendant l'armement de la surveillance ({} invalide(s), {} inattendu(s), {} jonction(s)).",
            report.invalid.len(),
            report.unexpected.len(),
            report.reparse_points.len()
        )));
    }

    let initial_pid = launch_game()?;
    std::thread::spawn(move || {
        // `watcher` doit rester vivant aussi longtemps que la boucle.
        let _watcher = watcher;

        let started = Instant::now();
        let mut last_full_scan = Instant::now();
        let mut verified_modules = HashSet::new();
        let mut module_scan_failures = HashMap::new();
        loop {
            match receiver.recv_timeout(Duration::from_secs(2)) {
                Ok(Ok(event)) => {
                    for path in event.paths {
                        if let Some(issue) =
                            validate_changed_path(&gta_root, &path, &expected, &mutable)
                        {
                            let mut pids = windows::game_processes(&gta_root);
                            pids.push(initial_pid);
                            pids.sort_unstable();
                            pids.dedup();
                            windows::terminate(&pids);
                            on_violation(issue);
                            return;
                        }
                    }
                }
                Ok(Err(error)) => {
                    let mut pids = windows::game_processes(&gta_root);
                    pids.push(initial_pid);
                    windows::terminate(&pids);
                    on_violation(violation(
                        "guard_error",
                        format!("Erreur de surveillance du dossier : {error}"),
                        None,
                    ));
                    return;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    windows::terminate(&[initial_pid]);
                    on_violation(violation(
                        "guard_error",
                        "Surveillance du dossier interrompue.".into(),
                        None,
                    ));
                    return;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }

            let pids = windows::game_processes(&gta_root);
            if pids.is_empty() {
                // SA-MP peut disparaître brièvement avant que gta_sa.exe soit
                // visible. On ne désarme qu'après cette fenêtre de démarrage.
                if started.elapsed() > Duration::from_secs(30) {
                    return;
                }
                continue;
            }
            for pid in &pids {
                if let Some(issue) =
                    windows::validate_modules(
                        *pid,
                        &gta_root,
                        &expected,
                        &mut verified_modules,
                        &mut module_scan_failures,
                    )
                {
                    windows::terminate(&pids);
                    on_violation(issue);
                    return;
                }
            }

            // Filet de sécurité contre une perte d'événement du watcher ou une
            // falsification d'horodatage : recalcul complet périodique.
            if last_full_scan.elapsed() >= Duration::from_secs(300) {
                match crate::updater::verify_integrity_for_profile(&manifest, &gta_root, hd_enabled)
                {
                    Ok(report) if report.ok => {
                        last_full_scan = Instant::now();
                    }
                    Ok(report) => {
                        windows::terminate(&pids);
                        on_violation(violation(
                            "periodic_integrity",
                            format!(
                                "Contrôle périodique échoué : {} fichier(s) invalide(s), {} inattendu(s).",
                                report.invalid.len(),
                                report.unexpected.len()
                            ),
                            None,
                        ));
                        return;
                    }
                    Err(error) => {
                        windows::terminate(&pids);
                        on_violation(violation(
                            "guard_error",
                            format!("Contrôle périodique impossible : {error}"),
                            None,
                        ));
                        return;
                    }
                }
            }
        }
    });
    Ok(initial_pid)
}

#[cfg(not(windows))]
pub fn launch_guarded<L, F>(
    _gta_root: PathBuf,
    _manifest: Manifest,
    _hd_enabled: bool,
    _launch_game: L,
    _on_violation: F,
) -> Result<u32>
where
    L: FnOnce() -> Result<u32>,
    F: Fn(RuntimeViolation) + Send + 'static,
{
    Err(LauncherError::Other(
        "Le lancement protégé n'est disponible que sous Windows.".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutable_rules_are_narrow_and_case_insensitive() {
        let rules = vec![
            "gtrp-assets/components/**".into(),
            "reshade-screenshots/**".into(),
            "*.log".into(),
        ];
        assert!(mutable_path_matches(
            "GTRP-ASSETS/components/cache/file.bin",
            &rules
        ));
        assert!(mutable_path_matches("modloader/modloader.log", &rules));
        assert!(!mutable_path_matches("modloader/cheat.asi", &rules));
    }
}
