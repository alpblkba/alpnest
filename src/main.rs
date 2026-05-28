use std::{env, fs, io, path::PathBuf};

use color_eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NavigationMode {
    Panel,
    View,
}

#[derive(Clone, Debug)]
struct ViewConfig {
    id: String,
    title: String,
    path: PathBuf,
}

#[derive(Clone, Debug)]
struct PanelConfig {
    id: String,
    title: String,
    views: Vec<ViewConfig>,
}

struct App {
    panels: Vec<PanelConfig>,
    active_panel: usize,
    active_views: Vec<usize>,
    mode: NavigationMode,
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        let panels = default_panels();
        let active_views = vec![0; panels.len()];

        Self {
            panels,
            active_panel: 0,
            active_views,
            mode: NavigationMode::Panel,
            should_quit: false,
        }
    }

    fn active_panel(&self) -> &PanelConfig {
        &self.panels[self.active_panel]
    }

    fn active_view_index(&self) -> usize {
        self.active_views
            .get(self.active_panel)
            .copied()
            .unwrap_or(0)
            .min(self.active_panel().views.len().saturating_sub(1))
    }

    fn active_view(&self) -> &ViewConfig {
        &self.active_panel().views[self.active_view_index()]
    }

    fn set_active_view(&mut self, index: usize) {
        if self.active_panel().views.is_empty() {
            return;
        }

        let max = self.active_panel().views.len() - 1;
        self.active_views[self.active_panel] = index.min(max);
    }

    fn next_panel(&mut self) {
        self.active_panel = (self.active_panel + 1) % self.panels.len();
        self.mode = NavigationMode::Panel;
    }

    fn previous_panel(&mut self) {
        if self.active_panel == 0 {
            self.active_panel = self.panels.len() - 1;
        } else {
            self.active_panel -= 1;
        }

        self.mode = NavigationMode::Panel;
    }

    fn next_view(&mut self) {
        let len = self.active_panel().views.len();
        if len <= 1 {
            return;
        }

        let next = (self.active_view_index() + 1) % len;
        self.set_active_view(next);
    }

    fn previous_view(&mut self) {
        let len = self.active_panel().views.len();
        if len <= 1 {
            return;
        }

        let current = self.active_view_index();
        let previous = if current == 0 { len - 1 } else { current - 1 };
        self.set_active_view(previous);
    }

    fn enter(&mut self) {
        self.mode = NavigationMode::View;
    }

    fn escape(&mut self) {
        self.mode = NavigationMode::Panel;
        self.set_active_view(0);
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Tab => self.next_panel(),
            KeyCode::BackTab => self.previous_panel(),
            KeyCode::Enter => self.enter(),
            KeyCode::Esc => self.escape(),
            KeyCode::Char('j') | KeyCode::Down => {
                if self.mode == NavigationMode::View {
                    self.next_view();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.mode == NavigationMode::View {
                    self.previous_view();
                }
            }
            KeyCode::Char('r') => {}
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            _ => {}
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(frame.area());

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(24),
                Constraint::Min(40),
                Constraint::Length(36),
            ])
            .split(root[1]);

        self.draw_header(frame, root[0]);
        self.draw_panels(frame, body[0]);
        self.draw_focus(frame, body[1]);
        self.draw_context(frame, body[2]);
        self.draw_footer(frame, root[2]);
    }

    fn draw_header(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let panel = self.active_panel();
        let view = self.active_view();

        let title = Line::from(vec![
            Span::styled(
                "terminal nest",
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  —  "),
            Span::styled(
                panel.title.as_str(),
                Style::default().fg(Color::LightMagenta),
            ),
            Span::raw(" / "),
            Span::styled(view.title.as_str(), Style::default().fg(Color::Gray)),
        ]);

        let header = Paragraph::new(title)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(" alpnest "));

        frame.render_widget(header, area);
    }

    fn draw_panels(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let mut lines = Vec::new();

        for (panel_index, panel) in self.panels.iter().enumerate() {
            let marker = if panel_index == self.active_panel {
                ">"
            } else {
                " "
            };
            let style = if panel_index == self.active_panel {
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::raw(" "),
                Span::styled(panel.title.as_str(), style),
            ]));

            if panel_index == self.active_panel && self.mode == NavigationMode::View {
                for (view_index, view) in panel.views.iter().enumerate() {
                    let view_marker = if view_index == self.active_view_index() {
                        ">"
                    } else {
                        " "
                    };
                    let view_style = if view_index == self.active_view_index() {
                        Style::default()
                            .fg(Color::LightCyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    };

                    lines.push(Line::from(vec![
                        Span::raw("   "),
                        Span::styled(view_marker, view_style),
                        Span::raw(" "),
                        Span::styled(view.title.as_str(), view_style),
                    ]));
                }
            }
        }

        let block_title = match self.mode {
            NavigationMode::Panel => " panels ",
            NavigationMode::View => " views ",
        };

        let widget = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(block_title))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_focus(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let panel = self.active_panel();
        let view = self.active_view();

        let text = fs::read_to_string(&view.path).unwrap_or_else(|_| {
            format!(
                "could not read {}\n\npanel: {}\nview: {}\n\nIf this is a generated view, run the relevant script first.",
                view.path.display(),
                panel.id,
                view.id,
            )
        });

        let title = format!(" {} / {} ", panel.title, view.title);

        let widget = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_context(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let mode = match self.mode {
            NavigationMode::Panel => "panel mode",
            NavigationMode::View => "view mode",
        };

        let text = format!(
            "context\n\nmode: {}\n\nalpnest is not a code editor.\n\nit is the home screen before the work starts.\n\nnext layers:\n- real TODO files\n- calendar snapshot\n- project scanner\n- mail summary\n- zellij layout",
            mode
        );

        let widget = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(" context "))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_footer(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let help = match self.mode {
            NavigationMode::Panel => {
                "tab/shift-tab: switch panel    enter: enter panel    r: refresh later    q: quit"
            }
            NavigationMode::View => {
                "j/k or ↑/↓: switch view    enter: open view    esc: back    tab: switch panel    q: quit"
            }
        };

        let widget = Paragraph::new(help)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        frame.render_widget(widget, area);
    }
}

