// Rust parity coverage for the Claude Code session-sync feature ported from
// context-router/cli/src/{hooks,hookSync,session}.ts and its
// cli.test.ts extension (see tests/PARITY.md's "Claude Code session sync"
// and "Session storage permissions redesign" sections). This is new
// coverage, not a 1:1 ported TS test, so it uses its own minimal fake
// backend rather than extending cli_compat.rs's skills/routers-oriented
// one — only the handful of routes this feature touches are implemented.
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flockfly::api::{Api, ApiFactory, CliError};
use flockfly::commands::{run_cli_with, Runtime};
use serde_json::{json, Value};
use tempfile::TempDir;

#[derive(Default)]
struct Backend {
    approved_email: Option<String>,
    token_email: HashMap<String, String>,
    // email -> personal collection id, auto-provisioned at login (mirrors
    // ensurePersonalCollection in services/collections.ts — every user gets
    // one, with no collection to configure).
    personal_collections: HashMap<String, String>,
    // (ownerEmail, key) -> normalized entries
    sessions: HashMap<(String, String), Vec<Value>>,
}

#[derive(Clone)]
struct FakeFactory(Arc<Mutex<Backend>>);

struct FakeApi {
    state: Arc<Mutex<Backend>>,
    token: Option<String>,
}

impl ApiFactory for FakeFactory {
    fn create(&self, _base_url: &str, token: Option<&str>) -> Box<dyn Api> {
        Box::new(FakeApi {
            state: Arc::clone(&self.0),
            token: token.map(str::to_owned),
        })
    }
}

impl FakeApi {
    fn email(&self, state: &Backend) -> Result<String, CliError> {
        self.token
            .as_ref()
            .and_then(|token| state.token_email.get(token))
            .cloned()
            .ok_or_else(|| CliError::new("Not authenticated.", Some("unauthenticated"), None))
    }
}

fn personal_collection_id(email: &str) -> String {
    format!("coll_personal_{}", email.replace(['@', '.'], "_"))
}

// Simplified stand-in for Murmur's normalize_claude_entry — enough to
// exercise the CLI's plumbing (auth, key/subpath threading, dedup-by-id),
// not full fidelity. Real normalization fidelity is Murmur's job, covered
// by murmur/tests/collection_sessions_native_api.rs.
fn fake_normalize(subpath: &Option<String>, raw: &Value) -> Value {
    let uuid = raw.get("uuid").and_then(Value::as_str).unwrap_or("no-uuid");
    let source = match subpath {
        Some(subpath) => format!("claude_code:{subpath}"),
        None => "claude_code".to_owned(),
    };
    json!({
        "id": format!("evt_claude_{uuid}"),
        "kind": if raw.get("message").is_some() { "message" } else { "harness_event" },
        "source": source,
        "raw": raw,
    })
}

