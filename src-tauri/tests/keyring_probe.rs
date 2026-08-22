//! One-off: full FileSecretStore round-trip probe.
use mylore_lib::infrastructure::keyring::{FileSecretStore, SecretStore};
use uuid::Uuid;

#[test]
fn probe_file_store_roundtrip() {
    let dir = std::env::temp_dir().join(format!("mylore-keyring-probe-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create dir");
    let store = FileSecretStore::load(dir.join("api_keys.json"));

    store.set("tmdb", "probe-secret").expect("SET");
    match store.get("tmdb") {
        Ok(Some(v)) => println!("GET ok: {v}"),
        Ok(None) => println!("GET: None"),
        Err(e) => println!("GET ERROR: {e}"),
    }
    store.delete("tmdb").expect("DELETE");
    assert_eq!(store.get("tmdb").unwrap(), None);
    println!("DELETE ok, roundtrip clean");

    let _ = std::fs::remove_dir_all(&dir);
}
