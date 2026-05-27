use std::io;
use std::fs;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap},
    DefaultTerminal, Frame,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Today,
    School,
    Projects,
    Mail,
}

impl Panel {
    fn all() -> &'static [Panel] {
        &[
            Panel::Today,
            Panel::School,
            Panel::Projects,
            Panel::Mail,
        ]
    }

    fn title(self) -> &'static str {
        match self {
            Panel::Today => "today",
            Panel::School => "school",
            Panel::Projects => "projects",
            Panel::Mail => "mail",
        }
    }
}

#[derive(Debug)]
struct App {
    active_panel: Panel,
    should_quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            active_panel: Panel::Today,
            should_quit: false,
        }
    }
}

impl App {
    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();

        let root = Block::default()
            .title(" alpnest ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(240, 160, 192)));

        frame.render_widget(root, area);

        let inner = area.inner(Margin {
            vertical: 1,
            horizontal: 2,
        });

        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(8),
                Constraint::Length(3),
            ])
            .split(inner);

        let header = Paragraph::new(Line::from(vec![
            Span::styled("terminal nest", Style::default().fg(Color::Rgb(255, 176, 192)).bold()),
            Span::raw("  —  "),
            Span::styled(
                "school · projects · mail · context",
                Style::default().fg(Color::Rgb(176, 145, 192)),
            ),
        ]))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::Rgb(80, 70, 100))),
        );

        frame.render_widget(header, vertical[0]);

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(22),
                Constraint::Percentage(48),
                Constraint::Percentage(32),
            ])
            .split(vertical[1]);

        self.draw_panels(frame, body[0]);
        self.draw_focus(frame, body[1]);
        self.draw_context(frame, body[2]);

        let footer = Paragraph::new(" tab/shift-tab: switch panel   r: refresh later   q: quit ")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Rgb(240, 192, 208)))
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Color::Rgb(80, 70, 100))),
            );

        frame.render_widget(footer, vertical[2]);
    }

    fn draw_panels(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let items: Vec<ListItem> = Panel::all()
            .iter()
            .map(|panel| {
                let label = if *panel == self.active_panel {
                    format!("> {}", panel.title())
                } else {
                    format!("  {}", panel.title())
                };

                let style = if *panel == self.active_panel {
                    Style::default()
                        .fg(Color::Rgb(255, 144, 240))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Rgb(240, 192, 208))
                };

                ListItem::new(label).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .title(" panels ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Rgb(176, 145, 192))),
        );

        frame.render_widget(list, area);
    }

    fn draw_focus(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let path = match self.active_panel {
            Panel::Today => "data/today.md",
            Panel::School => "data/school.md",
            Panel::Projects => "data/projects.md",
            Panel::Mail => "data/mail.md",
        };

        let text = fs::read_to_string(path)
        .unwrap_or_else(|_| format!("could not read {}", path));

        let paragraph = Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::Rgb(240, 192, 208)))
            .block(
                Block::default()
                    .title(format!(" {} ", self.active_panel.title()))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Rgb(255, 176, 192))),
            );

        frame.render_widget(paragraph, area);
    }

    fn draw_context(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let paragraph = Paragraph::new(
            "context\n\n\
             alpnest is not a code editor.\n\n\
             it is the home screen before the work starts.\n\n\
             next layers:\n\
             - real TODO files\n\
             - calendar snapshot\n\
             - project scanner\n\
             - mail summary\n\
             - zellij layout",
        )
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Rgb(176, 145, 192)))
        .block(
            Block::default()
                .title(" context ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Rgb(176, 145, 192))),
        );

        frame.render_widget(paragraph, area);
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                    KeyCode::Tab => self.next_panel(),
                    KeyCode::BackTab => self.previous_panel(),
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn next_panel(&mut self) {
        let panels = Panel::all();
        let current = panels
            .iter()
            .position(|panel| *panel == self.active_panel)
            .unwrap_or(0);
        self.active_panel = panels[(current + 1) % panels.len()];
    }

    fn previous_panel(&mut self) {
        let panels = Panel::all();
        let current = panels
            .iter()
            .position(|panel| *panel == self.active_panel)
            .unwrap_or(0);
        self.active_panel = panels[(current + panels.len() - 1) % panels.len()];
    }
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let mut terminal = ratatui::init();
    let result = App::default().run(&mut terminal);
    ratatui::restore();

    result?;

    Ok(())
}
