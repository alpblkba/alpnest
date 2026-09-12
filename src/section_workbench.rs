//! The section workbench: a standalone working surface for one section.
//!
//! Design rationale
//! ----------------
//! Trello-style boards make you *maintain containers* — you drag cards between
//! columns and the real state lives in a database you can only reach through
//! the UI. That is the wrong shape for a terminal-native, markdown-backed tool.
//!
//! The workbench inverts it: **the document is the state**. Steps are the
//! `- [ ]` / `- [x]` checkboxes already inside the section's markdown. Toggling
//! a step rewrites that exact line in the file, so editing in vim and toggling
//! in the TUI are the same operation on the same bytes. Nothing to sync, no
//! second source of truth, and the file stays readable without Alpnest.
//!
//! On top of that the view answers one question a board never does: *what do I
//! do next?* The first unfinished step is pinned and highlighted, so opening a
//! section tells you the next action instead of showing a wall of containers.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::content::{Content, Panel, Section};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkbenchFocus {
    Steps,
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// 0-based line in the body file, so a toggle can rewrite exactly one line.
    pub line_index: usize,
    pub text: String,
    pub done: bool,
    pub depth: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SectionWorkbench {
    pub content_title: String,
    pub panel_title: String,
    pub section_title: String,

    pub body_path: PathBuf,
    pub context_path: Option<PathBuf>,
    pub prompt_path: Option<PathBuf>,

    pub steps: Vec<Step>,
    pub selected_step: usize,
    pub focus: WorkbenchFocus,
    pub body_scroll: u16,
    pub show_local_draft: bool,
    pub status: Option<String>,
    pub loaded: bool,
}

impl Default for WorkbenchFocus {
    fn default() -> Self {
        Self::Steps
    }
}

impl SectionWorkbench {
    pub fn open(content: &Content, panel: &Panel, section: &Section) -> Self {
        let mut workbench = Self {
            content_title: content.title.clone(),
            panel_title: panel.title.clone(),
            section_title: section.title.clone(),
            body_path: section.body_path.clone(),
            context_path: section.context_path.clone(),
            prompt_path: panel.prompt_path.clone(),
            focus: WorkbenchFocus::Steps,
            loaded: true,
            ..Self::default()
        };

        workbench.reload();
        workbench
    }

    pub fn breadcrumb(&self) -> String {
        format!(
            "{} / {} / {}",
            self.content_title, self.panel_title, self.section_title
        )
    }

    pub fn reload(&mut self) {
        let text = fs::read_to_string(&self.body_path).unwrap_or_default();
        self.steps = parse_steps(&text);

        if self.selected_step >= self.steps.len() {
            self.selected_step = self.steps.len().saturating_sub(1);
        }
    }

    pub fn body_text(&self) -> String {
        fs::read_to_string(&self.body_path).unwrap_or_else(|err| {
            format!(
                "# unreadable\n\n{}\n\npath: {}",
                err,
                self.body_path.display()
            )
        })
    }

    pub fn context_text(&self) -> String {
        match self.context_path.as_deref() {
            Some(path) => fs::read_to_string(path).unwrap_or_else(|_| {
                "No context file could be read for this section.".to_string()
            }),
            None => {
                "This section has no context file yet.\n\nCook one from the section wizard, or \
                 create `<section>.context.md` beside the body."
                    .to_string()
            }
        }
    }

    pub fn local_draft_path(&self) -> PathBuf {
        crate::local_llm::draft_path(&self.body_path)
    }

    pub fn local_draft_text(&self) -> Option<String> {
        fs::read_to_string(self.local_draft_path()).ok()
    }

    pub fn toggle_brief_source(&mut self) {
        if self.show_local_draft {
            self.show_local_draft = false;
            self.status = Some("showing section context".to_string());
        } else if self.local_draft_path().is_file() {
            self.show_local_draft = true;
            self.status = Some("showing local draft".to_string());
        } else {
            self.status = Some("no local draft yet — press a to create one".to_string());
        }
    }

    pub fn total_steps(&self) -> usize {
        self.steps.len()
    }

    pub fn done_steps(&self) -> usize {
        self.steps.iter().filter(|step| step.done).count()
    }

    pub fn progress_ratio(&self) -> f64 {
        if self.steps.is_empty() {
            return 0.0;
        }

        self.done_steps() as f64 / self.steps.len() as f64
    }

