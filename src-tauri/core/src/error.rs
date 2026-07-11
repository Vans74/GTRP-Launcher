//! Type d'erreur unifié, sérialisable vers le frontend.
//!
//! Toutes les commandes Tauri renvoient `Result<T, LauncherError>` afin que
//! l'interface reçoive un message clair et exploitable plutôt qu'un plantage.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum LauncherError {
    #[error("Jeu introuvable : {0}")]
    GameNotFound(String),

    #[error("Serveur injoignable : {0}")]
    ServerUnreachable(String),

    #[error("Erreur réseau : {0}")]
    Network(String),

    #[error("Fichier corrompu ou invalide : {0}")]
    Integrity(String),

    #[error("Erreur d'entrée/sortie : {0}")]
    Io(String),

    #[error("Erreur de configuration : {0}")]
    Config(String),

    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for LauncherError {
    fn from(e: std::io::Error) -> Self {
        LauncherError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for LauncherError {
    fn from(e: serde_json::Error) -> Self {
        LauncherError::Other(format!("JSON : {e}"))
    }
}

// Sérialisation en simple chaîne pour le frontend.
impl Serialize for LauncherError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, LauncherError>;
