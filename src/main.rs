use std::{fs, io, time::Duration};

use alpnest::embedded_terminal::{EmbeddedTerminal, EmbeddedTerminalKind};
use alpnest::settings::TerminalLayout;
use alpnest::theme::{BadgeLevel, MAIL as MAIL_THEME, Theme};
use alpnest::{
    app::AppState,
    app_view::AppView,
    content_editor::{ContentEditorField, ContentEditorMode, EditableTextTarget},
    content_writer, external_editor,
    mail_config::{MailConfigAction, MailConfigFocus},
    panel_wizard::PanelWizardOperation,
    panel_writer,
    paths::AlpnestPaths,
    section_wizard::SectionOperation,
    section_workbench::WorkbenchFocus,
    section_writer,
    settings::SettingsField,
    ui::main_explorer::{MainExplorerSnapshot, MainExplorerView},
    wizard_log::{LogEntry, LogLevel},
};
use ansi_to_tui::IntoText;
use color_eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

/// A command queued for the right-hand pane (keychain prompt, mail sync).
struct PendingTask {
    title: String,
    argv: Vec<String>,
}

struct RuntimeApp {
    state: AppState,
    should_quit: bool,
    status: Option<String>,
    warning_popup: Option<String>,
    embedded_terminal: Option<EmbeddedTerminal>,
    pending_embedded_terminal_path: Option<std::path::PathBuf>,
    pending_embedded_shell: bool,
    pending_embedded_task: Option<PendingTask>,
    open_shell_after_editor_exit: bool,
    right_terminal_focused: bool,
    /// Mail sync started at launch, polled so the registry reloads when it
    /// finishes. Runs detached so the TUI never blocks on the network.
    background_sync: Option<std::process::Child>,
}

impl RuntimeApp {
    fn load() -> Result<Self> {
        let mut app = Self {
            state: AppState::load()?,
            should_quit: false,
            status: None,
            warning_popup: None,
            embedded_terminal: None,
            pending_embedded_terminal_path: None,
            pending_embedded_shell: false,
            pending_embedded_task: None,
            open_shell_after_editor_exit: false,
            right_terminal_focused: false,
            background_sync: None,
        };

        app.start_background_sync();
        Ok(app)
    }

    /// Kick off a mail sync at launch so opening Alpnest shows current mail.
    ///
    /// Deliberately fire-and-forget: output is discarded, failures only set a
    /// status line, and the registry reloads once the child exits. Ollama is
    /// woken by the sync only when it actually has new mail to summarize.
    fn start_background_sync(&mut self) {
        if self.background_sync.is_some() {
            return;
        }

        let Ok(registry) = alpnest::mail::account::MailAccountRegistry::load() else {
            return;
        };

        if !registry.accounts.iter().any(|account| account.enabled) {
            return;
        }

        let Some(script) = alpnest::mail_config::sync_script_path() else {
            return;
        };

        let spawned = std::process::Command::new("python3")
            .args([&script, "--all", "--bodies", "--summarize"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();

        match spawned {
            Ok(child) => {
                self.background_sync = Some(child);
                self.status = Some("fetching mail in the background…".to_string());
            }
            Err(err) => {
                self.status = Some(format!("background mail sync did not start: {err}"));
            }
        }
    }

    fn poll_background_sync(&mut self) {
        let Some(child) = self.background_sync.as_mut() else {
            return;
        };

        match child.try_wait() {
            Ok(Some(status)) => {
                self.background_sync = None;

                if let Err(err) = self.state.reload() {
                    self.status = Some(format!("mail synced, but reload failed: {err}"));
                    return;
                }

                self.status = Some(if status.success() {
                    "mail sync finished".to_string()
                } else {
                    "mail sync finished with errors — press m to check accounts".to_string()
                });
            }
            Ok(None) => {}
            Err(err) => {
                self.background_sync = None;
                self.status = Some(format!("mail sync could not be polled: {err}"));
            }
        }
    }

    /// The palette for the surface currently on screen. Mail deliberately
    /// runs its own so account configuration never looks like note browsing.
    fn theme(&self) -> Theme {
        if self.state.current_view == AppView::ConfigureMail {
            MAIL_THEME
        } else {
            self.state.theme
        }
    }

    fn terminal_busy(&self) -> bool {
        self.embedded_terminal.is_some()
            || self.pending_embedded_terminal_path.is_some()
            || self.pending_embedded_shell
            || self.pending_embedded_task.is_some()
    }

    fn queue_task(&mut self, title: String, argv: Vec<String>) {
        self.pending_embedded_task = Some(PendingTask {
            title: title.clone(),
            argv,
        });
        self.right_terminal_focused = true;
        self.status = Some(format!("running {title} in the right pane"));
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('t')
            && !matches!(
                self.state.current_view,
                AppView::BuildPanel | AppView::CookSection
            )
        {
            self.toggle_right_terminal();
            return;
        }

        if self.warning_popup.is_some() {
            self.warning_popup = None;
            return;
        }

        if self.right_terminal_focused {
            self.handle_embedded_terminal_key(key);
            return;
        }

        // ctrl-c and ctrl-q both quit from anywhere, including inside the
        // wizards, so there is always one way out that does not depend on
        // which view happens to be focused.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
        {
            self.should_quit = true;
            return;
        }

        match self.state.current_view {
            AppView::ContentEditor => {
                self.handle_content_editor_key(key);
                return;
            }
            AppView::BuildPanel => {
                self.handle_panel_wizard_key(key);
                return;
            }
            AppView::Settings => {
                self.handle_settings_key(key);
                return;
            }
            AppView::CookSection => {
                self.handle_section_wizard_key(key);
                return;
            }
            AppView::ConfigureMail => {
                self.handle_mail_config_key(key);
                return;
            }
            AppView::SectionWorkbench => {
                self.handle_workbench_key(key);
                return;
            }
            _ => {}
        }

        // Below here every binding is an unmodified key. Without this guard a
        // stray ctrl-<letter> falls through to the plain-letter arm and fires
        // the wrong view — ctrl-a opening the content editor, and so on.
        if modified(key) {
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => {
                if self.state.current_view == AppView::MainExplorer {
                    self.state.move_next_row();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.state.current_view == AppView::MainExplorer {
                    self.state.move_prev_row();
                }
            }
            KeyCode::Enter => {
                if self.state.current_view == AppView::MainExplorer {
                    // A section is a leaf, so opening it means entering its
                    // workbench. Mail and calendar leaves are messages and
                    // dates, not units of work: they stay in the explorer and
                    // simply render in the right pane.
                    if self.state.selected_section().is_some()
                        && self.state.selection_supports_sections()
                    {
                        if let Err(reason) = self.state.open_section_workbench() {
                            self.status = Some(reason);
                        }
                    } else {
                        self.state.enter();
                    }
                }
            }
            KeyCode::Char('E') => self.open_selected_context_markdown(),
            KeyCode::Esc | KeyCode::Backspace => self.state.back(),
            KeyCode::Char('a') => self.state.open_content_editor(),
            KeyCode::Char('b') => {
                if let Err(reason) = self.state.open_panel_wizard() {
                    self.status = Some(reason);
                }
            }
            KeyCode::Char('c') => self.state.open_section_wizard(),
            KeyCode::Char('m') => self.state.open_mail_config(),
            KeyCode::Char('s') => self.state.open_settings(),
            KeyCode::Char('h') => self.state.switch_view(AppView::MainExplorer),
            _ => {}
        }
    }

    fn handle_settings_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('h') => self.state.switch_view(AppView::MainExplorer),
            KeyCode::Tab | KeyCode::Char('j') | KeyCode::Down => {
                self.state.settings.next_field();
            }
            KeyCode::BackTab | KeyCode::Char('k') | KeyCode::Up => {
                self.state.settings.previous_field();
            }
            KeyCode::Char(' ') => {
                self.state.settings.cycle_selected();
                self.save_settings_status();
            }
            KeyCode::Enter => match self.state.settings.selected_field {
                SettingsField::Save => self.save_settings_status(),
                SettingsField::Back => self.state.switch_view(AppView::MainExplorer),
                SettingsField::MailConfiguration => self.state.open_mail_config(),
                _ => {
                    self.state.settings.cycle_selected();
                    self.save_settings_status();
                }
            },
            _ => {}
        }
    }

    fn save_settings_status(&mut self) {
        // Theme changes should show up on the very next frame, not after a
        // restart, so the live theme is refreshed before the write.
        self.state.apply_theme_from_settings();

        match self.state.settings.save() {
            Ok(()) => self.status = Some("settings saved".to_string()),
            Err(err) => self.status = Some(format!("settings save failed: {err}")),
        }
    }

    fn handle_panel_wizard_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            if self.state.panel_wizard.is_editing_text() {
                self.state.panel_wizard.cancel_editing();
            }

            self.apply_panel_wizard_action();
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('t') {
            self.state.panel_wizard.toggle_inner_mode();
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('g') {
            self.state.panel_wizard.focus_inner();
            return;
        }

        if modified(key) {
            return;
        }

        if self.state.panel_wizard.is_editing_text() {
            match key.code {
                KeyCode::Esc => self.state.panel_wizard.cancel_editing(),
                KeyCode::Backspace => self.state.panel_wizard.backspace(),
                KeyCode::Enter => {
                    self.state.panel_wizard.enter_current();
                }
                KeyCode::Char(c) => self.state.panel_wizard.push_char(c),
                _ => {}
            }

            return;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Backspace => {
                if !self.state.panel_wizard.back_or_exit() {
                    self.state.switch_view(AppView::MainExplorer);
                }
            }
            KeyCode::Char('p') => self.state.panel_wizard.focus_panels(),
            KeyCode::Char('d') => self.state.panel_wizard.focus_defaults(),
            KeyCode::Char('r') => self.state.panel_wizard.apply_rename_placeholder(),
            KeyCode::Char('f') => self.state.panel_wizard.focus_fields(),
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                self.state.panel_wizard.move_next();
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                self.state.panel_wizard.move_prev();
            }
            KeyCode::Char(' ') => {
                self.state.panel_wizard.toggle_or_cycle_current();
            }
            KeyCode::Enter => {
                if !self.state.panel_wizard.enter_current() {
                    self.apply_panel_wizard_action();
                }
            }
            KeyCode::Char(c) => self.state.panel_wizard.push_char(c),
            _ => {}
        }
    }

