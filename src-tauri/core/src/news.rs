//! Récupération des actualités / changelog du serveur (fichier `news.json` distant).

use crate::error::{LauncherError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewsItem {
    pub title: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub image: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct NewsFeed {
    #[serde(default)]
    pub items: Vec<NewsItem>,
}

/// Récupère le flux de news. En cas d'erreur réseau, renvoie un flux vide plutôt
/// que de bloquer l'affichage du launcher.
pub fn fetch_news(url: &str) -> Result<NewsFeed> {
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(|e| LauncherError::Network(format!("news : {e}")))?;
    let text = resp
        .into_string()
        .map_err(|e| LauncherError::Network(format!("lecture news : {e}")))?;
    let feed: NewsFeed = serde_json::from_str(&text)?;
    Ok(feed)
}
