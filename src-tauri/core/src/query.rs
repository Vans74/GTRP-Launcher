//! Client du protocole de query SA-MP (UDP).
//!
//! Permet de récupérer en direct l'état du serveur : en ligne/hors ligne,
//! nombre de joueurs, gamemode, langue, ping. Aucune dépendance externe :
//! `std::net::UdpSocket` avec timeout, ce qui est robuste et sans surprise.
//!
//! Format d'une requête « info » :
//!   "SAMP" (4o) + IP (4o) + port (2o, little-endian) + 'i'
//! Réponse (après ré-écho de l'en-tête de 11 octets) :
//!   password(1o) players(u16) maxplayers(u16)
//!   hostname(len u32 + bytes) gamemode(len u32 + bytes) language(len u32 + bytes)
//! Tous les entiers sont en little-endian.

use crate::error::{LauncherError, Result};
use serde::Serialize;
use std::io::{Cursor, Read};
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct ServerStatus {
    pub online: bool,
    pub password: bool,
    pub players: u16,
    pub max_players: u16,
    pub hostname: String,
    pub gamemode: String,
    pub language: String,
    pub ping_ms: u32,
}

impl ServerStatus {
    fn offline() -> Self {
        ServerStatus {
            online: false,
            password: false,
            players: 0,
            max_players: 0,
            hostname: String::new(),
            gamemode: String::new(),
            language: String::new(),
            ping_ms: 0,
        }
    }
}

fn resolve_ipv4(host: &str, port: u16) -> Result<(Ipv4Addr, SocketAddr)> {
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| LauncherError::ServerUnreachable(format!("résolution DNS de {host} : {e}")))?;
    for addr in addrs {
        if let SocketAddr::V4(v4) = addr {
            return Ok((*v4.ip(), addr));
        }
    }
    Err(LauncherError::ServerUnreachable(format!(
        "aucune adresse IPv4 pour {host}"
    )))
}

fn build_info_packet(ip: Ipv4Addr, port: u16) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(11);
    pkt.extend_from_slice(b"SAMP");
    pkt.extend_from_slice(&ip.octets());
    pkt.extend_from_slice(&port.to_le_bytes());
    pkt.push(b'i');
    pkt
}

fn read_u16_le(cur: &mut Cursor<&[u8]>) -> Result<u16> {
    let mut b = [0u8; 2];
    cur.read_exact(&mut b)
        .map_err(|_| LauncherError::ServerUnreachable("réponse tronquée (u16)".into()))?;
    Ok(u16::from_le_bytes(b))
}

fn read_u32_le(cur: &mut Cursor<&[u8]>) -> Result<u32> {
    let mut b = [0u8; 4];
    cur.read_exact(&mut b)
        .map_err(|_| LauncherError::ServerUnreachable("réponse tronquée (u32)".into()))?;
    Ok(u32::from_le_bytes(b))
}

fn read_string(cur: &mut Cursor<&[u8]>) -> Result<String> {
    let len = read_u32_le(cur)? as usize;
    if len > 4096 {
        return Err(LauncherError::ServerUnreachable(
            "longueur de chaîne aberrante dans la réponse".into(),
        ));
    }
    let mut buf = vec![0u8; len];
    cur.read_exact(&mut buf)
        .map_err(|_| LauncherError::ServerUnreachable("réponse tronquée (string)".into()))?;
    // SA-MP renvoie généralement du Windows-1252/Latin-1 ; on décode sans planter.
    Ok(decode_latin1(&buf))
}

/// Décodage tolérant Latin-1 -> UTF-8 (évite toute erreur sur accents FR).
fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

