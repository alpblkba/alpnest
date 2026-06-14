use std::{fs, io};

use alpnest::{
    app::AppState,
    app_view::AppView,
    ui::main_explorer::{MainExplorerSnapshot, MainExplorerView},
};
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
}

impl RuntimeApp {
    fn load() -> Result<Self> {
        Ok(Self {
            state: AppState::load()?,
            should_quit: false,
            status: None,
        })
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
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
                    self.state.enter();
                }
            }
            KeyCode::Esc | KeyCode::Backspace => self.state.back(),
            KeyCode::Char('r') => match self.state.reload() {
                Ok(()) => self.status = Some("registry reloaded".to_string()),
                Err(err) => self.status = Some(format!("reload failed: {err}")),
            },
            KeyCode::Char('a') => self.state.switch_view(AppView::ContentEditor),
            KeyCode::Char('b') => self.state.switch_view(AppView::BuildPanel),
            KeyCode::Char('c') => self.state.switch_view(AppView::CookSection),
            KeyCode::Char('m') => self.state.switch_view(AppView::ConfigureMail),
            KeyCode::Char('h') => self.state.switch_view(AppView::MainExplorer),
            _ => {}
        }
    }

    fn draw(&self, frame: &mut Frame) {
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
            view => self.draw_placeholder_view(frame, root[1], view),
        }

        self.draw_footer(frame, root[2]);
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

    fn draw_main_explorer(&self, frame: &mut Frame, area: Rect) {
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
                        (false, 0) => Style::default().fg(Color::Gray),
                        (false, 1) => Style::default().fg(Color::DarkGray),
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

    fn draw_focus(&self, frame: &mut Frame, area: Rect, snapshot: &MainExplorerSnapshot) {
        let (title, text) = match snapshot.body_path.as_deref() {
            Some(path) => (
                format!(" body: {path} "),
                read_text(path, "selected body file is missing or unreadable"),
            ),
            None => (
                " body ".to_string(),
                "# empty selection\n\nNo body file is attached to this selection yet.".to_string(),
            ),
        };

        let widget = Paragraph::new(markdown_lines(&text))
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_context(&self, frame: &mut Frame, area: Rect, snapshot: &MainExplorerSnapshot) {
        let text = match snapshot.context_path.as_deref() {
            Some(path) => read_text(path, "selected context file is missing or unreadable"),
            None => "# context\n\nNo context file is attached to this selection yet.".to_string(),
        };

        let widget = Paragraph::new(markdown_lines(&text))
            .block(Block::default().borders(Borders::ALL).title(" context "))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_content_editor(&self, frame: &mut Frame, area: Rect) {
        let selected_title = self
            .state
            .selected_content()
            .map(|content| content.title.as_str())
            .unwrap_or("none");

        let selected_path = self
            .state
            .selected_body_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "not attached yet".to_string());

        let text = format!(
            "# Add / Edit Content\n\n\
    Current selection\n\
    - selected content: {selected_title}\n\
    - selected body: {selected_path}\n\n\
    Form draft\n\
    - mode: add new content / edit selected content\n\
    - type: content | panel | section | milestone | note\n\
    - title: <human readable title>\n\
    - slug: <filesystem-safe name>\n\
    - parent: <content/panel/container path>\n\
    - body file: overview.md\n\
    - context file: context.md\n\
    - optional files: notes.md, prompt.md, git.md\n\
    - manifest: .<slug>.cfg\n\n\
    Path preview\n\
    data/contents/<slug>/overview.md\n\
    data/contents/<slug>/context.md\n\
    data/contents/<slug>/.<slug>.cfg\n\n\
    Rules\n\
    - content, panel, and section should stay typed and explicit.\n\
    - generated/runtime/mail state should not be stored in content manifests.\n\
    - this screen is the authoring surface; the main explorer stays only for navigation.\n\n\
    Next implementation step\n\
    Add local input state, then bind tab/shift-tab to fields and ctrl-s to write files.\n\n\
    Press h or Esc to return to the main explorer."
        );

        let widget = Paragraph::new(markdown_lines(&text))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" add / edit content "),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
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

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let help = match self.state.current_view {
            AppView::MainExplorer => {
                "j/k or ↑/↓ move    enter open    esc/backspace back    a add/edit content    b build panel    c cook section    m configure mail    q quit"
            }
            _ => "h or esc return to main explorer    q quit",
        };

        let widget = Paragraph::new(help)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        frame.render_widget(widget, area);
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
        terminal.draw(|frame| app.draw(frame))?;

        if let Event::Key(key) = event::read()? {
            app.handle_key(key);
        }
    }

    Ok(())
}
