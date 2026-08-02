use serde::{Deserialize, Serialize};

pub const INIT_SNIPPET: &str = r#"## Flockfly Skills

Before starting substantial knowledge work, search your routed skills:

`flockfly search "<task>"`

If a relevant skill appears, load it:

`flockfly load <skillId>`

Follow the loaded skill's progressive-disclosure instructions. Load referenced files only when the skill asks for them:

`flockfly load <skillId> <path...>`
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