impl Api for FakeApi {
    fn request(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value, CliError> {
        let mut state = self.state.lock().unwrap();
        let (base, query) = path.split_once('?').unwrap_or((path, ""));

        match (method, base) {
            ("POST", "/v1/auth/cli/start") => Ok(json!({
                "cliAuthId": "auth_abc",
                "verificationUrl": "http://browser.test/login#cliAuthId=auth_abc"
            })),
            ("POST", "/v1/auth/cli/poll") => {
                if let Some(email) = state.approved_email.clone() {
                    let token = format!("ffly_{}", email.replace(['@', '.'], "_"));
                    state.token_email.insert(token.clone(), email.clone());
                    // Mirrors ensureUserByEmail calling ensurePersonalCollection
                    // on every login — idempotent, always exists after this.
                    state
                        .personal_collections
                        .entry(email.clone())
                        .or_insert_with(|| personal_collection_id(&email));
                    Ok(json!({ "status": "approved", "token": token }))
                } else {
                    Ok(json!({ "status": "pending" }))
                }
            }
            ("GET", "/v1/me") => {
                let email = self.email(&state)?;
                let username = email.split('@').next().unwrap_or("user");
                Ok(json!({
                    "user": { "id": format!("user_{username}"), "email": email, "username": username, "createdAt": "2026-01-01T00:00:00Z" },
                    "org": { "id": format!("org_{username}"), "name": format!("{username}'s org"), "createdAt": "2026-01-01T00:00:00Z" }
                }))
            }
            ("GET", "/v1/collections") if query == "scope=manageable" => {
                let email = self.email(&state)?;
                let collection_id = state
                    .personal_collections
                    .get(&email)
                    .cloned()
                    .unwrap_or_else(|| personal_collection_id(&email));
                Ok(json!({
                    "collections": [{
                        "id": collection_id,
                        "name": format!("{}'s sessions", email.split('@').next().unwrap_or("user")),
                        "personalOwnerUserId": format!("user_{}", email.split('@').next().unwrap_or("user")),
                    }]
                }))
            }
            ("GET", path)
                if path.starts_with("/v1/collections/") && path.ends_with("/sessions/entries") =>
            {
                let collection_id = path
                    .strip_prefix("/v1/collections/")
                    .unwrap()
                    .strip_suffix("/sessions/entries")
                    .unwrap();
                let key = query
                    .split('&')
                    .find_map(|pair| pair.strip_prefix("key="))
                    .map(|value| urlencoding::decode(value).unwrap().into_owned())
                    .unwrap_or_default();
                let Some(owner_email) = state
                    .personal_collections
                    .iter()
                    .find(|(_, id)| id.as_str() == collection_id)
                    .map(|(email, _)| email.clone())
                else {
                    return Err(CliError::new(
                        "Session not found.",
                        Some("session_not_found"),
                        None,
                    ));
                };
                let entries = state
                    .sessions
                    .get(&(owner_email.clone(), key.clone()))
                    .cloned()
                    .unwrap_or_default();
                if entries.is_empty() && !state.sessions.contains_key(&(owner_email, key)) {
                    return Err(CliError::new(
                        "Session not found.",
                        Some("session_not_found"),
                        None,
                    ));
                }
                Ok(json!({ "entries": entries, "nextCursor": Value::Null }))
            }
            ("POST", "/v1/sessions") => {
                let email = self.email(&state)?;
                let key = body
                    .as_ref()
                    .and_then(|b| b.get("key"))
                    .and_then(Value::as_str)
                    .unwrap()
                    .to_owned();
                state.sessions.entry((email.clone(), key)).or_default();
                Ok(
                    json!({ "session": { "ownerId": format!("user_{}", email.split('@').next().unwrap_or("user")), "status": "running" } }),
                )
            }
            ("POST", "/v1/sessions/logs/native") => {
                let email = self.email(&state)?;
                let body = body.unwrap();
                let key = body.get("key").and_then(Value::as_str).unwrap().to_owned();
                let subpath = body
                    .get("subpath")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let entries = body
                    .get("entries")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let session_key = (email, key);
                if !state.sessions.contains_key(&session_key) {
                    return Err(CliError::new(
                        "Session not found.",
                        Some("session_not_found"),
                        None,
                    ));
                }
                let normalized: Vec<Value> = entries
                    .iter()
                    .map(|entry| fake_normalize(&subpath, entry))
                    .collect();
                state
                    .sessions
                    .get_mut(&session_key)
                    .unwrap()
                    .extend(normalized);
                Ok(json!({ "pushed": { "entryCount": entries.len() } }))
            }
            ("POST", "/v1/sessions/reconcile/native") => {
                let email = self.email(&state)?;
                let body = body.unwrap();
                let key = body.get("key").and_then(Value::as_str).unwrap().to_owned();
                let subpath = body
                    .get("subpath")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let entries = body
                    .get("entries")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let session_key = (email, key);
                if !state.sessions.contains_key(&session_key) {
                    return Err(CliError::new(
                        "Session not found.",
                        Some("session_not_found"),
                        None,
                    ));
                }
                let existing_ids: HashSet<String> = state.sessions[&session_key]
                    .iter()
                    .filter_map(|e| e.get("id").and_then(Value::as_str).map(str::to_owned))
                    .collect();
                let mut added = 0;
                for entry in &entries {
                    let normalized = fake_normalize(&subpath, entry);
                    let id = normalized
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    if !existing_ids.contains(&id) {
                        state
                            .sessions
                            .get_mut(&session_key)
                            .unwrap()
                            .push(normalized);
                        added += 1;
                    }
                }
                Ok(json!({ "reconciled": { "addedCount": added } }))
            }
            _ => Err(CliError::message(format!(
                "no fake handler for {method} {path}"
            ))),
        }
    }
}

struct TestRuntime {
    env: HashMap<String, String>,
    stdout: Vec<String>,
    stderr: Vec<String>,
    state: Arc<Mutex<Backend>>,
    approve_email: Option<String>,
    stdin: String,
}

impl Runtime for TestRuntime {
    fn env(&self) -> &HashMap<String, String> {
        &self.env
    }
    fn out(&mut self, text: &str) {
        self.stdout.push(text.to_owned());
    }
    fn err(&mut self, text: &str) {
        self.stderr.push(text.to_owned());
    }
    fn confirm(&mut self, _question: &str) -> bool {
        false
    }
    fn open_browser(&mut self, _url: &str) {
        if let Some(email) = self.approve_email.clone() {
            self.state.lock().unwrap().approved_email = Some(email);
        }
    }
    fn sleep(&mut self, _duration: Duration) {}
    fn read_stdin(&mut self) -> String {
        self.stdin.clone()
    }
}

struct Harness {
    _config: TempDir,
    env: HashMap<String, String>,
    state: Arc<Mutex<Backend>>,
    factory: FakeFactory,
}

struct ResultCapture {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Harness {
    fn new() -> Self {
        let config = tempfile::tempdir().unwrap();
        let env = HashMap::from([
            (
                "FLOCKFLY_API_URL".to_owned(),
                "http://fake-api.test".to_owned(),
            ),
            (
                "FLOCKFLY_CONFIG_DIR".to_owned(),
                config.path().display().to_string(),
            ),
            ("HOME".to_owned(), config.path().display().to_string()),
        ]);
        let state = Arc::new(Mutex::new(Backend::default()));
        Self {
            _config: config,
            env,
            factory: FakeFactory(Arc::clone(&state)),
            state,
        }
    }

