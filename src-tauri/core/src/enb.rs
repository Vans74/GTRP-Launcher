//! Déploiement des contenus permanents GTRP et des graphismes HD optionnels.
//!
//! Le modpack est stocké dans `{gta_root}/gtrp-assets/enb/`. La plupart des
//! contenus (modèles, skins, sons, interface, radar, modloader et ASI loader)
//! sont toujours copiés dans le jeu. Seuls les chemins déclarés dans
//! `.gtrp-hd-paths` dépendent du bouton « Graphismes HD ».
//!
//! Le moteur ENB reste un composant autonome : le launcher le télécharge depuis
//! l'URL décrite par `.gtrp-hd-component.json`, vérifie son SHA-256 et n'extrait
//! que les fichiers explicitement autorisés. Le modpack GTRP ne réhéberge que
//! ses réglages additionnels.

use crate::error::{LauncherError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

/// Dossier relatif (dans le jeu) où le modpack dépose ses fichiers en attente.
pub const ENB_STAGING_REL: &str = "gtrp-assets/enb";

/// Inventaire du dernier déploiement géré par le launcher.
pub const ENB_MARKER: &str = ".gtrp_enb_active";

/// Liste des chemins conditionnels au bouton Graphismes HD.
pub const HD_PATHS_FILE: &str = ".gtrp-hd-paths";

/// Source du composant graphique autonome.
pub const HD_COMPONENT_FILE: &str = ".gtrp-hd-component.json";

/// Nom neutre sous lequel le modpack livre l'Ultimate ASI Loader.
pub const LOADER_SRC_NAME: &str = "vorbisFileLoader.dll";

/// Nom sous lequel le loader doit être installé pour charger les ASI.
pub const LOADER_TARGET_NAME: &str = "vorbisFile.dll";

/// Sauvegarde du `vorbisFile.dll` d'origine du jeu.
pub const LOADER_BACKUP_NAME: &str = "vorbisFileHooked.dll";

/// Chemins graphiques historiques : ce fallback rend le nouveau launcher sûr
/// même avant que le nouveau `.gtrp-hd-paths` soit reçu par le modpack.
const DEFAULT_HD_PATHS: &[&str] = &[
    "d3d9.dll",
    "d3d9.dll.orig-splash",
    "ReShade.ini",
    "ReShadePreset.ini",
    "reshade-shaders/",
    "skygfx.asi",
    "skygfx.ini",
    "skygfx1.ini",
    "skygfx2.ini",
    "skygfx3.ini",
    "skygrad.asi",
    "SAMPGraphicRestore.asi",
    "SALodLights.asi",
    "SALodLights.dat",
    "SALodLights.ini",
    "neo/",
    "data/colorcycle.dat",
    "models/",
    "enblocal.ini",
    "enbseries.ini",
    "enbseries/",
    "enbbloom.fx",
    "enbdepthoffield.fx",
    "enbeffect.fx",
    "enbeffectprepass.fx",
    "enbenvmap.fx",
    "enblens.fx",
    "enblighting.fx",
    "enbsky.fx",
    "enbunderwater.fx",
    "enbvehicle.fx",
    "enbwater.fx",
    "modloader/OE Mod/",
    "modloader/Vanilla + roads/",
    "modloader/Proper Shaders/",
];

const PROJECT2DFX_FILES: &[&str] = &["SALodLights.asi", "SALodLights.dat", "SALodLights.ini"];

/// Fichiers de réglage/log qu'ENB peut générer à l'exécution et qui ne figurent
/// donc pas nécessairement dans l'inventaire écrit avant le lancement du jeu.
const HD_RUNTIME_ORPHANS: &[&str] = &[
    "enbbloom.fx.ini",
    "enbdepthoffield.fx.ini",
    "enbeffect.fx.ini",
    "enbeffectprepass.fx.ini",
    "enbenvmap.fx.ini",
    "enblens.fx.ini",
    "enblighting.fx.ini",
    "enbsky.fx.ini",
    "enbunderwater.fx.ini",
    "enbvehicle.fx.ini",
    "enbwater.fx.ini",
    "enbseries.log",
];

#[derive(Debug, Clone, Serialize)]
pub struct EnbPrepareResult {
    /// Indique si le moteur graphique HD est actif (les contenus permanents sont
    /// déployés dans les deux cas).
    pub applied: bool,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
struct HdComponentManifest {
    name: String,
    url: String,
    sha256: String,
    cache_key: String,
    archive_prefix: String,
    #[serde(default)]
    destination: Option<String>,
    #[serde(default)]
    include: Vec<String>,
}

pub fn staging_dir(gta_root: &Path) -> PathBuf {
    gta_root.join(ENB_STAGING_REL)
}

pub fn is_pack_installed(gta_root: &Path) -> bool {
    let staging = staging_dir(gta_root);
    staging.is_dir()
        && fs::read_dir(&staging)
            .map(|mut entries| entries.any(|e| e.is_ok()))
            .unwrap_or(false)
}

/// Prépare le jeu avant chaque lancement. Les contenus permanents sont toujours
/// actifs ; `hd_enabled` ne contrôle que les chemins graphiques déclarés.
pub fn prepare(gta_root: &Path, hd_enabled: bool) -> Result<EnbPrepareResult> {
    let staging = staging_dir(gta_root);
    if !is_pack_installed(gta_root) {
        return Ok(EnbPrepareResult {
            applied: false,
            message: "Pack GTRP introuvable — lance d'abord une mise à jour du modpack.".into(),
        });
    }

    let rules = load_hd_rules(&staging)?;
    let component = load_hd_component_manifest(&staging)?;
    let component_payload = if hd_enabled {
        component
            .as_ref()
            .map(|manifest| ensure_hd_component(gta_root, manifest))
            .transpose()?
    } else {
        component
            .as_ref()
            .map(|manifest| component_payload_dir(gta_root, manifest))
            .filter(|path| path.is_dir())
    };

    // Retire le déploiement précédent (y compris un ancien pack monolithique),
    // puis reconstruit l'état voulu de façon déterministe.
    let component_destination = component
        .as_ref()
        .map(|manifest| component_destination(gta_root, manifest))
        .transpose()?;
    cleanup_previous_deployment(
        gta_root,
        &staging,
        component_payload.as_deref(),
        component_destination.as_deref(),
    )?;
    purge_project2dfx_orphans(gta_root);

    let mut deployed = Vec::new();

    // Le composant officiel est copié d'abord ; le preset GTRP présent dans le
    // staging est ensuite copié par-dessus.
    if hd_enabled {
        if let Some(payload) = component_payload.as_deref() {
            let destination = component_destination
                .as_deref()
                .ok_or_else(|| {
                    LauncherError::Integrity("destination du composant HD absente".into())
                })?;
            copy_tree(payload, destination, gta_root, &mut deployed)?;
        }
    }

    copy_staging_files(
        &staging,
        gta_root,
        &staging,
        &rules,
        hd_enabled,
        &mut deployed,
    )?;

    if hd_enabled {
        deploy_project2dfx(gta_root, &staging)?;
    }

    // Le loader et modloader restent actifs, même lorsque le moteur HD est coupé.
    install_asi_loader(gta_root, &staging)?;
    write_deployment_marker(gta_root, hd_enabled, &deployed)?;

    let message = if hd_enabled {
        "Graphismes HD activés ; véhicules, skins, sons et interface chargés.".into()
    } else {
        "Graphismes HD désactivés ; véhicules, skins, sons et interface restent actifs.".into()
    };

    Ok(EnbPrepareResult {
        applied: hd_enabled,
        message,
    })
}

fn load_hd_rules(staging: &Path) -> Result<Vec<String>> {
    let mut rules = DEFAULT_HD_PATHS
        .iter()
        .map(|rule| normalize_rule(rule))
        .collect::<Result<Vec<_>>>()?;

    let path = staging.join(HD_PATHS_FILE);
    if let Ok(content) = fs::read_to_string(path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let normalized = normalize_rule(line)?;
            if !rules.iter().any(|rule| rule == &normalized) {
                rules.push(normalized);
            }
        }
    }
    Ok(rules)
}

fn normalize_rule(rule: &str) -> Result<String> {
    let had_trailing_slash = rule.trim().ends_with(['/', '\\']);
    let normalized = rule
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    validate_relative_str(&normalized)?;
    let mut normalized = normalized.to_ascii_lowercase();
    if had_trailing_slash && !normalized.ends_with('/') {
        normalized.push('/');
    }
    Ok(normalized)
}

fn validate_relative_str(path: &str) -> Result<()> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains(':')
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(LauncherError::Integrity(format!(
            "chemin de mod non autorisé : {path}"
        )));
    }
    Ok(())
}

