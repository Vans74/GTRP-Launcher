//! Test manuel du module de query contre le serveur réel.
//! Usage : cargo run --example query -- <host> <port>

use gtrp_core::query::query_status;
use std::time::Duration;

fn main() {
    let mut args = std::env::args().skip(1);
    let host = args.next().unwrap_or_else(|| "51.255.92.237".to_string());
    let port: u16 = args
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3400);

    println!("Query -> {host}:{port}");
    let status = query_status(&host, port, Duration::from_millis(3000));
    println!("{status:#?}");
}
