use std::path::PathBuf;

use crate::content::Content;
use crate::content_editor::slugify;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelWizardOperation {
    Build,
    Rebuild,
    Destroy,
}

impl PanelWizardOperation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Build => "build panels",
            Self::Rebuild => "rebuild panels",
            Self::Destroy => "destroy panels",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Build => Self::Rebuild,
            Self::Rebuild => Self::Destroy,
            Self::Destroy => Self::Build,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelWizardField {
    Operation,
    NumberOfPanels,
    Apply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelWizardFocus {
    Fields,
    Panels,
    Inner,
    Defaults,
    Notifications,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelWizardInnerMode {
    FullPath,
    FileTree,
}

impl PanelWizardInnerMode {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelLogLevel {
    Note,
    Info,
    Warning,
    Error,
}

impl PanelLogLevel {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Note => "[note]",
            Self::Info => "[info]",
            Self::Warning => "[warning]",
            Self::Error => "[ERROR]",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PanelLogEntry {
    pub level: PanelLogLevel,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct PanelDefaults {
    pub create_overview: bool,
    pub create_overview_context: bool,
    pub create_prompt: bool,
    pub create_notes: bool,
    pub create_notes_context: bool,
    pub deadline_days: String,
}

impl Default for PanelDefaults {
    fn default() -> Self {
        Self {
            create_overview: true,
            create_overview_context: true,
            create_prompt: true,
            create_notes: false,
            create_notes_context: false,
            deadline_days: "0".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PanelWizardState {
    pub operation: PanelWizardOperation,
    pub focus: PanelWizardFocus,
    pub selected_field: PanelWizardField,
    pub target_content_title: String,
    pub target_content_path: PathBuf,
    pub existing_panels: Vec<String>,
    pub panel_count_text: String,
    pub panel_names: Vec<String>,
    pub panel_defaults: Vec<PanelDefaults>,
    pub selected_panel_index: usize,
    pub selected_default_index: usize,
    pub inner_mode: PanelWizardInnerMode,
    pub inner_scroll: usize,
    pub logs: Vec<PanelLogEntry>,
    pub confirm_destroy: bool,

    pub editing_panel_count: bool,
    pub editing_panel_name: bool,
    pub editing_deadline_days: bool,
}

impl Default for PanelWizardState {
    fn default() -> Self {
        Self {
            operation: PanelWizardOperation::Build,
            focus: PanelWizardFocus::Fields,
            selected_field: PanelWizardField::Operation,
            target_content_title: "no content".to_string(),
            target_content_path: PathBuf::new(),
            existing_panels: Vec::new(),
            panel_count_text: "1".to_string(),
            panel_names: vec!["panel1".to_string()],
            panel_defaults: vec![PanelDefaults::default()],
            selected_panel_index: 0,
            selected_default_index: 0,
            inner_mode: PanelWizardInnerMode::FullPath,
            inner_scroll: 0,
            logs: vec![PanelLogEntry {
                level: PanelLogLevel::Note,
                message: "panel wizard ready".to_string(),
            }],
            confirm_destroy: false,
            editing_panel_count: false,
            editing_panel_name: false,
            editing_deadline_days: false,
        }
    }
}

impl PanelWizardState {
    pub fn from_content(content: &Content) -> Self {
        let existing_panels = content
            .panels
            .iter()
            .map(|panel| panel.title.clone())
            .collect::<Vec<_>>();

        let mut state = Self {
            target_content_title: content.title.clone(),
            target_content_path: content.path.clone(),
            existing_panels,
            ..Self::default()
        };

        state.panel_names = vec!["panel1".to_string()];
        state.panel_defaults = vec![PanelDefaults::default()];
        state.log_note(format!(
            "target content set to {}",
            state.target_content_title
        ));
        state
    }

    pub fn refresh_existing_panels(&mut self, content: &Content) {
        self.target_content_title = content.title.clone();
        self.target_content_path = content.path.clone();
        self.existing_panels = content
            .panels
            .iter()
            .map(|panel| panel.title.clone())
            .collect::<Vec<_>>();

        if self.selected_panel_index >= self.visible_panel_len().max(1) {
            self.selected_panel_index = self.visible_panel_len().saturating_sub(1);
        }
    }

    pub fn is_editing_text(&self) -> bool {
        self.editing_panel_count || self.editing_panel_name || self.editing_deadline_days
    }

    pub fn set_operation(&mut self, operation: PanelWizardOperation) {
        self.operation = operation;
        self.stop_editing();
        self.confirm_destroy = false;

        match self.operation {
            PanelWizardOperation::Build => {
                self.focus = PanelWizardFocus::Fields;
                self.selected_field = PanelWizardField::Operation;
                self.ensure_build_panel_count();
            }
            PanelWizardOperation::Rebuild | PanelWizardOperation::Destroy => {
                self.focus = PanelWizardFocus::Panels;
                self.selected_panel_index = 0;
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
            PanelWizardFocus::Fields => {
                self.selected_field = match self.selected_field {
                    PanelWizardField::Operation => PanelWizardField::NumberOfPanels,
                    PanelWizardField::NumberOfPanels => PanelWizardField::Apply,
                    PanelWizardField::Apply => PanelWizardField::Operation,
                };
            }
            PanelWizardFocus::Panels => {
                let len = self.visible_panel_len();
                if len > 0 {
                    self.selected_panel_index = (self.selected_panel_index + 1).min(len - 1);
                }
            }
            PanelWizardFocus::Defaults => {
                self.selected_default_index = (self.selected_default_index + 1).min(5);
            }
            PanelWizardFocus::Inner => {
                self.inner_scroll = self.inner_scroll.saturating_add(1);
            }
            PanelWizardFocus::Notifications => {}
        }
    }

    pub fn move_prev(&mut self) {
        if self.is_editing_text() {
            return;
        }

        match self.focus {
            PanelWizardFocus::Fields => {
                self.selected_field = match self.selected_field {
                    PanelWizardField::Operation => PanelWizardField::Apply,
                    PanelWizardField::NumberOfPanels => PanelWizardField::Operation,
                    PanelWizardField::Apply => PanelWizardField::NumberOfPanels,
                };
            }
            PanelWizardFocus::Panels => {
                self.selected_panel_index = self.selected_panel_index.saturating_sub(1);
            }
            PanelWizardFocus::Defaults => {
                self.selected_default_index = self.selected_default_index.saturating_sub(1);
            }
            PanelWizardFocus::Inner => {
                self.inner_scroll = self.inner_scroll.saturating_sub(1);
            }
            PanelWizardFocus::Notifications => {}
        }
    }

    pub fn focus_panels(&mut self) {
        self.stop_editing();
        self.focus = PanelWizardFocus::Panels;
        self.confirm_destroy = false;
    }

    pub fn focus_defaults(&mut self) {
        self.stop_editing();

        if self.operation != PanelWizardOperation::Destroy {
            self.focus = PanelWizardFocus::Defaults;
            self.confirm_destroy = false;

            if let Some(title) = self.selected_panel_title().map(str::to_string) {
                self.log_info(format!("setting defaults for {title}"));
            }
        } else {
            self.log_warning("destroy mode has no defaults");
        }
    }

    pub fn focus_inner(&mut self) {
        self.stop_editing();
        self.focus = PanelWizardFocus::Inner;
        self.confirm_destroy = false;
    }

    pub fn focus_fields(&mut self) {
        self.stop_editing();
        self.focus = PanelWizardFocus::Fields;
        self.confirm_destroy = false;
    }

    pub fn back_or_exit(&mut self) -> bool {
        if self.is_editing_text() {
            self.cancel_editing();
            return true;
        }

        if self.confirm_destroy {
            self.confirm_destroy = false;
            self.log_note("destroy confirmation cancelled");
            return true;
        }

        match self.focus {
            PanelWizardFocus::Fields => false,
            PanelWizardFocus::Panels
            | PanelWizardFocus::Inner
            | PanelWizardFocus::Defaults
            | PanelWizardFocus::Notifications => {
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

    pub fn enter_current(&mut self) -> bool {
        match self.focus {
            PanelWizardFocus::Fields => match self.selected_field {
                PanelWizardField::Operation => {
                    self.cycle_operation();
                    true
                }
                PanelWizardField::NumberOfPanels => {
                    if self.editing_panel_count {
                        self.finish_panel_count_edit();
                    } else {
                        self.start_panel_count_edit();
                    }
                    true
                }
                PanelWizardField::Apply => false,
            },
            PanelWizardFocus::Panels if self.operation == PanelWizardOperation::Build => {
                if self.editing_panel_name {
                    self.finish_panel_name_edit();
                } else {
                    self.start_panel_name_edit();
                }
                true
            }
            PanelWizardFocus::Defaults => {
                if self.selected_default_index == 5 {
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
            PanelWizardFocus::Inner
            | PanelWizardFocus::Notifications
            | PanelWizardFocus::Panels => true,
        }
    }

    pub fn toggle_or_cycle_current(&mut self) {
        self.confirm_destroy = false;

        match self.focus {
            PanelWizardFocus::Fields => match self.selected_field {
                PanelWizardField::Operation => self.cycle_operation(),
                PanelWizardField::NumberOfPanels => {
                    if self.editing_panel_count {
                        self.finish_panel_count_edit();
                    } else {
                        self.start_panel_count_edit();
                    }
                }
                PanelWizardField::Apply => {}
            },
            PanelWizardFocus::Defaults => {
                if self.selected_default_index == 5 {
                    if self.editing_deadline_days {
                        self.finish_deadline_days_edit();
                    } else {
                        self.start_deadline_days_edit();
                    }
                } else {
                    self.toggle_default();
                }
            }
            PanelWizardFocus::Panels if self.operation == PanelWizardOperation::Build => {
                if self.editing_panel_name {
                    self.finish_panel_name_edit();
                } else {
                    self.start_panel_name_edit();
                }
            }
            PanelWizardFocus::Panels
            | PanelWizardFocus::Inner
            | PanelWizardFocus::Notifications => {}
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.confirm_destroy = false;

        if self.editing_panel_count {
            if c.is_ascii_digit() {
                self.panel_count_text.push(c);
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

        if self.editing_panel_name {
            if let Some(name) = self.panel_names.get_mut(self.selected_panel_index) {
                name.push(c);
            }
            return;
        }

        match self.focus {
            PanelWizardFocus::Panels if self.operation == PanelWizardOperation::Build => {
                self.start_panel_name_edit();
                if let Some(name) = self.panel_names.get_mut(self.selected_panel_index) {
                    name.push(c);
                }
            }
            _ => {}
        }
    }

    pub fn backspace(&mut self) {
        self.confirm_destroy = false;

        if self.editing_panel_count {
            self.panel_count_text.pop();
            return;
        }

        if self.editing_deadline_days {
            if let Some(defaults) = self.current_defaults_mut() {
                defaults.deadline_days.pop();
            }
            return;
        }

        if self.editing_panel_name {
            if let Some(name) = self.panel_names.get_mut(self.selected_panel_index) {
                name.pop();
            }
        }
    }

    pub fn cancel_editing(&mut self) {
        if self.editing_panel_count {
            self.finish_panel_count_edit();
            return;
        }

        if self.editing_panel_name {
            self.finish_panel_name_edit();
            return;
        }

        if self.editing_deadline_days {
            self.finish_deadline_days_edit();
        }
    }

    pub fn apply_rename_placeholder(&mut self) {
        if self.operation == PanelWizardOperation::Build {
            self.start_panel_name_edit();
        } else {
            self.log_warning("rename/mv is reserved for panel wizard v1");
        }
    }

    pub fn visible_panel_len(&self) -> usize {
        match self.operation {
            PanelWizardOperation::Build => self.panel_names.len(),
            PanelWizardOperation::Rebuild | PanelWizardOperation::Destroy => {
                self.existing_panels.len()
            }
        }
    }

    pub fn selected_panel_title(&self) -> Option<&str> {
        match self.operation {
            PanelWizardOperation::Build => self
                .panel_names
                .get(self.selected_panel_index)
                .map(String::as_str),
            PanelWizardOperation::Rebuild | PanelWizardOperation::Destroy => self
                .existing_panels
                .get(self.selected_panel_index)
                .map(String::as_str),
        }
    }

    pub fn selected_panel_slug(&self) -> Option<String> {
        self.selected_panel_title().map(slugify)
    }

    pub fn defaults_for_panel(&self, index: usize) -> PanelDefaults {
        self.panel_defaults.get(index).cloned().unwrap_or_default()
    }

    pub fn field_rows(&self) -> Vec<(String, bool)> {
        vec![
            (
                format!("operation: {}", self.operation.label()),
                self.focus == PanelWizardFocus::Fields
                    && self.selected_field == PanelWizardField::Operation,
            ),
            (
                format!(
                    "target content: {}",
                    if self.target_content_title.is_empty() {
                        "<none>"
                    } else {
                        &self.target_content_title
                    }
                ),
                false,
            ),
            (
                match self.operation {
                    PanelWizardOperation::Build => {
                        if self.editing_panel_count {
                            format!("number of panels: {}_", self.panel_count_text)
                        } else {
                            format!("number of panels: {}", self.panel_count_text)
                        }
                    }
                    _ => format!("number of panels: {}", self.existing_panels.len()),
                },
                self.focus == PanelWizardFocus::Fields
                    && self.selected_field == PanelWizardField::NumberOfPanels,
            ),
            (
                match self.operation {
                    PanelWizardOperation::Build => "apply: build panels".to_string(),
                    PanelWizardOperation::Rebuild => "apply: rebuild selected panel".to_string(),
                    PanelWizardOperation::Destroy => {
                        if self.confirm_destroy {
                            "apply: confirm destroy selected panel".to_string()
                        } else {
                            "apply: request destroy confirmation".to_string()
                        }
                    }
                },
                self.focus == PanelWizardFocus::Fields
                    && self.selected_field == PanelWizardField::Apply,
            ),
        ]
    }

    pub fn panel_rows(&self) -> Vec<(String, bool)> {
        match self.operation {
            PanelWizardOperation::Build => self
                .panel_names
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    let label = if self.editing_panel_name && self.selected_panel_index == index {
                        format!("{}. {}_", index + 1, name)
                    } else {
                        format!(
                            "{}. {}",
                            index + 1,
                            if name.is_empty() { "<empty>" } else { name }
                        )
                    };

                    (
                        label,
                        self.focus == PanelWizardFocus::Panels
                            && self.selected_panel_index == index,
                    )
                })
                .collect(),
            PanelWizardOperation::Rebuild | PanelWizardOperation::Destroy => self
                .existing_panels
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    (
                        format!("{}. {}", index + 1, name),
                        self.focus == PanelWizardFocus::Panels
                            && self.selected_panel_index == index,
                    )
                })
                .collect(),
        }
    }

    pub fn default_rows(&self) -> Vec<(String, bool)> {
        if self.operation == PanelWizardOperation::Destroy {
            return vec![("destroy mode has no defaults".to_string(), false)];
        }

        let defaults = self.current_defaults().cloned().unwrap_or_default();

        let deadline = if self.editing_deadline_days {
            format!("deadline days: {}_", defaults.deadline_days)
        } else {
            format!("deadline days: {}", defaults.deadline_days)
        };

        let rows = vec![
            format!("[{}] create overview.md", mark(defaults.create_overview)),
            format!(
                "[{}] create overview.context.md",
                mark(defaults.create_overview_context)
            ),
            format!("[{}] create .prompt.md", mark(defaults.create_prompt)),
            format!("[{}] create notes.md", mark(defaults.create_notes)),
            format!(
                "[{}] create notes.context.md",
                mark(defaults.create_notes_context)
            ),
            deadline,
        ];

        rows.into_iter()
            .enumerate()
            .map(|(index, row)| {
                (
                    row,
                    self.focus == PanelWizardFocus::Defaults
                        && self.selected_default_index == index,
                )
            })
            .collect()
    }

    pub fn inner_lines(&self) -> Vec<String> {
        match self.inner_mode {
            PanelWizardInnerMode::FullPath => self.full_path_lines(),
            PanelWizardInnerMode::FileTree => self.filetree_lines(),
        }
    }

    pub fn full_path_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("mode: {}", self.inner_mode.label()), String::new()];

        match self.operation {
            PanelWizardOperation::Build => {
                for (index, name) in self.panel_names.iter().enumerate() {
                    if name.trim().is_empty() {
                        continue;
                    }

                    let defaults = self.defaults_for_panel(index);
                    let slug = slugify(name);
                    let marker = if self.selected_panel_index == index {
                        ">"
                    } else {
                        " "
                    };

                    lines.push(format!("{marker} {name}"));
                    lines.push(format!(
                        "    dir: {}",
                        self.target_content_path.join(&slug).display()
                    ));

                    for file in files_for_defaults(&defaults) {
                        lines.push(format!(
                            "    file: {}",
                            self.target_content_path.join(&slug).join(file).display()
                        ));
                    }

                    lines.push(String::new());
                }
            }
            PanelWizardOperation::Rebuild | PanelWizardOperation::Destroy => {
                for (index, name) in self.existing_panels.iter().enumerate() {
                    let slug = slugify(name);
                    let marker = if self.selected_panel_index == index {
                        ">"
                    } else {
                        " "
                    };

                    lines.push(format!("{marker} {name}"));
                    lines.push(format!(
                        "    dir: {}",
                        self.target_content_path.join(&slug).display()
                    ));
                    lines.push(String::new());
                }
            }
        }

        if lines.len() <= 2 {
            lines.push("no panels to preview".to_string());
        }

        scroll_lines(lines, self.inner_scroll)
    }

    pub fn filetree_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("mode: {}", self.inner_mode.label()),
            format!("{}/", self.target_content_title),
        ];

        match self.operation {
            PanelWizardOperation::Build => {
                for (index, name) in self.panel_names.iter().enumerate() {
                    if name.trim().is_empty() {
                        continue;
                    }

                    let defaults = self.defaults_for_panel(index);
                    let slug = slugify(name);
                    lines.push(format!("  {slug}/"));

                    if defaults.create_overview {
                        lines.push("    overview.md".to_string());
                    }
                    if defaults.create_overview_context {
                        lines.push("    overview.context.md".to_string());
                    }
                    if defaults.create_prompt {
                        lines.push("    .prompt.md".to_string());
                    }
                    if defaults.create_notes {
                        lines.push("    notes.md".to_string());
                    }
                    if defaults.create_notes_context {
                        lines.push("    notes.context.md".to_string());
                    }
                }
            }
            PanelWizardOperation::Rebuild | PanelWizardOperation::Destroy => {
                for name in &self.existing_panels {
                    if name.trim().is_empty() {
                        continue;
                    }

                    let slug = slugify(name);
                    lines.push(format!("  {slug}/"));
                }
            }
        }

        scroll_lines(lines, self.inner_scroll)
    }

    pub fn log_note(&mut self, message: impl Into<String>) {
        self.push_log(PanelLogLevel::Note, message);
    }

    pub fn log_info(&mut self, message: impl Into<String>) {
        self.push_log(PanelLogLevel::Info, message);
    }

    pub fn log_warning(&mut self, message: impl Into<String>) {
        self.push_log(PanelLogLevel::Warning, message);
    }

    pub fn log_error(&mut self, message: impl Into<String>) {
        self.push_log(PanelLogLevel::Error, message);
    }

    pub fn absorb_logs(&mut self, logs: Vec<PanelLogEntry>) {
        for log in logs {
            self.logs.push(log);
        }

        self.trim_logs();
    }

    fn current_defaults(&self) -> Option<&PanelDefaults> {
        self.panel_defaults.get(self.selected_panel_index)
    }

    fn current_defaults_mut(&mut self) -> Option<&mut PanelDefaults> {
        self.ensure_defaults_len();
        self.panel_defaults.get_mut(self.selected_panel_index)
    }

    fn start_panel_count_edit(&mut self) {
        self.stop_editing();
        self.editing_panel_count = true;
        self.panel_count_text.clear();
        // edit mode is visible through the trailing underscore.
    }

    fn finish_panel_count_edit(&mut self) {
        if self.panel_count_text.trim().is_empty() || self.panel_count_text == "0" {
            self.panel_count_text = "1".to_string();
        }

        self.editing_panel_count = false;
        self.ensure_build_panel_count();
        self.log_note(format!("panel count set to {}", self.panel_names.len()));
    }

    fn start_panel_name_edit(&mut self) {
        if self.operation != PanelWizardOperation::Build {
            return;
        }

        self.stop_editing();
        self.focus = PanelWizardFocus::Panels;
        self.editing_panel_name = true;

        if let Some(name) = self.panel_names.get_mut(self.selected_panel_index) {
            name.clear();
        }

        // edit mode is visible through the trailing underscore.
    }

    fn finish_panel_name_edit(&mut self) {
        self.editing_panel_name = false;

        if let Some(name) = self.panel_names.get_mut(self.selected_panel_index) {
            if name.trim().is_empty() {
                *name = format!("panel{}", self.selected_panel_index + 1);
            }
        }
    }

    fn start_deadline_days_edit(&mut self) {
        self.stop_editing();
        self.focus = PanelWizardFocus::Defaults;
        self.selected_default_index = 5;
        self.editing_deadline_days = true;

        if let Some(defaults) = self.current_defaults_mut() {
            defaults.deadline_days.clear();
        }

        // edit mode is visible through the trailing underscore.
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
        self.editing_panel_count = false;
        self.editing_panel_name = false;
        self.editing_deadline_days = false;
    }

    fn push_log(&mut self, level: PanelLogLevel, message: impl Into<String>) {
        self.logs.push(PanelLogEntry {
            level,
            message: message.into(),
        });

        self.trim_logs();
    }

    fn trim_logs(&mut self) {
        const MAX_LOGS: usize = 80;

        if self.logs.len() > MAX_LOGS {
            let drain_count = self.logs.len() - MAX_LOGS;
            self.logs.drain(0..drain_count);
        }
    }

    fn ensure_build_panel_count(&mut self) {
        let requested = self
            .panel_count_text
            .parse::<usize>()
            .unwrap_or(1)
            .clamp(1, 99);

        while self.panel_names.len() < requested {
            let next = self.panel_names.len() + 1;
            self.panel_names.push(format!("panel{next}"));
        }

        self.panel_names.truncate(requested);
        self.ensure_defaults_len();

        if self.selected_panel_index >= self.panel_names.len() {
            self.selected_panel_index = self.panel_names.len().saturating_sub(1);
        }
    }

    fn ensure_defaults_len(&mut self) {
        while self.panel_defaults.len() < self.panel_names.len() {
            self.panel_defaults.push(PanelDefaults::default());
        }

        self.panel_defaults.truncate(self.panel_names.len());
    }

    fn toggle_default(&mut self) {
        let selected_default_index = self.selected_default_index;

        let Some(defaults) = self.current_defaults_mut() else {
            return;
        };

        match selected_default_index {
            0 => defaults.create_overview = !defaults.create_overview,
            1 => defaults.create_overview_context = !defaults.create_overview_context,
            2 => defaults.create_prompt = !defaults.create_prompt,
            3 => defaults.create_notes = !defaults.create_notes,
            4 => defaults.create_notes_context = !defaults.create_notes_context,
            5 => {}
            _ => {}
        }
    }
}

fn files_for_defaults(defaults: &PanelDefaults) -> Vec<&'static str> {
    let mut files = Vec::new();

    files.push(".panel.cfg");

    if defaults.create_overview {
        files.push("overview.md");
    }
    if defaults.create_overview_context {
        files.push("overview.context.md");
    }
    if defaults.create_prompt {
        files.push(".prompt.md");
    }
    if defaults.create_notes {
        files.push("notes.md");
    }
    if defaults.create_notes_context {
        files.push("notes.context.md");
    }

    files
}

fn mark(value: bool) -> &'static str {
    if value { "x" } else { " " }
}

fn scroll_lines(lines: Vec<String>, scroll: usize) -> Vec<String> {
    if scroll == 0 {
        lines
    } else {
        lines.into_iter().skip(scroll).collect()
    }
}
