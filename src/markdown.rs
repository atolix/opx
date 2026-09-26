use anyhow::{Context, Result};
use std::{fs, path::Path};

use crate::document::{Document, Task};

pub fn load(path: &Path) -> Result<Document> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    parse(&source)
}

pub fn save(path: &Path, document: &Document) -> Result<()> {
    fs::write(path, &document.source).with_context(|| format!("failed to write {}", path.display()))
}

pub fn parse(source: &str) -> Result<Document> {
    let mut tasks = Vec::new();
    let mut headings: Vec<(usize, String)> = Vec::new();
    let lines = source.split_inclusive('\n').collect::<Vec<_>>();
    let mut offset = 0;
    let mut index = 0;
    let mut line_number = 0;
    let mut in_fence = false;
    while line_number < lines.len() {
        let line = lines[line_number];
        let content = line.trim_end_matches(['\r', '\n']);
        if !in_fence {
            if let Some((level, heading)) = parse_heading(content) {
                headings.retain(|(heading_level, _)| *heading_level < level);
                headings.push((level, heading));
            }
        }
        if !in_fence {
            if let Some((marker_offset, checked, title)) = parse_task(content, offset) {
                let mut language = None;
                let mut command = None;
                let details = parse_task_details(&lines, line_number + 1);
                let mut lookahead = line_number + 1;
                while lookahead < lines.len() && lines[lookahead].trim().is_empty() {
                    lookahead += 1;
                }
                if lookahead < lines.len() {
                    if let Some(fence_language) = opening_fence(lines[lookahead].trim()) {
                        let mut code = Vec::new();
                        let mut end = lookahead + 1;
                        while end < lines.len() && !is_closing_fence(lines[end].trim()) {
                            code.push(lines[end].trim_end_matches(['\r', '\n']));
                            end += 1;
                        }
                        if end < lines.len() {
                            language = Some(fence_language);
                            command = Some(code.join("\n"));
                        }
                    }
                }
                tasks.push(Task {
                    index,
                    section: headings
                        .last()
                        .map(|(_, title)| title.clone())
                        .unwrap_or_default(),
                    title,
                    checked,
                    language,
                    command,
                    details,
                    section_path: headings.iter().map(|(_, title)| title.clone()).collect(),
                    section_level: headings.last().map(|(level, _)| *level).unwrap_or(0),
                    marker_offset,
                });
                index += 1;
            }
        }
        if is_fence_line(content) {
            in_fence = !in_fence;
        }
        offset += line.len();
        line_number += 1;
    }
    Ok(Document {
        source: source.to_owned(),
        tasks,
    })
}

fn parse_heading(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && trimmed.chars().nth(hashes) == Some(' ') {
        Some((
            hashes,
            trimmed[hashes + 1..]
                .trim_end_matches('#')
                .trim()
                .to_owned(),
        ))
    } else {
        None
    }
}

fn parse_task(line: &str, offset: usize) -> Option<(usize, bool, String)> {
    let leading = line.len() - line.trim_start().len();
    let body = &line[leading..];
    if !body.starts_with("- [") || body.len() < 6 || !matches!(body.as_bytes().get(4), Some(b']')) {
        return None;
    }
    let state = body.as_bytes()[3];
    if state != b' ' && state != b'x' && state != b'X' {
        return None;
    }
    Some((
        offset + leading + 3,
        state != b' ',
        body[5..].trim().to_owned(),
    ))
}

fn opening_fence(line: &str) -> Option<String> {
    let rest = line.strip_prefix("```")?;
    if rest.contains('`') {
        return None;
    }
    Some(rest.trim().to_owned())
}

fn is_closing_fence(line: &str) -> bool {
    line == "```"
}

fn is_fence_line(line: &str) -> bool {
    line == "```" || (line.starts_with("```") && !line[3..].contains('`'))
}

fn parse_task_details(lines: &[&str], start: usize) -> Option<String> {
    let mut line_number = start;
    while line_number < lines.len() && lines[line_number].trim().is_empty() {
        line_number += 1;
    }

    let mut details = Vec::new();
    let mut in_fence = false;
    while line_number < lines.len() {
        let content = lines[line_number].trim_end_matches(['\r', '\n']);
        if !in_fence && (parse_heading(content).is_some() || parse_task(content, 0).is_some()) {
            break;
        }
        if !in_fence && opening_fence(content.trim()).is_some() {
            in_fence = true;
            line_number += 1;
            continue;
        }
        if in_fence {
            if is_closing_fence(content.trim()) {
                in_fence = false;
            }
            line_number += 1;
            continue;
        }
        if !content.trim().is_empty() {
            details.push(content.trim().to_owned());
        }
        line_number += 1;
    }
    (!details.is_empty()).then(|| details.join("\n"))
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
        assert_eq!(changed.tasks[0].checked, true);
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
}
