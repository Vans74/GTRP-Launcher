use std::env;

fn main() {
    const KEY_NAME: &str = "GTRP_GATE_CLIENT_KEY";

    println!("cargo:rerun-if-env-changed={KEY_NAME}");

    // Une version distribuée sans cette clé passe tous les contrôles locaux,
    // mais ne peut jamais obtenir le ticket serveur. On refuse donc la
    // compilation plutôt que de produire silencieusement un launcher inutilisable.
    if env::var("PROFILE").as_deref() != Ok("release") {
        return;
    }

    let key = env::var(KEY_NAME).unwrap_or_default();
    if key.len() < 32 || !key.is_ascii() || key.contains(',') {
        panic!(
            "{} absent ou invalide : une build release doit recevoir \
             une clé ASCII d'au moins 32 caractères",
            KEY_NAME
        );
    }
}