fn relative_string(path: &Path, base: &Path) -> Result<String> {
    let rel = path
        .strip_prefix(base)
        .map_err(|_| LauncherError::Io("chemin de mod invalide".into()))?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

fn is_metadata_path(rel: &str) -> bool {
    rel.eq_ignore_ascii_case(HD_PATHS_FILE) || rel.eq_ignore_ascii_case(HD_COMPONENT_FILE)
}

fn is_hd_path(rel: &str, rules: &[String]) -> bool {
    let rel = rel.replace('\\', "/").to_ascii_lowercase();
    rules.iter().any(|rule| {
        if rule.ends_with('/') {
            rel.starts_with(rule)
        } else {
            rel == *rule
        }
    })
}

fn copy_staging_files(
    src: &Path,
    game_root: &Path,
    src_base: &Path,
    rules: &[String],
    hd_enabled: bool,
    deployed: &mut Vec<String>,
) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            copy_staging_files(&path, game_root, src_base, rules, hd_enabled, deployed)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }

        let rel = relative_string(&path, src_base)?;
        if is_metadata_path(&rel) || (!hd_enabled && is_hd_path(&rel, rules)) {
            continue;
        }
        let dest = game_root.join(&rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&path, &dest)?;
        deployed.push(rel);
    }
    Ok(())
}

