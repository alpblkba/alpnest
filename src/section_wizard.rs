//! Cook section wizard state.
//!
//! Sections are the leaf layer of the Alpnest grammar: a panel holds an
//! arbitrary number of them, and each one is a `<slug>.md` / `<slug>.context.md`
//! pair. This wizard is the in-app way to cook, rename, and remove that pair so
//! the leaf layer stops needing hand-edited files.

use std::path::PathBuf;

use crate::content::{Content, Panel};
use crate::content_editor::slugify;
use crate::wizard_log::{LogEntry, LogLevel, trim};

const MAX_LOGS: usize = 80;
const DEFAULT_ROW_COUNT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionOperation {
    Cook,
    Edit,
    Remove,
}

impl SectionOperation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cook => "cook sections",
            Self::Edit => "edit section",
            Self::Remove => "remove section",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Cook => Self::Edit,
            Self::Edit => Self::Remove,
            Self::Remove => Self::Cook,
        }
    }
}

/// Starter bodies so a cooked section is never an empty file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionTemplate {
    Blank,
    Notes,
    Milestones,
    Exercise,
    Reference,
}

impl SectionTemplate {
    pub fn label(self) -> &'static str {
        match self {
            Self::Blank => "blank",
            Self::Notes => "notes",
            Self::Milestones => "milestones",
            Self::Exercise => "exercise",
            Self::Reference => "reference",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Blank => Self::Notes,
            Self::Notes => Self::Milestones,
            Self::Milestones => Self::Exercise,
            Self::Exercise => Self::Reference,
            Self::Reference => Self::Blank,
        }
    }

    pub fn body(self, title: &str) -> String {
        match self {
            Self::Blank => format!("# {title}\n\n"),
            Self::Notes => format!("# {title}\n\n## open questions\n\n## notes\n\n"),
            Self::Milestones => {
                format!("# {title}\n\n## ms0\n\n- [ ] define the goal\n\n## ms1\n\n")
            }
            Self::Exercise => {
                format!("# {title}\n\n## task\n\n## attempt\n\n## result\n\n")
            }
            Self::Reference => {
                format!("# {title}\n\n## sources\n\n## excerpts\n\n")
            }
        }
    }

    pub fn context(self, title: &str) -> String {
        match self {
            Self::Blank => format!("# {title} context\n\nWhat this section is for.\n"),
            Self::Notes => format!(
                "# {title} context\n\nRunning notes for this panel. Probe here for what is still unresolved.\n"
            ),
            Self::Milestones => format!(
                "# {title} context\n\nOrdered milestones for this panel. Probe here for progress state.\n"
            ),
            Self::Exercise => format!(
                "# {title} context\n\nExercises and worked attempts for this panel.\n"
            ),
            Self::Reference => format!(
                "# {title} context\n\nExternal references collected for this panel.\n"
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionField {
    Operation,
    NumberOfSections,
    Apply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionFocus {
    Fields,
    Sections,
    Defaults,
    Inner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionInnerMode {
    FullPath,
    FileTree,
}

impl SectionInnerMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::FullPath => "full path",
            Self::FileTree => "filetree",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::FullPath => Self::FileTree,
            Self::FileTree => Self::FullPath,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SectionDefaults {
    pub create_body: bool,
    pub create_context: bool,
    pub template: SectionTemplate,
    pub deadline_days: String,
}

impl Default for SectionDefaults {
    fn default() -> Self {
        Self {
            create_body: true,
            create_context: true,
            template: SectionTemplate::Blank,
            deadline_days: "0".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SectionWizardState {
    pub operation: SectionOperation,
    pub focus: SectionFocus,
    pub selected_field: SectionField,

    pub target_content_title: String,
    pub target_panel_title: String,
    pub target_panel_path: PathBuf,
    pub has_target: bool,

    pub existing_sections: Vec<String>,
    pub section_count_text: String,
    pub section_names: Vec<String>,
    pub section_defaults: Vec<SectionDefaults>,
    pub selected_section_index: usize,
    pub selected_default_index: usize,
    pub rename_buffer: String,

    pub inner_mode: SectionInnerMode,
    pub inner_scroll: usize,
    pub logs: Vec<LogEntry>,
    pub confirm_remove: bool,

    pub editing_section_count: bool,
    pub editing_section_name: bool,
    pub editing_rename: bool,
    pub editing_deadline_days: bool,
}

impl Default for SectionWizardState {
    fn default() -> Self {
        Self {
            operation: SectionOperation::Cook,
            focus: SectionFocus::Fields,
            selected_field: SectionField::Operation,
            target_content_title: "no content".to_string(),
            target_panel_title: "no panel".to_string(),
            target_panel_path: PathBuf::new(),
            has_target: false,
            existing_sections: Vec::new(),
            section_count_text: "1".to_string(),
            section_names: vec!["section1".to_string()],
            section_defaults: vec![SectionDefaults::default()],
            selected_section_index: 0,
            selected_default_index: 0,
            rename_buffer: String::new(),
            inner_mode: SectionInnerMode::FullPath,
            inner_scroll: 0,
            logs: vec![LogEntry::new(
                LogLevel::Note,
                "select a panel first, then cook sections inside it",
            )],
            confirm_remove: false,
            editing_section_count: false,
            editing_section_name: false,
            editing_rename: false,
            editing_deadline_days: false,
        }
    }
}

impl SectionWizardState {
    /// Cooking only makes sense inside a panel, so the wizard is always built
    /// from the currently selected content + panel.
    pub fn from_panel(content: &Content, panel: &Panel) -> Self {
        let existing_sections = panel
            .sections
            .iter()
            .map(|section| section.id.clone())
            .collect::<Vec<_>>();

        let mut state = Self {
            target_content_title: content.title.clone(),
            target_panel_title: panel.title.clone(),
            target_panel_path: panel.path.clone(),
            has_target: !panel.synthetic,
            existing_sections,
            logs: Vec::new(),
            ..Self::default()
        };

        if panel.synthetic {
            state.log_warning(format!(
                "{} is a generated panel; sections cannot be cooked here",
                panel.title
            ));
        } else {
            state.log_note(format!(
                "cooking into {} / {}",
                content.title, panel.title
            ));
        }

        state
    }

    pub fn without_target(reason: impl Into<String>) -> Self {
        let mut state = Self {
            has_target: false,
            logs: Vec::new(),
            ..Self::default()
        };

        state.log_warning(reason);
        state
    }

    pub fn refresh_existing_sections(&mut self, content: &Content, panel: &Panel) {
        self.target_content_title = content.title.clone();
        self.target_panel_title = panel.title.clone();
        self.target_panel_path = panel.path.clone();
        self.has_target = !panel.synthetic;
        self.existing_sections = panel
            .sections
            .iter()
            .map(|section| section.id.clone())
            .collect();

        let len = self.visible_section_len();

        if len == 0 {
            self.selected_section_index = 0;
        } else if self.selected_section_index >= len {
            self.selected_section_index = len - 1;
        }
    }

    pub fn is_editing_text(&self) -> bool {
        self.editing_section_count
            || self.editing_section_name
            || self.editing_rename
            || self.editing_deadline_days
    }

    pub fn set_operation(&mut self, operation: SectionOperation) {
        self.operation = operation;
        self.stop_editing();
        self.confirm_remove = false;
        self.inner_scroll = 0;

        match self.operation {
            SectionOperation::Cook => {
                self.focus = SectionFocus::Fields;
                self.selected_field = SectionField::Operation;
                self.ensure_cook_section_count();
            }
            SectionOperation::Edit | SectionOperation::Remove => {
                self.focus = SectionFocus::Sections;
                self.selected_section_index = 0;
                self.sync_rename_buffer();
            }
        }
    }

    pub fn cycle_operation(&mut self) {
        self.set_operation(self.operation.next());
        self.log_note(format!("operation changed to {}", self.operation.label()));
    }

    pub fn move_next(&mut self) {
        if self.is_editing_text() {
            return;
        }

        match self.focus {
            SectionFocus::Fields => {
                self.selected_field = match self.selected_field {
                    SectionField::Operation => SectionField::NumberOfSections,
                    SectionField::NumberOfSections => SectionField::Apply,
                    SectionField::Apply => SectionField::Operation,
                };
            }
            SectionFocus::Sections => {
                let len = self.visible_section_len();
                if len > 0 {
                    self.selected_section_index = (self.selected_section_index + 1).min(len - 1);
                    self.sync_rename_buffer();
                }
            }
            SectionFocus::Defaults => {
                self.selected_default_index =
                    (self.selected_default_index + 1).min(DEFAULT_ROW_COUNT - 1);
            }
            SectionFocus::Inner => {
                self.inner_scroll = self.inner_scroll.saturating_add(1);
            }
        }
    }

    pub fn move_prev(&mut self) {
        if self.is_editing_text() {
            return;
        }

        match self.focus {
            SectionFocus::Fields => {
                self.selected_field = match self.selected_field {
                    SectionField::Operation => SectionField::Apply,
                    SectionField::NumberOfSections => SectionField::Operation,
                    SectionField::Apply => SectionField::NumberOfSections,
                };
            }
            SectionFocus::Sections => {
                self.selected_section_index = self.selected_section_index.saturating_sub(1);
                self.sync_rename_buffer();
            }
            SectionFocus::Defaults => {
                self.selected_default_index = self.selected_default_index.saturating_sub(1);
            }
            SectionFocus::Inner => {
                self.inner_scroll = self.inner_scroll.saturating_sub(1);
            }
        }
    }

    pub fn focus_sections(&mut self) {
        self.stop_editing();
        self.focus = SectionFocus::Sections;
        self.confirm_remove = false;
    }

    pub fn focus_defaults(&mut self) {
        self.stop_editing();
        self.confirm_remove = false;

        if self.operation == SectionOperation::Cook {
            self.focus = SectionFocus::Defaults;
        } else {
            self.log_warning(format!("{} has no defaults", self.operation.label()));
        }
    }

    pub fn focus_inner(&mut self) {
        self.stop_editing();
        self.focus = SectionFocus::Inner;
        self.confirm_remove = false;
    }

    pub fn focus_fields(&mut self) {
        self.stop_editing();
        self.focus = SectionFocus::Fields;
        self.confirm_remove = false;
    }

    pub fn back_or_exit(&mut self) -> bool {
        if self.is_editing_text() {
            self.cancel_editing();
            return true;
        }

        if self.confirm_remove {
            self.confirm_remove = false;
            self.log_note("remove confirmation cancelled");
            return true;
        }

        match self.focus {
            SectionFocus::Fields => false,
            SectionFocus::Sections | SectionFocus::Defaults | SectionFocus::Inner => {
                self.focus_fields();
                true
            }
        }
    }

    pub fn toggle_inner_mode(&mut self) {
        self.stop_editing();
        self.inner_mode = self.inner_mode.toggle();
        self.inner_scroll = 0;
        self.log_note(format!("inner box changed to {}", self.inner_mode.label()));
    }

    /// Returns `false` when the caller should run the apply action instead.
    pub fn enter_current(&mut self) -> bool {
        match self.focus {
            SectionFocus::Fields => match self.selected_field {
                SectionField::Operation => {
                    self.cycle_operation();
                    true
                }
                SectionField::NumberOfSections => {
                    if self.operation != SectionOperation::Cook {
                        return true;
                    }

                    if self.editing_section_count {
                        self.finish_section_count_edit();
                    } else {
                        self.start_section_count_edit();
                    }
                    true
                }
                SectionField::Apply => false,
            },
            SectionFocus::Sections => {
                match self.operation {
                    SectionOperation::Cook => {
                        if self.editing_section_name {
                            self.finish_section_name_edit();
                        } else {
                            self.start_section_name_edit();
                        }
                    }
                    SectionOperation::Edit => {
                        if self.editing_rename {
                            self.finish_rename_edit();
                        } else {
                            self.start_rename_edit();
                        }
                    }
                    SectionOperation::Remove => {}
                }
                true
            }
            SectionFocus::Defaults => {
                if self.selected_default_index == 3 {
                    if self.editing_deadline_days {
                        self.finish_deadline_days_edit();
                    } else {
                        self.start_deadline_days_edit();
                    }
                } else {
                    self.toggle_default();
                }
                true
            }
            SectionFocus::Inner => true,
        }
    }

    pub fn toggle_or_cycle_current(&mut self) {
        self.confirm_remove = false;

        match self.focus {
            SectionFocus::Fields => match self.selected_field {
                SectionField::Operation => self.cycle_operation(),
                SectionField::NumberOfSections => {
                    if self.operation == SectionOperation::Cook {
                        if self.editing_section_count {
                            self.finish_section_count_edit();
                        } else {
                            self.start_section_count_edit();
                        }
                    }
                }
                SectionField::Apply => {}
            },
            SectionFocus::Defaults => {
                if self.selected_default_index == 3 {
                    if self.editing_deadline_days {
                        self.finish_deadline_days_edit();
                    } else {
                        self.start_deadline_days_edit();
                    }
                } else {
                    self.toggle_default();
                }
            }
            SectionFocus::Sections | SectionFocus::Inner => {}
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.confirm_remove = false;

        if self.editing_section_count {
            if c.is_ascii_digit() {
                self.section_count_text.push(c);
            }
            return;
        }

        if self.editing_deadline_days {
            if c.is_ascii_digit() {
                if let Some(defaults) = self.current_defaults_mut() {
                    defaults.deadline_days.push(c);
                }
            }
            return;
        }

        if self.editing_rename {
            self.rename_buffer.push(c);
            return;
        }

        if self.editing_section_name {
            if let Some(name) = self.section_names.get_mut(self.selected_section_index) {
                name.push(c);
            }
            return;
        }

        if self.focus == SectionFocus::Sections {
            match self.operation {
                SectionOperation::Cook => {
                    self.start_section_name_edit();
                    if let Some(name) = self.section_names.get_mut(self.selected_section_index) {
                        name.push(c);
                    }
                }
                SectionOperation::Edit => {
                    self.start_rename_edit();
                    self.rename_buffer.push(c);
                }
                SectionOperation::Remove => {}
            }
        }
    }

    pub fn backspace(&mut self) {
        self.confirm_remove = false;

        if self.editing_section_count {
            self.section_count_text.pop();
            return;
        }

        if self.editing_deadline_days {
            if let Some(defaults) = self.current_defaults_mut() {
                defaults.deadline_days.pop();
            }
            return;
        }

        if self.editing_rename {
            self.rename_buffer.pop();
            return;
        }

        if self.editing_section_name {
            if let Some(name) = self.section_names.get_mut(self.selected_section_index) {
                name.pop();
            }
        }
    }

    pub fn cancel_editing(&mut self) {
        if self.editing_section_count {
            self.finish_section_count_edit();
            return;
        }

        if self.editing_section_name {
            self.finish_section_name_edit();
            return;
        }

        if self.editing_rename {
            self.finish_rename_edit();
            return;
        }

        if self.editing_deadline_days {
            self.finish_deadline_days_edit();
        }
    }

    pub fn visible_section_len(&self) -> usize {
        match self.operation {
            SectionOperation::Cook => self.section_names.len(),
            SectionOperation::Edit | SectionOperation::Remove => self.existing_sections.len(),
        }
    }

    pub fn selected_section_name(&self) -> Option<&str> {
        match self.operation {
            SectionOperation::Cook => self
                .section_names
                .get(self.selected_section_index)
                .map(String::as_str),
            SectionOperation::Edit | SectionOperation::Remove => self
                .existing_sections
                .get(self.selected_section_index)
                .map(String::as_str),
        }
    }

    pub fn defaults_for_section(&self, index: usize) -> SectionDefaults {
        self.section_defaults.get(index).cloned().unwrap_or_default()
    }

    pub fn field_rows(&self) -> Vec<(String, String, bool)> {
        let count_value = match self.operation {
            SectionOperation::Cook => {
                if self.editing_section_count {
                    format!("{}_", self.section_count_text)
                } else {
                    self.section_count_text.clone()
                }
            }
            _ => self.existing_sections.len().to_string(),
        };

        let apply_value = match self.operation {
            SectionOperation::Cook => "cook sections".to_string(),
            SectionOperation::Edit => "rename selected section".to_string(),
            SectionOperation::Remove => {
                if self.confirm_remove {
                    "confirm remove selected section".to_string()
                } else {
                    "request remove confirmation".to_string()
                }
            }
        };

        vec![
            (
                "operation".to_string(),
                self.operation.label().to_string(),
                self.focus == SectionFocus::Fields
                    && self.selected_field == SectionField::Operation,
            ),
            (
                "target panel".to_string(),
                format!("{} / {}", self.target_content_title, self.target_panel_title),
                false,
            ),
            (
                "number of sections".to_string(),
                count_value,
                self.focus == SectionFocus::Fields
                    && self.selected_field == SectionField::NumberOfSections,
            ),
            (
                "apply".to_string(),
                apply_value,
                self.focus == SectionFocus::Fields && self.selected_field == SectionField::Apply,
            ),
        ]
    }

    pub fn section_rows(&self) -> Vec<(String, bool)> {
        match self.operation {
            SectionOperation::Cook => self
                .section_names
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    let label =
                        if self.editing_section_name && self.selected_section_index == index {
                            format!("{}. {}_", index + 1, name)
                        } else {
                            format!(
                                "{}. {}.md",
                                index + 1,
                                if name.is_empty() { "<empty>" } else { name }
                            )
                        };

                    (
                        label,
                        self.focus == SectionFocus::Sections
                            && self.selected_section_index == index,
                    )
                })
                .collect(),
            SectionOperation::Edit => self
                .existing_sections
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    let label = if self.editing_rename && self.selected_section_index == index {
                        format!("{}. {} -> {}_", index + 1, name, self.rename_buffer)
                    } else {
                        format!("{}. {}.md", index + 1, name)
                    };

                    (
                        label,
                        self.focus == SectionFocus::Sections
                            && self.selected_section_index == index,
                    )
                })
                .collect(),
            SectionOperation::Remove => self
                .existing_sections
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    (
                        format!("{}. {}.md", index + 1, name),
                        self.focus == SectionFocus::Sections
                            && self.selected_section_index == index,
                    )
                })
                .collect(),
        }
    }

    /// `(label, checked, is_toggle, selected)` — the view renders toggles as
    /// checkboxes and the rest as value rows.
    pub fn default_rows(&self) -> Vec<(String, bool, bool, bool)> {
        if self.operation != SectionOperation::Cook {
            return vec![(
                format!("{} has no defaults", self.operation.label()),
                false,
                false,
                false,
            )];
        }

        let defaults = self
            .section_defaults
            .get(self.selected_section_index)
            .cloned()
            .unwrap_or_default();

        let deadline = if self.editing_deadline_days {
            format!("deadline days: {}_", defaults.deadline_days)
        } else {
            format!("deadline days: {}", defaults.deadline_days)
        };

        let rows = vec![
            ("create <section>.md".to_string(), defaults.create_body, true),
            (
                "create <section>.context.md".to_string(),
                defaults.create_context,
                true,
            ),
            (
                format!("template: {}", defaults.template.label()),
                false,
                false,
            ),
            (deadline, false, false),
        ];

        rows.into_iter()
            .enumerate()
            .map(|(index, (label, checked, is_toggle))| {
                (
                    label,
                    checked,
                    is_toggle,
                    self.focus == SectionFocus::Defaults && self.selected_default_index == index,
                )
            })
            .collect()
    }

    pub fn inner_lines(&self) -> Vec<String> {
        let lines = match self.inner_mode {
            SectionInnerMode::FullPath => self.full_path_lines(),
            SectionInnerMode::FileTree => self.filetree_lines(),
        };

        if self.inner_scroll == 0 {
            lines
        } else {
            lines.into_iter().skip(self.inner_scroll).collect()
        }
    }

    fn full_path_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("mode: {}", self.inner_mode.label()), String::new()];

        if !self.has_target {
            lines.push("no writable panel selected".to_string());
            return lines;
        }

        match self.operation {
            SectionOperation::Cook => {
                for (index, name) in self.section_names.iter().enumerate() {
                    if name.trim().is_empty() {
                        continue;
                    }

                    let defaults = self.defaults_for_section(index);
                    let slug = slugify(name);
                    let marker = if self.selected_section_index == index {
                        ">"
                    } else {
                        " "
                    };

                    lines.push(format!("{marker} {name}"));

                    if defaults.create_body {
                        lines.push(format!(
                            "    file: {}",
                            self.target_panel_path.join(format!("{slug}.md")).display()
                        ));
                    }

                    if defaults.create_context {
                        lines.push(format!(
                            "    file: {}",
                            self.target_panel_path
                                .join(format!("{slug}.context.md"))
                                .display()
                        ));
                    }

                    lines.push(String::new());
                }
            }
            SectionOperation::Edit | SectionOperation::Remove => {
                for (index, name) in self.existing_sections.iter().enumerate() {
                    let marker = if self.selected_section_index == index {
                        ">"
                    } else {
                        " "
                    };

                    lines.push(format!("{marker} {name}"));
                    lines.push(format!(
                        "    file: {}",
                        self.target_panel_path.join(format!("{name}.md")).display()
                    ));
                    lines.push(format!(
                        "    file: {}",
                        self.target_panel_path
                            .join(format!("{name}.context.md"))
                            .display()
                    ));
                    lines.push(String::new());
                }
            }
        }

        if lines.len() <= 2 {
            lines.push("no sections to preview".to_string());
        }

        lines
    }

    fn filetree_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("mode: {}", self.inner_mode.label()),
            format!("{}/", self.target_content_title),
            format!("  {}/", slugify(&self.target_panel_title)),
        ];

        match self.operation {
            SectionOperation::Cook => {
                for (index, name) in self.section_names.iter().enumerate() {
                    if name.trim().is_empty() {
                        continue;
                    }

                    let defaults = self.defaults_for_section(index);
                    let slug = slugify(name);

                    if defaults.create_body {
                        lines.push(format!("    {slug}.md"));
                    }

                    if defaults.create_context {
                        lines.push(format!("    {slug}.context.md"));
                    }
                }
            }
            SectionOperation::Edit | SectionOperation::Remove => {
                for name in &self.existing_sections {
                    lines.push(format!("    {name}.md"));
                    lines.push(format!("    {name}.context.md"));
                }
            }
        }

        lines
    }

    pub fn log_note(&mut self, message: impl Into<String>) {
        self.push_log(LogLevel::Note, message);
    }

    pub fn log_info(&mut self, message: impl Into<String>) {
        self.push_log(LogLevel::Info, message);
    }

    pub fn log_warning(&mut self, message: impl Into<String>) {
        self.push_log(LogLevel::Warning, message);
    }

    pub fn log_error(&mut self, message: impl Into<String>) {
        self.push_log(LogLevel::Error, message);
    }

    pub fn absorb_logs(&mut self, logs: Vec<LogEntry>) {
        self.logs.extend(logs);
        trim(&mut self.logs, MAX_LOGS);
    }

    fn push_log(&mut self, level: LogLevel, message: impl Into<String>) {
        self.logs.push(LogEntry::new(level, message));
        trim(&mut self.logs, MAX_LOGS);
    }

    fn current_defaults_mut(&mut self) -> Option<&mut SectionDefaults> {
        self.ensure_defaults_len();
        self.section_defaults.get_mut(self.selected_section_index)
    }

    fn start_section_count_edit(&mut self) {
        self.stop_editing();
        self.editing_section_count = true;
        self.section_count_text.clear();
    }

    fn finish_section_count_edit(&mut self) {
        if self.section_count_text.trim().is_empty() || self.section_count_text == "0" {
            self.section_count_text = "1".to_string();
        }

        self.editing_section_count = false;
        self.ensure_cook_section_count();
        self.log_note(format!("section count set to {}", self.section_names.len()));
    }

    fn start_section_name_edit(&mut self) {
        if self.operation != SectionOperation::Cook {
            return;
        }

        self.stop_editing();
        self.focus = SectionFocus::Sections;
        self.editing_section_name = true;

        if let Some(name) = self.section_names.get_mut(self.selected_section_index) {
            name.clear();
        }
    }

    fn finish_section_name_edit(&mut self) {
        self.editing_section_name = false;
        let fallback = format!("section{}", self.selected_section_index + 1);

        if let Some(name) = self.section_names.get_mut(self.selected_section_index) {
            if name.trim().is_empty() {
                *name = fallback;
            }
        }
    }

    fn start_rename_edit(&mut self) {
        if self.operation != SectionOperation::Edit {
            return;
        }

        self.stop_editing();
        self.focus = SectionFocus::Sections;
        self.editing_rename = true;
        self.rename_buffer.clear();
    }

    fn finish_rename_edit(&mut self) {
        self.editing_rename = false;

        if self.rename_buffer.trim().is_empty() {
            self.sync_rename_buffer();
        }
    }

    fn start_deadline_days_edit(&mut self) {
        self.stop_editing();
        self.focus = SectionFocus::Defaults;
        self.selected_default_index = 3;
        self.editing_deadline_days = true;

        if let Some(defaults) = self.current_defaults_mut() {
            defaults.deadline_days.clear();
        }
    }

    fn finish_deadline_days_edit(&mut self) {
        self.editing_deadline_days = false;

        if let Some(defaults) = self.current_defaults_mut() {
            if defaults.deadline_days.trim().is_empty() {
                defaults.deadline_days = "0".to_string();
            }
        }
    }

    fn stop_editing(&mut self) {
        self.editing_section_count = false;
        self.editing_section_name = false;
        self.editing_rename = false;
        self.editing_deadline_days = false;
    }

    fn sync_rename_buffer(&mut self) {
        if self.operation == SectionOperation::Edit {
            self.rename_buffer = self
                .selected_section_name()
                .map(str::to_string)
                .unwrap_or_default();
        }
    }

    fn ensure_cook_section_count(&mut self) {
        let requested = self
            .section_count_text
            .parse::<usize>()
            .unwrap_or(1)
            .clamp(1, 99);

        while self.section_names.len() < requested {
            let next = self.section_names.len() + 1;
            self.section_names.push(format!("section{next}"));
        }

        self.section_names.truncate(requested);
        self.ensure_defaults_len();

        if self.selected_section_index >= self.section_names.len() {
            self.selected_section_index = self.section_names.len().saturating_sub(1);
        }
    }

    fn ensure_defaults_len(&mut self) {
        while self.section_defaults.len() < self.section_names.len() {
            self.section_defaults.push(SectionDefaults::default());
        }

        self.section_defaults.truncate(self.section_names.len());
    }

    fn toggle_default(&mut self) {
        let index = self.selected_default_index;

        let Some(defaults) = self.current_defaults_mut() else {
            return;
        };

        match index {
            0 => defaults.create_body = !defaults.create_body,
            1 => defaults.create_context = !defaults.create_context,
            2 => defaults.template = defaults.template.next(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cook_state() -> SectionWizardState {
        let mut state = SectionWizardState {
            has_target: true,
            target_panel_path: PathBuf::from("/tmp/panel"),
            existing_sections: vec!["overview".to_string(), "notes".to_string()],
            ..SectionWizardState::default()
        };
        state.set_operation(SectionOperation::Cook);
        state
    }

    #[test]
    fn section_count_grows_and_shrinks_names() {
        let mut state = cook_state();

        state.section_count_text = "3".to_string();
        state.ensure_cook_section_count();
        assert_eq!(state.section_names.len(), 3);
        assert_eq!(state.section_defaults.len(), 3);

        state.section_count_text = "1".to_string();
        state.ensure_cook_section_count();
        assert_eq!(state.section_names.len(), 1);
        assert_eq!(state.section_defaults.len(), 1);
        assert_eq!(state.selected_section_index, 0);
    }

    #[test]
    fn edit_mode_prefills_rename_from_selection() {
        let mut state = cook_state();
        state.set_operation(SectionOperation::Edit);

        assert_eq!(state.rename_buffer, "overview");

        state.move_next();
        assert_eq!(state.rename_buffer, "notes");
    }

    #[test]
    fn remove_mode_has_no_defaults() {
        let mut state = cook_state();
        state.set_operation(SectionOperation::Remove);

        let rows = state.default_rows();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].0.contains("no defaults"));
    }

    #[test]
    fn back_walks_focus_before_exiting() {
        let mut state = cook_state();
        state.focus_sections();

        assert!(state.back_or_exit());
        assert_eq!(state.focus, SectionFocus::Fields);
        assert!(!state.back_or_exit());
    }

    #[test]
    fn synthetic_panels_are_not_writable_targets() {
        let state = SectionWizardState::without_target("mail panels are generated");
        assert!(!state.has_target);
        assert!(state.logs.iter().any(|entry| entry.level == LogLevel::Warning));
    }

    #[test]
    fn templates_cycle_and_render_titles() {
        assert_eq!(SectionTemplate::Blank.next(), SectionTemplate::Notes);
        assert!(SectionTemplate::Notes.body("mmai").starts_with("# mmai"));
        assert!(
            SectionTemplate::Milestones
                .context("rv32i")
                .contains("rv32i context")
        );
    }
}