    fn run(&self, args: &[&str]) -> ResultCapture {
        self.run_with_stdin(args, "")
    }

    fn run_with_stdin(&self, args: &[&str], stdin: &str) -> ResultCapture {
        let mut runtime = TestRuntime {
            env: self.env.clone(),
            stdout: vec![],
            stderr: vec![],
            state: Arc::clone(&self.state),
            approve_email: None,
            stdin: stdin.to_owned(),
        };
        let code = run_cli_with(args, &mut runtime, &self.factory);
        ResultCapture {
            code,
            stdout: runtime.stdout.join("\n"),
            stderr: runtime.stderr.join("\n"),
        }
    }

    fn login(&self, email: &str) -> ResultCapture {
        let mut runtime = TestRuntime {
            env: self.env.clone(),
            stdout: vec![],
            stderr: vec![],
            state: Arc::clone(&self.state),
            approve_email: Some(email.to_owned()),
            stdin: String::new(),
        };
        let code = run_cli_with(&["login"], &mut runtime, &self.factory);
        ResultCapture {
            code,
            stdout: runtime.stdout.join("\n"),
            stderr: runtime.stderr.join("\n"),
        }
    }

    // Sessions are keyed directly by owner email here (this harness has
    // direct access to Backend state, unlike the HTTP-only TypeScript
    // tests, so it doesn't need to resolve a collection id first).
    fn session_entry_count(&self, owner_email: &str, key: &str) -> usize {
        self.state
            .lock()
            .unwrap()
            .sessions
            .get(&(owner_email.to_owned(), key.to_owned()))
            .map(Vec::len)
            .unwrap_or(0)
    }
}

fn transcript_path(dir: &TempDir, project: &str, session: &str) -> String {
    let path = dir
        .path()
        .join("projects")
        .join(project)
        .join(format!("{session}.jsonl"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    path.display().to_string()
}

fn fixture_contents() -> String {
    fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/claude-transcripts/sample-session.jsonl"),
    )
    .unwrap()
}

#[test]
fn init_configures_and_installs_the_global_hook_and_hooks_remove_is_idempotent() {
    let harness = Harness::new();
    harness.login("hook-init@example.com");

    let init = harness.run(&["init", "--sessions"]);
    assert_eq!(init.code, 0, "stderr: {}", init.stderr);
    assert!(init.stdout.contains("Configured Claude Code session sync."));

    let settings: Value = serde_json::from_str(
        &fs::read_to_string(format!("{}/.claude/settings.json", harness.env["HOME"])).unwrap(),
    )
    .unwrap();
    assert_eq!(
        settings["hooks"]["Stop"][0]["hooks"][0]["command"],
        "flockfly session sync --hook"
    );
    assert_eq!(
        settings["hooks"]["SessionEnd"][0]["hooks"][0]["command"],
        "flockfly session sync --hook --reconcile"
    );

    let removed = harness.run(&["hooks", "remove"]);
    assert_eq!(removed.code, 0);
    assert!(removed.stdout.contains("Removed Flockfly hooks"));
    let removed_again = harness.run(&["hooks", "remove"]);
    assert!(removed_again
        .stdout
        .contains("No Flockfly hooks were installed"));
}

#[test]
fn session_sync_hook_pushes_transcript_entries_end_to_end_into_the_publishers_personal_collection()
{
    let harness = Harness::new();
    harness.login("hook-sync@example.com");
    harness.run(&["init", "--sessions"]);

    let transcripts = tempfile::tempdir().unwrap();
    let path = transcript_path(&transcripts, "-Users-jkim-repo", "sess-1");
    fs::write(&path, fixture_contents()).unwrap();

    let stdin = json!({ "transcript_path": path, "session_id": "sess-1" }).to_string();
    let sync = harness.run_with_stdin(&["session", "sync", "--hook"], &stdin);
    assert_eq!(sync.code, 0, "stderr: {}", sync.stderr);
    assert_eq!(
        harness.session_entry_count("hook-sync@example.com", "-Users-jkim-repo/sess-1"),
        3
    );
}

#[test]
fn session_sync_hook_is_best_effort_when_the_transcript_is_missing() {
    let harness = Harness::new();
    harness.login("hook-missing-file@example.com");
    harness.run(&["init", "--sessions"]);
    let stdin =
        json!({ "transcript_path": "/nonexistent/session.jsonl", "session_id": "s" }).to_string();
    let missing_file = harness.run_with_stdin(&["session", "sync", "--hook"], &stdin);
    assert_eq!(missing_file.code, 0);
}

#[test]
fn session_sync_hook_only_pushes_newly_appended_lines_on_a_second_fire() {
    let harness = Harness::new();
    harness.login("hook-incremental@example.com");
    harness.run(&["init", "--sessions"]);

    let transcripts = tempfile::tempdir().unwrap();
    let path = transcript_path(&transcripts, "-Users-jkim-repo", "sess-2");
    fs::write(
        &path,
        format!(
            "{}\n",
            json!({"uuid": "e1", "message": {"role": "user", "content": []}})
        ),
    )
    .unwrap();

    let stdin = json!({ "transcript_path": path, "session_id": "sess-2" }).to_string();
    harness.run_with_stdin(&["session", "sync", "--hook"], &stdin);
    assert_eq!(
        harness.session_entry_count("hook-incremental@example.com", "-Users-jkim-repo/sess-2"),
        1
    );

    let mut contents = fs::read_to_string(&path).unwrap();
    contents.push_str(&format!(
        "{}\n",
        json!({"uuid": "e2", "message": {"role": "assistant", "content": []}})
    ));
    fs::write(&path, contents).unwrap();

    harness.run_with_stdin(&["session", "sync", "--hook"], &stdin);
    assert_eq!(
        harness.session_entry_count("hook-incremental@example.com", "-Users-jkim-repo/sess-2"),
        2
    );
}

#[test]
fn reconcile_backfills_entries_a_prior_incremental_push_missed() {
    let harness = Harness::new();
    harness.login("hook-reconcile@example.com");
    harness.run(&["init", "--sessions"]);

    let transcripts = tempfile::tempdir().unwrap();
    let path = transcript_path(&transcripts, "-Users-jkim-repo", "sess-3");
    let entry_a = json!({"uuid": "e1", "message": {"role": "user", "content": []}});
    let entry_b = json!({"uuid": "e2", "message": {"role": "assistant", "content": []}});
    let entry_c = json!({"uuid": "e3", "message": {"role": "user", "content": []}});
    fs::write(&path, format!("{entry_a}\n{entry_b}\n")).unwrap();

    let stdin = json!({ "transcript_path": path, "session_id": "sess-3" }).to_string();
    harness.run_with_stdin(&["session", "sync", "--hook"], &stdin);
    assert_eq!(
        harness.session_entry_count("hook-reconcile@example.com", "-Users-jkim-repo/sess-3"),
        2
    );

    let mut contents = fs::read_to_string(&path).unwrap();
    contents.push_str(&format!("{entry_c}\n"));
    fs::write(&path, contents).unwrap();

    let reconcile = harness.run_with_stdin(&["session", "sync", "--hook", "--reconcile"], &stdin);
    assert_eq!(reconcile.code, 0, "stderr: {}", reconcile.stderr);
    assert_eq!(
        harness.session_entry_count("hook-reconcile@example.com", "-Users-jkim-repo/sess-3"),
        3
    );
}
