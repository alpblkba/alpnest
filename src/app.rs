use std::io;

use crate::app_view::AppView;
use crate::content::{Content, ContentRegistry, Panel, Section};

#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub content_index: usize,
    pub panel_index: Option<usize>,
    pub section_index: Option<usize>,
}

#[derive(Debug)]
pub struct AppState {
    pub current_view: AppView,
    pub registry: ContentRegistry,
    pub selection: Selection,
}

impl AppState {
    pub fn load() -> io::Result<Self> {
        let registry = ContentRegistry::load_default()?;

        Ok(Self {
            current_view: AppView::default(),
            registry,
            selection: Selection::default(),
        })
    }

    pub fn selected_content(&self) -> Option<&Content> {
        self.registry.content(self.selection.content_index)
    }

    pub fn selected_panel(&self) -> Option<&Panel> {
        let content = self.selected_content()?;
        let index = self.selection.panel_index?;
        content.panels.get(index)
    }

    pub fn selected_section(&self) -> Option<&Section> {
        let panel = self.selected_panel()?;
        let index = self.selection.section_index?;
        panel.sections.get(index)
    }

    pub fn selected_body_path(&self) -> Option<&std::path::Path> {
        if let Some(section) = self.selected_section() {
            return Some(section.body_path.as_path());
        }

        self.selected_content()
            .and_then(|content| content.body_path.as_deref())
    }

    pub fn selected_context_path(&self) -> Option<&std::path::Path> {
        if let Some(section) = self.selected_section() {
            if let Some(path) = section.context_path.as_deref() {
                return Some(path);
            }
        }

        self.selected_content()
            .and_then(|content| content.context_path.as_deref())
    }

    pub fn move_next_content(&mut self) {
        if self.registry.contents.is_empty() {
            return;
        }

        self.selection.content_index =
            (self.selection.content_index + 1).min(self.registry.contents.len() - 1);
        self.selection.panel_index = None;
        self.selection.section_index = None;
    }

    pub fn move_prev_content(&mut self) {
        if self.selection.content_index > 0 {
            self.selection.content_index -= 1;
        }

        self.selection.panel_index = None;
        self.selection.section_index = None;
    }

    pub fn enter(&mut self) {
        match (
            self.selected_content(),
            self.selection.panel_index,
            self.selection.section_index,
        ) {
            (Some(content), None, None) if !content.panels.is_empty() => {
                self.selection.panel_index = Some(0);
                self.selection.section_index = None;
            }
            (Some(content), Some(panel_index), None) => {
                if let Some(panel) = content.panels.get(panel_index) {
                    if !panel.sections.is_empty() {
                        self.selection.section_index = Some(0);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn back(&mut self) {
        if self.selection.section_index.is_some() {
            self.selection.section_index = None;
        } else if self.selection.panel_index.is_some() {
            self.selection.panel_index = None;
        }
    }

    pub fn switch_view(&mut self, view: AppView) {
        self.current_view = view;
    }
}
