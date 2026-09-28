use crate::document::{Document, Task};

pub fn parse(source: &str) -> anyhow::Result<Document> {
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
                let context = parse_task_context(&lines, line_number + 1);
                tasks.push(Task {
                    index,
                    section: headings
                        .last()
                        .map(|(_, title)| title.clone())
                        .unwrap_or_default(),
                    title,
                    checked,
                    language: context.language,
                    command: context.command,
                    details: context.details,
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

struct TaskContext {
    language: Option<String>,
    command: Option<String>,
    details: Option<String>,
}

fn parse_task_context(lines: &[&str], start: usize) -> TaskContext {
    let mut line_number = start;
    while line_number < lines.len() && lines[line_number].trim().is_empty() {
        line_number += 1;
    }

    let mut language = None;
    let mut command = None;
    let mut details = Vec::new();
    let mut in_fence = false;
    let mut first_content = true;
    while line_number < lines.len() {
        let content = lines[line_number].trim_end_matches(['\r', '\n']);
        if !in_fence && (parse_heading(content).is_some() || parse_task(content, 0).is_some()) {
            break;
        }
        if !in_fence && opening_fence(content.trim()).is_some() {
            let fence_language = opening_fence(content.trim()).unwrap_or_default();
            in_fence = true;
            let mut code = Vec::new();
            line_number += 1;
            while line_number < lines.len() && !is_closing_fence(lines[line_number].trim()) {
                code.push(lines[line_number].trim_end_matches(['\r', '\n']));
                line_number += 1;
            }
            if line_number < lines.len() {
                if first_content && language.is_none() {
                    language = Some(fence_language);
                    command = Some(code.join("\n"));
                }
                in_fence = false;
                line_number += 1;
            }
            first_content = false;
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
            first_content = false;
        }
        line_number += 1;
    }
    TaskContext {
        language,
        command,
        details: (!details.is_empty()).then(|| details.join("\n")),
    }
}
