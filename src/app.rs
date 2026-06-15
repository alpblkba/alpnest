use std::io;

use crate::app_view::AppView;
use crate::content::{Content, ContentRegistry, Panel, Section};
use crate::content_editor::ContentEditorState;
use crate::panel_wizard::PanelWizardState;
use crate::settings::AlpnestSettings;

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
    pub content_editor: ContentEditorState,
    pub settings: AlpnestSettings,
    pub panel_wizard: PanelWizardState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionTarget {
    Content(usize),
    Panel(usize, usize),
    Section(usize, usize, usize),
}

impl AppState {
    pub fn load() -> io::Result<Self> {
        let registry = ContentRegistry::load_default()?;
        let content_editor = ContentEditorState::with_existing_contents(content_titles(&registry));
        let settings = AlpnestSettings::load()?;
        let panel_wizard = registry
            .content(0)
            .map(PanelWizardState::from_content)
            .unwrap_or_default();

        Ok(Self {
            current_view: AppView::default(),
            registry,
            selection: Selection::default(),
            content_editor,
            settings,
            panel_wizard,
        })
    }

    pub fn reload(&mut self) -> io::Result<()> {
        let registry = ContentRegistry::load_default()?;
        self.content_editor.existing_content_titles = content_titles(&registry);
        self.registry = registry;
        self.clamp_selection();
        Ok(())
    }

    pub fn open_content_editor(&mut self) {
        self.content_editor.existing_content_titles = content_titles(&self.registry);
        self.current_view = AppView::ContentEditor;
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

        if let Some(panel) = self.selected_panel() {
            if let Some(section) = panel.sections.first() {
                if let Some(path) = section.context_path.as_deref() {
                    return Some(path);
                }
            }
        }

        self.selected_content()
            .and_then(|content| content.context_path.as_deref())
    }

    pub fn move_next_row(&mut self) {
        let targets = self.visible_targets();
        if targets.is_empty() {
            self.selection = Selection::default();
            return;
        }

        let current = self.current_target();
        let current_index = targets
            .iter()
            .position(|target| *target == current)
            .unwrap_or(0);

        let next_index = (current_index + 1).min(targets.len() - 1);
        self.apply_target(targets[next_index]);
    }

    pub fn move_prev_row(&mut self) {
        let targets = self.visible_targets();
        if targets.is_empty() {
            self.selection = Selection::default();
            return;
        }

        let current = self.current_target();
        let current_index = targets
            .iter()
            .position(|target| *target == current)
            .unwrap_or(0);

        let next_index = current_index.saturating_sub(1);
        self.apply_target(targets[next_index]);
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
        if self.current_view != AppView::MainExplorer {
            self.current_view = AppView::MainExplorer;
            return;
        }

        if self.selection.section_index.is_some() {
            self.selection.section_index = None;
        } else if self.selection.panel_index.is_some() {
            self.selection.panel_index = None;
        }
    }

    pub fn switch_view(&mut self, view: AppView) {
        self.current_view = view;
    }

    pub fn open_settings(&mut self) {
        self.current_view = AppView::Settings;
    }

    pub fn open_panel_wizard(&mut self) {
        if let Some(content) = self.selected_content() {
            self.panel_wizard = PanelWizardState::from_content(content);
        }
        self.current_view = AppView::BuildPanel;
    }

    fn clamp_selection(&mut self) {
        if self.registry.contents.is_empty() {
            self.selection = Selection::default();
            return;
        }

        self.selection.content_index = self
            .selection
            .content_index
            .min(self.registry.contents.len() - 1);

        let panel_len = self
            .selected_content()
            .map(|content| content.panels.len())
            .unwrap_or(0);

        if panel_len == 0 {
            self.selection.panel_index = None;
            self.selection.section_index = None;
            return;
        }

        let Some(panel_index) = self.selection.panel_index else {
            self.selection.section_index = None;
            return;
        };

        let panel_index = panel_index.min(panel_len - 1);
        self.selection.panel_index = Some(panel_index);

        let section_len = self
            .selected_content()
            .and_then(|content| content.panels.get(panel_index))
            .map(|panel| panel.sections.len())
            .unwrap_or(0);

        if section_len == 0 {
            self.selection.section_index = None;
            return;
        }

        if let Some(section_index) = self.selection.section_index {
            self.selection.section_index = Some(section_index.min(section_len - 1));
        }
    }

    fn visible_targets(&self) -> Vec<SelectionTarget> {
        let mut targets = Vec::new();

        for (content_index, content) in self.registry.contents.iter().enumerate() {
            targets.push(SelectionTarget::Content(content_index));

            if self.selection.content_index != content_index {
                continue;
            }

            for (panel_index, panel) in content.panels.iter().enumerate() {
                targets.push(SelectionTarget::Panel(content_index, panel_index));

                if self.selection.panel_index != Some(panel_index) {
                    continue;
                }

                for (section_index, _) in panel.sections.iter().enumerate() {
                    targets.push(SelectionTarget::Section(
                        content_index,
                        panel_index,
                        section_index,
                    ));
                }
            }
        }

        targets
    }

    fn current_target(&self) -> SelectionTarget {
        match (self.selection.panel_index, self.selection.section_index) {
            (Some(panel_index), Some(section_index)) => {
                SelectionTarget::Section(self.selection.content_index, panel_index, section_index)
            }
            (Some(panel_index), None) => {
                SelectionTarget::Panel(self.selection.content_index, panel_index)
            }
            _ => SelectionTarget::Content(self.selection.content_index),
        }
    }

    fn apply_target(&mut self, target: SelectionTarget) {
        match target {
            SelectionTarget::Content(content_index) => {
                self.selection.content_index = content_index;
                self.selection.panel_index = None;
                self.selection.section_index = None;
            }
            SelectionTarget::Panel(content_index, panel_index) => {
                self.selection.content_index = content_index;
                self.selection.panel_index = Some(panel_index);
                self.selection.section_index = None;
            }
            SelectionTarget::Section(content_index, panel_index, section_index) => {
                self.selection.content_index = content_index;
                self.selection.panel_index = Some(panel_index);
                self.selection.section_index = Some(section_index);
            }
        }
    }
}

fn content_titles(registry: &ContentRegistry) -> Vec<String> {
    registry
        .contents
        .iter()
        .map(|content| content.title.clone())
        .collect()
}
