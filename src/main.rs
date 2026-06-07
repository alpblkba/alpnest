use std::{env, fs, io, path::PathBuf, process::Command};

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
        let scope = self.view_navigation_scope();
        if scope.len() <= 1 {
            return;
        }

        let current = self.active_view_index();
        let current_pos = scope
            .iter()
            .position(|index| *index == current)
            .unwrap_or(0);
        let next = scope[(current_pos + 1) % scope.len()];
        self.set_active_view(next);
    }

    fn previous_view(&mut self) {
        let scope = self.view_navigation_scope();
        if scope.len() <= 1 {
            return;
        }

        let current = self.active_view_index();
        let current_pos = scope
            .iter()
            .position(|index| *index == current)
            .unwrap_or(0);
        let previous_pos = if current_pos == 0 {
            scope.len() - 1
        } else {
            current_pos - 1
        };
        self.set_active_view(scope[previous_pos]);
    }

    fn enter(&mut self) {
        match self.mode {
            NavigationMode::Panel => self.mode = NavigationMode::View,
            NavigationMode::View => {
                if let Some(child_index) =
                    self.first_child_view_index(self.active_view().id.as_str())
                {
                    self.set_active_view(child_index);
                    self.mode = NavigationMode::View;
                    return;
                }

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
            NavigationMode::View => {
                if let Some(parent_index) = self.active_view_parent_index() {
                    self.set_active_view(parent_index);
                } else {
                    self.mode = NavigationMode::Panel;
                }
            }
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

        if let Some(path) = detail_path_for_context(
            data_home,
            self.active_panel().id.as_str(),
            &self.active_path(),
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

    fn first_child_view_index(&self, parent_id: &str) -> Option<usize> {
        let prefix = format!("{parent_id}-");

        self.active_panel()
            .views
            .iter()
            .position(|view| view.id.starts_with(&prefix))
    }

    fn active_view_parent_index(&self) -> Option<usize> {
        self.view_parent_index(self.active_view_index())
    }

    fn view_parent_index(&self, view_index: usize) -> Option<usize> {
        let view_id = self.active_panel().views.get(view_index)?.id.as_str();

        self.active_panel()
            .views
            .iter()
            .enumerate()
            .filter(|(index, parent)| {
                *index != view_index && view_id.starts_with(format!("{}-", parent.id).as_str())
            })
            .max_by_key(|(_, parent)| parent.id.len())
            .map(|(index, _)| index)
    }

    fn child_view_indices(&self, parent_index: usize) -> Vec<usize> {
        let Some(parent) = self.active_panel().views.get(parent_index) else {
            return Vec::new();
        };

        let prefix = format!("{}-", parent.id);

        self.active_panel()
            .views
            .iter()
            .enumerate()
            .filter_map(|(index, view)| {
                if view.id.starts_with(&prefix) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    fn top_level_view_indices(&self) -> Vec<usize> {
        self.active_panel()
            .views
            .iter()
            .enumerate()
            .filter_map(|(index, _)| {
                if self.view_parent_index(index).is_none() {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    fn view_navigation_scope(&self) -> Vec<usize> {
        if let Some(parent_index) = self.active_view_parent_index() {
            return self.child_view_indices(parent_index);
        }

        self.top_level_view_indices()
    }

    fn should_show_view_in_sidebar(&self, view_index: usize) -> bool {
        let Some(parent_index) = self.view_parent_index(view_index) else {
            return true;
        };

        let active_index = self.active_view_index();
        active_index == parent_index || self.view_parent_index(active_index) == Some(parent_index)
    }

    fn sidebar_view_indent(&self, view_index: usize) -> &'static str {
        if self.view_parent_index(view_index).is_some() {
            "      "
        } else {
            "   "
        }
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

        self.draw_header(frame, root[0]);
        self.draw_body(frame, root[1]);
        self.draw_footer(frame, root[2]);
    }

    fn draw_body(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        self.draw_fixed_body(frame, area);
    }

    fn draw_fixed_body(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Min(50)])
            .split(area);

        let left_stack = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(body[0]);

        self.draw_panels(frame, left_stack[0]);
        self.draw_context(frame, left_stack[1]);
        self.draw_focus(frame, body[1]);
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
                    if !self.should_show_view_in_sidebar(view_index) {
                        continue;
                    }

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
                        Span::raw(self.sidebar_view_indent(view_index)),
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
        let text = self.active_context_text();

        let widget = Paragraph::new(styled_content_lines(&text, None))
            .block(Block::default().borders(Borders::ALL).title(" context "))
            .wrap(Wrap { trim: false });

        frame.render_widget(widget, area);
    }

    fn active_context_text(&self) -> String {
        if let Some(path) = self.active_context_path() {
            if let Ok(text) = fs::read_to_string(&path) {
                if !text.trim().is_empty() {
                    return text;
                }
            }
        }

        self.fallback_context_text()
    }

    fn active_context_path(&self) -> Option<PathBuf> {
        context_path_for(&self.active_path())
    }

    fn fallback_context_text(&self) -> String {
        let mode = match self.mode {
            NavigationMode::Panel => "panel mode",
            NavigationMode::View => "view mode",
            NavigationMode::Content => "content mode",
        };

        format!(
            "# context\n\nmode: {}\n\nNo context file was found for this view yet.\n\nExpected sibling examples:\n- context.md next to project overview files\n- *.context.md next to flat data files\n\nnext layers:\n- project scanner\n- generated git snapshots\n- zellij layout\n- calendar snapshot",
            mode
        )
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

#[cfg(test)]
fn detail_path_for_data_home(data_home: PathBuf, panel_id: &str, slug: &str) -> Option<PathBuf> {
    detail_path_candidates(data_home, panel_id, slug)
        .into_iter()
        .find(|path| path.exists())
}

fn detail_path_for_context(
    data_home: PathBuf,
    panel_id: &str,
    active_path: &std::path::Path,
    slug: &str,
) -> Option<PathBuf> {
    detail_path_candidates_for_context(data_home, panel_id, active_path, slug)
        .into_iter()
        .find(|path| path.exists())
}

#[cfg(test)]
fn detail_path_candidates(data_home: PathBuf, panel_id: &str, slug: &str) -> Vec<PathBuf> {
    detail_path_candidates_for_context(data_home, panel_id, std::path::Path::new(""), slug)
}

fn detail_path_candidates_for_context(
    data_home: PathBuf,
    panel_id: &str,
    active_path: &std::path::Path,
    slug: &str,
) -> Vec<PathBuf> {
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

    let mut candidates = vec![
        generated.join(projection_dir).join(format!("{slug}.md")),
        generated.join(format!("{slug}.md")),
    ];

    if let Some(active_dir) = active_content_dir(active_path) {
        candidates.push(active_dir.join(slug).join("overview.md"));
        candidates.push(active_dir.join(format!("{slug}.md")));
        candidates.push(active_dir.join("milestones").join(format!("{slug}.md")));
    }

    candidates.extend([
        PathBuf::from("data")
            .join(panel_id)
            .join(slug)
            .join("overview.md"),
        PathBuf::from("data")
            .join(panel_id)
            .join(format!("{slug}.md")),
    ]);

    candidates
}

fn active_content_dir(active_path: &std::path::Path) -> Option<PathBuf> {
    if active_path.file_name()? == "overview.md" {
        return active_path.parent().map(|parent| parent.to_path_buf());
    }

    None
}

fn context_path_for(path: &std::path::Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_string_lossy();
    let parent = path.parent()?;

    if file_name == "overview.md" {
        let candidate = parent.join("context.md");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let stem = path.file_stem()?.to_string_lossy();
    let candidate = parent.join(format!("{stem}.context.md"));
    if candidate.exists() {
        return Some(candidate);
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
    let panels = load_panels_from_registry(&PathBuf::from("data").join("panels"));

    if panels.is_empty() {
        fallback_panels()
    } else {
        panels
    }
}

#[derive(Clone, Debug, Default)]
struct SimpleToml {
    id: Option<String>,
    title: Option<String>,
    kind: Option<String>,
    generated: Option<String>,
    path: Option<String>,
    repo: Option<String>,
    order: Option<i64>,
}

fn load_panels_from_registry(panels_root: &std::path::Path) -> Vec<PanelConfig> {
    let mut panel_dirs = read_dirs_sorted(panels_root);
    panel_dirs.sort_by_key(|path| {
        let config = read_simple_toml(&path.join("panel.toml"));
        (
            config.order.unwrap_or_else(|| numeric_prefix_order(path)),
            path.file_name().map(|name| name.to_os_string()),
        )
    });

    panel_dirs
        .into_iter()
        .filter_map(|panel_dir| load_panel_from_dir(&panel_dir))
        .collect()
}

fn load_panel_from_dir(panel_dir: &std::path::Path) -> Option<PanelConfig> {
    let config = read_simple_toml(&panel_dir.join("panel.toml"));
    let fallback_id = strip_numeric_prefix(panel_dir.file_name()?.to_string_lossy().as_ref());
    let id = config.id.clone().unwrap_or_else(|| fallback_id.clone());
    let title = config.title.clone().unwrap_or_else(|| title_from_slug(&id));

    let overview_path = configured_path(&config, panel_dir.join("overview.md"));

    let mut views = vec![ViewConfig {
        id: "overview".to_string(),
        title: "overview".to_string(),
        path: overview_path,
    }];

    let mut child_views = load_child_views_from_dir(&panel_dir.join("views"));
    views.append(&mut child_views);

    Some(PanelConfig { id, title, views })
}

fn load_child_views_from_dir(views_dir: &std::path::Path) -> Vec<ViewConfig> {
    let mut dirs = read_dirs_sorted(views_dir);
    dirs.sort_by_key(|path| {
        let config = read_simple_toml(&path.join("view.toml"));
        (
            config.order.unwrap_or_else(|| numeric_prefix_order(path)),
            path.file_name().map(|name| name.to_os_string()),
        )
    });

    let mut views = Vec::new();

    for view_dir in dirs {
        let Some(view_id) = view_id_from_dir(&view_dir) else {
            continue;
        };

        let config = read_simple_toml(&view_dir.join("view.toml"));
        let title = config
            .title
            .clone()
            .unwrap_or_else(|| title_from_slug(&view_id));
        let overview_path = configured_path(&config, view_dir.join("overview.md"));

        views.push(ViewConfig {
            id: view_id.clone(),
            title,
            path: overview_path.clone(),
        });

        views.push(ViewConfig {
            id: format!("{view_id}-overview"),
            title: "overview".to_string(),
            path: overview_path,
        });

        for (suffix, title, file_name) in [
            ("context", "context", "context.md"),
            ("notes", "notes", "notes.md"),
            ("prompt", "prompt", "prompt.md"),
            ("git", "git", "git.md"),
        ] {
            let path = view_dir.join(file_name);
            if path.exists() {
                views.push(ViewConfig {
                    id: format!("{view_id}-{suffix}"),
                    title: title.to_string(),
                    path,
                });
            }
        }

        let mut milestone_files = read_markdown_files_sorted(&view_dir.join("milestones"));
        milestone_files.sort_by_key(|path| path.file_name().map(|name| name.to_os_string()));

        for milestone in milestone_files {
            let Some(stem) = milestone
                .file_stem()
                .map(|value| value.to_string_lossy().to_string())
            else {
                continue;
            };

            views.push(ViewConfig {
                id: format!("{view_id}-{}", slugify_context(&stem)),
                title: stem,
                path: milestone,
            });
        }
    }

    views
}

fn configured_path(config: &SimpleToml, fallback: PathBuf) -> PathBuf {
    if let Some(generated) = config.generated.as_deref() {
        return generated_path(generated, fallback.to_string_lossy().as_ref());
    }

    if let Some(path) = config.path.as_deref() {
        return expand_home_path(path);
    }

    fallback
}

fn read_dirs_sorted(path: &std::path::Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };

    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();

    dirs.sort();
    dirs
}

fn read_markdown_files_sorted(path: &std::path::Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file() && path.extension().is_some_and(|extension| extension == "md")
        })
        .collect();

    files.sort();
    files
}

fn read_simple_toml(path: &std::path::Path) -> SimpleToml {
    let Ok(text) = fs::read_to_string(path) else {
        return SimpleToml::default();
    };

    let mut config = SimpleToml::default();

    for raw_line in text.lines() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let key = key.trim();
        let value = parse_simple_toml_value(value.trim());

        match key {
            "id" => config.id = Some(value),
            "title" => config.title = Some(value),
            "kind" => config.kind = Some(value),
            "generated" => config.generated = Some(value),
            "path" => config.path = Some(value),
            "repo" => config.repo = Some(value),
            "order" => config.order = value.parse::<i64>().ok(),
            _ => {}
        }
    }

    config
}

fn parse_simple_toml_value(value: &str) -> String {
    let trimmed = value.trim();

    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

fn numeric_prefix_order(path: &std::path::Path) -> i64 {
    let Some(name) = path.file_name().map(|name| name.to_string_lossy()) else {
        return i64::MAX;
    };

    let Some((prefix, _)) = name.split_once('-') else {
        return i64::MAX;
    };

    prefix.parse::<i64>().unwrap_or(i64::MAX)
}

fn strip_numeric_prefix(name: &str) -> String {
    if let Some((prefix, rest)) = name.split_once('-') {
        if prefix.chars().all(|ch| ch.is_ascii_digit()) {
            return rest.to_string();
        }
    }

    name.to_string()
}

fn view_id_from_dir(path: &std::path::Path) -> Option<String> {
    let config = read_simple_toml(&path.join("view.toml"));
    config.id.or_else(|| {
        path.file_name()
            .map(|name| strip_numeric_prefix(name.to_string_lossy().as_ref()))
    })
}

fn title_from_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn expand_home_path(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }

    PathBuf::from(path)
}

fn fallback_panels() -> Vec<PanelConfig> {
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

fn should_launch_session() -> bool {
    env::var_os("ALPNEST_NO_SESSION").is_none()
        && env::var_os("ZELLIJ").is_none()
        && env::var_os("TMUX").is_none()
}

fn session_launcher_path() -> Option<PathBuf> {
    let cwd_launcher = env::current_dir()
        .ok()
        .map(|path| path.join("scripts").join("alpnest-session.sh"));

    if let Some(path) = cwd_launcher {
        if path.exists() {
            return Some(path);
        }
    }

    let home = env::var("HOME").ok()?;
    let repo_launcher = PathBuf::from(home)
        .join("Documents")
        .join("GitHub")
        .join("alpnest")
        .join("scripts")
        .join("alpnest-session.sh");

    if repo_launcher.exists() {
        Some(repo_launcher)
    } else {
        None
    }
}

fn launch_session_if_needed() -> Result<bool> {
    if !should_launch_session() {
        return Ok(false);
    }

    let Some(launcher) = session_launcher_path() else {
        return Ok(false);
    };

    let current_exe = env::current_exe()?;
    let status = Command::new(launcher)
        .env("ALPNEST_COMMAND", current_exe)
        .status()?;

    Ok(status.success())
}

fn main() -> Result<()> {
    color_eyre::install()?;

    if launch_session_if_needed()? {
        return Ok(());
    }

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
        assert!(candidates[2].ends_with("data/mail/mail10/overview.md"));
        assert!(candidates[3].ends_with("data/mail/mail10.md"));

        let kit_candidates = detail_path_candidates(alpnest_data_home(), "mail", "kit10");
        assert!(kit_candidates[0].ends_with("generated/mail/kit/kit10.md"));

        let gmail_candidates = detail_path_candidates(alpnest_data_home(), "mail", "gmail10");
        assert!(gmail_candidates[0].ends_with("generated/mail/gmail/gmail10.md"));
    }

    #[test]
    fn project_detail_candidates_check_nested_overview_before_flat_file() {
        let candidates = detail_path_candidates(alpnest_data_home(), "projects", "alpnest");

        assert!(candidates[0].ends_with("generated/projects/feed/alpnest.md"));
        assert!(candidates[1].ends_with("generated/projects/alpnest.md"));
        assert!(candidates[2].ends_with("data/projects/alpnest/overview.md"));
        assert!(candidates[3].ends_with("data/projects/alpnest.md"));
        assert_eq!(candidates.len(), 4);
    }

    #[test]
    fn context_detail_candidates_prefer_active_course_directory() {
        let candidates = detail_path_candidates_for_context(
            alpnest_data_home(),
            "school",
            std::path::Path::new("data/school/mmai/overview.md"),
            "notes",
        );

        assert!(candidates[2].ends_with("data/school/mmai/notes/overview.md"));
        assert!(candidates[3].ends_with("data/school/mmai/notes.md"));
        assert!(candidates[4].ends_with("data/school/mmai/milestones/notes.md"));
        assert!(candidates[5].ends_with("data/school/notes/overview.md"));
        assert!(candidates[6].ends_with("data/school/notes.md"));
    }

    #[test]
    fn context_path_prefers_project_context_next_to_overview() {
        let data_home = std::env::temp_dir().join(format!(
            "alpnest-context-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let overview_path = data_home
            .join("data")
            .join("projects")
            .join("alpnest")
            .join("overview.md");
        let context_path = data_home
            .join("data")
            .join("projects")
            .join("alpnest")
            .join("context.md");

        std::fs::create_dir_all(overview_path.parent().unwrap()).unwrap();
        std::fs::write(&overview_path, "# alpnest\n").unwrap();
        std::fs::write(&context_path, "# context\n").unwrap();

        assert_eq!(context_path_for(&overview_path), Some(context_path));
    }

    #[test]
    fn default_project_views_use_nested_overview_paths() {
        let projects_panel = default_panels()
            .into_iter()
            .find(|panel| panel.id == "projects")
            .expect("projects panel should exist");

        let alpnest_view = projects_panel
            .views
            .iter()
            .find(|view| view.id == "alpnest")
            .expect("alpnest view should exist");

        assert!(
            alpnest_view
                .path
                .ends_with("data/panels/20-projects/views/10-alpnest/overview.md")
                || alpnest_view.path.ends_with("projects/alpnest/overview.md")
        );
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
    fn school_panel_exposes_mmai_course_subviews() {
        let school_panel = default_panels()
            .into_iter()
            .find(|panel| panel.id == "school")
            .expect("school panel should exist");

        assert!(school_panel.views.iter().any(|view| {
            view.id == "mmai"
                && (view
                    .path
                    .ends_with("data/panels/10-school/views/16-mmai/overview.md")
                    || view.path.ends_with("data/school/mmai/overview.md"))
        }));
        assert!(school_panel.views.iter().any(|view| {
            view.id == "mmai-notes"
                && (view
                    .path
                    .ends_with("data/panels/10-school/views/16-mmai/notes.md")
                    || view.path.ends_with("data/school/mmai/notes.md"))
        }));
        assert!(school_panel.views.iter().any(|view| {
            view.id == "mmai-prompt"
                && (view
                    .path
                    .ends_with("data/panels/10-school/views/16-mmai/prompt.md")
                    || view.path.ends_with("data/school/mmai/prompt.md"))
        }));
        assert!(school_panel.views.iter().any(|view| {
            view.id == "mmai-ms0"
                && (view
                    .path
                    .ends_with("data/panels/10-school/views/16-mmai/milestones/ms0.md")
                    || view.path.ends_with("data/school/mmai/milestones/ms0.md"))
        }));
        assert!(school_panel.views.iter().any(|view| {
            view.id == "mmai-2026-06-07-bridging-day"
                && (view.path.ends_with(
                    "data/panels/10-school/views/16-mmai/milestones/2026-06-07-bridging-day.md",
                ) || view
                    .path
                    .ends_with("data/school/mmai/milestones/2026-06-07-bridging-day.md"))
        }));
    }

    #[test]
    fn school_panel_exposes_mmai_course_group_and_child_views() {
        let school_panel = default_panels()
            .into_iter()
            .find(|panel| panel.id == "school")
            .expect("school panel should exist");

        assert!(school_panel.views.iter().any(|view| {
            view.id == "mmai"
                && (view
                    .path
                    .ends_with("data/panels/10-school/views/16-mmai/overview.md")
                    || view.path.ends_with("data/school/mmai/overview.md"))
        }));
        assert!(school_panel.views.iter().any(|view| {
            view.id == "mmai-overview"
                && (view
                    .path
                    .ends_with("data/panels/10-school/views/16-mmai/overview.md")
                    || view.path.ends_with("data/school/mmai/overview.md"))
        }));
        assert!(school_panel.views.iter().any(|view| {
            view.id == "mmai-context"
                && (view
                    .path
                    .ends_with("data/panels/10-school/views/16-mmai/context.md")
                    || view.path.ends_with("data/school/mmai/context.md"))
        }));
        assert!(school_panel.views.iter().any(|view| {
            view.id == "mmai-notes"
                && (view
                    .path
                    .ends_with("data/panels/10-school/views/16-mmai/notes.md")
                    || view.path.ends_with("data/school/mmai/notes.md"))
        }));
        assert!(school_panel.views.iter().any(|view| {
            view.id == "mmai-prompt"
                && (view
                    .path
                    .ends_with("data/panels/10-school/views/16-mmai/prompt.md")
                    || view.path.ends_with("data/school/mmai/prompt.md"))
        }));
        assert!(school_panel.views.iter().any(|view| {
            view.id == "mmai-ms0"
                && (view
                    .path
                    .ends_with("data/panels/10-school/views/16-mmai/milestones/ms0.md")
                    || view.path.ends_with("data/school/mmai/milestones/ms0.md"))
        }));
        assert!(school_panel.views.iter().any(|view| {
            view.id == "mmai-2026-06-07-bridging-day"
                && (view.path.ends_with(
                    "data/panels/10-school/views/16-mmai/milestones/2026-06-07-bridging-day.md",
                ) || view
                    .path
                    .ends_with("data/school/mmai/milestones/2026-06-07-bridging-day.md"))
        }));
    }

    #[test]
    fn mail_panel_exposes_overview_kit_and_gmail_views() {
        let mail_panel = default_panels()
            .into_iter()
            .find(|panel| panel.id == "mail")
            .expect("mail panel should exist");

        assert!(mail_panel.views.len() >= 3);
        assert!(
            mail_panel
                .views
                .iter()
                .any(|view| view.id == "overview" && view.title == "overview")
        );
        assert!(
            mail_panel
                .views
                .iter()
                .any(|view| view.id == "kit" && view.title == "KIT")
        );
        assert!(
            mail_panel
                .views
                .iter()
                .any(|view| view.id == "gmail" && view.title == "Gmail")
        );
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
