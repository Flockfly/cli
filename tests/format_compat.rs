use flockfly::format::{
    format_loaded_files, format_search_results, LoadedFile, SearchResult, INIT_SNIPPET,
};

#[test]
fn ts_prints_rank_skill_id_and_frontmatter_only() {
    let output = format_search_results(&[
        SearchResult {
            rank: 1,
            skill_id: "skill_abc123".into(),
            score: 110.0,
            name: "pdd".into(),
            description: "Transform ideas into plans.".into(),
        },
        SearchResult {
            rank: 2,
            skill_id: "skill_def456".into(),
            score: 10.0,
            name: "codebase-summary".into(),
            description: "Analyze a codebase.".into(),
        },
    ]);

    assert_eq!(
        output,
        [
            "1. skill_abc123",
            "   name: pdd",
            "   description: Transform ideas into plans.",
            "",
            "2. skill_def456",
            "   name: codebase-summary",
            "   description: Analyze a codebase.",
        ]
        .join("\n")
    );
    assert!(!output.contains("110"));
}

#[test]
fn ts_handles_empty_results() {
    assert_eq!(format_search_results(&[]), "No matching skills found.");
}

#[test]
fn ts_prints_a_single_file_raw() {
    assert_eq!(
        format_loaded_files(&[LoadedFile {
            path: "SKILL.md".into(),
            content: "# Hello\n".into(),
        }]),
        "# Hello\n"
    );
}

#[test]
fn ts_adds_boundaries_for_multiple_files() {
    let output = format_loaded_files(&[
        LoadedFile {
            path: "references/a.md".into(),
            content: "Alpha\n".into(),
        },
        LoadedFile {
            path: "references/b.md".into(),
            content: "Beta\n".into(),
        },
    ]);

    assert_eq!(
        output,
        "--- references/a.md ---\nAlpha\n\n--- references/b.md ---\nBeta"
    );
}

#[test]
fn ts_loads_the_flockfly_discovery_skill_with_its_activation_description() {
    assert!(INIT_SNIPPET.contains("flockfly load skill_pxJxZr7CMBMk"));
    assert!(INIT_SNIPPET.contains(
        "Discover and apply reusable Flockfly skills for non-trivial work that may benefit from a playbook, SOP, template, policy, domain guide, or workflow."
    ));
    assert!(INIT_SNIPPET.contains(
        "Use when the task is substantial, procedural, or knowledge-heavy and a routed skill may improve the result; skip trivial edits, simple factual answers, named test commands, and tasks already governed by an explicitly selected local skill."
    ));
}