/// Copie un composant déjà extrait et ajoute ses chemins relatifs à l'inventaire.
fn copy_tree(
    src: &Path,
    dst: &Path,
    game_root: &Path,
    deployed: &mut Vec<String>,
) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &target, game_root, deployed)?;
        } else if path.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &target)?;
            let rel = target
                .strip_prefix(game_root)
                .map_err(|_| LauncherError::Io("chemin de composant HD invalide".into()))?;
            let rel = rel.to_string_lossy().replace('\\', "/");
            validate_relative_str(&rel)?;
            deployed.push(rel);
        }
    }
    Ok(())
}

fn load_hd_component_manifest(staging: &Path) -> Result<Option<HdComponentManifest>> {
    let path = staging.join(HD_COMPONENT_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    let manifest: HdComponentManifest = serde_json::from_str(&content)?;
    validate_component_manifest(&manifest)?;
    Ok(Some(manifest))
}

fn validate_component_manifest(manifest: &HdComponentManifest) -> Result<()> {
    let trusted_url = manifest.url.starts_with("https://")
        || manifest.url == "http://enbdev.com/enbseries_gtasa_v0430.zip";
    if manifest.name.trim().is_empty()
        || !trusted_url
        || manifest.sha256.len() != 64
        || !manifest.sha256.chars().all(|c| c.is_ascii_hexdigit())
        || manifest.cache_key.is_empty()
        || manifest.cache_key.starts_with('.')
        || !manifest
            .cache_key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(LauncherError::Integrity(
            "description du composant HD invalide".into(),
        ));
    }
    normalize_archive_prefix(&manifest.archive_prefix)?;
    component_destination(Path::new("."), manifest)?;
    normalize_component_includes(&manifest.include)?;
    Ok(())
}

