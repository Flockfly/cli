use std::fmt;

use reqwest::blocking::Client;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct CliError {
    pub message: String,
    pub code: Option<String>,
    pub extra: Option<Value>,
}

impl CliError {
    pub fn new(message: impl Into<String>, code: Option<&str>, extra: Option<Value>) -> Self {
        Self {
            message: message.into(),
            code: code.map(str::to_owned),
            extra,
        }
    }

    pub fn message(message: impl Into<String>) -> Self {
        Self::new(message, None, None)
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for CliError {}

pub trait Api {
    fn request(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value, CliError>;
}

pub trait ApiFactory {
    fn create(&self, base_url: &str, token: Option<&str>) -> Box<dyn Api>;
}

#[derive(Clone, Default)]
pub struct HttpApiFactory;

impl ApiFactory for HttpApiFactory {
    fn create(&self, base_url: &str, token: Option<&str>) -> Box<dyn Api> {
        Box::new(HttpApi {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_owned(),
            token: token.map(str::to_owned),
        })
    }
}

struct HttpApi {
    client: Client,
    base_url: String,
    token: Option<String>,
}

impl Api for HttpApi {
    fn request(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value, CliError> {
        let method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|error| CliError::message(error.to_string()))?;
        let mut request = self
            .client
            .request(method, format!("{}{}", self.base_url, path));
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().map_err(|_| {
            CliError::message(format!(
                "Could not reach the Flockfly API at {}. Is it running?",
                self.base_url
            ))
        })?;
        let status = response.status();
        let text = response
            .text()
            .map_err(|error| CliError::message(error.to_string()))?;
        let parsed = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::Null)
        };

        if status.is_success() {
            return Ok(parsed);
        }

        if let Some(error) = parsed.get("error").and_then(Value::as_object) {
            if let (Some(code), Some(message)) = (
                error.get("code").and_then(Value::as_str),
                error.get("message").and_then(Value::as_str),
            ) {
                let mut extra = error.clone();
                extra.remove("code");
                extra.remove("message");
                extra.remove("requestId");
                return Err(CliError::new(
                    actionable_message(code, message),
                    Some(code),
                    Some(Value::Object(extra)),
                ));
            }
        }

        Err(CliError::message(format!(
            "Request failed with status {}.",
            status.as_u16()
        )))
    }
}

fn actionable_message(code: &str, message: &str) -> String {
    match code {
        "unauthenticated" => "You are not logged in. Run `flockfly login` first.".to_owned(),
        "router_not_found" => {
            format!("{message} Run `flockfly routers list` to see your routers.")
        }
        "router_access_denied" => {
            format!("{message} Skills can only be attached to routers you belong to.")
        }
        "collection_not_found" => message.to_owned(),
        "collection_access_denied" => {
            format!("{message} You may not have publish access to this collection yet.")
        }
        _ => message.to_owned(),
    }
}
