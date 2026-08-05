// `flockfly session sync` — the Claude Code hook entrypoint. Reads a hook
// event's JSON off stdin, derives the collection-session key for its
// transcript(s), and pushes (or reconciles) new content into the
// configured collection. Ported from the TypeScript reference
// implementation (context-router/cli/src/session.ts).
use std::path::Path;

use serde::Deserialize;
use serde_json::json;

use crate::api::{ApiFactory, CliError};
use crate::commands::Runtime;
use crate::config::load_credentials;
use crate::hook_sync::{
    derive_claude_session_key, get_offset, load_sync_state, read_complete_new_lines,
    save_sync_state, set_offset,
};

pub struct SessionSyncOptions {
    pub hook: bool,
    pub reconcile: bool,
    pub collection: Option<String>,
}

#[derive(Deserialize)]
struct HookInput {
    transcript_path: Option<String>,
    agent_transcript_path: Option<String>,
    session_id: Option<String>,
}

// Best-effort, never propagates an error up to the caller — Claude Code
// must never be blocked by a sync failure (network blip, Flockfly down,
// malformed transcript, whatever). Failures are written to stderr only.
pub fn session_sync_command(
    runtime: &mut dyn Runtime,
    factory: &dyn ApiFactory,
    options: SessionSyncOptions,
) {
    if let Err(error) = run_session_sync(runtime, factory, options) {
        runtime.err(&format!("flockfly session sync: {error}"));
    }
}

fn run_session_sync(
    runtime: &mut dyn Runtime,
    factory: &dyn ApiFactory,
    options: SessionSyncOptions,
) -> Result<(), CliError> {
    if !options.hook {
        runtime.err(
            "flockfly session sync currently requires --hook (invoke it as a Claude Code hook).",
        );
        return Ok(());
    }
    let env = runtime.env().clone();
    let collection_id = options
        .collection
        .or_else(|| crate::config::load_sync_config(&env).map(|config| config.collection_id));
    let Some(collection_id) = collection_id else {
        runtime.err("flockfly session sync: no collection configured. Run `flockfly init --collection <id>` first.");
        return Ok(());
    };

    let raw = runtime.read_stdin();
    let hook_input: HookInput = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(_) => {
            runtime.err("flockfly session sync: could not parse hook input JSON on stdin.");
            return Ok(());
        }
    };

    let mut paths = Vec::new();
    if let Some(p) = hook_input.transcript_path.filter(|p| !p.is_empty()) {
        paths.push(p);
    }
    if let Some(p) = hook_input.agent_transcript_path.filter(|p| !p.is_empty()) {
        paths.push(p);
    }
    if paths.is_empty() {
        return Ok(());
    }

    let credentials = load_credentials(&env)
        .ok_or_else(|| CliError::message("You are not logged in. Run `flockfly login` first."))?;
    let client = factory.create(&credentials.api_url, Some(&credentials.token));
    let mut state = load_sync_state(&env);
    let mut state_changed = false;

    for transcript_path in &paths {
        if !Path::new(transcript_path).exists() {
            continue;
        }
        let key_info = derive_claude_session_key(transcript_path, hook_input.session_id.as_deref());

        // Idempotent — safe to call unconditionally on every hook fire, even
        // when there turns out to be nothing new to push below.
        let mut create_body = json!({ "key": key_info.key, "harness": "claude_code" });
        if let Some(session_id) = &hook_input.session_id {
            create_body["externalSessionId"] = json!(session_id);
        }
        if let Err(error) = client.request(
            "POST",
            &format!(
                "/v1/collections/{}/sessions",
                urlencoding::encode(&collection_id)
            ),
            Some(create_body),
        ) {
            runtime.err(&format!(
                "flockfly session sync: failed for {transcript_path}: {error}"
            ));
            continue;
        }

        if options.reconcile {
            let read = match read_complete_new_lines(transcript_path, 0) {
                Ok(read) => read,
                Err(error) => {
                    runtime.err(&format!(
                        "flockfly session sync: failed for {transcript_path}: {error}"
                    ));
                    continue;
                }
            };
            if read.entries.is_empty() {
                continue;
            }
            let mut body =
                json!({ "key": key_info.key, "harness": "claude_code", "entries": read.entries });
            if let Some(subpath) = &key_info.subpath {
                body["subpath"] = json!(subpath);
            }
            if let Err(error) = client.request(
                "POST",
                &format!(
                    "/v1/collections/{}/sessions/reconcile/native",
                    urlencoding::encode(&collection_id)
                ),
                Some(body),
            ) {
                runtime.err(&format!(
                    "flockfly session sync: failed for {transcript_path}: {error}"
                ));
            }
        } else {
            let offset = get_offset(&state, transcript_path);
            let read = match read_complete_new_lines(transcript_path, offset) {
                Ok(read) => read,
                Err(error) => {
                    runtime.err(&format!(
                        "flockfly session sync: failed for {transcript_path}: {error}"
                    ));
                    continue;
                }
            };
            if read.new_offset == offset {
                continue;
            }
            if !read.entries.is_empty() {
                let mut body = json!({ "key": key_info.key, "harness": "claude_code", "entries": read.entries });
                if let Some(subpath) = &key_info.subpath {
                    body["subpath"] = json!(subpath);
                }
                if let Err(error) = client.request(
                    "POST",
                    &format!(
                        "/v1/collections/{}/sessions/logs/native",
                        urlencoding::encode(&collection_id)
                    ),
                    Some(body),
                ) {
                    runtime.err(&format!(
                        "flockfly session sync: failed for {transcript_path}: {error}"
                    ));
                    continue;
                }
            }
            // Advance the offset only after a successful call (or for
            // malformed-only trailing lines) — mirrors claude-code-hook-sync.py
            // exactly, so a failed push retries the same lines next hook fire.
            set_offset(&mut state, transcript_path, read.new_offset);
            state_changed = true;
        }
    }

    if state_changed {
        save_sync_state(&env, &state).map_err(|error| CliError::message(error.to_string()))?;
    }

    Ok(())
}
