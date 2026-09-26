use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Task {
    pub index: usize,
    pub section: String,
    pub title: String,
    pub checked: bool,
    pub language: Option<String>,
    pub command: Option<String>,
    #[serde(skip)]
    pub section_path: Vec<String>,
    #[serde(skip)]
    pub section_level: usize,
    #[serde(skip)]
    pub marker_offset: usize,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub source: String,
    pub tasks: Vec<Task>,
}

impl Document {
    pub fn completed_count(&self) -> usize {
        self.tasks.iter().filter(|task| task.checked).count()
    }

    pub fn progress(&self) -> f64 {
        if self.tasks.is_empty() {
            0.0
        } else {
            self.completed_count() as f64 / self.tasks.len() as f64
        }
    }

    pub fn next_unchecked(&self, after: Option<usize>) -> Option<&Task> {
        let start = after.map_or(0, |index| index.saturating_add(1));
        self.tasks[start..]
            .iter()
            .find(|task| !task.checked)
            .or_else(|| self.tasks.iter().find(|task| !task.checked))
    }

    pub fn with_checked(&self, index: usize, checked: bool) -> anyhow::Result<Self> {
        let task = self
            .tasks
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("task index out of range: {index}"))?;
        let mut source = self.source.clone();
        source.replace_range(
            task.marker_offset..task.marker_offset + 1,
            if checked { "x" } else { " " },
        );
        crate::markdown::parse(&source)
    }
}
