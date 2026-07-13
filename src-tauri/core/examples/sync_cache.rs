use gtrp_core::samp_cache;
use std::path::PathBuf;

fn main() {
    let documents = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: sync_cache <dossier Documents>");
    let result = samp_cache::sync_cache(&documents, |progress| {
        eprintln!(
            "{}: {}/{} ({} / {} octets)",
            progress.current_file,
            progress.files_done,
            progress.files_total,
            progress.bytes_done,
            progress.bytes_total
        );
    })
    .expect("synchronisation cache SA-MP");
    println!("{result:#?}");
}
