//! Logique métier du launcher GTRP, indépendante de l'interface (Tauri).
//!
//! Cette crate ne dépend d'aucune bibliothèque graphique : elle est donc
//! compilable et testable sur n'importe quelle plateforme (Linux/CI inclus).

pub mod config;
pub mod enb;
pub mod error;
pub mod gta;
pub mod laa;
pub mod launch;
pub mod news;
pub mod query;
pub mod samp_cache;
pub mod settings;
pub mod updater;

pub use error::{LauncherError, Result};