    fn apply_panel_wizard_action(&mut self) {
        match self.state.panel_wizard.operation {
            PanelWizardOperation::Build => {
                match panel_writer::build_panels_from_wizard(&self.state.panel_wizard) {
                    Ok(logs) => {
                        self.state.panel_wizard.absorb_logs(logs);

                        if let Err(err) = self.state.reload() {
                            self.state
                                .panel_wizard
                                .log_error(format!("reload failed after build: {err}"));
                        }

                        if let Some(content) = self.state.selected_content() {
                            let content = content.clone();
                            self.state.panel_wizard.refresh_existing_panels(&content);
                        }
                    }
                    Err(err) => {
                        self.state
                            .panel_wizard
                            .log_error(format!("build failed: {err}"));
                    }
                }
            }
            PanelWizardOperation::Rebuild => {
                self.state
                    .panel_wizard
                    .log_warning("rebuild is reserved for panel wizard v1");
            }
            PanelWizardOperation::Destroy => {
                if !self.state.panel_wizard.confirm_destroy {
                    self.state.panel_wizard.confirm_destroy = true;

                    if let Some(title) = self.state.panel_wizard.selected_panel_title() {
                        self.state
                            .panel_wizard
                            .log_warning(format!("press enter again to destroy panel {title}"));
                    } else {
                        self.state
                            .panel_wizard
                            .log_error("no panel selected for destroy");
                    }

                    return;
                }

                match panel_writer::destroy_selected_panel_from_wizard(&self.state.panel_wizard) {
                    Ok(logs) => {
                        self.state.panel_wizard.confirm_destroy = false;
                        self.state.panel_wizard.absorb_logs(logs);

                        if let Err(err) = self.state.reload() {
                            self.state
                                .panel_wizard
                                .log_error(format!("reload failed after destroy: {err}"));
                        }

                        if let Some(content) = self.state.selected_content() {
                            let content = content.clone();
                            self.state.panel_wizard.refresh_existing_panels(&content);
                        }
                    }
                    Err(err) => {
                        self.state.panel_wizard.confirm_destroy = false;
                        self.state
                            .panel_wizard
                            .log_error(format!("destroy failed: {err}"));
                    }
                }
            }
        }
    }

    fn handle_section_wizard_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            if self.state.section_wizard.is_editing_text() {
                self.state.section_wizard.cancel_editing();
            }

