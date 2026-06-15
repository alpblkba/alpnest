use std::{fs, io, time::Duration};

use alpnest::embedded_terminal::{EmbeddedTerminal, EmbeddedTerminalKind};
use alpnest::settings::TerminalLayout;
use alpnest::{
    app::AppState,
    app_view::AppView,
    content_editor::{ContentEditorField, ContentEditorMode, EditableTextTarget},
    content_writer, external_editor,
    paths::AlpnestPaths,
    settings::SettingsField,
    ui::main_explorer::{MainExplorerSnapshot, MainExplorerView},
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
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

struct RuntimeApp {
    state: AppState,
    should_quit: bool,
    status: Option<String>,
    warning_popup: Option<String>,
    embedded_terminal: Option<EmbeddedTerminal>,
    pending_embedded_terminal_path: Option<std::path::PathBuf>,
    pending_embedded_shell: bool,
    open_shell_after_editor_exit: bool,
    right_terminal_focused: bool,
}

impl RuntimeApp {
    fn load() -> Result<Self> {
        Ok(Self {
            state: AppState::load()?,
            should_quit: false,
            status: None,
            warning_popup: None,
            embedded_terminal: None,
            pending_embedded_terminal_path: None,
            pending_embedded_shell: false,
            open_shell_after_editor_exit: false,
            right_terminal_focused: false,
        })
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('t') {
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

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        match self.state.current_view {
            AppView::ContentEditor => {
                self.handle_content_editor_key(key);
                return;
            }
            AppView::Settings => {
                self.handle_settings_key(key);
                return;
            }
            _ => {}
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
                    self.state.enter();
                }
            }
            KeyCode::Char('E') => self.open_selected_context_markdown(),
            KeyCode::Esc | KeyCode::Backspace => self.state.back(),
            KeyCode::Char('a') => self.state.open_content_editor(),
            KeyCode::Char('b') => self.state.switch_view(AppView::BuildPanel),
            KeyCode::Char('c') => self.state.switch_view(AppView::CookSection),
            KeyCode::Char('m') => self.state.switch_view(AppView::ConfigureMail),
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
                _ => {
                    self.state.settings.cycle_selected();
                    self.save_settings_status();
                }
            },
            _ => {}
        }
    }

    fn save_settings_status(&mut self) {
        match self.state.settings.save() {
            Ok(()) => self.status = Some("settings saved".to_string()),
            Err(err) => self.status = Some(format!("settings save failed: {err}")),
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
            AppView::Settings => self.draw_settings(frame, root[1]),
            view => self.draw_placeholder_view(frame, root[1], view),
        }

        self.draw_footer(frame, root[2]);

        if let Some(message) = &self.warning_popup {
            self.draw_warning_popup(frame, frame.area(), message);
        }
    }

    fn draw_header(&self, frame: &mut Frame, area: Rect) {
        let title = match self.state.current_view {
            AppView::MainExplorer => MainExplorerView::snapshot(&self.state).title,
            view => format!("Alpnest / {}", view.title()),
        };

        let mut spans = vec![
            Span::styled(
                "alpnest",
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  —  "),
            Span::styled(title, Style::default().fg(Color::Gray)),
        ];

        if let Some(status) = &self.status {
            spans.push(Span::raw("  —  "));
            spans.push(Span::styled(
                status.clone(),
                Style::default().fg(Color::LightGreen),
            ));
        }

        let header = Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" terminal nest "),
            );

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
        let lines = if snapshot.rows.is_empty() {
            vec![Line::from(Span::styled(
                "no contents found",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            snapshot
                .rows
                .iter()
                .map(|row| {
                    let indent = "  ".repeat(row.depth);
                    let marker = if row.selected { ">" } else { " " };
                    let style = match (row.selected, row.depth) {
                        (true, 0) => Style::default()
                            .fg(Color::LightMagenta)
                            .add_modifier(Modifier::BOLD),
                        (true, 1) => Style::default()
                            .fg(Color::LightCyan)
                            .add_modifier(Modifier::BOLD),
                        (true, _) => Style::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                        (false, 0) => Style::default().fg(Color::White),
                        (false, 1) => Style::default().fg(Color::Gray),
                        (false, _) => Style::default().fg(Color::DarkGray),
                    };

                    Line::from(vec![
                        Span::raw(indent),
                        Span::styled(marker, style),
                        Span::raw(" "),
                        Span::styled(row.label.clone(), style),
                    ])
                })
                .collect()
        };

        let widget = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" contents "))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_context(&self, frame: &mut Frame, area: Rect, snapshot: &MainExplorerSnapshot) {
        let text = match snapshot.context_path.as_deref() {
            Some(path) => read_text(path, "context file could not be read"),
            None => "context\n\nNo context file is attached to this selection yet.".to_string(),
        };

        let widget = Paragraph::new(markdown_lines(&text))
            .block(Block::default().borders(Borders::ALL).title(" context "))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_focus(&mut self, frame: &mut Frame, area: Rect, snapshot: &MainExplorerSnapshot) {
        if self.embedded_terminal.is_some()
            || self.pending_embedded_terminal_path.is_some()
            || self.pending_embedded_shell
        {
            self.draw_embedded_terminal(frame, area);
            return;
        }

        let text = match snapshot.body_path.as_deref() {
            Some(path) => read_text(path, "body file could not be read"),
            None => {
                "empty selection\n\nNo body file is attached to this selection yet.".to_string()
            }
        };

        let widget = Paragraph::new(markdown_lines(&text))
            .block(Block::default().borders(Borders::ALL).title(" body "))
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

        let editor = &self.state.content_editor;

        let mut option_lines = vec![
            Line::from(Span::styled(
                "add/edit content",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        for (_, label, selected) in editor.field_rows() {
            let marker = if selected { ">" } else { " " };
            let style = if selected {
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            option_lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::raw(" "),
                Span::styled(label, style),
            ]));
        }

        let options = Paragraph::new(option_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" content setup "),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(options, left_stack[0]);

        let mut preview_lines = vec![
            Line::from(Span::styled(
                "path preview",
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        for path in editor.path_preview_lines() {
            preview_lines.push(Line::from(path));
        }

        let preview = Paragraph::new(preview_lines)
            .block(Block::default().borders(Borders::ALL).title(" preview "))
            .wrap(Wrap { trim: false });

        frame.render_widget(preview, left_stack[1]);

        if self.embedded_terminal.is_some()
            || self.pending_embedded_terminal_path.is_some()
            || self.pending_embedded_shell
        {
            self.draw_embedded_terminal(frame, body[1]);
            return;
        }

        let editor_text = editor.editor_text().to_string();
        let right = Paragraph::new(markdown_lines(&editor_text))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(editor.editor_title()),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(right, body[1]);
    }

    fn draw_embedded_terminal(&mut self, frame: &mut Frame, area: Rect) {
        self.ensure_embedded_terminal_started(area);

        let title = if let Some(terminal) = &self.embedded_terminal {
            match terminal.kind {
                EmbeddedTerminalKind::Editor => {
                    let file = terminal
                        .active_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("markdown");

                    if self.right_terminal_focused {
                        format!(" right terminal: editing {file} ")
                    } else {
                        format!(" right terminal: running {file} ")
                    }
                }
                EmbeddedTerminalKind::Shell => {
                    if self.right_terminal_focused {
                        " right terminal: shell focused ".to_string()
                    } else {
                        " right terminal: shell running ".to_string()
                    }
                }
            }
        } else if self.pending_embedded_shell {
            " right terminal: starting shell ".to_string()
        } else {
            " right terminal: starting ".to_string()
        };

        let widget = if let Some(terminal) = self.embedded_terminal.as_mut() {
            let bytes = terminal.formatted_bytes();
            let ansi_text = String::from_utf8_lossy(&bytes);

            match ansi_text.as_ref().into_text() {
                Ok(text) => Paragraph::new(text)
                    .block(Block::default().borders(Borders::ALL).title(title))
                    .wrap(Wrap { trim: false }),
                Err(_) => Paragraph::new("embedded terminal render error")
                    .block(Block::default().borders(Borders::ALL).title(title))
                    .wrap(Wrap { trim: false }),
            }
        } else if self.pending_embedded_shell {
            let lines = vec![
                Line::from(Span::styled(
                    "starting embedded shell...",
                    Style::default()
                        .fg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("cwd: {}", self.launch_cwd().display())),
            ];

            Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title(title))
                .wrap(Wrap { trim: false })
        } else if let Some(path) = &self.pending_embedded_terminal_path {
            let lines = vec![
                Line::from(Span::styled(
                    "starting embedded editor...",
                    Style::default()
                        .fg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("file: {}", path.display())),
            ];

            Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title(title))
                .wrap(Wrap { trim: false })
        } else {
            Paragraph::new("no embedded terminal")
                .block(Block::default().borders(Borders::ALL).title(title))
                .wrap(Wrap { trim: false })
        };

        frame.render_widget(widget, area);
    }

    fn draw_settings(&self, frame: &mut Frame, area: Rect) {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(58), Constraint::Min(50)])
            .split(area);

        let mut rows = vec![
            Line::from(Span::styled(
                "alpnest settings",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        for (_, label, selected) in self.state.settings.rows() {
            let marker = if selected { ">" } else { " " };
            let style = if selected {
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            rows.push(Line::from(vec![
                Span::styled(marker, style),
                Span::raw(" "),
                Span::styled(label, style),
            ]));
        }

        let settings = Paragraph::new(rows)
            .block(Block::default().borders(Borders::ALL).title(" setup "))
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

        let right = Paragraph::new(markdown_lines(&help))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" explanation "),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(right, body[1]);
    }

    fn draw_placeholder_view(&self, frame: &mut Frame, area: Rect, view: AppView) {
        let text = format!(
            "# {}\n\nThis app view is reserved but not implemented yet.\n\nPlanned direction:\n- Build or reshape panels\n- Cook sections through local-first workflows\n- Configure local mail accounts\n\nPress h or Esc to return to the main explorer.",
            view.title()
        );

        let widget = Paragraph::new(markdown_lines(&text))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", view.title())),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_warning_popup(&self, frame: &mut Frame, area: Rect, message: &str) {
        let popup_area = centered_rect(54, 18, area);

        let text = vec![
            Line::from(Span::styled(
                "warning",
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(message.to_string()),
            Line::from(""),
            Line::from(Span::styled(
                "press any key to continue",
                Style::default().fg(Color::Gray),
            )),
        ];

        let widget = Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" alpnest notice "),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(ratatui::widgets::Clear, popup_area);
        frame.render_widget(widget, popup_area);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let help = match self.state.current_view {
            AppView::MainExplorer => {
                if self.right_terminal_focused {
                    "right terminal focused    ctrl-t toggle shell/close    vim :wq/:q exits editor    ctrl-g unfocus terminal"
                } else {
                    "j/k move    enter open tree    E edit context    ctrl-t terminal    a content    b panel    c section    m mail    s settings    q quit"
                }
            }
            AppView::ContentEditor => {
                if self.right_terminal_focused {
                    "right terminal focused    ctrl-t toggle shell/close    vim :wq/:q exits editor    ctrl-g unfocus terminal"
                } else {
                    "tab/j move    space cycle/toggle    enter edit markdown/select    ctrl-o edit in right terminal    ctrl-s create    esc cancel"
                }
            }
            AppView::Settings => {
                "tab/j move    space/enter change setting    save is automatic    h/esc back"
            }
            _ => "h or esc return to main explorer    q quit",
        };

        let widget = Paragraph::new(help)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        frame.render_widget(widget, area);
    }
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

fn markdown_lines(text: &str) -> Vec<Line<'static>> {
    text.lines().map(markdown_line).collect()
}

fn markdown_line(line: &str) -> Line<'static> {
    let trimmed = line.trim_start();

    if trimmed.starts_with("# ") {
        return Line::from(Span::styled(
            trimmed.trim_start_matches("# ").to_string(),
            Style::default()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if trimmed.starts_with("## ") {
        return Line::from(Span::styled(
            trimmed.trim_start_matches("## ").to_string(),
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if trimmed.starts_with("- ") {
        return Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::LightGreen)),
            Span::raw(trimmed.trim_start_matches("- ").to_string()),
        ]);
    }

    Line::from(line.to_string())
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
        terminal.draw(|frame| app.draw(frame))?;

        if event::poll(Duration::from_millis(30))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        }
    }

    Ok(())
}
