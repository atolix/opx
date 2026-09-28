mod parser;

use anyhow::{Context, Result};
use std::{fs, io::Write, path::Path};

use tempfile::NamedTempFile;

use crate::document::Document;

pub use parser::parse;

pub fn load(path: &Path) -> Result<Document> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    parse(&source)
}

pub fn save(path: &Path, document: &Document) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary file near {}", path.display()))?;
    temporary
        .write_all(document.source.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;
    temporary
        .as_file()
        .sync_all()
        .with_context(|| format!("failed to flush {}", path.display()))?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to replace {}", path.display()))
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Deploy\n\n## Build\n\n- [ ] Docker imageをbuildする\n\n```sh\ndocker build -t app .\n```\n\n- [X] imageを確認する\n\n## Database\n\n- [x] migrationを実行する\n\n```sh\nbundle exec rails db:migrate\n```\n";

    #[test]
    fn parses_tasks_headings_and_commands() {
        let document = parse(SAMPLE).unwrap();
        assert_eq!(document.tasks.len(), 3);
        assert_eq!(document.tasks[0].section, "Build");
        assert!(!document.tasks[0].checked);
        assert_eq!(document.tasks[0].language.as_deref(), Some("sh"));
        assert_eq!(
            document.tasks[0].command.as_deref(),
            Some("docker build -t app .")
        );
        assert!(document.tasks[1].checked);
        assert_eq!(document.tasks[2].section, "Database");
    }

    #[test]
    fn associates_text_after_task_as_details() {
        let document = parse(
            "## Deploy\n\n- [ ] deployする\n\n本番環境では承認後に実行します。\n\n- [ ] 確認する\n",
        )
        .unwrap();
        assert_eq!(
            document.tasks[0].details.as_deref(),
            Some("本番環境では承認後に実行します。")
        );
        assert_eq!(document.tasks[1].details, None);
    }

    #[test]
    fn keeps_heading_hierarchy_for_nested_sections() {
        let document = parse("# Deploy\n## Production\n### Rollout\n- [ ] check\n").unwrap();
        assert_eq!(document.tasks[0].section, "Rollout");
        assert_eq!(document.tasks[0].section_level, 3);
        assert_eq!(
            document.tasks[0].section_path,
            ["Deploy", "Production", "Rollout"]
        );
    }

    #[test]
    fn changing_checkbox_preserves_everything_else() {
        let document = parse(SAMPLE).unwrap();
        let changed = document.with_checked(0, true).unwrap();
        let mut expected = SAMPLE.to_owned();
        expected.replace_range(
            document.tasks[0].marker_offset..document.tasks[0].marker_offset + 1,
            "x",
        );
        assert_eq!(changed.source, expected);
        assert!(changed.tasks[0].checked);
    }

    #[test]
    fn accepts_uppercase_checked_marker() {
        let document = parse("- [X] done\n- [ ] todo\n").unwrap();
        assert!(document.tasks[0].checked);
        assert!(!document.tasks[1].checked);
    }

    #[test]
    fn ignores_task_like_text_inside_fences() {
        let document = parse("```md\n- [ ] example\n```\n\n- [ ] real\n").unwrap();
        assert_eq!(document.tasks.len(), 1);
        assert_eq!(document.tasks[0].title, "real");
    }

    #[test]
    fn saves_and_reloads_the_markdown_source() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("runbook.md");
        let document = parse("# Test\n\n- [ ] verify\n").unwrap();

        save(&path, &document).unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.source, document.source);
        assert_eq!(loaded.tasks[0].title, "verify");
    }
}