            self.apply_section_wizard_action();
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('t') {
            self.state.section_wizard.toggle_inner_mode();
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('g') {
            self.state.section_wizard.focus_inner();
            return;
        }

        if modified(key) {
            return;
        }

        if self.state.section_wizard.is_editing_text() {
            match key.code {
                KeyCode::Esc => self.state.section_wizard.cancel_editing(),
                KeyCode::Backspace => self.state.section_wizard.backspace(),
                KeyCode::Enter => {
                    self.state.section_wizard.enter_current();
                }
                KeyCode::Char(c) => self.state.section_wizard.push_char(c),
                _ => {}
            }

            return;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Backspace => {
                if !self.state.section_wizard.back_or_exit() {
                    self.state.switch_view(AppView::MainExplorer);
                }
            }
            KeyCode::Char('s') => self.state.section_wizard.focus_sections(),
            KeyCode::Char('d') => self.state.section_wizard.focus_defaults(),
            KeyCode::Char('f') => self.state.section_wizard.focus_fields(),
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                self.state.section_wizard.move_next();
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                self.state.section_wizard.move_prev();
            }
            KeyCode::Char(' ') => self.state.section_wizard.toggle_or_cycle_current(),
            KeyCode::Enter => {
                if !self.state.section_wizard.enter_current() {
                    self.apply_section_wizard_action();
                }
            }
            KeyCode::Char(c) => self.state.section_wizard.push_char(c),
            _ => {}
        }
    }

    fn apply_section_wizard_action(&mut self) {
        if !self.state.section_wizard.has_target {
            self.state
                .section_wizard
                .log_error("no writable panel selected; go back and pick one");
            return;
        }

        // Remove is the only destructive operation here, so it gets the same
        // two-step confirmation the panel wizard uses.
        if self.state.section_wizard.operation == SectionOperation::Remove
            && !self.state.section_wizard.confirm_remove
        {
            self.state.section_wizard.confirm_remove = true;

            match self.state.section_wizard.selected_section_name() {
                Some(name) => {
                    let name = name.to_string();
                    self.state
                        .section_wizard
                        .log_warning(format!("press enter again to remove {name}"));
                }
                None => {
                    self.state
                        .section_wizard
                        .log_error("no section selected for remove");
                }
            }

            return;
        }

        match section_writer::apply_wizard(&self.state.section_wizard) {
            Ok(logs) => {
                self.state.section_wizard.confirm_remove = false;
                self.state.section_wizard.absorb_logs(logs);

                if let Err(err) = self.state.reload() {
                    self.state
                        .section_wizard
                        .log_error(format!("reload failed: {err}"));
                    return;
                }

                self.state.refresh_section_wizard_target();
            }
            Err(err) => {
                self.state.section_wizard.confirm_remove = false;
                self.state
                    .section_wizard
                    .log_error(format!("{} failed: {err}", self.state.section_wizard.operation.label()));
            }
        }
    }

    fn handle_workbench_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('t') {
            self.toggle_right_terminal();
            return;
        }

        match key.code {
            KeyCode::Esc => {
                self.state.switch_view(AppView::MainExplorer);
            }
            KeyCode::Tab => self.state.section_workbench.toggle_focus(),
            KeyCode::Char('j') | KeyCode::Down => self.state.section_workbench.move_next(),
            KeyCode::Char('k') | KeyCode::Up => self.state.section_workbench.move_prev(),
            KeyCode::Char(' ') | KeyCode::Enter => {
                if let Err(err) = self.state.section_workbench.toggle_selected_step() {
                    self.state.section_workbench.status = Some(format!("toggle failed: {err}"));
                }
            }
            KeyCode::Char('n') => self.state.section_workbench.jump_to_next_action(),
            KeyCode::Char('r') => {
                self.state.section_workbench.reload();
                self.state.section_workbench.status = Some("reloaded from disk".to_string());
            }
            KeyCode::Char('e') => {
                let path = self.state.section_workbench.body_path.clone();
                self.open_existing_markdown(path);
            }
            KeyCode::Char('c') => {
                if let Some(path) = self.state.section_workbench.context_path.clone() {
                    self.open_existing_markdown(path);
                } else {
                    self.state.section_workbench.status =
                        Some("this section has no context file".to_string());
                }
            }
            _ => {}
        }
    }

    fn handle_mail_config_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.state.mail_config.cancel_text_edit();
            self.state.mail_config.selected_field = alpnest::mail_config::MailConfigField::Save;
            let action = self.state.mail_config.activate();
            self.handle_mail_config_action(action);
            return;
        }

        if modified(key) {
            return;
        }

        if self.state.mail_config.editing_text {
            match key.code {
                KeyCode::Esc => self.state.mail_config.cancel_text_edit(),
                KeyCode::Enter | KeyCode::Tab => self.state.mail_config.commit_text_edit(),
                KeyCode::Backspace => self.state.mail_config.backspace(),
                KeyCode::Char(c) => self.state.mail_config.push_char(c),
                _ => {}
            }

            return;
        }

        match key.code {
            KeyCode::Esc => self.state.switch_view(AppView::MainExplorer),
            KeyCode::Char('n') => self.state.mail_config.start_new_account(),
            KeyCode::Char('a') => self.state.mail_config.focus_accounts(),
            KeyCode::Char('f') => self.state.mail_config.focus_form(),
            KeyCode::Char('S') => {
                let action = self.state.mail_config.sync_all_action();
                self.handle_mail_config_action(action);
            }
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => self.state.mail_config.move_next(),
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                self.state.mail_config.move_prev()
            }
            KeyCode::Left => self.state.mail_config.focus_accounts(),
            KeyCode::Right => self.state.mail_config.focus_form(),
            KeyCode::Enter | KeyCode::Char(' ') => {
                let action = self.state.mail_config.activate();
                self.handle_mail_config_action(action);
            }
            _ => {}
        }
    }

    fn handle_mail_config_action(&mut self, action: MailConfigAction) {
        match action {
            MailConfigAction::None => {}
            MailConfigAction::RunCommand { title, argv } => self.queue_task(title, argv),
            MailConfigAction::Reload => {
                if let Err(err) = self.state.reload() {
                    self.state
                        .mail_config
                        .log_error(format!("reload failed: {err}"));
                }
            }
        }
    }

    fn handle_content_editor_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.create_content_from_editor();
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('o') {
            self.open_content_editor_markdown();
            return;
        }

        if modified(key) {
            return;
        }

        match key.code {
            KeyCode::Esc => self.state.switch_view(AppView::MainExplorer),
            KeyCode::Tab | KeyCode::Char('j') | KeyCode::Down => {
                self.state.content_editor.next_field();
            }
            KeyCode::BackTab | KeyCode::Char('k') | KeyCode::Up => {
                self.state.content_editor.previous_field();
            }
            KeyCode::Backspace => {
                self.state.content_editor.backspace();
            }
            KeyCode::Enter => match self.state.content_editor.selected_field {
                ContentEditorField::Create => self.create_content_from_editor(),
                ContentEditorField::Cancel => self.state.switch_view(AppView::MainExplorer),
                ContentEditorField::OverviewEditor | ContentEditorField::ContextEditor => {
                    if external_editor::uses_builtin_editor(&self.state.settings) {
                        self.state.content_editor.newline();
                    } else {
                        self.open_content_editor_markdown();
                    }
                }
                _ => self.state.content_editor.toggle_or_cycle_current(),
            },
            KeyCode::Char(' ') => match self.state.content_editor.selected_field {
                ContentEditorField::OverviewEditor | ContentEditorField::ContextEditor => {
                    self.state.content_editor.push_char(' ');
                }
                _ => self.state.content_editor.toggle_or_cycle_current(),
            },
            KeyCode::Char(c) => {
                self.state.content_editor.push_char(c);
            }
            _ => {}
        }
    }

    fn open_content_editor_markdown(&mut self) {
        let target = self.state.content_editor.editor_target;
        let Ok(paths) = AlpnestPaths::resolve() else {
            self.status = Some("failed to resolve ALPNEST paths".to_string());
            return;
        };

        let draft_dir = paths.home.join("drafts");
        if let Err(err) = fs::create_dir_all(&draft_dir) {
            self.status = Some(format!("failed to create draft dir: {err}"));
            return;
        }

        let slug = self.state.content_editor.slug();
        let stem = if slug.is_empty() {
            "new-content"
        } else {
            &slug
        };

        let file_name = match target {
            EditableTextTarget::Overview => format!("{stem}.overview.md"),
            EditableTextTarget::Context => format!("{stem}.context.md"),
        };

        let draft_path = draft_dir.join(file_name);

        let current_text = match target {
            EditableTextTarget::Overview => self.state.content_editor.overview_buffer.clone(),
            EditableTextTarget::Context => self.state.content_editor.context_buffer.clone(),
        };

        if let Err(err) = fs::write(&draft_path, current_text) {
            self.status = Some(format!("failed to write draft markdown: {err}"));
            return;
        }

        self.open_markdown_with_configured_backend(draft_path);
    }

    fn open_selected_context_markdown(&mut self) {
        let Some(path) = self
            .state
            .selected_context_path()
            .map(|path| path.to_path_buf())
        else {
            self.warning_popup =
                Some("There is no context markdown file attached to this selection.".to_string());
            return;
        };

        self.open_existing_markdown(path);
    }

    fn open_existing_markdown(&mut self, path: std::path::PathBuf) {
        self.open_markdown_with_configured_backend(path);
    }

    fn open_markdown_with_configured_backend(&mut self, path: std::path::PathBuf) {
        match self.state.settings.terminal_layout {
            TerminalLayout::BuiltInRightPane => {
                self.open_markdown_in_embedded_terminal(path);
            }
            TerminalLayout::Auto => {
                self.open_markdown_in_embedded_terminal(path);
            }
            _ => match external_editor::open_markdown_file(&path, &self.state.settings) {
                Ok(()) => {
                    if self.state.settings.reload_after_external_edit {
                        if let Err(err) = self.state.reload() {
                            self.status = Some(format!("edited, but reload failed: {err}"));
                            return;
                        }
                    }
                    self.status = Some(format!("edited {}", path.display()));
                }
                Err(err) => {
                    self.status = Some(format!("external editor failed: {err}"));
                }
            },
        }
    }

    fn open_markdown_in_embedded_terminal(&mut self, path: std::path::PathBuf) {
        self.pending_embedded_terminal_path = Some(path.clone());
        self.right_terminal_focused = true;
        self.status = Some(format!("opening right terminal for {}", path.display()));
    }

    fn ensure_embedded_terminal_started(&mut self, area: Rect) {
        if self.embedded_terminal.is_some() {
            return;
        }

        let cols = area.width.saturating_sub(2).max(20);
        let rows = area.height.saturating_sub(2).max(8);

        if let Some(task) = self.pending_embedded_task.take() {
            let cwd = self.launch_cwd();

            match EmbeddedTerminal::spawn_task(&task.argv, &cwd, cols, rows) {
                Ok(terminal) => {
                    self.embedded_terminal = Some(terminal);
                    self.right_terminal_focused = true;
                    self.status = Some(format!("running {}", task.title));
                }
                Err(err) => {
                    self.embedded_terminal = None;
                    self.right_terminal_focused = false;
                    self.status = Some(format!("{} failed to start: {err}", task.title));
                }
            }

            return;
        }

        if self.pending_embedded_shell {
            self.pending_embedded_shell = false;
            let cwd = self.launch_cwd();

            match EmbeddedTerminal::spawn_shell(&cwd, cols, rows) {
                Ok(terminal) => {
                    self.embedded_terminal = Some(terminal);
                    self.right_terminal_focused = true;
                    self.status = Some(format!("right terminal shell at {}", cwd.display()));
                }
                Err(err) => {
                    self.embedded_terminal = None;
                    self.right_terminal_focused = false;
                    self.status = Some(format!("embedded shell failed: {err}"));
                }
            }

            return;
        }

        let Some(path) = self.pending_embedded_terminal_path.take() else {
            return;
        };

        let editor = self.state.settings.text_editor.command();

        match EmbeddedTerminal::spawn_editor(editor, &path, cols, rows) {
            Ok(terminal) => {
                self.embedded_terminal = Some(terminal);
                self.right_terminal_focused = true;
                self.status = Some(format!("editing {}", path.display()));
            }
            Err(err) => {
                self.embedded_terminal = None;
                self.right_terminal_focused = false;
                self.status = Some(format!("embedded terminal failed: {err}"));
            }
        }
    }

    fn poll_embedded_terminal(&mut self) {
        let Some(terminal) = self.embedded_terminal.as_mut() else {
            return;
        };

        match terminal.is_finished() {
            Ok(true) => {
                let finished_path = terminal.active_path.clone();
                self.embedded_terminal = None;

                if self.open_shell_after_editor_exit {
                    self.open_shell_after_editor_exit = false;
                    self.pending_embedded_shell = true;
                    self.right_terminal_focused = true;
                } else {
                    self.right_terminal_focused = false;
                }

                if self.state.settings.reload_after_external_edit {
                    if let Err(err) = self.state.reload() {
                        self.status = Some(format!("editor closed, but reload failed: {err}"));
                        return;
                    }
                }

                self.status = Some(format!("edited {}", finished_path.display()));
            }
            Ok(false) => {}
            Err(err) => {
                self.embedded_terminal = None;
                self.right_terminal_focused = false;
                self.status = Some(format!("embedded terminal poll failed: {err}"));
            }
        }
    }

    fn toggle_right_terminal(&mut self) {
        if let Some(terminal) = self.embedded_terminal.as_mut() {
            match terminal.kind {
                EmbeddedTerminalKind::Shell => {
                    let _ = terminal.write_bytes(b"exit\r");
                    self.embedded_terminal = None;
                    self.right_terminal_focused = false;
                    self.status = Some("right terminal closed".to_string());
                }
                EmbeddedTerminalKind::Editor => {
                    let _ = terminal.write_bytes(b"\x1b:wq!\r");
                    self.open_shell_after_editor_exit = true;
                    self.right_terminal_focused = true;
                    self.status = Some("closing editor, then opening shell".to_string());
                }
                EmbeddedTerminalKind::Task => {
                    let _ = terminal.terminate();
                    self.embedded_terminal = None;
                    self.right_terminal_focused = false;
                    self.status = Some("task pane closed".to_string());
                }
            }
            return;
        }

        self.pending_embedded_terminal_path = None;
        self.pending_embedded_shell = true;
        self.right_terminal_focused = true;
        self.status = Some("opening right terminal shell".to_string());
    }

    fn launch_cwd(&self) -> std::path::PathBuf {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    }

    fn handle_embedded_terminal_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('g') {
            self.right_terminal_focused = false;
            self.status =
                Some("right terminal unfocused; press e/E/ctrl-o to focus/open again".to_string());
            return;
        }

        let Some(terminal) = self.embedded_terminal.as_mut() else {
            self.right_terminal_focused = false;
            return;
        };

        let bytes = key_to_terminal_bytes(key);

        if bytes.is_empty() {
            return;
        }

        if let Err(err) = terminal.write_bytes(&bytes) {
            self.status = Some(format!("embedded terminal write failed: {err}"));
        }
    }

    fn create_content_from_editor(&mut self) {
        let result = match self.state.content_editor.mode {
            ContentEditorMode::RemoveExistingContent => {
                content_writer::remove_content_from_draft(&self.state.content_editor)
            }
            _ => content_writer::create_content_from_draft(&self.state.content_editor),
        };

        match result {
            Ok(message) => {
                self.status = Some(message);
                if let Err(err) = self.state.reload() {
                    self.status = Some(format!("operation succeeded, but reload failed: {err}"));
                }
                self.state.switch_view(AppView::MainExplorer);
            }
            Err(err) => {
                self.status = Some(format!("content operation failed: {err}"));
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(10),
                Constraint::Length(5),
            ])
            .split(frame.area());

        self.draw_header(frame, root[0]);

        match self.state.current_view {
            AppView::MainExplorer => self.draw_main_explorer(frame, root[1]),
            AppView::ContentEditor => self.draw_content_editor(frame, root[1]),
            AppView::BuildPanel => self.draw_panel_wizard(frame, root[1]),
            AppView::CookSection => self.draw_section_wizard(frame, root[1]),
            AppView::SectionWorkbench => self.draw_section_workbench(frame, root[1]),
            AppView::ConfigureMail => self.draw_mail_config(frame, root[1]),
            AppView::Settings => self.draw_settings(frame, root[1]),
        }

        self.draw_footer(frame, root[2]);

        if let Some(message) = &self.warning_popup {
            self.draw_warning_popup(frame, frame.area(), message);
        }
    }

    fn draw_header(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme();

        let title = match self.state.current_view {
            AppView::MainExplorer => MainExplorerView::snapshot(&self.state).title,
            view => view.title().to_string(),
        };

        let mut crumbs = vec![
            Span::styled(
                "alpnest",
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ╱  ", theme.faint_text()),
            Span::styled("terminal nest", theme.dim_text()),
            Span::styled("  ╱  ", theme.faint_text()),
            Span::styled(
                title,
                Style::default()
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ),
        ];

        if self.state.current_view != AppView::MainExplorer {
            crumbs.push(Span::styled("  ╱  ", theme.faint_text()));
            crumbs.push(Span::styled(self.state.theme.label, theme.faint_text()));
        }

        let status_line = match &self.status {
            Some(status) => Line::from(Span::styled(
                status.clone(),
                Style::default().fg(theme.success),
            )),
            None => Line::from(Span::styled(
                match self.state.current_view {
                    AppView::MainExplorer => "navigate the nest",
                    AppView::ContentEditor => "add or edit a content",
                    AppView::BuildPanel => "build or reshape panels",
                    AppView::CookSection => "cook, rename or remove sections",
                    AppView::SectionWorkbench => "work the section",
                    AppView::ConfigureMail => "configure mail accounts",
                    AppView::Settings => "alpnest-wide settings",
                },
                theme.faint_text(),
            )),
        };

        let header = Paragraph::new(vec![Line::from(crumbs), status_line])
            .alignment(Alignment::Center)
            .block(theme.plain_block());

        frame.render_widget(header, area);
    }

    fn draw_main_explorer(&mut self, frame: &mut Frame, area: Rect) {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(34), Constraint::Min(50)])
            .split(area);

        let left_stack = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
            .split(body[0]);

        let snapshot = MainExplorerView::snapshot(&self.state);

        self.draw_content_tree(frame, left_stack[0], &snapshot);
        self.draw_context(frame, left_stack[1], &snapshot);
        self.draw_focus(frame, body[1], &snapshot);
    }

    fn draw_content_tree(&self, frame: &mut Frame, area: Rect, snapshot: &MainExplorerSnapshot) {
        let theme = self.theme();

        let lines = if snapshot.rows.is_empty() {
            vec![Line::from(Span::styled("no contents found", theme.faint_text()))]
        } else {
            snapshot
                .rows
                .iter()
                .map(|row| {
                    let indent = "  ".repeat(row.depth);

                    let base = match row.depth {
                        0 => theme.depth_content,
                        1 => theme.depth_panel,
                        _ => theme.depth_section,
                    };

                    let style = if row.selected {
                        Style::default()
                            .fg(theme.selection_fg)
                            .bg(theme.selection_bg)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(base)
                    };

                    let marker_style = if row.selected {
                        Style::default().fg(theme.accent).bg(theme.selection_bg)
                    } else {
                        Style::default().fg(theme.faint)
                    };

                    Line::from(vec![
                        Span::styled(if row.selected { "▎" } else { " " }, marker_style),
                        Span::styled(indent, style),
                        Span::styled(
                            match row.depth {
                                0 => "",
                                1 => "├ ",
                                _ => "· ",
                            },
                            if row.selected {
                                marker_style
                            } else {
                                Style::default().fg(theme.faint)
                            },
                        ),
                        Span::styled(row.label.clone(), style),
                    ])
                })
                .collect()
        };

        let widget = Paragraph::new(lines)
            .block(theme.block("contents", true))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_context(&self, frame: &mut Frame, area: Rect, snapshot: &MainExplorerSnapshot) {
        let theme = self.theme();

        let text = match snapshot.context_path.as_deref() {
            Some(path) => read_text(path, "context file could not be read"),
            None => "context\n\nNo context file is attached to this selection yet.".to_string(),
        };

        let widget = Paragraph::new(markdown_lines(&text, theme))
            .block(theme.block("context", false))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_focus(&mut self, frame: &mut Frame, area: Rect, snapshot: &MainExplorerSnapshot) {
        if self.terminal_busy() {
            self.draw_embedded_terminal(frame, area);
            return;
        }

        let theme = self.theme();

        let text = match snapshot.body_path.as_deref() {
            Some(path) => read_text(path, "body file could not be read"),
            None => {
                "empty selection\n\nNo body file is attached to this selection yet.".to_string()
            }
        };

        let title = snapshot
            .body_path
            .as_deref()
            .and_then(|path| path.rsplit('/').next())
            .unwrap_or("body")
            .to_string();

        let widget = Paragraph::new(markdown_lines(&text, theme))
            .block(theme.block(&title, false))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_content_editor(&mut self, frame: &mut Frame, area: Rect) {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(56), Constraint::Min(50)])
            .split(area);

        let left_stack = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(12), Constraint::Length(10)])
            .split(body[0]);

        let theme = self.theme();
        let editor = &self.state.content_editor;

        let mut option_lines = vec![
            Line::from(Span::styled("add / edit content", theme.heading())),
            Line::from(""),
        ];

        for (_, label, selected) in editor.field_rows() {
            option_lines.push(theme.list_row(label, selected));
        }

        let options = Paragraph::new(option_lines)
            .block(theme.block("content setup", true))
            .wrap(Wrap { trim: false });

        frame.render_widget(options, left_stack[0]);

        let mut preview_lines = vec![Line::from(Span::styled("path preview", theme.subheading()))];

        for path in editor.path_preview_lines() {
            preview_lines.push(Line::from(Span::styled(path, theme.dim_text())));
        }

        let preview = Paragraph::new(preview_lines)
            .block(theme.block("preview", false))
            .wrap(Wrap { trim: false });

        frame.render_widget(preview, left_stack[1]);

        if self.terminal_busy() {
            self.draw_embedded_terminal(frame, body[1]);
            return;
        }

        let editor_text = editor.editor_text().to_string();
        let right = Paragraph::new(markdown_lines(&editor_text, theme))
            .block(theme.block(editor.editor_title().trim(), false))
            .wrap(Wrap { trim: false });

        frame.render_widget(right, body[1]);
    }

    fn draw_embedded_terminal(&mut self, frame: &mut Frame, area: Rect) {
        self.ensure_embedded_terminal_started(area);

        let theme = self.theme();
        let focused = self.right_terminal_focused;

        let title = if let Some(terminal) = &self.embedded_terminal {
            match terminal.kind {
                EmbeddedTerminalKind::Editor => {
                    let file = terminal
                        .active_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("markdown");

                    if focused {
                        format!("right pane: editing {file}")
                    } else {
                        format!("right pane: running {file}")
                    }
                }
                EmbeddedTerminalKind::Shell => {
                    if focused {
                        "right pane: shell focused".to_string()
                    } else {
                        "right pane: shell running".to_string()
                    }
                }
                EmbeddedTerminalKind::Task => "right pane: task".to_string(),
            }
        } else if let Some(task) = &self.pending_embedded_task {
            format!("right pane: starting {}", task.title)
        } else if self.pending_embedded_shell {
            "right pane: starting shell".to_string()
        } else {
            "right pane: starting".to_string()
        };

        let block = theme.block(&title, focused);

        let widget = if let Some(terminal) = self.embedded_terminal.as_mut() {
            let bytes = terminal.formatted_bytes();
            let ansi_text = String::from_utf8_lossy(&bytes);

            match ansi_text.as_ref().into_text() {
                Ok(text) => Paragraph::new(text).block(block).wrap(Wrap { trim: false }),
                Err(_) => Paragraph::new("embedded terminal render error")
                    .block(block)
                    .wrap(Wrap { trim: false }),
            }
        } else {
            let (headline, detail) = if let Some(task) = &self.pending_embedded_task {
                (
                    "starting task...".to_string(),
                    format!("command: {}", task.argv.join(" ")),
                )
            } else if self.pending_embedded_shell {
                (
                    "starting embedded shell...".to_string(),
                    format!("cwd: {}", self.launch_cwd().display()),
                )
            } else if let Some(path) = &self.pending_embedded_terminal_path {
                (
                    "starting embedded editor...".to_string(),
                    format!("file: {}", path.display()),
                )
            } else {
                ("no embedded terminal".to_string(), String::new())
            };

            let lines = vec![
                Line::from(Span::styled(
                    headline,
                    Style::default()
                        .fg(theme.success)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(detail, theme.dim_text())),
            ];

            Paragraph::new(lines).block(block).wrap(Wrap { trim: false })
        };

        frame.render_widget(widget, area);
    }

    fn draw_panel_wizard(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme();
        let wizard = &self.state.panel_wizard;

        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Min(14),
                Constraint::Length(9),
                Constraint::Length(3),
            ])
            .split(area);

        let header_rows = wizard
            .field_rows()
            .into_iter()
            .map(|(text, selected)| theme.list_row(text, selected))
            .collect::<Vec<_>>();

        let header = Paragraph::new(header_rows)
            .block(theme.block("panel wizard", true))
            .wrap(Wrap { trim: false });

        frame.render_widget(header, outer[0]);

        let middle = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(50), Constraint::Length(42)])
            .split(outer[1]);

        let inner = Paragraph::new(preview_lines(wizard.inner_lines(), theme))
            .block(theme.block(wizard.inner_mode.label(), false))
            .wrap(Wrap { trim: false });

        frame.render_widget(inner, middle[0]);

        let panel_lines = wizard
            .panel_rows()
            .into_iter()
            .map(|(text, selected)| theme.list_row(text, selected))
            .collect::<Vec<_>>();

        let panels = Paragraph::new(panel_lines)
            .block(theme.block("panels", false))
            .wrap(Wrap { trim: false });

        frame.render_widget(panels, middle[1]);

        let bottom = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(43), Constraint::Percentage(57)])
            .split(outer[2]);

        let default_lines = wizard
            .default_rows()
            .into_iter()
            .map(|(text, selected)| match text.strip_prefix("[x] ") {
                Some(label) => theme.checkbox(label, true, selected),
                None => match text.strip_prefix("[ ] ") {
                    Some(label) => theme.checkbox(label, false, selected),
                    None => theme.list_row(text, selected),
                },
            })
            .collect::<Vec<_>>();

        let defaults_title = format!(
            "defaults: {}",
            wizard.selected_panel_title().unwrap_or("no panel")
        );

        let defaults = Paragraph::new(default_lines)
            .block(theme.block(&defaults_title, false))
            .wrap(Wrap { trim: false });

        frame.render_widget(defaults, bottom[0]);

        let notifications = Paragraph::new(log_lines(&wizard.logs, theme, 6))
            .block(theme.block("notifications", false))
            .wrap(Wrap { trim: false });

        frame.render_widget(notifications, bottom[1]);

        let mut hints = vec![
            ("j/k", "move"),
            ("enter", "select"),
            ("ctrl-s", "apply"),
            ("ctrl-t", "preview"),
            ("p", "panels"),
            ("f", "fields"),
        ];

        if wizard.operation != PanelWizardOperation::Destroy {
            hints.push(("d", "defaults"));
        }

        hints.push(("esc", "back"));

        let footer = Paragraph::new(theme.key_hints(&hints))
            .alignment(Alignment::Center)
            .block(theme.plain_block());

        frame.render_widget(footer, outer[3]);
    }

    fn draw_section_wizard(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme();
        let wizard = &self.state.section_wizard;

        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Min(14),
                Constraint::Length(9),
                Constraint::Length(3),
            ])
            .split(area);

        let header_rows = wizard
            .field_rows()
            .into_iter()
            .map(|(label, value, selected)| theme.field_row(&label, &value, selected))
            .collect::<Vec<_>>();

        let header = Paragraph::new(header_rows)
            .block(theme.block("cook section", true))
            .wrap(Wrap { trim: false });

        frame.render_widget(header, outer[0]);

        let middle = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(50), Constraint::Length(42)])
            .split(outer[1]);

        let inner = Paragraph::new(preview_lines(wizard.inner_lines(), theme))
            .block(theme.block(wizard.inner_mode.label(), false))
            .wrap(Wrap { trim: false });

        frame.render_widget(inner, middle[0]);

        let section_lines = if wizard.visible_section_len() == 0 {
            vec![Line::from(Span::styled(
                match wizard.operation {
                    SectionOperation::Cook => "set a section count first",
                    _ => "this panel has no sections yet",
                },
                theme.faint_text(),
            ))]
        } else {
            wizard
                .section_rows()
                .into_iter()
                .map(|(text, selected)| theme.list_row(text, selected))
                .collect()
        };

        let sections = Paragraph::new(section_lines)
            .block(theme.block("sections", false))
            .wrap(Wrap { trim: false });

        frame.render_widget(sections, middle[1]);

        let bottom = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(43), Constraint::Percentage(57)])
            .split(outer[2]);

        let default_lines = wizard
            .default_rows()
            .into_iter()
            .map(|(label, checked, is_toggle, selected)| {
                if is_toggle {
                    theme.checkbox(&label, checked, selected)
                } else {
                    theme.list_row(label, selected)
                }
            })
            .collect::<Vec<_>>();

        let defaults_title = format!(
            "defaults: {}",
            wizard.selected_section_name().unwrap_or("no section")
        );

        let defaults = Paragraph::new(default_lines)
            .block(theme.block(&defaults_title, false))
            .wrap(Wrap { trim: false });

        frame.render_widget(defaults, bottom[0]);

        let notifications = Paragraph::new(log_lines(&wizard.logs, theme, 6))
            .block(theme.block("notifications", false))
            .wrap(Wrap { trim: false });

        frame.render_widget(notifications, bottom[1]);

        let mut hints = vec![
            ("j/k", "move"),
            ("enter", "select"),
            ("ctrl-s", "apply"),
            ("ctrl-t", "preview"),
            ("s", "sections"),
            ("f", "fields"),
        ];

        if wizard.operation == SectionOperation::Cook {
            hints.push(("d", "defaults"));
        }

        hints.push(("esc", "back"));

        let footer = Paragraph::new(theme.key_hints(&hints))
            .alignment(Alignment::Center)
            .block(theme.plain_block());

        frame.render_widget(footer, outer[3]);
    }

    fn draw_section_workbench(&mut self, frame: &mut Frame, area: Rect) {
        let theme = self.theme();

        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(12), Constraint::Length(3)])
            .split(area);

        self.draw_workbench_meter(frame, outer[0]);

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(34),
                Constraint::Min(40),
                Constraint::Length(34),
            ])
            .split(outer[1]);

        self.draw_workbench_steps(frame, body[0]);
        self.draw_workbench_focus(frame, body[1]);
        self.draw_workbench_brief(frame, body[2]);

        let footer = Paragraph::new(theme.key_hints(&[
            ("j/k", "move"),
            ("space", "toggle step"),
            ("n", "next action"),
            ("tab", "steps/body"),
            ("e", "edit body"),
            ("c", "edit context"),
            ("ctrl-t", "terminal"),
            ("r", "reload"),
            ("esc", "back"),
        ]))
        .alignment(Alignment::Center)
        .block(theme.plain_block());

        frame.render_widget(footer, outer[2]);
    }

    /// Breadcrumb on the left, a btop-style completion meter on the right.
    fn draw_workbench_meter(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme();
        let workbench = &self.state.section_workbench;

        let total = workbench.total_steps();
        let done = workbench.done_steps();

        const WIDTH: usize = 24;
        let filled = if total == 0 {
            0
        } else {
            ((workbench.progress_ratio() * WIDTH as f64).round() as usize).min(WIDTH)
        };

        let bar_color = if total == 0 {
            theme.faint
        } else if done == total {
            theme.success
        } else if workbench.progress_ratio() >= 0.5 {
            theme.accent_alt
        } else {
            theme.warning
        };

        let mut spans = vec![
            Span::styled(
                workbench.breadcrumb(),
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("█".repeat(filled), Style::default().fg(bar_color)),
            Span::styled("░".repeat(WIDTH - filled), Style::default().fg(theme.faint)),
        ];

        spans.push(Span::styled(
            if total == 0 {
                "  no steps".to_string()
            } else {
                format!("  {done}/{total}")
            },
            theme.dim_text(),
        ));

        if let Some(status) = &workbench.status {
            spans.push(Span::styled("   ·   ", theme.faint_text()));
            spans.push(Span::styled(status.clone(), Style::default().fg(theme.success)));
        }

        let widget = Paragraph::new(Line::from(spans)).block(theme.plain_block());
        frame.render_widget(widget, area);
    }

    fn draw_workbench_steps(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme();
        let workbench = &self.state.section_workbench;
        let focused = workbench.focus == WorkbenchFocus::Steps;

        let mut lines = Vec::new();

        if workbench.total_steps() == 0 {
            lines.push(Line::from(Span::styled(
                "No steps yet.",
                theme.dim_text(),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Add checkbox lines to the body and",
                theme.faint_text(),
            )));
            lines.push(Line::from(Span::styled(
                "they become tracked steps here:",
                theme.faint_text(),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  - [ ] read chapter 3",
                Style::default().fg(theme.accent_alt),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press e to edit the body.",
                theme.faint_text(),
            )));
        } else {
            for (depth, text, done, selected, is_next) in workbench.step_rows() {
                let indent = "  ".repeat(depth);

                let box_style = if done {
                    Style::default().fg(theme.success)
                } else if is_next {
                    Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.faint)
                };

                let text_style = match (selected, done) {
                    (true, _) => theme.selected_row(),
                    (false, true) => theme.faint_text(),
                    (false, false) => theme.text(),
                };

                lines.push(Line::from(vec![
                    Span::styled(
                        if selected { "▎" } else { " " },
                        if selected {
                            Style::default().fg(theme.accent).bg(theme.selection_bg)
                        } else {
                            Style::default().fg(theme.faint)
                        },
                    ),
                    Span::styled(indent, text_style),
                    Span::styled(if done { "☑ " } else { "☐ " }, box_style),
                    Span::styled(text, text_style),
                ]));
            }
        }

        let title = if workbench.total_steps() == 0 {
            "steps".to_string()
        } else {
            format!("steps · {}/{}", workbench.done_steps(), workbench.total_steps())
        };

        let widget = Paragraph::new(lines)
            .block(theme.block(&title, focused))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_workbench_focus(&mut self, frame: &mut Frame, area: Rect) {
        if self.terminal_busy() {
            self.draw_embedded_terminal(frame, area);
            return;
        }

        let theme = self.theme();
        let workbench = &self.state.section_workbench;
        let focused = workbench.focus == WorkbenchFocus::Body;

        let text = workbench.body_text();

        let widget = Paragraph::new(markdown_lines(&text, theme))
            .scroll((workbench.body_scroll, 0))
            .block(theme.block(&workbench.section_title, focused))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    /// The guidance column: what this section is for, plus derived signals.
    fn draw_workbench_brief(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme();
        let workbench = &self.state.section_workbench;

        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(6), Constraint::Length(9)])
            .split(area);

        let brief = Paragraph::new(markdown_lines(&workbench.context_text(), theme))
            .block(theme.block("brief", false))
            .wrap(Wrap { trim: false });

        frame.render_widget(brief, split[0]);

        let mut lines = Vec::new();

        if let Some(step) = workbench.next_action() {
            lines.push(Line::from(Span::styled("next action", theme.subheading())));
            lines.push(Line::from(Span::styled(
                step.text.clone(),
                Style::default().fg(theme.warning).add_modifier(Modifier::BOLD),
            )));
        } else if workbench.total_steps() > 0 {
            lines.push(Line::from(Span::styled(
                "all steps complete",
                Style::default().fg(theme.success).add_modifier(Modifier::BOLD),
            )));
        }

        if !lines.is_empty() {
            lines.push(Line::from(""));
        }

        for (label, value) in workbench.signal_rows() {
            lines.push(Line::from(vec![
                Span::styled(format!("{label}: "), theme.faint_text()),
                Span::styled(value, theme.dim_text()),
            ]));
        }

        let signals = Paragraph::new(lines)
            .block(theme.block("signals", false))
            .wrap(Wrap { trim: false });

        frame.render_widget(signals, split[1]);
    }

    fn draw_mail_config(&mut self, frame: &mut Frame, area: Rect) {
        let theme = self.theme();

        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(16), Constraint::Length(8), Constraint::Length(3)])
            .split(area);

        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(36), Constraint::Min(44)])
            .split(outer[0]);

        let config = &self.state.mail_config;

        let account_lines = config
            .account_rows()
            .into_iter()
            .map(|(text, selected)| theme.list_row(text, selected))
            .collect::<Vec<_>>();

        let accounts = Paragraph::new(account_lines)
            .block(theme.block(
                &format!("accounts ({})", config.registry.len()),
                config.focus == MailConfigFocus::Accounts,
            ))
            .wrap(Wrap { trim: false });

        frame.render_widget(accounts, top[0]);

        let form_lines = config
            .form_rows()
            .into_iter()
            .map(|(label, value, selected, is_action)| {
                if is_action {
                    theme.list_row(format!("→ {value}"), selected)
                } else {
                    theme.field_row(&label, &value, selected)
                }
            })
            .collect::<Vec<_>>();

        let form_title = if config.draft_is_new {
            "new account".to_string()
        } else if config.dirty {
            format!("{} (unsaved)", config.draft.id)
        } else {
            config.draft.id.clone()
        };

        let form = Paragraph::new(form_lines)
            .block(theme.block(&form_title, config.focus == MailConfigFocus::Form))
            .wrap(Wrap { trim: false });

        frame.render_widget(form, top[1]);

        let bottom = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(outer[1]);

        let mut hint_lines = vec![Line::from(vec![
            Span::styled("credential ", theme.dim_text()),
            theme.badge(
                config.credential_state().label(),
                match config.credential_state() {
                    alpnest::mail::keychain::CredentialState::Stored => BadgeLevel::Ok,
                    alpnest::mail::keychain::CredentialState::Missing => BadgeLevel::Warn,
                    alpnest::mail::keychain::CredentialState::NotApplicable => BadgeLevel::Idle,
                    alpnest::mail::keychain::CredentialState::Unsupported => BadgeLevel::Bad,
                },
            ),
        ])];

        for line in config.hint_lines() {
            hint_lines.push(Line::from(Span::styled(line, theme.dim_text())));
        }

        let hints = Paragraph::new(hint_lines)
            .block(theme.block("provider notes", false))
            .wrap(Wrap { trim: false });

        frame.render_widget(hints, bottom[0]);

        let notifications = Paragraph::new(log_lines(&config.logs, theme, 6))
            .block(theme.block("mail log", false))
            .wrap(Wrap { trim: false });

        frame.render_widget(notifications, bottom[1]);

        let footer = Paragraph::new(theme.key_hints(&[
            ("j/k", "move"),
            ("←/→", "list/form"),
            ("enter", "edit/run"),
            ("n", "new account"),
            ("ctrl-s", "save"),
            ("S", "sync all"),
            ("esc", "back"),
        ]))
        .alignment(Alignment::Center)
        .block(theme.plain_block());

        frame.render_widget(footer, outer[2]);

        if self.terminal_busy() {
            let overlay = centered_rect(78, 62, area);
            frame.render_widget(ratatui::widgets::Clear, overlay);
            self.draw_embedded_terminal(frame, overlay);
        }
    }

    fn draw_settings(&self, frame: &mut Frame, area: Rect) {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(58), Constraint::Min(50)])
            .split(area);

        let theme = self.theme();

        let mut rows = vec![
            Line::from(Span::styled("alpnest settings", theme.heading())),
            Line::from(""),
        ];

        for (_, label, selected) in self.state.settings.rows() {
            rows.push(theme.list_row(label, selected));
        }

        let settings = Paragraph::new(rows)
            .block(theme.block("setup", true))
            .wrap(Wrap { trim: false });

        frame.render_widget(settings, body[0]);

        let help = format!(
            "# Settings\n\n\
Text editor and terminal layout are Alpnest-wide settings.\n\n\
Current behavior\n\
- editor command: {}\n\
- terminal layout: {}\n\
- reload after external edit: {}\n\n\
Supported terminal layouts\n\
- built-in embedded right pane: opens vim/editor inside Alpnest's own right pane\n\
- auto: zellij if detected, else tmux if detected, else same-terminal suspend\n\
- same terminal / suspend TUI: temporarily leaves Alpnest, opens editor, then returns\n\
- zellij right pane: opens the editor in a right pane and waits for it to finish\n\
- tmux right pane: opens the editor in a right pane and waits for it to finish\n\n\
Supported editors\n\
- vi, vim, nvim, nano, hx, emacs\n\n\
Alp workflow target\n\
- editor: neovim / nvim\n\
- terminal layout: zellij right pane\n\n\
Config file is stored under ALPNEST_HOME/config/alpnest.toml.",
            self.state.settings.text_editor.command(),
            self.state.settings.terminal_layout.label(),
            if self.state.settings.reload_after_external_edit {
                "yes"
            } else {
                "no"
            },
        );

        let right = Paragraph::new(markdown_lines(&help, theme))
            .block(theme.block("explanation", false))
            .wrap(Wrap { trim: false });

        frame.render_widget(right, body[1]);
    }

    fn draw_warning_popup(&self, frame: &mut Frame, area: Rect, message: &str) {
        let theme = self.theme();
        let popup_area = centered_rect(54, 18, area);

        let text = vec![
            Line::from(Span::styled(
                "warning",
                Style::default()
                    .fg(theme.danger)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(message.to_string(), theme.text())),
            Line::from(""),
            Line::from(Span::styled("press any key to continue", theme.faint_text())),
        ];

        let widget = Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(theme.block("alpnest notice", true))
            .wrap(Wrap { trim: false });

        frame.render_widget(ratatui::widgets::Clear, popup_area);
        frame.render_widget(widget, popup_area);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme();

        let terminal_hints = [
            ("ctrl-t", "toggle shell"),
            ("ctrl-g", "unfocus"),
            (":wq", "exit editor"),
        ];

        let hints: &[(&str, &str)] = match self.state.current_view {
            _ if self.right_terminal_focused => &terminal_hints,
            AppView::MainExplorer => &[
                ("j/k", "move"),
                ("enter", "open"),
                ("E", "edit context"),
                ("ctrl-t", "terminal"),
                ("a", "content"),
                ("b", "panel"),
                ("c", "section"),
                ("m", "mail"),
                ("s", "settings"),
                ("q", "quit"),
            ],
            AppView::ContentEditor => &[
                ("tab/j", "move"),
                ("space", "cycle"),
                ("enter", "edit"),
                ("ctrl-o", "right pane"),
                ("ctrl-s", "create"),
                ("esc", "cancel"),
            ],
            AppView::Settings => &[
                ("tab/j", "move"),
                ("space", "change"),
                ("enter", "open/change"),
                ("esc", "back"),
            ],
            AppView::BuildPanel
            | AppView::CookSection
            | AppView::ConfigureMail
            | AppView::SectionWorkbench => &[("", "controls are in the view's own footer")],
        };

        let widget = Paragraph::new(theme.key_hints(hints))
            .alignment(Alignment::Center)
            .block(theme.plain_block());

        frame.render_widget(widget, area);
    }
}