fn component_destination(gta_root: &Path, manifest: &HdComponentManifest) -> Result<PathBuf> {
    match manifest.destination.as_deref() {
        // Compatibilité avec les descripteurs Proper Shaders déjà distribués.
        None => Ok(gta_root.join("modloader").join("Proper Shaders")),
        Some(value) if value.trim().is_empty() || value.trim() == "." => {
            Ok(gta_root.to_path_buf())
        }
        Some(value) => {
            let normalized = value
                .trim()
                .replace('\\', "/")
                .trim_matches('/')
                .to_string();
            validate_relative_str(&normalized)?;
            Ok(gta_root.join(normalized))
        }
    }
}

fn normalize_component_includes(includes: &[String]) -> Result<Vec<String>> {
    includes.iter().map(|rule| normalize_rule(rule)).collect()
}

fn component_path_is_included(rel: &str, includes: &[String]) -> bool {
    if includes.is_empty() {
        return true;
    }
    let rel = rel
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase();
    includes.iter().any(|rule| {
        if rule.ends_with('/') {
            let prefix = rule.trim_end_matches('/');
            rel == prefix || rel.starts_with(rule.as_str())
        } else {
            rel == rule.as_str()
        }
    })
}

fn normalize_archive_prefix(prefix: &str) -> Result<String> {
    let prefix = prefix
        .trim()
        .replace('\\', "/")
        .trim_matches('/')
        .to_string();
    validate_relative_str(&prefix)?;
    Ok(format!("{prefix}/"))
}

fn components_root(gta_root: &Path) -> PathBuf {
    gta_root.join("gtrp-assets").join("components")
}

fn component_payload_dir(gta_root: &Path, manifest: &HdComponentManifest) -> PathBuf {
    components_root(gta_root)
        .join(&manifest.cache_key)
        .join("content")
}

fn ensure_hd_component(gta_root: &Path, manifest: &HdComponentManifest) -> Result<PathBuf> {
    let root = components_root(gta_root);
    let downloads = root.join("downloads");
    fs::create_dir_all(&downloads)?;
    let archive_path = downloads.join(format!("{}.zip", manifest.cache_key));

    let archive_valid = archive_path.is_file()
        && crate::updater::sha256_file(&archive_path)
            .map(|hash| hash.eq_ignore_ascii_case(&manifest.sha256))
            .unwrap_or(false);
    if !archive_valid {
        let _ = fs::remove_file(&archive_path);
        crate::updater::download_verify(&manifest.url, &archive_path, &manifest.sha256, |_| {})?;
    }

    let component_root = root.join(&manifest.cache_key);
    let payload = component_root.join("content");
    let ready_marker = component_root.join(".source_sha256");
    let ready = fs::read_to_string(&ready_marker)
        .map(|hash| hash.trim().eq_ignore_ascii_case(&manifest.sha256))
        .unwrap_or(false)
        && payload.is_dir();
    if ready {
        return Ok(payload);
    }

    if component_root.exists() {
        fs::remove_dir_all(&component_root)?;
    }
    fs::create_dir_all(&payload)?;
    fs::create_dir_all(component_root.join("license"))?;

    let file = fs::File::open(&archive_path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        LauncherError::Integrity(format!("archive {} invalide : {e}", manifest.name))
    })?;
    let prefix = normalize_archive_prefix(&manifest.archive_prefix)?;
    let includes = normalize_component_includes(&manifest.include)?;
    let mut extracted_files = 0usize;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| LauncherError::Integrity(format!("lecture {} : {e}", manifest.name)))?;
        let name = entry.name().replace('\\', "/");

        if let Some(rel) = name.strip_prefix(&prefix) {
            if rel.is_empty() {
                continue;
            }
            validate_relative_str(rel.trim_end_matches('/'))?;
            if !component_path_is_included(rel, &includes) {
                continue;
            }
            let destination = payload.join(rel);
            if entry.is_dir() || name.ends_with('/') {
                fs::create_dir_all(&destination)?;
            } else {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut out = fs::File::create(&destination)?;
                std::io::copy(&mut entry, &mut out)?;
                out.flush()?;
                extracted_files += 1;
            }
            continue;
        }

        // Conserve les textes d'origine à côté du cache, sans les injecter dans
        // la racine du jeu ni les altérer.
        let base_name = Path::new(&name).file_name().and_then(|v| v.to_str());
        if matches!(
            base_name,
            Some(
                "License.txt"
                    | "LICENSE"
                    | "Readme.txt"
                    | "README.txt"
                    | "Readme (or die).txt"
                    | "Third Party.txt"
            )
        ) && !entry.is_dir()
        {
            let destination = component_root.join("license").join(base_name.unwrap());
            let mut out = fs::File::create(destination)?;
            std::io::copy(&mut entry, &mut out)?;
            out.flush()?;
        }
    }

    if extracted_files == 0 {
        let _ = fs::remove_dir_all(&component_root);
        return Err(LauncherError::Integrity(format!(
            "{} ne contient aucun fichier sous {}",
            manifest.name, prefix
        )));
    }

    fs::write(&ready_marker, manifest.sha256.as_bytes())?;
    Ok(payload)
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

