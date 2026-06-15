use std::fs;
use std::io;

use crate::paths::AlpnestPaths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorCommand {
    Vi,
    Vim,
    Neovim,
    Nano,
    Helix,
    Emacs,
}

impl EditorCommand {
    pub fn command(self) -> &'static str {
        match self {
            Self::Vi => "vi",
            Self::Vim => "vim",
            Self::Neovim => "nvim",
            Self::Nano => "nano",
            Self::Helix => "hx",
            Self::Emacs => "emacs",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Vi => "vi",
            Self::Vim => "vim",
            Self::Neovim => "neovim / nvim",
            Self::Nano => "nano",
            Self::Helix => "helix / hx",
            Self::Emacs => "emacs",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Vi => Self::Vim,
            Self::Vim => Self::Neovim,
            Self::Neovim => Self::Nano,
            Self::Nano => Self::Helix,
            Self::Helix => Self::Emacs,
            Self::Emacs => Self::Vi,
        }
    }

    fn from_config(value: &str) -> Self {
        match value.trim() {
            "vim" => Self::Vim,
            "nvim" | "neovim" => Self::Neovim,
            "nano" => Self::Nano,
            "hx" | "helix" => Self::Helix,
            "emacs" => Self::Emacs,
            _ => Self::Vi,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLayout {
    BuiltInRightPane,
    Auto,
    Suspend,
    TmuxRightPane,
    ZellijRightPane,
}

impl TerminalLayout {
    pub fn label(self) -> &'static str {
        match self {
            Self::BuiltInRightPane => "built-in embedded right pane",
            Self::Auto => "auto",
            Self::Suspend => "same terminal / suspend TUI",
            Self::TmuxRightPane => "tmux right pane",
            Self::ZellijRightPane => "zellij right pane",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::BuiltInRightPane => Self::Auto,
            Self::Auto => Self::Suspend,
            Self::Suspend => Self::ZellijRightPane,
            Self::ZellijRightPane => Self::TmuxRightPane,
            Self::TmuxRightPane => Self::BuiltInRightPane,
        }
    }

    fn from_config(value: &str) -> Self {
        match value.trim() {
            "auto" => Self::Auto,
            "suspend" => Self::Suspend,
            "tmux" | "tmux_right_pane" => Self::TmuxRightPane,
            "zellij" | "zellij_right_pane" => Self::ZellijRightPane,
            _ => Self::BuiltInRightPane,
        }
    }

    pub fn config_value(self) -> &'static str {
        match self {
            Self::BuiltInRightPane => "built_in_right_pane",
            Self::Auto => "auto",
            Self::Suspend => "suspend",
            Self::TmuxRightPane => "tmux_right_pane",
            Self::ZellijRightPane => "zellij_right_pane",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    TextEditor,
    TerminalLayout,
    ReloadAfterExternalEdit,
    Theme,
    Keymap,
    AgenticDevelopment,
    MailConfiguration,
    CalendarConfiguration,
    Save,
    Back,
}

#[derive(Debug, Clone)]
pub struct AlpnestSettings {
    pub selected_field: SettingsField,
    pub text_editor: EditorCommand,
    pub terminal_layout: TerminalLayout,
    pub reload_after_external_edit: bool,
    pub theme: String,
    pub keymap: String,
    pub agentic_development: bool,
}

impl Default for AlpnestSettings {
    fn default() -> Self {
        Self {
            selected_field: SettingsField::TextEditor,
            text_editor: EditorCommand::Vi,
            terminal_layout: TerminalLayout::BuiltInRightPane,
            reload_after_external_edit: true,
            theme: "default".to_string(),
            keymap: "default".to_string(),
            agentic_development: false,
        }
    }
}

impl AlpnestSettings {
    pub fn load() -> io::Result<Self> {
        let paths = AlpnestPaths::resolve()?;
        let mut settings = Self::default();

        let Ok(raw) = fs::read_to_string(paths.config_file) else {
            return Ok(settings);
        };

        for line in raw.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            let key = key.trim();
            let value = value.trim().trim_matches('"');

            match key {
                "text_editor" => settings.text_editor = EditorCommand::from_config(value),
                "terminal_layout" => settings.terminal_layout = TerminalLayout::from_config(value),
                "reload_after_external_edit" => {
                    settings.reload_after_external_edit = matches!(value, "true" | "yes" | "1")
                }
                "theme" => settings.theme = value.to_string(),
                "keymap" => settings.keymap = value.to_string(),
                "agentic_development" => {
                    settings.agentic_development = matches!(value, "true" | "yes" | "1")
                }
                _ => {}
            }
        }

        Ok(settings)
    }

    pub fn save(&self) -> io::Result<()> {
        let paths = AlpnestPaths::resolve()?;
        fs::create_dir_all(&paths.config_dir)?;

        let raw = format!(
            "text_editor = \"{}\"\nterminal_layout = \"{}\"\nreload_after_external_edit = {}\ntheme = \"{}\"\nkeymap = \"{}\"\nagentic_development = {}\n",
            self.text_editor.command(),
            self.terminal_layout.config_value(),
            self.reload_after_external_edit,
            self.theme,
            self.keymap,
            self.agentic_development,
        );

        fs::write(paths.config_file, raw)
    }

    pub fn next_field(&mut self) {
        self.selected_field = match self.selected_field {
            SettingsField::TextEditor => SettingsField::TerminalLayout,
            SettingsField::TerminalLayout => SettingsField::ReloadAfterExternalEdit,
            SettingsField::ReloadAfterExternalEdit => SettingsField::Theme,
            SettingsField::Theme => SettingsField::Keymap,
            SettingsField::Keymap => SettingsField::AgenticDevelopment,
            SettingsField::AgenticDevelopment => SettingsField::MailConfiguration,
            SettingsField::MailConfiguration => SettingsField::CalendarConfiguration,
            SettingsField::CalendarConfiguration => SettingsField::Save,
            SettingsField::Save => SettingsField::Back,
            SettingsField::Back => SettingsField::TextEditor,
        };
    }

    pub fn previous_field(&mut self) {
        self.selected_field = match self.selected_field {
            SettingsField::TextEditor => SettingsField::Back,
            SettingsField::TerminalLayout => SettingsField::TextEditor,
            SettingsField::ReloadAfterExternalEdit => SettingsField::TerminalLayout,
            SettingsField::Theme => SettingsField::ReloadAfterExternalEdit,
            SettingsField::Keymap => SettingsField::Theme,
            SettingsField::AgenticDevelopment => SettingsField::Keymap,
            SettingsField::MailConfiguration => SettingsField::AgenticDevelopment,
            SettingsField::CalendarConfiguration => SettingsField::MailConfiguration,
            SettingsField::Save => SettingsField::CalendarConfiguration,
            SettingsField::Back => SettingsField::Save,
        };
    }

    pub fn cycle_selected(&mut self) {
        match self.selected_field {
            SettingsField::TextEditor => self.text_editor = self.text_editor.next(),
            SettingsField::TerminalLayout => self.terminal_layout = self.terminal_layout.next(),
            SettingsField::ReloadAfterExternalEdit => {
                self.reload_after_external_edit = !self.reload_after_external_edit;
            }
            SettingsField::AgenticDevelopment => {
                self.agentic_development = !self.agentic_development;
            }
            _ => {}
        }
    }

    pub fn rows(&self) -> Vec<(SettingsField, String, bool)> {
        let fields = [
            SettingsField::TextEditor,
            SettingsField::TerminalLayout,
            SettingsField::ReloadAfterExternalEdit,
            SettingsField::Theme,
            SettingsField::Keymap,
            SettingsField::AgenticDevelopment,
            SettingsField::MailConfiguration,
            SettingsField::CalendarConfiguration,
            SettingsField::Save,
            SettingsField::Back,
        ];

        fields
            .into_iter()
            .map(|field| {
                let label = match field {
                    SettingsField::TextEditor => {
                        format!("text editor: {}", self.text_editor.label())
                    }
                    SettingsField::TerminalLayout => {
                        format!("terminal layout: {}", self.terminal_layout.label())
                    }
                    SettingsField::ReloadAfterExternalEdit => {
                        format!(
                            "reload after external edit: {}",
                            if self.reload_after_external_edit {
                                "yes"
                            } else {
                                "no"
                            }
                        )
                    }
                    SettingsField::Theme => {
                        format!("theme: {}  [placeholder]", self.theme)
                    }
                    SettingsField::Keymap => {
                        format!("keymap: {}  [placeholder]", self.keymap)
                    }
                    SettingsField::AgenticDevelopment => {
                        format!(
                            "agentic development mode: {}  [placeholder]",
                            if self.agentic_development {
                                "yes"
                            } else {
                                "no"
                            }
                        )
                    }
                    SettingsField::MailConfiguration => {
                        "mail configuration defaults  [placeholder]".to_string()
                    }
                    SettingsField::CalendarConfiguration => {
                        "calendar configuration defaults  [placeholder]".to_string()
                    }
                    SettingsField::Save => "save settings".to_string(),
                    SettingsField::Back => "back to main explorer".to_string(),
                };

                (field, label, field == self.selected_field)
            })
            .collect()
    }
}