/// Preview panes mark the active row with a leading `>`; colour it so the
/// eye lands on it without reading.
fn preview_lines(lines: Vec<String>, theme: Theme) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|line| {
            if line.starts_with("> ") {
                Line::from(Span::styled(
                    line,
                    Style::default()
                        .fg(theme.accent_alt)
                        .add_modifier(Modifier::BOLD),
                ))
            } else if line.trim_start().starts_with("file:")
                || line.trim_start().starts_with("dir:")
            {
                Line::from(Span::styled(line, theme.faint_text()))
            } else {
                Line::from(Span::styled(line, theme.dim_text()))
            }
        })
        .collect()
}

fn log_lines(logs: &[LogEntry], theme: Theme, take: usize) -> Vec<Line<'static>> {
    logs.iter()
        .rev()
        .take(take)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|entry| {
            let style = match entry.level {
                LogLevel::Note => Style::default().fg(theme.dim),
                LogLevel::Info => Style::default().fg(theme.success),
                LogLevel::Warning => Style::default().fg(theme.warning),
                LogLevel::Error => Style::default().fg(theme.danger),
            };

            Line::from(vec![
                Span::styled(format!("{} ", entry.level.tag()), style),
                Span::styled(entry.message.clone(), theme.text()),
            ])
        })
        .collect()
}