fn default_panels() -> Vec<PanelConfig> {
    vec![
        PanelConfig {
            id: "today".to_string(),
            title: "today".to_string(),
            views: vec![ViewConfig {
                id: "overview".to_string(),
                title: "overview".to_string(),
                path: PathBuf::from("data/today.md"),
            }],
        },
        PanelConfig {
            id: "school".to_string(),
            title: "school".to_string(),
            views: vec![ViewConfig {
                id: "overview".to_string(),
                title: "overview".to_string(),
                path: PathBuf::from("data/school.md"),
            }],
        },
        PanelConfig {
            id: "projects".to_string(),
            title: "projects".to_string(),
            views: vec![ViewConfig {
                id: "overview".to_string(),
                title: "overview".to_string(),
                path: PathBuf::from("data/projects.md"),
            }],
        },
        PanelConfig {
            id: "mail".to_string(),
            title: "mail".to_string(),
            views: vec![
                ViewConfig {
                    id: "overview".to_string(),
                    title: "overview".to_string(),
                    path: generated_path("mail.md", "data/mail.md"),
                },
                ViewConfig {
                    id: "kit".to_string(),
                    title: "KIT".to_string(),
                    path: generated_path("mail_kit.md", "data/mail.md"),
                },
                ViewConfig {
                    id: "gmail".to_string(),
                    title: "Gmail".to_string(),
                    path: generated_path("mail_gmail.md", "data/mail.md"),
                },
            ],
        },
    ]
}

fn generated_path(name: &str, fallback: &str) -> PathBuf {
    let path = alpnest_data_home().join("generated").join(name);

    if path.exists() {
        path
    } else {
        PathBuf::from(fallback)
    }
}

fn alpnest_data_home() -> PathBuf {
    if let Ok(value) = env::var("ALPNEST_DATA_HOME") {
        return PathBuf::from(value);
    }

    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share/alpnest")
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();

    while !app.should_quit {
        terminal.draw(|frame| app.draw(frame))?;

        if event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    color_eyre::install()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
