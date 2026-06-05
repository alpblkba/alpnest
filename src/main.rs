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

#[derive(Clone, Debug)]
struct DetailView {
    title: String,
    path: PathBuf,
}

struct App {
    panels: Vec<PanelConfig>,
    active_panel: usize,
    active_views: Vec<usize>,
    active_detail: Option<DetailView>,
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
            active_detail: None,
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
        self.active_detail = None;
        self.content_cursor = 0;
    }

    fn next_panel(&mut self) {
        self.active_panel = (self.active_panel + 1) % self.panels.len();
        self.active_detail = None;
        self.mode = NavigationMode::Panel;
        self.content_cursor = 0;
    }

    fn previous_panel(&mut self) {
        if self.active_panel == 0 {
            self.active_panel = self.panels.len() - 1;
        } else {
            self.active_panel -= 1;
        }

        self.active_detail = None;
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
        if self.active_detail.is_some() {
            self.active_detail = None;
            self.mode = NavigationMode::View;
            self.content_cursor = 0;
            return;
        }

        match self.mode {
            NavigationMode::Content => self.mode = NavigationMode::View,
            NavigationMode::View => self.mode = NavigationMode::Panel,
            NavigationMode::Panel => {
                self.mode = NavigationMode::Panel;
                self.set_active_view(0);
            }
        }
    }

    fn active_path(&self) -> PathBuf {
        self.active_detail
            .as_ref()
            .map(|detail| detail.path.clone())
            .unwrap_or_else(|| self.active_view().path.clone())
    }

    fn active_title(&self) -> String {
        self.active_detail
            .as_ref()
            .map(|detail| detail.title.clone())
            .unwrap_or_else(|| self.active_view().title.clone())
    }

    fn active_text(&self) -> String {
        let panel = self.active_panel();
        let view = self.active_view();
        let path = self.active_path();

        fs::read_to_string(&path).unwrap_or_else(|_| {
            format!(
                "could not read {}\n\npanel: {}\nview: {}\n\nIf this is a generated view, run the relevant script first.",
                path.display(),
                panel.id,
                view.id,
            )
        })
    }

    fn content_items(&self) -> Vec<ContentItem> {
        if self.active_detail.is_some() {
            return Vec::new();
        }

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
        self.open_selected_content_item_with_data_home(alpnest_data_home());
    }

    fn open_selected_content_item_with_data_home(&mut self, data_home: PathBuf) {
        let Some(item) = self.content_items().get(self.content_cursor).cloned() else {
            return;
        };

        if let Some(view_index) = self.find_view_index(&item.slug) {
            self.set_active_view(view_index);
            self.mode = NavigationMode::Content;
            return;
        }

        if let Some(path) = detail_path_for_data_home(
            data_home,
            self.active_panel().id.as_str(),
            item.slug.as_str(),
        ) {
            self.active_detail = Some(DetailView {
                title: item.label.clone(),
                path,
            });
            self.content_cursor = 0;
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
        let title = self.active_title();

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
            Span::styled(title, Style::default().fg(Color::Gray)),
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
                    let view_title = sidebar_view_title(panel.id.as_str(), view);
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
                        Span::styled(view_title, view_style),
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

        let text = self.active_text();

        let title = format!(" {} / {} ", panel.title, self.active_title());
        let selected_content =
            if self.mode == NavigationMode::Content && self.active_detail.is_none() {
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
            let item = content_item_from_line(line)?;

            if item.slug.is_empty() {
                return None;
            }

            Some(item)
        })
        .collect()
}

fn content_item_from_line(line: &str) -> Option<ContentItem> {
    let trimmed = line.trim();

    if trimmed.starts_with("tags:") || trimmed.is_empty() {
        return None;
    }

    if let Some(item) = mail_slot_item_from_line(trimmed) {
        return Some(item);
    }

    let label = content_item_label(trimmed)?;
    let explicit_slug = trailing_mail_slot_label(trimmed);
    let slug = explicit_slug.unwrap_or_else(|| slugify_context(label.as_str()));

    Some(ContentItem { label, slug })
}

