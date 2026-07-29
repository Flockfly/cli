use std::collections::HashMap;

use flockfly::config::{api_url, load_credentials, DEFAULT_API_URL};

#[test]
fn ci_token_is_used_without_a_credentials_file() {
    let env = HashMap::from([("FLOCKFLY_TOKEN".to_owned(), "ffly_ci_secret".to_owned())]);
    let credentials = load_credentials(&env).unwrap();

    assert_eq!(credentials.token, "ffly_ci_secret");
    assert_eq!(credentials.api_url, DEFAULT_API_URL);
    assert_eq!(api_url(&env), DEFAULT_API_URL);
}

#[test]
fn ci_token_honors_the_api_url_override() {
    let env = HashMap::from([
        ("FLOCKFLY_TOKEN".to_owned(), "ffly_ci_secret".to_owned()),
        (
            "FLOCKFLY_API_URL".to_owned(),
            "http://127.0.0.1:8799".to_owned(),
        ),
    ]);
    let credentials = load_credentials(&env).unwrap();

    assert_eq!(credentials.api_url, "http://127.0.0.1:8799");
}
