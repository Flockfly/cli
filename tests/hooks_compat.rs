use std::collections::HashMap;
use std::fs;

use flockfly::hooks::{claude_settings_path, install_global_hook, remove_global_hook, HOOK_MARKER};
use serde_json::{json, Value};
use tempfile::TempDir;

fn env_for(home: &TempDir) -> HashMap<String, String> {
    HashMap::from([("HOME".to_owned(), home.path().display().to_string())])
}

fn settings(env: &HashMap<String, String>) -> Value {
    let contents = fs::read_to_string(claude_settings_path(env)).unwrap();
    serde_json::from_str(&contents).unwrap()
}

#[test]
fn installs_stop_subagentstop_sessionend_hooks_pointing_at_session_sync() {
    let home = tempfile::tempdir().unwrap();
    let env = env_for(&home);

    let (path, events) = install_global_hook(&env).unwrap();
    assert_eq!(path, claude_settings_path(&env));
    assert_eq!(events, vec!["Stop", "SubagentStop", "SessionEnd"]);

    let parsed = settings(&env);
    assert_eq!(
        parsed["hooks"]["Stop"][0]["hooks"][0]["command"],
        "flockfly session sync --hook"
    );
    assert_eq!(parsed["hooks"]["Stop"][0]["hooks"][0]["timeout"], 30);
    assert_eq!(
        parsed["hooks"]["SubagentStop"][0]["hooks"][0]["command"],
        "flockfly session sync --hook"
    );
    assert_eq!(
        parsed["hooks"]["SessionEnd"][0]["hooks"][0]["command"],
        "flockfly session sync --hook --reconcile"
    );
    assert!(parsed["hooks"]["SessionEnd"][0]["hooks"][0]
        .get("timeout")
        .is_none());
    for event in ["Stop", "SubagentStop", "SessionEnd"] {
        let command = parsed["hooks"][event][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(command.contains(HOOK_MARKER));
    }
}

#[test]
fn is_idempotent_installing_twice_produces_byte_identical_settings() {
    let home = tempfile::tempdir().unwrap();
    let env = env_for(&home);

    install_global_hook(&env).unwrap();
    let first = fs::read_to_string(claude_settings_path(&env)).unwrap();
    install_global_hook(&env).unwrap();
    let second = fs::read_to_string(claude_settings_path(&env)).unwrap();
    assert_eq!(first, second);
}

#[test]
fn preserves_unrelated_hooks_and_settings_keys_already_present() {
    let home = tempfile::tempdir().unwrap();
    let env = env_for(&home);
    let path = claude_settings_path(&env);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        serde_json::to_string(&json!({
            "permissions": { "deny": ["Read(./secret/**)"] },
            "hooks": {
                "Stop": [{ "matcher": "", "hooks": [{ "type": "command", "command": "some-other-tool stop" }] }],
                "PostToolUse": [{ "matcher": "Task", "hooks": [{ "type": "command", "command": "entire hooks post-task" }] }],
            },
        }))
        .unwrap(),
    )
    .unwrap();

    install_global_hook(&env).unwrap();
    let parsed = settings(&env);
    assert_eq!(parsed["permissions"]["deny"][0], "Read(./secret/**)");
    assert_eq!(
        parsed["hooks"]["PostToolUse"][0]["hooks"][0]["command"],
        "entire hooks post-task"
    );

    let stop_commands: Vec<String> = parsed["hooks"]["Stop"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|block| block["hooks"].as_array().unwrap().clone())
        .map(|hook| hook["command"].as_str().unwrap().to_owned())
        .collect();
    assert!(stop_commands.contains(&"some-other-tool stop".to_owned()));
    assert!(stop_commands.contains(&"flockfly session sync --hook".to_owned()));
}

#[test]
fn remove_global_hook_strips_only_flockflys_entries_and_reports_removed_false_when_nothing_installed(
) {
    let home = tempfile::tempdir().unwrap();
    let env = env_for(&home);

    let (_, removed_before) = remove_global_hook(&env).unwrap();
    assert!(!removed_before);

    install_global_hook(&env).unwrap();
    let (_, removed_after) = remove_global_hook(&env).unwrap();
    assert!(removed_after);
    let parsed = settings(&env);
    for event in ["Stop", "SubagentStop", "SessionEnd"] {
        assert!(parsed["hooks"]
            .get(event)
            .map(|v| v.as_array().unwrap().is_empty())
            .unwrap_or(true));
    }

    let (_, removed_again) = remove_global_hook(&env).unwrap();
    assert!(!removed_again);
}