fn content_item_label(line: &str) -> Option<String> {
    let trimmed = line.trim();

    if trimmed.starts_with("tags:") || trimmed.is_empty() {
        return None;
    }

    if let Some(rest) = trimmed.strip_prefix("- ") {
        return Some(compact_content_label(rest));
    }

    None
}

fn compact_content_label(text: &str) -> String {
    let without_hidden_slug = strip_trailing_mail_slot_label(text.trim());
    let mut value = strip_mail_slot_prefix(without_hidden_slug);

    if let Some((_, after_date)) = value.split_once(" · ") {
        value = after_date.trim().to_string();
    }

    if let Some((_, after_slot)) = value.split_once(" | ") {
        if is_mail_slot_id(value.split_once(" | ").map(|(slot, _)| slot).unwrap_or("")) {
            value = after_slot.trim().to_string();
        }
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
    content_item_from_line(line).is_some()
}

fn mail_slot_item_from_line(line: &str) -> Option<ContentItem> {
    let rest = line.strip_prefix("- ")?;

    if let Some(slot_id) = trailing_mail_slot_label(line) {
        return Some(ContentItem {
            label: compact_content_label(rest),
            slug: slot_id,
        });
    }

    let (candidate, label) = rest.split_once(" | ")?;
    let candidate = candidate.trim();

    if is_mail_slot_id(candidate) {
        Some(ContentItem {
            label: label.trim().to_string(),
            slug: candidate.to_string(),
        })
    } else {
        None
    }
}

fn strip_mail_slot_prefix(line: &str) -> String {
    let Some(rest) = line.strip_prefix("- ") else {
        return line.to_string();
    };
    let Some((candidate, label)) = rest.split_once(" | ") else {
        return line.to_string();
    };

    if is_mail_slot_id(candidate.trim()) {
        format!("- {}", label.trim())
    } else {
        line.to_string()
    }
}

fn trailing_mail_slot_label(line: &str) -> Option<String> {
    let label = line.trim_end().rsplit_once('[')?.1.strip_suffix(']')?;
    if is_mail_slot_id(label) {
        Some(label.to_string())
    } else {
        None
    }
}

fn strip_trailing_mail_slot_label(line: &str) -> &str {
    let trimmed = line.trim_end();
    let Some((before, label_with_close)) = trimmed.rsplit_once('[') else {
        return trimmed;
    };
    let Some(label) = label_with_close.strip_suffix(']') else {
        return trimmed;
    };

    if is_mail_slot_id(label) {
        before.trim_end()
    } else {
        trimmed
    }
}

fn is_mail_slot_id(value: &str) -> bool {
    let number = value
        .strip_prefix("mail")
        .or_else(|| value.strip_prefix("kit"))
        .or_else(|| value.strip_prefix("gmail"));

    number.is_some_and(|number| !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit()))
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

fn detail_path_for_data_home(data_home: PathBuf, panel_id: &str, slug: &str) -> Option<PathBuf> {
    detail_path_candidates(data_home, panel_id, slug)
        .into_iter()
        .find(|path| path.exists())
}

fn detail_path_candidates(data_home: PathBuf, panel_id: &str, slug: &str) -> Vec<PathBuf> {
    let generated = data_home.join("generated").join(panel_id);
    let projection_dir = if slug.starts_with("mail") {
        "feed"
    } else if slug.starts_with("kit") {
        "kit"
    } else if slug.starts_with("gmail") {
        "gmail"
    } else {
        "feed"
    };

    vec![
        generated.join(projection_dir).join(format!("{slug}.md")),
        generated.join(format!("{slug}.md")),
        PathBuf::from("data")
            .join(panel_id)
            .join(format!("{slug}.md")),
    ]
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
    strip_mail_slot_prefix(strip_trailing_mail_slot_label(line).trim_end())
        .replace(" (unknown)", "")
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
                    path: generated_path("mail_feed.md", "data/mail.md"),
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

fn sidebar_view_title<'a>(panel_id: &str, view: &'a ViewConfig) -> &'a str {
    if panel_id == "mail" {
        match view.id.as_str() {
            "overview" => "overview",
            "kit" => "KIT",
            "gmail" => "Gmail",
            _ => view.title.as_str(),
        }
    } else {
        view.title.as_str()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mail0_slot_slug_and_label() {
        let item = content_item_from_line(
            "- Wednesday, 27. May 2026 at 21:49:48 · START Munich: Registration pending approval [mail0]",
        )
        .expect("mail0 feed line should be selectable");

        assert_eq!(item.slug, "mail0");
        assert_eq!(item.label, "START Munich");
        assert!(!item.label.contains("mail0"));
    }

    #[test]
    fn parses_mail10_as_distinct_from_mail1() {
        let mail1 = content_item_from_line("- mail1 | **Sender**: one").unwrap();
        let mail10 = content_item_from_line("- mail10 | **Sender**: ten").unwrap();
        let kit10 = content_item_from_line("- kit10 | **KIT Sender**: ten").unwrap();
        let gmail19 = content_item_from_line("- gmail19 | **Gmail Sender**: nineteen").unwrap();
        let hidden_mail10 = content_item_from_line("- Friday · Sender: ten [mail10]").unwrap();
        let hidden_gmail19 =
            content_item_from_line("- Friday · Sender: nineteen [gmail19]").unwrap();

        assert_eq!(mail1.slug, "mail1");
        assert_eq!(mail10.slug, "mail10");
        assert_eq!(kit10.slug, "kit10");
        assert_eq!(gmail19.slug, "gmail19");
        assert_eq!(hidden_mail10.slug, "mail10");
        assert_eq!(hidden_gmail19.slug, "gmail19");
        assert_eq!(mail10.label, "**Sender**: ten");
        assert!(!mail10.label.contains("mail10 |"));
    }

    #[test]
    fn hides_trailing_navigation_slot_labels() {
        let line = "- Something readable [mail19]";
        let item = content_item_from_line(line).expect("hidden slot label should be selectable");

        assert_eq!(item.slug, "mail19");
        assert_eq!(clean_display_line(line), "- Something readable");
    }

    #[test]
    fn hides_mail_slot_prefix_from_display() {
        let line = "- mail10 | Wednesday · START Munich: Open Registration";

        assert_eq!(
            clean_display_line(line),
            "- Wednesday · START Munich: Open Registration"
        );

        assert_eq!(
            clean_display_line("- kit10 | Wednesday · KIT: Example"),
            "- Wednesday · KIT: Example"
        );
        assert_eq!(
            clean_display_line("- gmail10 | Wednesday · Gmail: Example"),
            "- Wednesday · Gmail: Example"
        );
        assert_eq!(
            clean_display_line("- Wednesday · Sender: Subject [mail10]"),
            "- Wednesday · Sender: Subject"
        );
    }

    #[test]
    fn mail_detail_candidates_check_feed_directory_first() {
        let candidates = detail_path_candidates(alpnest_data_home(), "mail", "mail10");

        assert!(candidates[0].ends_with("generated/mail/feed/mail10.md"));
        assert!(candidates[1].ends_with("generated/mail/mail10.md"));
        assert!(candidates[2].ends_with("data/mail/mail10.md"));

        let kit_candidates = detail_path_candidates(alpnest_data_home(), "mail", "kit10");
        assert!(kit_candidates[0].ends_with("generated/mail/kit/kit10.md"));

        let gmail_candidates = detail_path_candidates(alpnest_data_home(), "mail", "gmail10");
        assert!(gmail_candidates[0].ends_with("generated/mail/gmail/gmail10.md"));
    }

    #[test]
    fn mail_detail_path_resolves_existing_feed_file() {
        let data_home = std::env::temp_dir().join(format!(
            "alpnest-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let detail_path = data_home
            .join("generated")
            .join("mail")
            .join("feed")
            .join("mail10.md");

        std::fs::create_dir_all(detail_path.parent().unwrap()).unwrap();
        std::fs::write(&detail_path, "# test\n").unwrap();

        assert_eq!(
            detail_path_for_data_home(data_home, "mail", "mail10"),
            Some(detail_path)
        );
    }

    #[test]
    fn mail_panel_exposes_overview_kit_and_gmail_views() {
        let mail_panel = default_panels()
            .into_iter()
            .find(|panel| panel.id == "mail")
            .expect("mail panel should exist");

        assert_eq!(mail_panel.views.len(), 3);
        assert_eq!(mail_panel.views[0].id, "overview");
        assert_eq!(mail_panel.views[0].title, "overview");
        assert_eq!(mail_panel.views[1].id, "kit");
        assert_eq!(mail_panel.views[1].title, "KIT");
        assert_eq!(mail_panel.views[2].id, "gmail");
        assert_eq!(mail_panel.views[2].title, "Gmail");
    }

    #[test]
    fn opening_mail_detail_sets_active_detail_without_mutating_stable_view() {
        let data_home = std::env::temp_dir().join(format!(
            "alpnest-open-detail-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let detail_path = data_home
            .join("generated")
            .join("mail")
            .join("feed")
            .join("mail10.md");
        std::fs::create_dir_all(detail_path.parent().unwrap()).unwrap();
        std::fs::write(&detail_path, "# Subject\n\n## summary\n\nReadable detail.").unwrap();

        let overview_path = data_home.join("mail_feed.md");
        std::fs::write(
            &overview_path,
            "# mail\n\n- mail10 | Friday · Sender: Subject (admin, unread)\n  Summary preview.\n",
        )
        .unwrap();

        let mut app = App::new();
        app.active_panel = app
            .panels
            .iter()
            .position(|panel| panel.id == "mail")
            .expect("mail panel should exist");
        app.mode = NavigationMode::Content;
        let mail_view = &mut app.panels[app.active_panel].views[0];
        mail_view.path = overview_path.clone();
        let stable_title = mail_view.title.clone();
        let stable_path = mail_view.path.clone();

        app.open_selected_content_item_with_data_home(data_home);

        let detail = app
            .active_detail
            .as_ref()
            .expect("opening mail should create transient detail");
        assert_eq!(detail.path, detail_path);
        assert_eq!(app.panels[app.active_panel].views[0].title, stable_title);
        assert_eq!(app.panels[app.active_panel].views[0].path, stable_path);
    }

    #[test]
    fn escape_clears_active_detail_and_returns_to_stable_mail_view() {
        let mut app = App::new();
        app.active_panel = app
            .panels
            .iter()
            .position(|panel| panel.id == "mail")
            .expect("mail panel should exist");
        app.mode = NavigationMode::Content;
        app.active_detail = Some(DetailView {
            title: "A selected message".to_string(),
            path: PathBuf::from("generated/mail/feed/mail10.md"),
        });

        app.escape();

        assert!(app.active_detail.is_none());
        assert_eq!(app.mode, NavigationMode::View);
        assert_eq!(app.active_title(), "overview");
        assert_eq!(app.active_view().title, "overview");
    }

    #[test]
    fn opened_mail_detail_headings_are_not_selectable_content() {
        let detail_path = std::env::temp_dir().join(format!(
            "alpnest-mail-detail-test-{}.md",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &detail_path,
            "# Subject\n\n## summary\n\nShort version.\n\n## message 1\n\nFull body.",
        )
        .unwrap();

        let mut app = App::new();
        app.active_panel = app
            .panels
            .iter()
            .position(|panel| panel.id == "mail")
            .expect("mail panel should exist");
        app.active_detail = Some(DetailView {
            title: "A selected message".to_string(),
            path: detail_path,
        });

        assert!(app.content_items().is_empty());
        assert!(content_item_from_line("## summary").is_none());
        assert!(content_item_from_line("## message").is_none());
    }
}