fn install_asi_loader(gta_root: &Path, staging: &Path) -> Result<()> {
    let src = staging.join(LOADER_SRC_NAME);
    if !src.is_file() {
        return Ok(());
    }

    let target = gta_root.join(LOADER_TARGET_NAME);
    let backup = gta_root.join(LOADER_BACKUP_NAME);
    if !backup.exists() && target.is_file() && !files_equal(&target, &src) {
        fs::rename(&target, &backup)?;
    }
    fs::copy(&src, &target)?;
    Ok(())
}

fn uninstall_asi_loader(gta_root: &Path, staging: &Path) {
    let src = staging.join(LOADER_SRC_NAME);
    let target = gta_root.join(LOADER_TARGET_NAME);
    let backup = gta_root.join(LOADER_BACKUP_NAME);

    if target.is_file() {
        let is_ours = src.is_file() && files_equal(&target, &src);
        if is_ours || backup.is_file() {
            let _ = fs::remove_file(&target);
        }
    }
    if backup.is_file() && !target.exists() {
        let _ = fs::rename(&backup, &target);
    }
}

pub fn undeploy(gta_root: &Path) -> Result<()> {
    let staging = staging_dir(gta_root);
    let manifest = load_hd_component_manifest(&staging).ok().flatten();
    let component = manifest
        .as_ref()
        .map(|manifest| component_payload_dir(gta_root, manifest))
        .filter(|path| path.is_dir());
    let destination = manifest
        .as_ref()
        .and_then(|manifest| component_destination(gta_root, manifest).ok());
    cleanup_previous_deployment(
        gta_root,
        &staging,
        component.as_deref(),
        destination.as_deref(),
    )
}

fn cleanup_previous_deployment(
    gta_root: &Path,
    staging: &Path,
    component_payload: Option<&Path>,
    component_destination: Option<&Path>,
) -> Result<()> {
    let marker = gta_root.join(ENB_MARKER);
    if !marker.is_file() {
        purge_hd_runtime_orphans(gta_root);
        return Ok(());
    }

    let mut inventory_found = false;
    let mut empty_dir_candidates = Vec::new();
    if let Ok(content) = fs::read_to_string(&marker) {
        for line in content.lines() {
            let Some(rel) = line.strip_prefix("file=") else {
                continue;
            };
            validate_relative_str(rel)?;
            let target = gta_root.join(rel);
            if target.is_file() {
                let _ = fs::remove_file(&target);
            }
            let mut parent = target.parent();
            while let Some(directory) = parent {
                if directory == gta_root || !directory.starts_with(gta_root) {
                    break;
                }
                empty_dir_candidates.push(directory.to_path_buf());
                parent = directory.parent();
            }
            inventory_found = true;
        }
    }

    // Supprime uniquement les dossiers devenus vides, du plus profond au plus
    // proche de la racine. `remove_dir` laisse intacts les dossiers contenant
    // des fichiers qui ne sont pas gérés par le launcher.
    empty_dir_candidates.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    empty_dir_candidates.dedup();
    for directory in empty_dir_candidates {
        let _ = fs::remove_dir(directory);
    }

    // Migration depuis le marqueur v1 (« 1 »), sans inventaire.
    if !inventory_found {
        if staging.is_dir() {
            remove_tree_targets(staging, gta_root, staging)?;
        }
        if let (Some(payload), Some(destination)) = (component_payload, component_destination) {
            remove_tree_targets(payload, destination, payload)?;
        }
    }

    purge_project2dfx_orphans(gta_root);
    purge_hd_runtime_orphans(gta_root);
    uninstall_asi_loader(gta_root, staging);
    let _ = fs::remove_file(marker);
    Ok(())
}

