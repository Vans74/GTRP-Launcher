//! Point d'entrée du backend du launcher GTRP (couche Tauri).
//!
//! La logique métier vit dans la crate `gtrp-core` (testable hors GUI).
//! Ce fichier se contente d'exposer les commandes au frontend et de gérer les
//! chemins/événements propres à l'application.

use gtrp_core::config;
use gtrp_core::error::{LauncherError, Result};
use gtrp_core::{enb, gta, launch, news, query, samp_cache, settings, updater};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Résout le dossier de configuration de l'application (créé au besoin).
fn config_dir(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .app_config_dir()
        .map_err(|e| LauncherError::Config(format!("dossier config introuvable : {e}")))
}

/// Résout l'installation du jeu : d'abord via les réglages, sinon auto-détection.
fn resolve_install(app: &AppHandle) -> Result<gta::GameInstall> {
    let dir = config_dir(app)?;
    let s = settings::load(&dir);
    if let Some(path) = s.gta_path.as_ref() {
        return gta::from_gta_exe(Path::new(path));
    }
    gta::detect().ok_or_else(|| {
        LauncherError::GameNotFound(
            "GTA San Andreas introuvable. Sélectionne gta_sa.exe manuellement.".into(),
        )
    })
}

#[tauri::command]
fn get_config() -> config::PublicConfig {
    config::public_config()
}

#[tauri::command]
async fn get_server_status() -> query::ServerStatus {
    let host = config::SERVER_HOST.to_string();
    let port = config::SERVER_PORT;
    tauri::async_runtime::spawn_blocking(move || {
        query::query_status(&host, port, Duration::from_millis(2000))
    })
    .await
    .unwrap_or_else(|_| query::query_status("", 0, Duration::from_millis(1)))
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Result<settings::Settings> {
    let dir = config_dir(&app)?;
    Ok(settings::load(&dir))
}

#[tauri::command]
fn set_nickname(app: AppHandle, nickname: String) -> Result<settings::Settings> {
    if !settings::is_valid_nickname(&nickname) {
        return Err(LauncherError::Other(
            "Pseudo invalide : 3 à 24 caractères, sans espace ni accent.".into(),
        ));
    }
    let dir = config_dir(&app)?;
    let mut s = settings::load(&dir);
    s.nickname = nickname;
    settings::save(&dir, &s)?;
    Ok(s)
}

#[tauri::command]
fn detect_game() -> Option<gta::GameInstall> {
    gta::detect()
}

#[tauri::command]
fn set_game_path(app: AppHandle, gta_exe: String) -> Result<gta::GameInstall> {
    let install = gta::from_gta_exe(Path::new(&gta_exe))?;
    let dir = config_dir(&app)?;
    let mut s = settings::load(&dir);
    s.gta_path = Some(install.gta_exe.clone());
    s.samp_path = install.samp_exe.clone();
    settings::save(&dir, &s)?;
    Ok(install)
}

#[tauri::command]
fn set_enhanced_graphics(app: AppHandle, enabled: bool) -> Result<settings::Settings> {
    let dir = config_dir(&app)?;
    let mut s = settings::load(&dir);
    s.enhanced_graphics = enabled;
    settings::save(&dir, &s)?;
    Ok(s)
}

#[tauri::command]
fn launch_game(app: AppHandle) -> Result<enb::EnbPrepareResult> {
    let dir = config_dir(&app)?;
    let s = settings::load(&dir);
    if !settings::is_valid_nickname(&s.nickname) {
        return Err(LauncherError::Other(
            "Renseigne d'abord un pseudo valide.".into(),
        ));
    }
    let install = resolve_install(&app)?;

    let enb_result = enb::prepare(Path::new(&install.root), s.enhanced_graphics)?;

    launch::launch(&install, &s.nickname, config::SERVER_HOST, config::SERVER_PORT)?;

    Ok(enb_result)
}

#[tauri::command]
async fn check_updates(app: AppHandle) -> Result<updater::UpdatePlan> {
    let install = resolve_install(&app)?;
    let manifest_url = format!("{}/manifest.json", config::ASSET_BASE_URL);
    tauri::async_runtime::spawn_blocking(move || {
        let manifest = updater::fetch_manifest(&manifest_url)?;
        updater::plan_updates(&manifest, Path::new(&install.root))
    })
    .await
    .map_err(|e| LauncherError::Other(format!("tâche interrompue : {e}")))?
}

#[tauri::command]
async fn apply_updates(app: AppHandle) -> Result<()> {
    let install = resolve_install(&app)?;
    let cfg = config_dir(&app)?;
    let enhanced = settings::load(&cfg).enhanced_graphics;
    let manifest_url = format!("{}/manifest.json", config::ASSET_BASE_URL);
    let app_for_events = app.clone();
    let gta_root = install.root.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let manifest = updater::fetch_manifest(&manifest_url)?;
        let plan = updater::plan_updates(&manifest, Path::new(&gta_root))?;
        updater::apply_updates(&plan, Path::new(&gta_root), |p| {
            let _ = app_for_events.emit("update-progress", &p);
        })?;
        // Redéploie les graphismes si le joueur les avait activés (sinon SALodLights
        // et autres ASI restent absents de la racine après une mise à jour bundle).
        if enhanced {
            let _ = enb::prepare(Path::new(&gta_root), true)?;
        }
        let _ = app_for_events.emit("update-done", ());
        Ok::<(), LauncherError>(())
    })
    .await
    .map_err(|e| LauncherError::Other(format!("tâche interrompue : {e}")))?
}

/// Synchronise l'intégralité du catalogue artwork dans le cache natif SA-MP.
/// Le dossier Documents est résolu par l'API Windows/Tauri afin de respecter
/// OneDrive et les redirections de profil.
#[tauri::command]
async fn sync_samp_cache(app: AppHandle) -> Result<samp_cache::CacheSyncResult> {
    let documents = app
        .path()
        .document_dir()
        .map_err(|e| LauncherError::Config(format!("dossier Documents introuvable : {e}")))?;
    let app_for_events = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        samp_cache::sync_cache(&documents, |progress| {
            let _ = app_for_events.emit("update-progress", &progress);
        })
    })
    .await
    .map_err(|e| LauncherError::Other(format!("tâche cache interrompue : {e}")))?
}

#[tauri::command]
async fn verify_integrity(app: AppHandle) -> Result<updater::IntegrityReport> {
    let install = resolve_install(&app)?;
    let manifest_url = format!("{}/manifest.json", config::ASSET_BASE_URL);
    tauri::async_runtime::spawn_blocking(move || {
        let manifest = updater::fetch_manifest(&manifest_url)?;
        updater::verify_integrity(&manifest, Path::new(&install.root))
    })
    .await
    .map_err(|e| LauncherError::Other(format!("tâche interrompue : {e}")))?
}

#[tauri::command]
async fn get_news() -> Result<news::NewsFeed> {
    let url = format!("{}/news.json", config::ASSET_BASE_URL);
    tauri::async_runtime::spawn_blocking(move || news::fetch_news(&url))
        .await
        .map_err(|e| LauncherError::Other(format!("tâche interrompue : {e}")))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_server_status,
            load_settings,
            set_nickname,
            set_enhanced_graphics,
            detect_game,
            set_game_path,
            launch_game,
            check_updates,
            apply_updates,
            sync_samp_cache,
            verify_integrity,
            get_news,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au démarrage du launcher GTRP");
}
