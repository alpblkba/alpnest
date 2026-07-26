//! Alpnest theming.
//!
//! One `Theme` describes every colour Alpnest paints. Views never hardcode a
//! `Color`; they ask the theme. That keeps the cosmetics swappable from
//! settings and keeps the mail view free to run its own palette.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub name: &'static str,
    pub label: &'static str,

    pub fg: Color,
    pub dim: Color,
    pub faint: Color,

    pub accent: Color,
    pub accent_alt: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,

    pub border: Color,
    pub border_focused: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,

    pub depth_content: Color,
    pub depth_panel: Color,
    pub depth_section: Color,

    pub rounded: bool,
}

/// btop-flavoured default: saturated accents on a deep neutral base.
pub const NEST: Theme = Theme {
    name: "nest",
    label: "nest (default)",
    fg: Color::Rgb(220, 223, 228),
    dim: Color::Rgb(138, 145, 158),
    faint: Color::Rgb(88, 94, 106),
    accent: Color::Rgb(233, 105, 168),
    accent_alt: Color::Rgb(97, 214, 214),
    success: Color::Rgb(126, 209, 131),
    warning: Color::Rgb(232, 183, 96),
    danger: Color::Rgb(233, 106, 106),
    border: Color::Rgb(72, 78, 92),
    border_focused: Color::Rgb(233, 105, 168),
    selection_bg: Color::Rgb(46, 52, 68),
    selection_fg: Color::Rgb(240, 243, 248),
    depth_content: Color::Rgb(226, 229, 236),
    depth_panel: Color::Rgb(97, 214, 214),
    depth_section: Color::Rgb(158, 165, 178),
    rounded: true,
};

/// euporie-flavoured: soft, low-chroma, generous use of dim text.
pub const EUPORIE: Theme = Theme {
    name: "euporie",
    label: "euporie (soft)",
    fg: Color::Rgb(212, 208, 200),
    dim: Color::Rgb(146, 142, 134),
    faint: Color::Rgb(98, 95, 90),
    accent: Color::Rgb(174, 154, 214),
    accent_alt: Color::Rgb(132, 178, 196),
    success: Color::Rgb(150, 186, 148),
    warning: Color::Rgb(206, 178, 122),
    danger: Color::Rgb(198, 124, 124),
    border: Color::Rgb(84, 82, 78),
    border_focused: Color::Rgb(174, 154, 214),
    selection_bg: Color::Rgb(56, 54, 62),
    selection_fg: Color::Rgb(232, 228, 220),
    depth_content: Color::Rgb(216, 212, 204),
    depth_panel: Color::Rgb(132, 178, 196),
    depth_section: Color::Rgb(154, 150, 142),
    rounded: true,
};

/// High-contrast btop look for bright terminals and screenshots.
pub const BTOP: Theme = Theme {
    name: "btop",
    label: "btop (vivid)",
    fg: Color::Rgb(236, 239, 244),
    dim: Color::Rgb(150, 158, 172),
    faint: Color::Rgb(94, 102, 116),
    accent: Color::Rgb(120, 190, 255),
    accent_alt: Color::Rgb(88, 240, 200),
    success: Color::Rgb(120, 235, 140),
    warning: Color::Rgb(255, 198, 88),
    danger: Color::Rgb(255, 110, 110),
    border: Color::Rgb(62, 72, 90),
    border_focused: Color::Rgb(120, 190, 255),
    selection_bg: Color::Rgb(30, 52, 78),
    selection_fg: Color::Rgb(240, 248, 255),
    depth_content: Color::Rgb(236, 239, 244),
    depth_panel: Color::Rgb(88, 240, 200),
    depth_section: Color::Rgb(160, 168, 182),
    rounded: false,
};

/// No-colour fallback for plain terminals and `TERM=dumb` sessions.
pub const MONO: Theme = Theme {
    name: "mono",
    label: "mono (no colour)",
    fg: Color::White,
    dim: Color::Gray,
    faint: Color::DarkGray,
    accent: Color::White,
    accent_alt: Color::Gray,
    success: Color::White,
    warning: Color::Gray,
    danger: Color::White,
    border: Color::DarkGray,
    border_focused: Color::White,
    selection_bg: Color::DarkGray,
    selection_fg: Color::White,
    depth_content: Color::White,
    depth_panel: Color::Gray,
    depth_section: Color::DarkGray,
    rounded: false,
};

/// Mail runs its own palette on purpose: configuring an account is a
/// higher-stakes surface than browsing markdown, and it should not look
/// like the rest of the app.
pub const MAIL: Theme = Theme {
    name: "mail",
    label: "mail (signal)",
    fg: Color::Rgb(232, 226, 214),
    dim: Color::Rgb(158, 148, 130),
    faint: Color::Rgb(104, 96, 84),
    accent: Color::Rgb(240, 176, 84),
    accent_alt: Color::Rgb(196, 154, 232),
    success: Color::Rgb(140, 206, 140),
    warning: Color::Rgb(240, 200, 110),
    danger: Color::Rgb(232, 116, 104),
    border: Color::Rgb(92, 80, 62),
    border_focused: Color::Rgb(240, 176, 84),
    selection_bg: Color::Rgb(64, 52, 34),
    selection_fg: Color::Rgb(255, 244, 226),
    depth_content: Color::Rgb(232, 226, 214),
    depth_panel: Color::Rgb(240, 176, 84),
    depth_section: Color::Rgb(158, 148, 130),
    rounded: true,
};

pub const ALL: [Theme; 4] = [NEST, EUPORIE, BTOP, MONO];

impl Default for Theme {
    fn default() -> Self {
        NEST
    }
}

