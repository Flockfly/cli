use std::fs;
use std::path::Path;

use base64::Engine;
use serde::Serialize;

use crate::api::CliError;

const SKILL_FILE: &str = "SKILL.md";
const SKIP_DIRS: [&str; 4] = [".git", "node_modules", "__pycache__", ".DS_Store"];

#[derive(Clone, Debug)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFile {
    pub path: String,
    pub content_base64: String,
}

#[derive(Clone, Debug)]
pub struct PackagedSkill {
    pub frontmatter: SkillFrontmatter,
    pub files: Vec<PackageFile>,
}

pub fn package_skill_directory(directory: &Path) -> Result<PackagedSkill, CliError> {
    if !directory.exists() {
        return Err(CliError::message(format!(
            "Path not found: {}",
            directory.display()
        )));
    }
    if !directory.is_dir() {
        return Err(CliError::message(format!(
            "{} is not a directory. Publish a skill directory containing SKILL.md.",
            directory.display()
        )));
    }

    let root = fs::canonicalize(directory).map_err(io_error)?;
    let mut files = Vec::new();
    walk(&root, &root, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let skill_file = files
        .iter()
        .find(|file| file.path == SKILL_FILE)
        .ok_or_else(|| {
            CliError::message(format!(
                "No {SKILL_FILE} found in {}. A skill package is a directory with a {SKILL_FILE} at its root plus any referenced files.",
                directory.display()
            ))
        })?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&skill_file.content_base64)
        .map_err(|error| CliError::message(error.to_string()))?;
    let content = String::from_utf8(bytes).map_err(|error| CliError::message(error.to_string()))?;
    let frontmatter = parse_frontmatter(&content)?;

    Ok(PackagedSkill { frontmatter, files })
}

fn walk(root: &Path, current: &Path, files: &mut Vec<PackageFile>) -> Result<(), CliError> {
    let mut entries = fs::read_dir(current)
        .map_err(io_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io_error)?;
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if SKIP_DIRS.contains(&name.as_ref()) {
            continue;
        }

        let full = entry.path();
        let relative = slash_relative(root, &full)?;
        let metadata = fs::symlink_metadata(&full).map_err(io_error)?;
        if metadata.file_type().is_symlink() {
            let target = fs::canonicalize(&full).map_err(io_error)?;
            if target != root && !target.starts_with(root) {
                return Err(CliError::message(format!(
                    "Symlink escapes the skill directory: {relative}. Remove it or replace it with a real file."
                )));
            }
        }

        if fs::metadata(&full).map_err(io_error)?.is_dir() {
            walk(root, &full, files)?;
            continue;
        }

        let path = check_package_path(&relative)?;
        let bytes = fs::read(&full).map_err(io_error)?;
        files.push(PackageFile {
            path,
            content_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        });
    }
    Ok(())
}

fn slash_relative(root: &Path, path: &Path) -> Result<String, CliError> {
    path.strip_prefix(root)
        .map(|relative| {
            relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        })
        .map_err(|error| CliError::message(error.to_string()))
}

fn check_package_path(raw: &str) -> Result<String, CliError> {
    let invalid =
        |reason: &str| CliError::message(format!("Invalid package path {raw}: {reason}."));
    if raw.is_empty() {
        return Err(invalid("path is empty"));
    }
    if raw.len() > 512 {
        return Err(invalid("path is too long"));
    }
    if raw.contains('\\') {
        return Err(invalid("path must use forward slashes"));
    }
    if raw.contains('\0') {
        return Err(invalid("path contains a null byte"));
    }
    let mut chars = raw.chars();
    if raw.starts_with('/')
        || matches!((chars.next(), chars.next()), (Some(letter), Some(':')) if letter.is_ascii_alphabetic())
    {
        return Err(invalid("path must be relative"));
    }

    let mut normalized = Vec::new();
    for segment in raw.split('/') {
        match segment {
            "" | "." => {}
            ".." => return Err(invalid("path must not contain parent traversal")),
            value => normalized.push(value),
        }
    }
    if normalized.is_empty() {
        return Err(invalid("path is empty"));
    }
    Ok(normalized.join("/"))
}

fn parse_frontmatter(content: &str) -> Result<SkillFrontmatter, CliError> {
    let rest = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
        .ok_or_else(frontmatter_error)?;
    let mut offset = 0;
    let mut yaml = None;
    for line in rest.split_inclusive('\n') {
        let line_without_ending = line
            .strip_suffix('\n')
            .unwrap_or(line)
            .strip_suffix('\r')
            .unwrap_or_else(|| line.strip_suffix('\n').unwrap_or(line));
        if line_without_ending == "---" {
            yaml = Some(&rest[..offset]);
            break;
        }
        offset += line.len();
    }
    let yaml = yaml.ok_or_else(frontmatter_error)?;
    let value: serde_yaml::Value = serde_yaml::from_str(yaml).map_err(|_| frontmatter_error())?;
    let record = value.as_mapping().ok_or_else(frontmatter_error)?;
    let key = |name: &str| serde_yaml::Value::String(name.to_owned());
    let name = record
        .get(key("name"))
        .and_then(serde_yaml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::message("SKILL.md frontmatter is missing `name`."))?;
    let description = record
        .get(key("description"))
        .and_then(serde_yaml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::message("SKILL.md frontmatter is missing `description`."))?;

    Ok(SkillFrontmatter {
        name: name.to_owned(),
        description: description.to_owned(),
    })
}

fn frontmatter_error() -> CliError {
    CliError::message(
        "SKILL.md must start with YAML frontmatter containing `name` and `description`.",
    )
}

fn io_error(error: std::io::Error) -> CliError {
    CliError::message(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{check_package_path, parse_frontmatter};

    #[test]
    fn shared_paths_accept_and_normalize_simple_relative_paths() {
        assert_eq!(check_package_path("SKILL.md").unwrap(), "SKILL.md");
        assert_eq!(
            check_package_path("./references//guide.md").unwrap(),
            "references/guide.md"
        );
    }

    #[test]
    fn shared_paths_reject_unsafe_and_empty_paths() {
        for path in [
            "/etc/passwd",
            "C:/secret",
            "../secret",
            "references/../secret",
            r"references\secret",
            "\0",
            "",
            ".",
        ] {
            assert!(check_package_path(path).is_err(), "{path:?} was accepted");
        }
    }

    #[test]
    fn shared_frontmatter_validates_required_fields_and_yaml() {
        let parsed =
            parse_frontmatter("---\nname:  pdd  \ndescription:  Plan things.  \n---\nBody")
                .unwrap();
        assert_eq!(parsed.name, "pdd");
        assert_eq!(parsed.description, "Plan things.");

        assert!(parse_frontmatter("no frontmatter").is_err());
        assert!(parse_frontmatter("---\ndescription: x\n---\n").is_err());
        assert!(parse_frontmatter("---\nname: x\n---\n").is_err());
        assert!(parse_frontmatter("---\nname: [broken\n---\n").is_err());
        assert!(parse_frontmatter("---\nname: x\ndescription: y\n---not-a-close").is_err());
    }
}
