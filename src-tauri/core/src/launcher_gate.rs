//! Préautorisation courte consommée par le serveur lors de la connexion SA-MP.
//!
//! Cette couche bloque les connexions directes ordinaires. Elle complète le
//! contrôle local mais ne le remplace pas : une clé présente côté client peut
//! toujours être extraite par un attaquant suffisamment avancé.

use crate::config;
use crate::error::{LauncherError, Result};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

const CLIENT_KEY: Option<&str> = option_env!("GTRP_GATE_CLIENT_KEY");

#[derive(Serialize)]
struct GateRequest<'a> {
    nickname: &'a str,
    timestamp: u64,
    nonce: String,
    generation: u64,
    launcher_version: &'a str,
    proof: String,
}

#[derive(Deserialize)]
struct GateResponse {
    authorized: bool,
    expires_at: u64,
}

fn unix_time() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| LauncherError::Other("horloge système invalide".into()))
}

fn proof(
    key: &[u8],
    nickname: &str,
    timestamp: u64,
    nonce: &str,
    generation: u64,
    launcher_version: &str,
) -> Result<String> {
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|_| LauncherError::Integrity("clé de préautorisation invalide".into()))?;
    mac.update(nickname.as_bytes());
    mac.update(b"\n");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b"\n");
    mac.update(nonce.as_bytes());
    mac.update(b"\n");
    mac.update(generation.to_string().as_bytes());
    mac.update(b"\n");
    mac.update(launcher_version.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// Autorise ce pseudo et l'IP publique observée par le serveur web pendant une
/// courte fenêtre. L'appel est effectué alors que le watcher local est armé.
pub fn authorize(nickname: &str, generation: u64) -> Result<()> {
    let key = CLIENT_KEY
        .filter(|value| value.len() >= 32)
        .ok_or_else(|| {
            LauncherError::Integrity(
                "launcher non habilité à produire une préautorisation serveur".into(),
            )
        })?
        .as_bytes();
    let timestamp = unix_time()?;
    let mut nonce_bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = hex::encode(nonce_bytes);
    let launcher_version = env!("CARGO_PKG_VERSION");
    let request = GateRequest {
        nickname,
        timestamp,
        nonce: nonce.clone(),
        generation,
        launcher_version,
        proof: proof(
            key,
            nickname,
            timestamp,
            &nonce,
            generation,
            launcher_version,
        )?,
    };

    let request_body = serde_json::to_string(&request)
        .map_err(|error| LauncherError::Other(format!("préautorisation : {error}")))?;
    let response = ureq::post(config::LAUNCH_GATE_URL)
        .timeout(std::time::Duration::from_secs(8))
        .set("Content-Type", "application/json")
        .set(
            "User-Agent",
            &format!("GTRP-Launcher/{launcher_version}"),
        )
        .send_string(&request_body)
        .map_err(|error| {
            LauncherError::Network(format!("préautorisation serveur refusée : {error}"))
        })?;
    let response_body = response.into_string().map_err(|error| {
        LauncherError::Network(format!("réponse de préautorisation invalide : {error}"))
    })?;
    let response: GateResponse = serde_json::from_str(&response_body).map_err(|error| {
        LauncherError::Network(format!("réponse de préautorisation invalide : {error}"))
    })?;
    let now = unix_time()?;
    if !response.authorized || response.expires_at <= now {
        return Err(LauncherError::Integrity(
            "préautorisation serveur expirée ou refusée".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_is_stable_and_binds_every_field() {
        let key = b"0123456789abcdef0123456789abcdef";
        let first = proof(key, "Test_Player", 123, "abcd", 1, "0.1.18").unwrap();
        let same = proof(key, "Test_Player", 123, "abcd", 1, "0.1.18").unwrap();
        let changed = proof(key, "Test_Player", 123, "abce", 1, "0.1.18").unwrap();
        assert_eq!(first, same);
        assert_ne!(first, changed);
        assert_eq!(
            first,
            "482462bf95fde91c7c63ba9522df5c06acd3d098c214f9deb305eea096c6f949"
        );
    }
}