fn parse_info_response(data: &[u8]) -> Result<ServerStatus> {
    // 11 octets d'en-tête (ré-écho) + 1 octet 'i' déjà inclus dans les 11 ?
    // Réponse SA-MP : en-tête "SAMP"+IP+port (10) + 'i' (1) = 11, puis payload.
    if data.len() < 11 || &data[0..4] != b"SAMP" {
        return Err(LauncherError::ServerUnreachable(
            "en-tête de réponse invalide".into(),
        ));
    }
    let payload = &data[11..];
    let mut cur = Cursor::new(payload);

    let mut pass = [0u8; 1];
    cur.read_exact(&mut pass)
        .map_err(|_| LauncherError::ServerUnreachable("réponse tronquée (password)".into()))?;
    let players = read_u16_le(&mut cur)?;
    let max_players = read_u16_le(&mut cur)?;
    let hostname = read_string(&mut cur)?;
    let gamemode = read_string(&mut cur)?;
    let language = read_string(&mut cur)?;

    Ok(ServerStatus {
        online: true,
        password: pass[0] != 0,
        players,
        max_players,
        hostname,
        gamemode,
        language,
        ping_ms: 0,
    })
}

/// Interroge le serveur et renvoie son statut. Ne renvoie jamais d'erreur
/// « dure » pour un serveur hors ligne : dans ce cas `online = false`.
pub fn query_status(host: &str, port: u16, timeout: Duration) -> ServerStatus {
    match query_status_inner(host, port, timeout) {
        Ok(status) => status,
        Err(_) => ServerStatus::offline(),
    }
}

fn query_status_inner(host: &str, port: u16, timeout: Duration) -> Result<ServerStatus> {
    let (ip, addr) = resolve_ipv4(host, port)?;

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(timeout))?;
    socket.set_write_timeout(Some(timeout))?;
    socket.connect(addr)?;

    let packet = build_info_packet(ip, port);

    let started = Instant::now();
    socket.send(&packet)?;

    let mut buf = [0u8; 4096];
    let n = socket
        .recv(&mut buf)
        .map_err(|e| LauncherError::ServerUnreachable(format!("pas de réponse : {e}")))?;
    let ping = started.elapsed().as_millis() as u32;

    let mut status = parse_info_response(&buf[..n])?;
    status.ping_ms = ping;
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_packet_has_correct_header() {
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let pkt = build_info_packet(ip, 3400);
        assert_eq!(&pkt[0..4], b"SAMP");
        assert_eq!(&pkt[4..8], &[127, 0, 0, 1]);
        assert_eq!(&pkt[8..10], &3400u16.to_le_bytes());
        assert_eq!(pkt[10], b'i');
    }

    #[test]
    fn parse_valid_response() {
        // En-tête (11) + password(0) + players(5) + max(125) + strings
        let mut data = Vec::new();
        data.extend_from_slice(b"SAMP");
        data.extend_from_slice(&[127, 0, 0, 1]);
        data.extend_from_slice(&3400u16.to_le_bytes());
        data.push(b'i');
        data.push(0); // no password
        data.extend_from_slice(&5u16.to_le_bytes());
        data.extend_from_slice(&125u16.to_le_bytes());
        let host = b"Grand Theft RolePlay";
        data.extend_from_slice(&(host.len() as u32).to_le_bytes());
        data.extend_from_slice(host);
        let gm = b"GTRP";
        data.extend_from_slice(&(gm.len() as u32).to_le_bytes());
        data.extend_from_slice(gm);
        let lang = b"FR";
        data.extend_from_slice(&(lang.len() as u32).to_le_bytes());
        data.extend_from_slice(lang);

        let s = parse_info_response(&data).unwrap();
        assert!(s.online);
        assert!(!s.password);
        assert_eq!(s.players, 5);
        assert_eq!(s.max_players, 125);
        assert_eq!(s.hostname, "Grand Theft RolePlay");
        assert_eq!(s.gamemode, "GTRP");
        assert_eq!(s.language, "FR");
    }

    #[test]
    fn parse_rejects_bad_header() {
        let data = b"XXXX0000000000";
        assert!(parse_info_response(data).is_err());
    }

    #[test]
    fn latin1_accents_do_not_panic() {
        // 0xE9 = 'é' en Latin-1
        assert_eq!(decode_latin1(&[0xE9]), "é");
    }
}
