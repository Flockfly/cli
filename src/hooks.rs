// Installs/removes the global Claude Code hook that drives automatic
// session sync (see session.rs, hook_sync.rs). Global, not per-repo — one
// install in ~/.claude/settings.json covers every Claude Code session on
// the machine, matching `flockfly init --collection <id>`'s scope.
//
// Ported from the TypeScript reference implementation
// (context-router/cli/src/hooks.ts) — same marker-based idempotency
// strategy as murmur-cli's Kiro hook installer, adapted to Claude Code's
// actual nested `hooks.<Event>: [{matcher, hooks: [{type, command,
// timeout?}]}]` shape (structurally different from Kiro's flat
// `hooks.stop[]`).
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use serde_json::{json, Value};

/// Marker substring present in every command Flockfly installs — used to
/// find-and-replace our own entries idempotently without disturbing any
/// other hooks (e.g. another CLI's) that may already live in the same
/// settings.json.
pub const HOOK_MARKER: &str = "session sync --hook";

// Claude Code's own settings path isn't configurable via any Flockfly env
// var (unlike config_dir(), which reads FLOCKFLY_CONFIG_DIR) — tests
// inject a fake home directory via env["HOME"] instead.
pub fn claude_settings_path(env: &HashMap<String, String>) -> PathBuf {
    let home = env
        .get("HOME")
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".claude").join("settings.json")
}

fn load_settings(path: &PathBuf) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_else(|| json!({}))
}

fn save_settings(path: &PathBuf, settings: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = format!("{}\n", serde_json::to_string_pretty(settings)?);
    fs::write(path, contents)
}

fn block_hooks(block: &Value) -> Vec<Value> {
    block
        .get("hooks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn command_of(hook: &Value) -> &str {
    hook.get("command").and_then(Value::as_str).unwrap_or("")
}

// Strips any hook entry whose command contains HOOK_MARKER, dropping blocks
// left with no hooks. Used both to make room for a fresh install (then one
// managed block is appended) and to implement `hooks remove` (nothing gets
// re-appended).
fn strip_managed(blocks: &[Value]) -> Vec<Value> {
    blocks
        .iter()
        .map(|block| {
            let kept_hooks: Vec<Value> = block_hooks(block)
                .into_iter()
                .filter(|hook| !command_of(hook).contains(HOOK_MARKER))
                .collect();
            let mut updated = block.clone();
            updated["hooks"] = Value::Array(kept_hooks);
            updated
        })
        .filter(|block| !block_hooks(block).is_empty())
        .collect()
}

fn install_event(settings: &mut Value, event: &str, command: &str, timeout: Option<u64>) {
    if !settings.is_object() {
        *settings = json!({});
    }
    let hooks = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let existing = hooks
        .get(event)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut kept = strip_managed(&existing);
    let mut hook_entry = json!({ "type": "command", "command": command });
    if let Some(timeout) = timeout {
        hook_entry["timeout"] = json!(timeout);
    }
    kept.push(json!({ "matcher": "", "hooks": [hook_entry] }));
    hooks
        .as_object_mut()
        .unwrap()
        .insert(event.to_owned(), Value::Array(kept));
}

pub fn install_global_hook(env: &HashMap<String, String>) -> io::Result<(PathBuf, Vec<String>)> {
    let path = claude_settings_path(env);
    let mut settings = load_settings(&path);
    // Stop/SubagentStop fire on every turn — incremental push, offset-
    // tracked. SessionEnd is a full re-read reconcile: a safety-net catch-
    // up sweep that's cheap to run once per session and guarantees no data
    // loss even if incremental offset tracking ever desyncs.
    install_event(
        &mut settings,
        "Stop",
        "flockfly session sync --hook",
        Some(30),
    );
    install_event(
        &mut settings,
        "SubagentStop",
        "flockfly session sync --hook",
        Some(30),
    );
    install_event(
        &mut settings,
        "SessionEnd",
        "flockfly session sync --hook --reconcile",
        None,
    );
    save_settings(&path, &settings)?;
    Ok((
        path,
        vec![
            "Stop".to_owned(),
            "SubagentStop".to_owned(),
            "SessionEnd".to_owned(),
        ],
    ))
}

pub fn remove_global_hook(env: &HashMap<String, String>) -> io::Result<(PathBuf, bool)> {
    let path = claude_settings_path(env);
    if !path.exists() {
        return Ok((path, false));
    }
    let mut settings = load_settings(&path);
    let mut removed = false;
    for event in ["Stop", "SubagentStop", "SessionEnd"] {
        let Some(blocks) = settings
            .get("hooks")
            .and_then(|h| h.get(event))
            .and_then(Value::as_array)
            .cloned()
        else {
            continue;
        };
        let kept = strip_managed(&blocks);
        let before: usize = blocks.iter().map(|b| block_hooks(b).len()).sum();
        let after: usize = kept.iter().map(|b| block_hooks(b).len()).sum();
        if after < before {
            removed = true;
        }
        if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
            hooks.insert(event.to_owned(), Value::Array(kept));
        }
    }
    if removed {
        save_settings(&path, &settings)?;
    }
    Ok((path, removed))
}
