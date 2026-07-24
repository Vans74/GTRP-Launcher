//! Configuration statique du serveur GTRP.
//!
//! Ces valeurs sont volontairement centralisées ici : il suffit de les modifier
//! pour réutiliser le launcher sur une autre adresse ou un autre CDN.

use serde::Serialize;

/// Adresse publique du serveur SA-MP à laquelle les joueurs se connectent.
/// Peut être un domaine (résolu automatiquement) ou une IP.
pub const SERVER_HOST: &str = "51.255.92.237";

/// Port du serveur SA-MP (voir server.cfg -> `port`).
pub const SERVER_PORT: u16 = 3400;

/// Nom affiché dans le launcher.
pub const SERVER_NAME: &str = "Grand Theft RolePlay";

/// Site web officiel.
pub const WEB_URL: &str = "https://gtrp.fr";

/// Invitation Discord (à adapter).
pub const DISCORD_URL: &str = "https://discord.gg/gtrp";

/// URL de base des ressources distantes (manifest + fichiers du modpack + news).
/// Le launcher ira chercher `{ASSET_BASE_URL}/manifest.json` et `{ASSET_BASE_URL}/news.json`.
///
/// IMPORTANT : cette URL pointe vers un tag GitHub *permanent* (`modpack`), et non
/// vers une version figee. Pour publier un nouveau modpack, il suffit de remplacer
/// les assets (manifest.json + zip) de la release `modpack` : le launcher deja
/// installe chez les joueurs recuperera la mise a jour sans etre reconstruit.
pub const ASSET_BASE_URL: &str =
    "https://github.com/Vans74/GTRP-Launcher/releases/download/modpack";

/// Catalogue dynamique de tous les DFF/TXD déclarés dans models/artconfig.txt.
/// Il est régénéré atomiquement à chaque démarrage du serveur SA-MP.
pub const ARTWORK_MANIFEST_URL: &str =
    "https://afterhours-nsfw.fr/static/gtrp-artwork/manifest.json";
pub const ARTWORK_FILES_BASE_URL: &str =
    "https://afterhours-nsfw.fr/static/gtrp-artwork/files";
pub const ARTWORK_BUNDLES_BASE_URL: &str =
    "https://afterhours-nsfw.fr/static/gtrp-artwork/bundles";

/// Préautorisation à usage unique consommée par le serveur SA-MP.
pub const LAUNCH_GATE_URL: &str = "https://afterhours-nsfw.fr/api/gtrp-launch/issue";

#[derive(Debug, Clone, Serialize)]
pub struct PublicConfig {
    pub server_name: String,
    pub server_host: String,
    pub server_port: u16,
    pub web_url: String,
    pub discord_url: String,
    pub asset_base_url: String,
    pub launcher_version: String,
}

/// Renvoie la configuration exposée au frontend.
pub fn public_config() -> PublicConfig {
    PublicConfig {
        server_name: SERVER_NAME.to_string(),
        server_host: SERVER_HOST.to_string(),
        server_port: SERVER_PORT,
        web_url: WEB_URL.to_string(),
        discord_url: DISCORD_URL.to_string(),
        asset_base_url: ASSET_BASE_URL.to_string(),
        launcher_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}
