// Ported from context-router/cli/src/hookSync.ts, which itself ports
// murmur/scripts/claude-code-hook-sync.py's parse_session_key and
// read_complete_new_lines — plus local offset-state persistence for
// session.rs's push/reconcile orchestration.
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::config_dir;

pub struct ClaudeSessionKey {
    pub key: String,
    pub subpath: Option<String>,
}

// `key` is the collection-session address (project_key/session_id) — shared
// by a session's main transcript and every subagent transcript beneath it,
// so all of one Claude Code session's activity accumulates into one
// collection-session. `subpath` only differentiates subagent transcripts
// for `source` tagging (see murmur/src/store.rs's normalize_claude_entry).
pub fn derive_claude_session_key(
    transcript_path: &str,
    fallback_session_id: Option<&str>,
) -> ClaudeSessionKey {
    let path = Path::new(transcript_path);
    let parts: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    let projects_index = parts.iter().position(|part| part == "projects");
    let after_projects: Vec<String> = match projects_index {
        Some(index) => parts[index + 1..].to_vec(),
        None => Vec::new(),
    };
    let is_jsonl = transcript_path.ends_with(".jsonl");

    if after_projects.len() >= 2 && is_jsonl {
        let project_key = after_projects[0].clone();
        if after_projects.len() == 2 {
            let session_id = Path::new(&after_projects[1])
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| after_projects[1].clone());
            return ClaudeSessionKey {
                key: format!("{project_key}/{session_id}"),
                subpath: None,
            };
        }
        let session_id = after_projects[1].clone();
        let mut relative = after_projects[2..].to_vec();
        if let Some(last) = relative.last_mut() {
            *last = Path::new(last)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| last.clone());
        }
        return ClaudeSessionKey {
            key: format!("{project_key}/{session_id}"),
            subpath: Some(relative.join("/")),
        };
    }

    let session_id = fallback_session_id
        .map(str::to_owned)
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "session".to_owned());
    let dir = path
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    ClaudeSessionKey {
        key: format!("{}/{session_id}", slugify(&dir)),
        subpath: None,
    }
}

fn slugify(value: &str) -> String {
    let slug: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "unknown-project".to_owned()
    } else {
        trimmed.to_owned()
    }
}

pub struct ReadResult {
    pub entries: Vec<Value>,
    pub new_offset: u64,
    pub malformed: usize,
}

// Byte-offset based, only complete trailing lines, resets to 0 if the file
// shrank below the tracked offset (rotation/truncation), skips malformed
// JSON lines without failing the batch — same reliability guarantees as the
// proven claude-code-hook-sync.py.
pub fn read_complete_new_lines(transcript_path: &str, offset: u64) -> io::Result<ReadResult> {
    let size = fs::metadata(transcript_path)?.len();
    let start_offset = if size < offset { 0 } else { offset };
    let data = fs::read(transcript_path)?;
    let data = &data[start_offset as usize..];
    let Some(last_newline) = data.iter().rposition(|&b| b == b'\n') else {
        return Ok(ReadResult {
            entries: Vec::new(),
            new_offset: start_offset,
            malformed: 0,
        });
    };
    let complete = &data[..last_newline + 1];
    let new_offset = start_offset + complete.len() as u64;
    let text = String::from_utf8_lossy(complete);
    let mut entries = Vec::new();
    let mut malformed = 0usize;
    for line in text.split('\n') {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(line) {
            Ok(value) => entries.push(value),
            Err(_) => malformed += 1,
        }
    }
    Ok(ReadResult {
        entries,
        new_offset,
        malformed,
    })
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct FileOffset {
    pub offset: u64,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SyncState {
    pub version: u32,
    pub files: HashMap<String, FileOffset>,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            version: 1,
            files: HashMap::new(),
        }
    }
}

fn state_path(env: &HashMap<String, String>) -> PathBuf {
    config_dir(env).join("session-sync-state.json")
}

pub fn load_sync_state(env: &HashMap<String, String>) -> SyncState {
    fs::read_to_string(state_path(env))
        .ok()
        .and_then(|contents| serde_json::from_str::<SyncState>(&contents).ok())
        .filter(|state| state.version == 1)
        .unwrap_or_default()
}

pub fn save_sync_state(env: &HashMap<String, String>, state: &SyncState) -> io::Result<()> {
    let path = state_path(env);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = format!("{}\n", serde_json::to_string_pretty(state)?);
    fs::write(path, contents)
}

pub fn get_offset(state: &SyncState, transcript_path: &str) -> u64 {
    state
        .files
        .get(transcript_path)
        .map(|f| f.offset)
        .unwrap_or(0)
}

pub fn set_offset(state: &mut SyncState, transcript_path: &str, offset: u64) {
    state.files.insert(
        transcript_path.to_owned(),
        FileOffset {
            offset,
            updated_at: now_millis_since_epoch(),
        },
    );
}

// `updated_at` is diagnostic-only (never parsed back), so a raw
// milliseconds-since-epoch string avoids adding a datetime crate dependency
// just for one timestamp.
fn now_millis_since_epoch() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_millis())
}
