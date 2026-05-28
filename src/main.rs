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
    Content,
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
    content_cursor: usize,
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
            content_cursor: 0,
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
        self.content_cursor = 0;
    }

    fn next_panel(&mut self) {
        self.active_panel = (self.active_panel + 1) % self.panels.len();
        self.mode = NavigationMode::Panel;
        self.content_cursor = 0;
    }

    fn previous_panel(&mut self) {
        if self.active_panel == 0 {
            self.active_panel = self.panels.len() - 1;
        } else {
            self.active_panel -= 1;
        }

        self.mode = NavigationMode::Panel;
        self.content_cursor = 0;
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
        match self.mode {
            NavigationMode::Panel => self.mode = NavigationMode::View,
            NavigationMode::View => {
                self.mode = NavigationMode::Content;
                let item_count = self.content_items().len();
                self.content_cursor = if item_count == 0 {
                    0
                } else {
                    self.content_cursor.min(item_count - 1)
                };
            }
            NavigationMode::Content => self.open_selected_content_item(),
        }
    }

    fn escape(&mut self) {
        match self.mode {
            NavigationMode::Content => self.mode = NavigationMode::View,
            NavigationMode::View => self.mode = NavigationMode::Panel,
            NavigationMode::Panel => {
                self.mode = NavigationMode::Panel;
                self.set_active_view(0);
            }
        }
    }

    fn active_text(&self) -> String {
        let panel = self.active_panel();
        let view = self.active_view();

        fs::read_to_string(&view.path).unwrap_or_else(|_| {
            format!(
                "could not read {}\n\npanel: {}\nview: {}\n\nIf this is a generated view, run the relevant script first.",
                view.path.display(),
                panel.id,
                view.id,
            )
        })
    }

    fn content_items(&self) -> Vec<ContentItem> {
        content_items_from_text(&self.active_text())
    }

    fn next_content_item(&mut self) {
        let len = self.content_items().len();
        if len == 0 {
            self.content_cursor = 0;
            return;
        }

        self.content_cursor = (self.content_cursor + 1) % len;
    }

    fn previous_content_item(&mut self) {
        let len = self.content_items().len();
        if len == 0 {
            self.content_cursor = 0;
            return;
        }

        self.content_cursor = if self.content_cursor == 0 {
            len - 1
        } else {
            self.content_cursor - 1
        };
    }

    fn open_selected_content_item(&mut self) {
        let Some(item) = self.content_items().get(self.content_cursor).cloned() else {
            return;
        };

        if let Some(view_index) = self.find_view_index(&item.slug) {
            self.set_active_view(view_index);
            self.mode = NavigationMode::Content;
            return;
        }

        if let Some(path) = detail_path_for(self.active_panel().id.as_str(), item.slug.as_str()) {
            let title = item.label.clone();
            let view = ViewConfig {
                id: item.slug.clone(),
                title,
                path,
            };
            let panel_index = self.active_panel;
            self.panels[panel_index].views.push(view);
            self.set_active_view(self.panels[panel_index].views.len() - 1);
            self.mode = NavigationMode::Content;
        }
    }

    fn find_view_index(&self, slug: &str) -> Option<usize> {
        self.active_panel().views.iter().position(|view| {
            slugify_context(view.id.as_str()) == slug
                || slugify_context(view.title.as_str()) == slug
        })
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Tab => self.next_panel(),
            KeyCode::BackTab => self.previous_panel(),
            KeyCode::Enter => self.enter(),
            KeyCode::Esc => self.escape(),
            KeyCode::Char('j') | KeyCode::Down => match self.mode {
                NavigationMode::View => self.next_view(),
                NavigationMode::Content => self.next_content_item(),
                NavigationMode::Panel => {}
            },
            KeyCode::Char('k') | KeyCode::Up => match self.mode {
                NavigationMode::View => self.previous_view(),
                NavigationMode::Content => self.previous_content_item(),
                NavigationMode::Panel => {}
            },
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

            if panel_index == self.active_panel
                && matches!(self.mode, NavigationMode::View | NavigationMode::Content)
            {
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
            NavigationMode::Content => " content ",
        };

        let widget = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(block_title))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_focus(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let panel = self.active_panel();
        let view = self.active_view();

        let text = self.active_text();

        let title = format!(" {} / {} ", panel.title, view.title);
        let selected_content = if self.mode == NavigationMode::Content {
            Some(self.content_cursor)
        } else {
            None
        };
        let lines = styled_content_lines(&text, selected_content);

        let widget = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn draw_context(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let mode = match self.mode {
            NavigationMode::Panel => "panel mode",
            NavigationMode::View => "view mode",
            NavigationMode::Content => "content mode",
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
                "tab/shift-tab: switch panel    enter: enter views    r: refresh later    q: quit"
            }
            NavigationMode::View => {
                "j/k or ↑/↓: switch view    enter: focus content    esc: panels    tab: switch panel    q: quit"
            }
            NavigationMode::Content => {
                "j/k or ↑/↓: select content    enter: open selected    esc: views    tab: switch panel    q: quit"
            }
        };

        let widget = Paragraph::new(help)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        frame.render_widget(widget, area);
    }
}