/// True when a modifier is held that no plain-key binding should react to.
///
/// Every view binds bare letters (`a`, `b`, `q`, …). Without checking this, a
/// `ctrl-<letter>` that a handler does not claim falls through to the bare
/// arm and either fires the wrong action or types a stray character.
fn modified(key: KeyEvent) -> bool {
    key.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);

    horizontal[1]
}

fn key_to_terminal_bytes(key: KeyEvent) -> Vec<u8> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = key.code {
            let upper = c.to_ascii_uppercase();

            if upper.is_ascii_alphabetic() {
                return vec![(upper as u8) - b'A' + 1];
            }
        }
    }

    match key.code {
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => b"\r".to_vec(),
        KeyCode::Tab => b"\t".to_vec(),
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        _ => Vec::new(),
    }
}

fn read_text(path: &str, fallback: &str) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|err| format!("# error\n\n{fallback}\n\npath: {path}\nerror: {err}"))
}

fn markdown_lines(text: &str, theme: Theme) -> Vec<Line<'static>> {
    text.lines().map(|line| markdown_line(line, theme)).collect()
}

fn markdown_line(line: &str, theme: Theme) -> Line<'static> {
    let trimmed = line.trim_start();

    if let Some(rest) = trimmed.strip_prefix("### ") {
        return Line::from(Span::styled(
            rest.to_string(),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(rest) = trimmed.strip_prefix("## ") {
        return Line::from(Span::styled(rest.to_string(), theme.subheading()));
    }

    if let Some(rest) = trimmed.strip_prefix("# ") {
        return Line::from(Span::styled(rest.to_string(), theme.heading()));
    }

    if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
        return Line::from(vec![
            Span::styled("☐ ", Style::default().fg(theme.warning)),
            Span::styled(rest.to_string(), theme.text()),
        ]);
    }

    if let Some(rest) = trimmed.strip_prefix("- [x] ") {
        return Line::from(vec![
            Span::styled("☑ ", Style::default().fg(theme.success)),
            Span::styled(rest.to_string(), theme.dim_text()),
        ]);
    }

    if let Some(rest) = trimmed.strip_prefix("- ") {
        return Line::from(vec![
            Span::styled("• ", Style::default().fg(theme.accent_alt)),
            Span::styled(rest.to_string(), theme.text()),
        ]);
    }

    if trimmed.starts_with("```") {
        return Line::from(Span::styled(line.to_string(), theme.faint_text()));
    }

    if trimmed.starts_with("> ") {
        return Line::from(Span::styled(line.to_string(), theme.dim_text()));
    }

    Line::from(Span::styled(line.to_string(), theme.text()))
}

fn main() -> Result<()> {
    color_eyre::install()?;
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = RuntimeApp::load()?;

    while !app.should_quit {
        app.poll_embedded_terminal();
        app.poll_background_sync();
        terminal.draw(|frame| app.draw(frame))?;

        if event::poll(Duration::from_millis(30))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        }
    }

    Ok(())
}