impl Theme {
    pub fn from_name(value: &str) -> Self {
        let wanted = value.trim().to_ascii_lowercase();

        ALL.into_iter()
            .find(|theme| theme.name == wanted)
            .unwrap_or(NEST)
    }

    pub fn next(self) -> Self {
        let current = ALL
            .iter()
            .position(|theme| theme.name == self.name)
            .unwrap_or(0);

        ALL[(current + 1) % ALL.len()]
    }

    fn border_type(self) -> BorderType {
        if self.rounded {
            BorderType::Rounded
        } else {
            BorderType::Plain
        }
    }

    /// A bordered block whose title and frame brighten when focused.
    pub fn block(self, title: &str, focused: bool) -> Block<'static> {
        let (border_color, title_style) = if focused {
            (
                self.border_focused,
                Style::default()
                    .fg(self.border_focused)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (self.border, Style::default().fg(self.dim))
        };

        Block::default()
            .borders(Borders::ALL)
            .border_type(self.border_type())
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(format!(" {title} "), title_style))
    }

    pub fn plain_block(self) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .border_type(self.border_type())
            .border_style(Style::default().fg(self.border))
    }

    pub fn text(self) -> Style {
        Style::default().fg(self.fg)
    }

    pub fn dim_text(self) -> Style {
        Style::default().fg(self.dim)
    }

    pub fn faint_text(self) -> Style {
        Style::default().fg(self.faint)
    }

    pub fn heading(self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn subheading(self) -> Style {
        Style::default()
            .fg(self.accent_alt)
            .add_modifier(Modifier::BOLD)
    }

    /// Selected rows get a filled background rather than only a caret, which
    /// is what makes euporie and btop lists readable at a glance.
    pub fn selected_row(self) -> Style {
        Style::default()
            .fg(self.selection_fg)
            .bg(self.selection_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn row(self, selected: bool) -> Style {
        if selected {
            self.selected_row()
        } else {
            self.text()
        }
    }

    /// A list row rendered the way every Alpnest list renders one.
    pub fn list_row(self, label: impl Into<String>, selected: bool) -> Line<'static> {
        let style = self.row(selected);
        let marker = if selected { "▎ " } else { "  " };

        Line::from(vec![
            Span::styled(
                marker,
                if selected {
                    Style::default().fg(self.accent).bg(self.selection_bg)
                } else {
                    Style::default().fg(self.faint)
                },
            ),
            Span::styled(label.into(), style),
        ])
    }

    /// Footer hints as `key`+`label` chips instead of one long grey sentence.
    pub fn key_hints(self, hints: &[(&str, &str)]) -> Line<'static> {
        let mut spans = Vec::new();

        for (index, (key, label)) in hints.iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled("  ·  ", self.faint_text()));
            }

            spans.push(Span::styled(
                (*key).to_string(),
                Style::default()
                    .fg(self.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled((*label).to_string(), self.dim_text()));
        }

        Line::from(spans)
    }

    /// `[x]` / `[ ]` toggles, coloured so state reads without the label.
    pub fn checkbox(self, label: &str, checked: bool, selected: bool) -> Line<'static> {
        let mark_style = if checked {
            Style::default()
                .fg(self.success)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.faint)
        };

        let mut spans = vec![
            Span::styled(
                if selected { "▎ " } else { "  " },
                if selected {
                    Style::default().fg(self.accent)
                } else {
                    Style::default().fg(self.faint)
                },
            ),
            Span::styled(if checked { "[x]" } else { "[ ]" }, mark_style),
            Span::raw(" "),
            Span::styled(label.to_string(), self.row(selected)),
        ];

        if selected {
            spans = spans
                .into_iter()
                .map(|span| {
                    let style = span.style.bg(self.selection_bg);
                    Span::styled(span.content, style)
                })
                .collect();
        }

        Line::from(spans)
    }

    /// `label: value` where the value carries the emphasis.
    pub fn field_row(self, label: &str, value: &str, selected: bool) -> Line<'static> {
        let base = if selected {
            self.selected_row()
        } else {
            self.dim_text()
        };

        let value_style = if selected {
            Style::default()
                .fg(self.selection_fg)
                .bg(self.selection_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.fg)
        };

        Line::from(vec![
            Span::styled(
                if selected { "▎ " } else { "  " },
                if selected {
                    Style::default().fg(self.accent).bg(self.selection_bg)
                } else {
                    Style::default().fg(self.faint)
                },
            ),
            Span::styled(format!("{label}: "), base),
            Span::styled(value.to_string(), value_style),
        ])
    }

    /// A small inline status pill, used for connection and credential state.
    pub fn badge(self, text: &str, level: BadgeLevel) -> Span<'static> {
        let color = match level {
            BadgeLevel::Ok => self.success,
            BadgeLevel::Warn => self.warning,
            BadgeLevel::Bad => self.danger,
            BadgeLevel::Idle => self.faint,
        };

        Span::styled(
            format!(" {text} "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeLevel {
    Ok,
    Warn,
    Bad,
    Idle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_falls_back_to_nest() {
        assert_eq!(Theme::from_name("does-not-exist").name, "nest");
        assert_eq!(Theme::from_name("euporie").name, "euporie");
        assert_eq!(Theme::from_name("  BTOP ").name, "btop");
    }

    #[test]
    fn next_cycles_through_every_theme() {
        let mut theme = NEST;

        for _ in 0..ALL.len() {
            theme = theme.next();
        }

        assert_eq!(theme.name, NEST.name);
    }

    #[test]
    fn mail_theme_is_not_in_the_user_cycle() {
        assert!(!ALL.iter().any(|theme| theme.name == MAIL.name));
    }
}
