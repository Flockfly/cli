use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const DEFAULT_API_URL: &str = "https://api.flockfly.ai";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Credentials {
    pub api_url: String,
    pub token: String,
}

pub fn config_dir(env: &HashMap<String, String>) -> PathBuf {
    env.get("FLOCKFLY_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".flockfly")))
        .unwrap_or_else(|| PathBuf::from(".flockfly"))
}

pub fn api_url(env: &HashMap<String, String>) -> String {
    env.get("FLOCKFLY_API_URL")
        .cloned()
        .or_else(|| load_credentials(env).map(|credentials| credentials.api_url))
        .unwrap_or_else(|| DEFAULT_API_URL.to_owned())
}

pub fn load_credentials(env: &HashMap<String, String>) -> Option<Credentials> {
    if let Some(token) = env.get("FLOCKFLY_TOKEN").filter(|token| !token.is_empty()) {
        return Some(Credentials {
            api_url: env
                .get("FLOCKFLY_API_URL")
                .cloned()
                .unwrap_or_else(|| DEFAULT_API_URL.to_owned()),
            token: token.clone(),
        });
    }

    let contents = fs::read_to_string(credentials_path(env)).ok()?;
    let credentials: Credentials = serde_json::from_str(&contents).ok()?;
    if credentials.api_url.is_empty() || credentials.token.is_empty() {
        return None;
    }
    Some(credentials)
}

pub fn save_credentials(
    credentials: &Credentials,
    env: &HashMap<String, String>,
) -> io::Result<()> {
    let directory = config_dir(env);
    fs::create_dir_all(&directory)?;
    let path = credentials_path(env);
    let contents = format!("{}\n", serde_json::to_string_pretty(credentials)?);

    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true).mode(0o600);
        std::io::Write::write_all(&mut options.open(&path)?, contents.as_bytes())?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }

    #[cfg(not(unix))]
    fs::write(&path, contents)?;

    Ok(())
}

fn credentials_path(env: &HashMap<String, String>) -> PathBuf {
    config_dir(env).join("credentials.json")
}
