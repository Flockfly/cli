use std::fs;

use flockfly::hook_sync::{derive_claude_session_key, read_complete_new_lines};

fn fixture(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude-transcripts")
        .join(name)
        .display()
        .to_string()
}

#[test]
fn derives_key_and_subpath_from_a_main_transcript_path() {
    let path = "/Users/jkim/.claude/projects/-Users-jkim-repo/abc-123.jsonl";
    let key = derive_claude_session_key(path, None);
    assert_eq!(key.key, "-Users-jkim-repo/abc-123");
    assert!(key.subpath.is_none());
}

#[test]
fn derives_a_non_null_subpath_for_a_subagent_transcript_nested_under_the_session() {
    let path = "/Users/jkim/.claude/projects/-Users-jkim-repo/abc-123/subagents/agent-1.jsonl";
    let key = derive_claude_session_key(path, None);
    assert_eq!(key.key, "-Users-jkim-repo/abc-123");
    assert_eq!(key.subpath.as_deref(), Some("subagents/agent-1"));
}

#[test]
fn falls_back_to_a_slugified_directory_and_session_id_without_a_projects_segment() {
    let key = derive_claude_session_key("/tmp/weird place/session.jsonl", Some("fallback-session"));
    assert!(key.subpath.is_none());
    assert_eq!(key.key, "tmp-weird-place/fallback-session");
}

#[test]
fn reads_only_complete_trailing_lines_ignoring_a_partial_final_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("transcript.jsonl");
    fs::write(&path, "{\"n\":1}\n{\"n\":2}\n{\"n\":3 incomple").unwrap();
    let result = read_complete_new_lines(path.to_str().unwrap(), 0).unwrap();
    assert_eq!(result.entries.len(), 2);
    assert_eq!(result.entries[0]["n"], 1);
    assert_eq!(result.entries[1]["n"], 2);
    assert_eq!(result.malformed, 0);
    assert!(result.new_offset > 0);
}

#[test]
fn resumes_from_a_prior_offset_and_only_returns_newly_appended_complete_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("transcript.jsonl");
    fs::write(&path, "{\"n\":1}\n").unwrap();
    let first = read_complete_new_lines(path.to_str().unwrap(), 0).unwrap();
    assert_eq!(first.entries.len(), 1);

    let mut existing = fs::read_to_string(&path).unwrap();
    existing.push_str("{\"n\":2}\n{\"n\":3}\n");
    fs::write(&path, existing).unwrap();

    let second = read_complete_new_lines(path.to_str().unwrap(), first.new_offset).unwrap();
    assert_eq!(second.entries.len(), 2);
    assert_eq!(second.entries[0]["n"], 2);
    assert_eq!(second.entries[1]["n"], 3);
}

#[test]
fn resets_to_offset_zero_when_the_file_has_shrunk_below_the_tracked_offset() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("transcript.jsonl");
    fs::write(&path, "{\"n\":1}\n{\"n\":2}\n").unwrap();
    let first = read_complete_new_lines(path.to_str().unwrap(), 0).unwrap();

    fs::write(&path, "{\"n\":9}\n").unwrap();
    let second = read_complete_new_lines(path.to_str().unwrap(), first.new_offset).unwrap();
    assert_eq!(second.entries.len(), 1);
    assert_eq!(second.entries[0]["n"], 9);
}

#[test]
fn skips_malformed_json_lines_without_failing_the_batch() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("transcript.jsonl");
    fs::write(&path, "{\"n\":1}\nnot json\n{\"n\":2}\n").unwrap();
    let result = read_complete_new_lines(path.to_str().unwrap(), 0).unwrap();
    assert_eq!(result.entries.len(), 2);
    assert_eq!(result.malformed, 1);
}

#[test]
fn reads_the_shared_realistic_claude_transcript_fixture() {
    let path = fixture("sample-session.jsonl");
    let result = read_complete_new_lines(&path, 0).unwrap();
    assert_eq!(result.entries.len(), 3);
    assert_eq!(result.entries[0]["uuid"], "entry-user-1");
    assert_eq!(result.entries[1]["uuid"], "entry-tool-1");
    assert_eq!(result.entries[2]["uuid"], "entry-result-1");
}