#[derive(Clone, Debug)]
struct ContentItem {
    label: String,
    slug: String,
}

fn content_items_from_text(text: &str) -> Vec<ContentItem> {
    text.lines()
        .filter_map(|line| {
            let cleaned = clean_display_line(line);
            let label = content_item_label(cleaned.as_str())?;
            let slug = slugify_context(label.as_str());

            if slug.is_empty() {
                return None;
            }

            Some(ContentItem { label, slug })
        })
        .collect()
}

fn content_item_label(line: &str) -> Option<String> {
    let trimmed = line.trim();

    if trimmed.starts_with("tags:") || trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix("- ") {
        return Some(compact_content_label(rest));
    }

    if let Some(rest) = trimmed.strip_prefix("## ") {
        return Some(compact_content_label(rest));
    }

    None
}

fn compact_content_label(text: &str) -> String {
    let mut value = text.trim().to_string();

    if let Some((_, after_date)) = value.split_once(" · ") {
        value = after_date.trim().to_string();
    }

    if let Some((before_colon, _)) = value.split_once(':') {
        value = before_colon.trim().to_string();
    }

    value = value.trim_matches('*').trim().to_string();
    value = value
        .replace("[KIT-ILIAS]", "")
        .replace("[KIT-Student]", "");
    value.trim().to_string()
}

fn is_content_item_line(line: &str) -> bool {
    content_item_label(line).is_some()
}

fn slugify_context(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}

fn detail_path_for(panel_id: &str, slug: &str) -> Option<PathBuf> {
    let generated = alpnest_data_home()
        .join("generated")
        .join(panel_id)
        .join(format!("{slug}.md"));

    if generated.exists() {
        return Some(generated);
    }

    let fallback = PathBuf::from("data")
        .join(panel_id)
        .join(format!("{slug}.md"));

    if fallback.exists() {
        return Some(fallback);
    }

    None
}

