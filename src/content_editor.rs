#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentEditorMode {
    AddNewContent,
    EditExistingContent,
    RemoveExistingContent,
}

impl ContentEditorMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::AddNewContent => "add new content",
            Self::EditExistingContent => "edit existing content",
            Self::RemoveExistingContent => "remove existing content",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::AddNewContent => Self::EditExistingContent,
            Self::EditExistingContent => Self::RemoveExistingContent,
            Self::RemoveExistingContent => Self::AddNewContent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditableTextTarget {
    Overview,
    Context,
}

impl EditableTextTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "overview.md",
            Self::Context => "context.md",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentEditorKind {
    Minimal,
    Task,
    Project,
}

impl ContentEditorKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Task => "task",
            Self::Project => "project",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Minimal => Self::Task,
            Self::Task => Self::Project,
            Self::Project => Self::Minimal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentEditorField {
    Mode,
    ExistingContent,
    ContentName,
    ContentKind,
    OverviewEditor,
    ContextEditor,
    DeadlineEnabled,
    DeadlineMinutes,
    ProjectBaseDir,
    AutoGeneratePanelSection,
    Create,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct ContentEditorState {
    pub mode: ContentEditorMode,
    pub selected_field: ContentEditorField,
    pub selected_existing_content: usize,
    pub existing_content_titles: Vec<String>,
    pub content_name: String,
    pub content_kind: ContentEditorKind,
    pub overview_buffer: String,
    pub context_buffer: String,
    pub deadline_enabled: bool,
    pub deadline_minutes: String,
    pub project_base_dir: String,
    pub auto_generate_panel_section: bool,
    pub editor_target: EditableTextTarget,
}

impl Default for ContentEditorState {
    fn default() -> Self {
        Self {
            mode: ContentEditorMode::AddNewContent,
            selected_field: ContentEditorField::Mode,
            selected_existing_content: 0,
            existing_content_titles: Vec::new(),
            content_name: String::new(),
            content_kind: ContentEditorKind::Minimal,
            overview_buffer: "# overview\n\n".to_string(),
            context_buffer: "# context\n\n".to_string(),
            deadline_enabled: false,
            deadline_minutes: "60".to_string(),
            project_base_dir: String::new(),
            auto_generate_panel_section: false,
            editor_target: EditableTextTarget::Overview,
        }
    }
}

impl ContentEditorState {
    pub fn with_existing_contents(existing_content_titles: Vec<String>) -> Self {
        Self {
            existing_content_titles,
            ..Self::default()
        }
    }

    pub fn next_field(&mut self) {
        let fields = self.visible_fields();
        let current = fields
            .iter()
            .position(|field| *field == self.selected_field)
            .unwrap_or(0);
        self.selected_field = fields[(current + 1) % fields.len()];
        self.sync_editor_target();
    }

    pub fn previous_field(&mut self) {
        let fields = self.visible_fields();
        let current = fields
            .iter()
            .position(|field| *field == self.selected_field)
            .unwrap_or(0);
        self.selected_field = if current == 0 {
            fields[fields.len() - 1]
        } else {
            fields[current - 1]
        };
        self.sync_editor_target();
    }

    pub fn toggle_or_cycle_current(&mut self) {
        match self.selected_field {
            ContentEditorField::Mode => {
                self.mode = self.mode.next();
                self.selected_field = ContentEditorField::Mode;
            }
            ContentEditorField::ExistingContent => {
                if !self.existing_content_titles.is_empty() {
                    self.selected_existing_content =
                        (self.selected_existing_content + 1) % self.existing_content_titles.len();
                }
            }
            ContentEditorField::ContentKind => {
                self.content_kind = self.content_kind.next();

                if self.content_kind == ContentEditorKind::Project {
                    self.deadline_enabled = false;
                }

                self.ensure_visible_field();
            }
            ContentEditorField::DeadlineEnabled => {
                if self.content_kind != ContentEditorKind::Project {
                    self.deadline_enabled = !self.deadline_enabled;
                }
                self.ensure_visible_field();
            }
            ContentEditorField::AutoGeneratePanelSection => {
                self.auto_generate_panel_section = !self.auto_generate_panel_section;
            }
            ContentEditorField::OverviewEditor => {
                self.editor_target = EditableTextTarget::Overview;
            }
            ContentEditorField::ContextEditor => {
                self.editor_target = EditableTextTarget::Context;
            }
            _ => {}
        }
    }

    pub fn push_char(&mut self, c: char) {
        match self.selected_field {
            ContentEditorField::ContentName => self.content_name.push(c),
            ContentEditorField::DeadlineMinutes if c.is_ascii_digit() => {
                self.deadline_minutes.push(c);
            }
            ContentEditorField::ProjectBaseDir => self.project_base_dir.push(c),
            ContentEditorField::OverviewEditor => self.overview_buffer.push(c),
            ContentEditorField::ContextEditor => self.context_buffer.push(c),
            _ => {}
        }
    }

    pub fn newline(&mut self) {
        match self.selected_field {
            ContentEditorField::OverviewEditor => self.overview_buffer.push('\n'),
            ContentEditorField::ContextEditor => self.context_buffer.push('\n'),
            _ => {}
        }
    }

    pub fn backspace(&mut self) {
        match self.selected_field {
            ContentEditorField::ContentName => {
                self.content_name.pop();
            }
            ContentEditorField::DeadlineMinutes => {
                self.deadline_minutes.pop();
            }
            ContentEditorField::ProjectBaseDir => {
                self.project_base_dir.pop();
            }
            ContentEditorField::OverviewEditor => {
                self.overview_buffer.pop();
            }
            ContentEditorField::ContextEditor => {
                self.context_buffer.pop();
            }
            _ => {}
        }
    }

    pub fn visible_fields(&self) -> Vec<ContentEditorField> {
        let mut fields = vec![ContentEditorField::Mode];

        match self.mode {
            ContentEditorMode::AddNewContent => {
                fields.extend([
                    ContentEditorField::ContentName,
                    ContentEditorField::ContentKind,
                ]);

                match self.content_kind {
                    ContentEditorKind::Minimal => {
                        fields.extend([
                            ContentEditorField::OverviewEditor,
                            ContentEditorField::ContextEditor,
                            ContentEditorField::DeadlineEnabled,
                        ]);

                        if self.deadline_enabled {
                            fields.push(ContentEditorField::DeadlineMinutes);
                        }

                        fields.push(ContentEditorField::AutoGeneratePanelSection);
                    }
                    ContentEditorKind::Task => {
                        fields.extend([
                            ContentEditorField::ContextEditor,
                            ContentEditorField::DeadlineEnabled,
                        ]);

                        if self.deadline_enabled {
                            fields.push(ContentEditorField::DeadlineMinutes);
                        }

                        fields.push(ContentEditorField::AutoGeneratePanelSection);
                    }
                    ContentEditorKind::Project => {
                        fields.extend([
                            ContentEditorField::ProjectBaseDir,
                            ContentEditorField::ContextEditor,
                            ContentEditorField::AutoGeneratePanelSection,
                        ]);
                    }
                }

                fields.extend([ContentEditorField::Create, ContentEditorField::Cancel]);
            }
            ContentEditorMode::EditExistingContent => {
                fields.extend([
                    ContentEditorField::ExistingContent,
                    ContentEditorField::ContentName,
                    ContentEditorField::ContentKind,
                    ContentEditorField::ContextEditor,
                    ContentEditorField::Create,
                    ContentEditorField::Cancel,
                ]);
            }
            ContentEditorMode::RemoveExistingContent => {
                fields.extend([
                    ContentEditorField::ExistingContent,
                    ContentEditorField::Create,
                    ContentEditorField::Cancel,
                ]);
            }
        }

        fields
    }

    pub fn field_rows(&self) -> Vec<(ContentEditorField, String, bool)> {
        self.visible_fields()
            .into_iter()
            .map(|field| {
                let value = match field {
                    ContentEditorField::Mode => {
                        format!("mode: {}", self.mode.label())
                    }
                    ContentEditorField::ExistingContent => {
                        let selected = self
                            .existing_content_titles
                            .get(self.selected_existing_content)
                            .map(String::as_str)
                            .unwrap_or("no existing contents");
                        format!("existing content: {selected}")
                    }
                    ContentEditorField::ContentName => {
                        format!("content name: {}", empty_hint(&self.content_name, "<name>"))
                    }
                    ContentEditorField::ContentKind => {
                        format!("content type: {}", self.content_kind.label())
                    }
                    ContentEditorField::OverviewEditor => {
                        "edit initial overview.md on the right".to_string()
                    }
                    ContentEditorField::ContextEditor => {
                        "edit initial context.md on the right".to_string()
                    }
                    ContentEditorField::DeadlineEnabled => {
                        format!("add deadline?: {}", yes_no(self.deadline_enabled))
                    }
                    ContentEditorField::DeadlineMinutes => {
                        format!(
                            "deadline minutes: {}",
                            empty_hint(&self.deadline_minutes, "60")
                        )
                    }
                    ContentEditorField::ProjectBaseDir => {
                        format!(
                            "project contents base dir: {}",
                            empty_hint(&self.project_base_dir, "<directory containing git repos>")
                        )
                    }
                    ContentEditorField::AutoGeneratePanelSection => {
                        format!(
                            "auto-generate panel1/section1?: {}",
                            yes_no(self.auto_generate_panel_section)
                        )
                    }
                    ContentEditorField::Create => match self.mode {
                        ContentEditorMode::RemoveExistingContent => {
                            "remove selected content".to_string()
                        }
                        _ => "create / save".to_string(),
                    },
                    ContentEditorField::Cancel => "cancel".to_string(),
                };

                (field, value, field == self.selected_field)
            })
            .collect()
    }

    pub fn slug(&self) -> String {
        slugify(&self.content_name)
    }

    pub fn editor_title(&self) -> String {
        format!(" editing {} ", self.editor_target.label())
    }

    pub fn editor_text(&self) -> &str {
        match self.editor_target {
            EditableTextTarget::Overview => &self.overview_buffer,
            EditableTextTarget::Context => &self.context_buffer,
        }
    }

    pub fn path_preview_lines(&self) -> Vec<String> {
        let slug = self.slug();

        if slug.is_empty() {
            return vec!["$ALPNEST_HOME/contents/<content-name>/".to_string()];
        }

        let mut lines = vec![format!("$ALPNEST_HOME/contents/{slug}/")];

        match self.content_kind {
            ContentEditorKind::Minimal => {
                lines.push(format!("$ALPNEST_HOME/contents/{slug}/overview.md"));
                lines.push(format!("$ALPNEST_HOME/contents/{slug}/context.md"));
                lines.push(format!("$ALPNEST_HOME/contents/{slug}/.{slug}.cfg"));
            }
            ContentEditorKind::Task => {
                lines.push(format!("$ALPNEST_HOME/contents/{slug}/context.md"));
                lines.push(format!("$ALPNEST_HOME/contents/{slug}/.{slug}.cfg"));

                if self.auto_generate_panel_section {
                    lines.push(format!("$ALPNEST_HOME/contents/{slug}/panel1/overview.md"));
                    lines.push(format!(
                        "$ALPNEST_HOME/contents/{slug}/panel1/overview.context.md"
                    ));
                    lines.push(format!("$ALPNEST_HOME/contents/{slug}/panel1/section1.md"));
                    lines.push(format!(
                        "$ALPNEST_HOME/contents/{slug}/panel1/section1.context.md"
                    ));
                    lines.push(format!("$ALPNEST_HOME/contents/{slug}/panel1/.prompt.md"));
                }
            }
            ContentEditorKind::Project => {
                lines.push(format!("$ALPNEST_HOME/contents/{slug}/context.md"));
                lines.push(format!("$ALPNEST_HOME/contents/{slug}/.{slug}.cfg"));

                if self.auto_generate_panel_section {
                    lines.push(format!("$ALPNEST_HOME/contents/{slug}/panel1/overview.md"));
                    lines.push(format!(
                        "$ALPNEST_HOME/contents/{slug}/panel1/overview.context.md"
                    ));
                    lines.push(format!("$ALPNEST_HOME/contents/{slug}/panel1/section1.md"));
                    lines.push(format!(
                        "$ALPNEST_HOME/contents/{slug}/panel1/section1.context.md"
                    ));
                    lines.push(format!("$ALPNEST_HOME/contents/{slug}/panel1/.prompt.md"));
                }
            }
        }

        lines
    }

    pub fn validate_for_create(&self) -> Result<(), String> {
        if self.mode != ContentEditorMode::AddNewContent {
            return Err(
                "edit existing content is not implemented yet; use add new content first"
                    .to_string(),
            );
        }

        if self.slug().is_empty() {
            return Err("content name cannot be empty".to_string());
        }

        if self.content_kind == ContentEditorKind::Project
            && self.project_base_dir.trim().is_empty()
        {
            return Err("project content requires a project contents base directory".to_string());
        }

        if self.deadline_enabled {
            let minutes = self
                .deadline_minutes
                .parse::<u64>()
                .map_err(|_| "deadline minutes must be a positive integer".to_string())?;

            if minutes == 0 {
                return Err("deadline minutes must be greater than zero".to_string());
            }
        }

        Ok(())
    }

    fn ensure_visible_field(&mut self) {
        if !self.visible_fields().contains(&self.selected_field) {
            self.selected_field = ContentEditorField::ContentKind;
        }
    }

    fn sync_editor_target(&mut self) {
        match self.selected_field {
            ContentEditorField::OverviewEditor => self.editor_target = EditableTextTarget::Overview,
            ContentEditorField::ContextEditor => self.editor_target = EditableTextTarget::Context,
            _ => {}
        }
    }
}

fn empty_hint<'a>(value: &'a str, hint: &'a str) -> &'a str {
    if value.is_empty() { hint } else { value }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

pub fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}