    /// The first unfinished step: the single thing this view exists to surface.
    pub fn next_action(&self) -> Option<&Step> {
        self.steps.iter().find(|step| !step.done)
    }

    pub fn next_action_index(&self) -> Option<usize> {
        self.steps.iter().position(|step| !step.done)
    }

    pub fn jump_to_next_action(&mut self) {
        match self.next_action_index() {
            Some(index) => {
                self.selected_step = index;
                self.focus = WorkbenchFocus::Steps;
                self.status = Some("jumped to next action".to_string());
            }
            None => {
                self.status = Some("every step is done".to_string());
            }
        }
    }

    pub fn move_next(&mut self) {
        match self.focus {
            WorkbenchFocus::Steps => {
                if !self.steps.is_empty() {
                    self.selected_step = (self.selected_step + 1).min(self.steps.len() - 1);
                }
            }
            WorkbenchFocus::Body => {
                self.body_scroll = self.body_scroll.saturating_add(1);
            }
        }
    }

    pub fn move_prev(&mut self) {
        match self.focus {
            WorkbenchFocus::Steps => {
                self.selected_step = self.selected_step.saturating_sub(1);
            }
            WorkbenchFocus::Body => {
                self.body_scroll = self.body_scroll.saturating_sub(1);
            }
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            WorkbenchFocus::Steps => WorkbenchFocus::Body,
            WorkbenchFocus::Body => WorkbenchFocus::Steps,
        };
    }

    /// Flip the selected step's checkbox in the file itself.
    pub fn toggle_selected_step(&mut self) -> io::Result<()> {
        let Some(step) = self.steps.get(self.selected_step).cloned() else {
            self.status = Some("no steps in this section yet".to_string());
            return Ok(());
        };

        toggle_step_in_file(&self.body_path, step.line_index)?;
        self.reload();

        self.status = Some(if step.done {
            format!("reopened: {}", step.text)
        } else {
            format!("done: {}", step.text)
        });

        Ok(())
    }

    /// Rows for the steps rail: `(depth, text, done, selected, is_next)`.
    pub fn step_rows(&self) -> Vec<(usize, String, bool, bool, bool)> {
        let next = self.next_action_index();

        self.steps
            .iter()
            .enumerate()
            .map(|(index, step)| {
                (
                    step.depth,
                    step.text.clone(),
                    step.done,
                    index == self.selected_step,
                    Some(index) == next,
                )
            })
            .collect()
    }

    /// Derived facts about this section, shown under the brief.
    pub fn signal_rows(&self) -> Vec<(String, String)> {
        let mut rows = vec![(
            "steps".to_string(),
            if self.steps.is_empty() {
                "none yet".to_string()
            } else {
                format!("{} of {} done", self.done_steps(), self.total_steps())
            },
        )];

        rows.push((
            "next".to_string(),
            self.next_action()
                .map(|step| truncate(&step.text, 28))
                .unwrap_or_else(|| "nothing open".to_string()),
        ));

        rows.push((
            "context".to_string(),
            if self.context_path.is_some() {
                "attached".to_string()
            } else {
                "missing".to_string()
            },
        ));

        rows.push((
            "panel prompt".to_string(),
            if self.prompt_path.is_some() {
                "attached".to_string()
            } else {
                "none".to_string()
            },
        ));

        rows.push((
            "local draft".to_string(),
            if self.local_draft_path().is_file() {
                "ready; press v to view".to_string()
            } else {
                "not generated".to_string()
            },
        ));

        if let Ok(metadata) = fs::metadata(&self.body_path) {
            rows.push(("size".to_string(), format!("{} bytes", metadata.len())));
        }

        rows
    }
}

/// Extract `- [ ]` / `- [x]` lines as steps, keeping their line numbers.
pub fn parse_steps(text: &str) -> Vec<Step> {
    let mut steps = Vec::new();

    for (line_index, line) in text.lines().enumerate() {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();

        let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        else {
            continue;
        };

        let done = if rest.starts_with("[ ]") {
            false
        } else if rest.starts_with("[x]") || rest.starts_with("[X]") {
            true
        } else {
            continue;
        };

        let text = rest[3..].trim().to_string();

        if text.is_empty() {
            continue;
        }

        steps.push(Step {
            line_index,
            text,
            done,
            depth: indent / 2,
        });
    }

    steps
}