fn write_deployment_marker(gta_root: &Path, hd_enabled: bool, deployed: &[String]) -> Result<()> {
    let mut lines = String::from("version=2\n");
    lines.push_str(if hd_enabled { "hd=1\n" } else { "hd=0\n" });
    for rel in deployed {
        validate_relative_str(rel)?;
        lines.push_str("file=");
        lines.push_str(rel);
        lines.push('\n');
    }
    fs::write(gta_root.join(ENB_MARKER), lines)?;
    Ok(())
}

fn remove_tree_targets(src: &Path, dst_root: &Path, src_base: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path
            .strip_prefix(src_base)
            .map_err(|_| LauncherError::Io("chemin de mod invalide".into()))?;
        let target = dst_root.join(rel);
        if path.is_dir() {
            remove_tree_targets(&path, dst_root, src_base)?;
            if target.is_dir() {
                let _ = fs::remove_dir(&target);
            }
        } else if path.is_file() && target.is_file() {
            let _ = fs::remove_file(&target);
        }
    }
    Ok(())
}

fn purge_project2dfx_orphans(gta_root: &Path) {
    for name in PROJECT2DFX_FILES {
        let _ = fs::remove_file(gta_root.join(name));
    }
}

fn deploy_project2dfx(gta_root: &Path, staging: &Path) -> Result<()> {
    if !staging.join("SALodLights.asi").is_file() {
        return Ok(());
    }
    for name in PROJECT2DFX_FILES {
        let src = staging.join(name);
        if !src.is_file() {
            return Err(LauncherError::Other(format!(
                "Project2DFX incomplet dans le modpack : {name} manquant"
            )));
        }
        let dst = gta_root.join(name);
        fs::copy(&src, &dst)?;
        if !dst.is_file() {
            return Err(LauncherError::Other(format!(
                "Échec du déploiement de Project2DFX : {name}"
            )));
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

    fn create_split_pack(game: &Path) -> PathBuf {
        let staging = staging_dir(game);
        fs::create_dir_all(staging.join("modloader/Vehicles")).unwrap();
        fs::write(staging.join("d3d9.dll"), b"fake-hd").unwrap();
        fs::write(staging.join("modloader/Vehicles/car.dff"), b"always").unwrap();
        fs::write(staging.join(HD_PATHS_FILE), b"d3d9.dll\n").unwrap();
        staging
    }

    #[test]
    fn hd_toggle_keeps_permanent_content_active() {
        let game = tmp("split");
        let staging = create_split_pack(&game);

        let enabled = prepare(&game, true).unwrap();
        assert!(enabled.applied);
        assert!(game.join("d3d9.dll").is_file());
        assert!(game.join("modloader/Vehicles/car.dff").is_file());

        let disabled = prepare(&game, false).unwrap();
        assert!(!disabled.applied);
        assert!(!game.join("d3d9.dll").exists());
        assert!(game.join("modloader/Vehicles/car.dff").is_file());
        assert!(game.join(ENB_MARKER).is_file());
        assert!(!game.join(HD_PATHS_FILE).exists());

        undeploy(&game).unwrap();
        assert!(!game.join("modloader/Vehicles/car.dff").exists());
        assert!(!game.join(ENB_MARKER).exists());
        assert!(staging.join("modloader/Vehicles/car.dff").is_file());
        let _ = fs::remove_dir_all(&game);
    }

    #[test]
    fn prepare_without_pack_is_non_fatal() {
        let game = tmp("nopack");
        let result = prepare(&game, true).unwrap();
        assert!(!result.applied);
        let _ = fs::remove_dir_all(&game);
    }

    #[test]
    fn project2dfx_is_hd_only() {
        let game = tmp("p2dfx");
        let staging = create_split_pack(&game);
        for (name, body) in [
            ("SALodLights.asi", b"asi"),
            ("SALodLights.dat", b"dat"),
            ("SALodLights.ini", b"ini"),
        ] {
            fs::write(staging.join(name), body).unwrap();
        }

        prepare(&game, true).unwrap();
        assert!(game.join("SALodLights.asi").is_file());
        prepare(&game, false).unwrap();
        assert!(!game.join("SALodLights.asi").exists());
        let _ = fs::remove_dir_all(&game);
    }

    #[test]
    fn asi_loader_stays_active_until_full_undeploy() {
        let game = tmp("loader");
        let staging = create_split_pack(&game);
        fs::write(staging.join(LOADER_SRC_NAME), b"ULTIMATE-ASI-LOADER").unwrap();
        fs::write(game.join(LOADER_TARGET_NAME), b"ORIGINAL-VORBIS-AUDIO").unwrap();

        prepare(&game, true).unwrap();
        assert_eq!(
            fs::read(game.join(LOADER_TARGET_NAME)).unwrap(),
            b"ULTIMATE-ASI-LOADER"
        );
        prepare(&game, false).unwrap();
        assert_eq!(
            fs::read(game.join(LOADER_TARGET_NAME)).unwrap(),
            b"ULTIMATE-ASI-LOADER"
        );
        assert!(game.join(LOADER_BACKUP_NAME).is_file());

        undeploy(&game).unwrap();
        assert!(!game.join(LOADER_BACKUP_NAME).exists());
        assert_eq!(
            fs::read(game.join(LOADER_TARGET_NAME)).unwrap(),
            b"ORIGINAL-VORBIS-AUDIO"
        );
        let _ = fs::remove_dir_all(&game);
    }

    #[test]
    fn official_component_is_verified_extracted_and_hd_only() {
        let game = tmp("component");
        let staging = create_split_pack(&game);
        fs::create_dir_all(staging.join("modloader/Proper Shaders")).unwrap();
        fs::write(
            staging.join("modloader/Proper Shaders/ProperShaders.ini"),
            b"GTRP-PRESET",
        )
        .unwrap();

        let downloads = components_root(&game).join("downloads");
        fs::create_dir_all(&downloads).unwrap();
        let archive_path = downloads.join("proper-test.zip");
        let archive_file = fs::File::create(&archive_path).unwrap();
        let mut archive = zip::ZipWriter::new(archive_file);
        archive
            .start_file(
                "Proper Shaders/ProperShaders.asi",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(b"OFFICIAL-BINARY").unwrap();
        archive
            .start_file(
                "Proper Shaders/ProperShaders.ini",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(b"UPSTREAM-PRESET").unwrap();
        archive
            .start_file("License.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"UPSTREAM-LICENSE").unwrap();
        archive.finish().unwrap();

        let sha = crate::updater::sha256_file(&archive_path).unwrap();
        let descriptor = serde_json::json!({
            "name": "Proper test",
            "url": "https://invalid.example/proper.zip",
            "sha256": sha,
            "cache_key": "proper-test",
            "archive_prefix": "Proper Shaders/"
        });
        fs::write(
            staging.join(HD_COMPONENT_FILE),
            serde_json::to_vec_pretty(&descriptor).unwrap(),
        )
        .unwrap();

        prepare(&game, true).unwrap();
        assert_eq!(
            fs::read(game.join("modloader/Proper Shaders/ProperShaders.asi")).unwrap(),
            b"OFFICIAL-BINARY"
        );
        // Le preset additionnel GTRP doit gagner sur le preset de l'archive.
        assert_eq!(
            fs::read(game.join("modloader/Proper Shaders/ProperShaders.ini")).unwrap(),
            b"GTRP-PRESET"
        );
        assert_eq!(
            fs::read(components_root(&game).join("proper-test/license/License.txt")).unwrap(),
            b"UPSTREAM-LICENSE"
        );

        prepare(&game, false).unwrap();
        assert!(!game
            .join("modloader/Proper Shaders/ProperShaders.asi")
            .exists());
        assert!(!game
            .join("modloader/Proper Shaders/ProperShaders.ini")
            .exists());
        assert!(game.join("modloader/Vehicles/car.dff").is_file());
        // Le cache officiel est conservé pour une réactivation instantanée.
        assert!(archive_path.is_file());
        let _ = fs::remove_dir_all(&game);
    }

    #[test]
    fn component_can_deploy_selected_files_to_game_root() {
        let game = tmp("root_component");
        let staging = create_split_pack(&game);
        fs::write(staging.join("enbseries.ini"), b"GTRP-PRESET").unwrap();
        fs::write(
            staging.join(HD_PATHS_FILE),
            b"enbseries.asi\nenbseries.ini\nenbseries/\n",
        )
        .unwrap();

        let downloads = components_root(&game).join("downloads");
        fs::create_dir_all(&downloads).unwrap();
        let archive_path = downloads.join("enb-test.zip");
        let archive_file = fs::File::create(&archive_path).unwrap();
        let mut archive = zip::ZipWriter::new(archive_file);
        for (name, body) in [
            ("Pack/enbseries.asi", &b"ENB-BINARY"[..]),
            ("Pack/enbseries/enbhelper.dll", &b"ENB-HELPER"[..]),
            ("Pack/SilentPatchSA.asi", &b"UNWANTED"[..]),
        ] {
            archive
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            archive.write_all(body).unwrap();
        }
        archive.finish().unwrap();

        let sha = crate::updater::sha256_file(&archive_path).unwrap();
        let descriptor = serde_json::json!({
            "name": "ENB test",
            "url": "https://invalid.example/enb.zip",
            "sha256": sha,
            "cache_key": "enb-test",
            "archive_prefix": "Pack/",
            "destination": "",
            "include": ["enbseries.asi", "enbseries/"]
        });
        fs::write(
            staging.join(HD_COMPONENT_FILE),
            serde_json::to_vec_pretty(&descriptor).unwrap(),
        )
        .unwrap();

        prepare(&game, true).unwrap();
        assert_eq!(fs::read(game.join("enbseries.asi")).unwrap(), b"ENB-BINARY");
        assert_eq!(
            fs::read(game.join("enbseries/enbhelper.dll")).unwrap(),
            b"ENB-HELPER"
        );
        assert_eq!(fs::read(game.join("enbseries.ini")).unwrap(), b"GTRP-PRESET");
        assert!(!game.join("SilentPatchSA.asi").exists());
        fs::write(game.join("enbseries.log"), b"runtime log").unwrap();
        fs::write(game.join("enbeffect.fx.ini"), b"runtime settings").unwrap();

        prepare(&game, false).unwrap();
        assert!(!game.join("enbseries.asi").exists());
        assert!(!game.join("enbseries.ini").exists());
        assert!(!game.join("enbseries").exists());
        assert!(!game.join("enbseries.log").exists());
        assert!(!game.join("enbeffect.fx.ini").exists());
        assert!(game.join("modloader/Vehicles/car.dff").is_file());
        let _ = fs::remove_dir_all(&game);
    }

    #[test]
    fn only_the_pinned_enbdev_http_archive_is_allowed() {
        let base = HdComponentManifest {
            name: "ENB".into(),
            url: "http://enbdev.com/enbseries_gtasa_v0430.zip".into(),
            sha256: "a".repeat(64),
            cache_key: "enb-test".into(),
            archive_prefix: "WrapperVersion/".into(),
            destination: Some(String::new()),
            include: vec!["d3d9.dll".into()],
        };
        validate_component_manifest(&base).unwrap();

        let mut untrusted = base;
        untrusted.url = "http://example.com/enbseries_gtasa_v0430.zip".into();
        assert!(validate_component_manifest(&untrusted).is_err());
    }
}

fn purge_hd_runtime_orphans(gta_root: &Path) {
    for name in HD_RUNTIME_ORPHANS {
        let _ = fs::remove_file(gta_root.join(name));
    }
}
