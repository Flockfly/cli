use std::collections::HashMap;

use flockfly::config::{api_url, DEFAULT_API_URL};
use tempfile::tempdir;

#[test]
fn ts_connects_installed_clients_to_the_production_api_by_default() {
    let dir = tempdir().unwrap();
    let env = HashMap::from([(
        "FLOCKFLY_CONFIG_DIR".to_owned(),
        dir.path().join("missing").display().to_string(),
    )]);

    assert_eq!(DEFAULT_API_URL, "https://api.flockfly.ai");
    assert_eq!(api_url(&env), "https://api.flockfly.ai");
}

#[test]
fn ts_allows_local_development_to_override_the_api_url() {
    let env = HashMap::from([(
        "FLOCKFLY_API_URL".to_owned(),
        "http://127.0.0.1:8799".to_owned(),
    )]);

    assert_eq!(api_url(&env), "http://127.0.0.1:8799");
}