/// Rewrite one checkbox in place, leaving every other byte of the file alone.
pub fn toggle_step_in_file(path: &Path, line_index: usize) -> io::Result<()> {
    let text = fs::read_to_string(path)?;
    let ends_with_newline = text.ends_with('\n');
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();

    let Some(line) = lines.get_mut(line_index) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("line {line_index} is outside {}", path.display()),
        ));
    };

    let Some(marker_at) = line.find("- [").or_else(|| line.find("* [")) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "selected line is no longer a checkbox",
        ));
    };

    let box_at = marker_at + 3;

    let current = line.as_bytes().get(box_at).copied();
    let replacement = match current {
        Some(b' ') => 'x',
        Some(b'x') | Some(b'X') => ' ',
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "selected line is no longer a checkbox",
            ));
        }
    };

    line.replace_range(box_at..box_at + 1, &replacement.to_string());

    let mut rewritten = lines.join("\n");

    if ends_with_newline {
        rewritten.push('\n');
    }

    fs::write(path, rewritten)
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }

    let clipped: String = value.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", clipped.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# notes

Some prose that is not a step.

- [ ] read chapter 3
- [x] set up the toolchain
  - [ ] install vitis
* [X] star bullets count too
- not a checkbox
- [ ]
";

    #[test]
    fn parses_checkboxes_and_ignores_everything_else() {
        let steps = parse_steps(SAMPLE);

        assert_eq!(steps.len(), 4);
        assert_eq!(steps[0].text, "read chapter 3");
        assert!(!steps[0].done);
        assert!(steps[1].done);
        assert_eq!(steps[2].depth, 1, "nested steps keep their indent depth");
        assert!(steps[3].done, "* [X] is a done step");
    }

    #[test]
    fn empty_checkbox_text_is_not_a_step() {
        let steps = parse_steps("- [ ]\n");
        assert!(steps.is_empty());
    }

    #[test]
    fn next_action_is_the_first_unfinished_step() {
        let workbench = SectionWorkbench {
            steps: parse_steps(SAMPLE),
            ..SectionWorkbench::default()
        };

        assert_eq!(workbench.next_action().map(|s| s.text.as_str()), Some("read chapter 3"));
        assert_eq!(workbench.done_steps(), 2);
        assert_eq!(workbench.total_steps(), 4);
        assert!((workbench.progress_ratio() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn no_steps_means_zero_progress_not_a_divide_by_zero() {
        let workbench = SectionWorkbench::default();

        assert_eq!(workbench.progress_ratio(), 0.0);
        assert!(workbench.next_action().is_none());
    }

    fn temp_file(name: &str, contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("alpnest-workbench-{name}.md"));
        fs::write(&path, contents).expect("seed");
        path
    }

    #[test]
    fn toggling_rewrites_only_the_target_line() {
        let path = temp_file("toggle", SAMPLE);
        let steps = parse_steps(SAMPLE);

        toggle_step_in_file(&path, steps[0].line_index).expect("toggle");

        let after = fs::read_to_string(&path).expect("read back");
        assert!(after.contains("- [x] read chapter 3"));
        assert!(after.contains("- [x] set up the toolchain"));
        assert!(after.contains("Some prose that is not a step."));
        assert!(after.contains("- not a checkbox"));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn toggling_is_reversible() {
        let path = temp_file("reversible", "- [ ] one\n");

        toggle_step_in_file(&path, 0).expect("check");
        assert_eq!(fs::read_to_string(&path).unwrap(), "- [x] one\n");

        toggle_step_in_file(&path, 0).expect("uncheck");
        assert_eq!(fs::read_to_string(&path).unwrap(), "- [ ] one\n");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn toggling_preserves_a_missing_trailing_newline() {
        let path = temp_file("no-newline", "- [ ] one");

        toggle_step_in_file(&path, 0).expect("toggle");
        assert_eq!(fs::read_to_string(&path).unwrap(), "- [x] one");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn toggling_a_non_checkbox_line_is_an_error() {
        let path = temp_file("not-a-step", "# heading\n");
        assert!(toggle_step_in_file(&path, 0).is_err());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn toggling_past_the_end_is_an_error_not_a_panic() {
        let path = temp_file("out-of-range", "- [ ] one\n");
        assert!(toggle_step_in_file(&path, 99).is_err());
        let _ = fs::remove_file(&path);
    }
}
