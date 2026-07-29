#[cfg(unix)]
mod unix {
    use std::fs;
    use std::os::unix::fs::symlink;

    use flockfly::package::package_skill_directory;
    use tempfile::tempdir;

    const VALID_SKILL: &str = "---\nname: a\ndescription: b\n---\n";

    #[test]
    fn ts_packages_a_fixture_skill_with_sorted_relative_paths() {
        let fixture =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/skills/pdd");
        let packaged = package_skill_directory(&fixture).unwrap();

        assert_eq!(packaged.frontmatter.name, "pdd");
        assert_eq!(
            packaged
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "SKILL.md",
                "references/design-template.md",
                "references/task-template.md",
            ]
        );
    }

    #[test]
    fn ts_rejects_a_missing_path_and_non_directories() {
        let error =
            package_skill_directory(std::path::Path::new("/nonexistent/skill")).unwrap_err();
        assert!(error.to_string().contains("Path not found"));

        let dir = tempdir().unwrap();
        let file = dir.path().join("SKILL.md");
        fs::write(&file, VALID_SKILL).unwrap();
        let error = package_skill_directory(&file).unwrap_err();
        assert!(error.to_string().contains("not a directory"));
    }

    #[test]
    fn ts_rejects_a_directory_without_skill_md() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("notes.md"), "hello").unwrap();

        let error = package_skill_directory(dir.path()).unwrap_err();
        assert!(error.to_string().contains("No SKILL.md found"));
    }

    #[test]
    fn ts_rejects_missing_frontmatter_fields_with_actionable_messages() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("SKILL.md"), "---\nname: a\n---\nbody").unwrap();
        let error = package_skill_directory(dir.path()).unwrap_err();
        assert!(error.to_string().contains("missing `description`"));

        fs::write(dir.path().join("SKILL.md"), "no frontmatter").unwrap();
        let error = package_skill_directory(dir.path()).unwrap_err();
        assert!(error.to_string().contains("YAML frontmatter"));
    }

    #[test]
    fn ts_rejects_symlinks_that_escape_the_skill_directory() {
        let outside = tempdir().unwrap();
        fs::write(outside.path().join("secret.md"), "secret").unwrap();
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("SKILL.md"), VALID_SKILL).unwrap();
        symlink(outside.path().join("secret.md"), dir.path().join("leak.md")).unwrap();

        let error = package_skill_directory(dir.path()).unwrap_err();
        assert!(error.to_string().contains("Symlink escapes"));
    }

    #[test]
    fn ts_allows_symlinks_that_stay_inside_the_skill_directory() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("SKILL.md"), VALID_SKILL).unwrap();
        fs::create_dir(dir.path().join("references")).unwrap();
        fs::write(dir.path().join("references/real.md"), "content").unwrap();
        symlink(
            dir.path().join("references/real.md"),
            dir.path().join("alias.md"),
        )
        .unwrap();

        let packaged = package_skill_directory(dir.path()).unwrap();
        assert!(packaged.files.iter().any(|file| file.path == "alias.md"));
    }
}
