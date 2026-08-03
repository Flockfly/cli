use serde::{Deserialize, Serialize};

pub const INIT_SNIPPET: &str = r#"## Flockfly Skills

Before starting substantial knowledge work, load the Flockfly discovery skill:

`flockfly load skill_pxJxZr7CMBMk`

The skill description is:

Discover and apply reusable Flockfly skills for non-trivial work that may benefit from a playbook, SOP, template, policy, domain guide, or workflow. Use when the task is substantial, procedural, or knowledge-heavy and a routed skill may improve the result; skip trivial edits, simple factual answers, named test commands, and tasks already governed by an explicitly selected local skill.

"#;

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub rank: u64,
    pub skill_id: String,
    pub score: f64,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LoadedFile {
    pub path: String,
    pub content: String,
}

pub fn format_search_results(results: &[SearchResult]) -> String {
    if results.is_empty() {
        return "No matching skills found.".to_owned();
    }

    results
        .iter()
        .map(|result| {
            format!(
                "{}. {}\n   name: {}\n   description: {}",
                result.rank, result.skill_id, result.name, result.description
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn format_loaded_files(files: &[LoadedFile]) -> String {
    if let [file] = files {
        return file.content.clone();
    }

    files
        .iter()
        .map(|file| {
            format!(
                "--- {} ---\n{}",
                file.path,
                file.content.strip_suffix('\n').unwrap_or(&file.content)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