fn styled_content_lines(text: &str, selected_content: Option<usize>) -> Vec<Line<'static>> {
    let mut content_index = 0;

    text.lines()
        .filter_map(|line| {
            let cleaned = clean_display_line(line);

            if cleaned.trim().is_empty() {
                return Some(Line::from(""));
            }

            if cleaned.trim_start().starts_with("tags:") {
                return None;
            }

            let is_item = is_content_item_line(&cleaned);
            if is_item {
                let current_index = content_index;
                content_index += 1;

                if selected_content == Some(current_index) {
                    return Some(Line::from(vec![
                        Span::styled(
                            "> ",
                            Style::default()
                                .fg(Color::LightCyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            cleaned,
                            Style::default()
                                .fg(Color::LightCyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
            }

            Some(styled_display_line(&cleaned))
        })
        .collect()
}

fn clean_display_line(line: &str) -> String {
    line.replace(" (unknown)", "")
        .replace("(unknown)", "")
        .trim_end()
        .to_string()
}

fn styled_display_line(line: &str) -> Line<'static> {
    if let Some(rest) = line.strip_prefix("- ") {
        return styled_bullet_line(rest);
    }

    if let Some(rest) = line.strip_prefix("## ") {
        return styled_heading_line("## ", rest);
    }

    if let Some(rest) = line.strip_prefix("# ") {
        return styled_heading_line("# ", rest);
    }

    Line::from(styled_inline_segments(line, Style::default()))
}

fn styled_bullet_line(rest: &str) -> Line<'static> {
    if let Some((date, remainder)) = rest.split_once(" · ") {
        let mut spans = vec![
            Span::raw("- "),
            Span::styled(date.to_string(), Style::default().fg(Color::Yellow)),
            Span::raw(" · "),
        ];
        spans.extend(styled_inline_segments(remainder, Style::default()));
        return Line::from(spans);
    }

    let mut spans = vec![Span::raw("- ")];
    spans.extend(styled_inline_segments(rest, Style::default()));
    Line::from(spans)
}

fn styled_heading_line(prefix: &str, rest: &str) -> Line<'static> {
    if let Some((date, remainder)) = rest.split_once(" · ") {
        let mut spans = vec![
            Span::styled(
                prefix.to_string(),
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                date.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " · ".to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];
        spans.extend(styled_inline_segments(
            remainder,
            Style::default().add_modifier(Modifier::BOLD),
        ));
        return Line::from(spans);
    }

    let mut spans = vec![Span::styled(
        prefix.to_string(),
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    )];
    spans.extend(styled_inline_segments(
        rest,
        Style::default().add_modifier(Modifier::BOLD),
    ));
    Line::from(spans)
}

fn styled_inline_segments(text: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut index = 0;

    while index < text.len() {
        let remaining = &text[index..];

        if let Some(label) = next_label(remaining) {
            if label.start > 0 {
                spans.extend(styled_bold_segments(&remaining[..label.start], base_style));
            }

            spans.push(Span::styled(
                label.text.to_string(),
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ));
            index += label.end;
            continue;
        }

        spans.extend(styled_bold_segments(remaining, base_style));
        break;
    }

    spans
}

fn styled_bold_segments(text: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut remaining = text;

    loop {
        let Some(start) = remaining.find("**") else {
            if !remaining.is_empty() {
                spans.push(Span::styled(remaining.to_string(), base_style));
            }
            break;
        };

        if start > 0 {
            spans.push(Span::styled(remaining[..start].to_string(), base_style));
        }

        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("**") else {
            spans.push(Span::styled(remaining[start..].to_string(), base_style));
            break;
        };

        spans.push(Span::styled(
            after_start[..end].to_string(),
            base_style.add_modifier(Modifier::BOLD),
        ));
        remaining = &after_start[end + 2..];
    }

    spans
}

struct LabelMatch<'a> {
    start: usize,
    end: usize,
    text: &'a str,
}

fn next_label(text: &str) -> Option<LabelMatch<'_>> {
    let bracket = find_label(text, '[', ']');
    let paren = find_label(text, '(', ')');

    match (bracket, paren) {
        (Some(left), Some(right)) => Some(if left.start <= right.start {
            left
        } else {
            right
        }),
        (Some(label), None) | (None, Some(label)) => Some(label),
        (None, None) => None,
    }
}

fn find_label(text: &str, open: char, close: char) -> Option<LabelMatch<'_>> {
    let start = text.find(open)?;
    let after_open = start + open.len_utf8();
    let relative_end = text[after_open..].find(close)?;
    let end = after_open + relative_end + close.len_utf8();
    let inner = &text[after_open..after_open + relative_end];

    if !is_label_text(inner) {
        return None;
    }

    Some(LabelMatch {
        start,
        end,
        text: &text[start..end],
    })
}

fn is_label_text(text: &str) -> bool {
    let normalized = text.trim().to_ascii_lowercase();

    matches!(
        normalized.as_str(),
        "summarized"
            | "metadata"
            | "body"
            | "review"
            | "action"
            | "deadline"
            | "school"
            | "admin"
            | "meeting"
            | "assignment"
            | "exam"
            | "lab"
            | "seminar"
            | "research"
            | "opportunity"
            | "newsletter"
            | "social"
            | "noise"
    )
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
            views: vec![
                ViewConfig {
                    id: "overview".to_string(),
                    title: "overview".to_string(),
                    path: PathBuf::from("data/projects.md"),
                },
                ViewConfig {
                    id: "alpnest".to_string(),
                    title: "alpnest".to_string(),
                    path: generated_path("projects/alpnest.md", "data/projects/alpnest.md"),
                },
                ViewConfig {
                    id: "hardware-security".to_string(),
                    title: "hardware-security".to_string(),
                    path: generated_path(
                        "projects/hardware-security.md",
                        "data/projects/hardware-security.md",
                    ),
                },
                ViewConfig {
                    id: "iot-lab".to_string(),
                    title: "iot-lab".to_string(),
                    path: generated_path("projects/iot-lab.md", "data/projects/iot-lab.md"),
                },
                ViewConfig {
                    id: "rv32i-mla".to_string(),
                    title: "rv32i-mla".to_string(),
                    path: generated_path("projects/rv32i-mla.md", "data/projects/rv32i-mla.md"),
                },
                ViewConfig {
                    id: "leetcode-solutions".to_string(),
                    title: "leetcode-solutions".to_string(),
                    path: generated_path(
                        "projects/leetcode-solutions.md",
                        "data/projects/leetcode-solutions.md",
                    ),
                },
            ],
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
