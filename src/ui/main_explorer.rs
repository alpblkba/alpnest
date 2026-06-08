use crate::app::AppState;

#[derive(Debug, Clone)]
pub struct NavigationRow {
    pub depth: usize,
    pub label: String,
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub struct MainExplorerSnapshot {
    pub title: String,
    pub rows: Vec<NavigationRow>,
    pub body_path: Option<String>,
    pub context_path: Option<String>,
}

pub struct MainExplorerView;

impl MainExplorerView {
    pub fn snapshot(app: &AppState) -> MainExplorerSnapshot {
        let title = selected_title(app);
        let rows = navigation_rows(app);
        let body_path = app
            .selected_body_path()
            .map(|path| path.display().to_string());
        let context_path = app
            .selected_context_path()
            .map(|path| path.display().to_string());

        MainExplorerSnapshot {
            title,
            rows,
            body_path,
            context_path,
        }
    }
}

fn selected_title(app: &AppState) -> String {
    let Some(content) = app.selected_content() else {
        return "Alpnest / Main Explorer".to_string();
    };

    let mut parts = vec![
        "Alpnest".to_string(),
        app.current_view.title().to_string(),
        content.title.clone(),
    ];

    if let Some(panel) = app.selected_panel() {
        parts.push(panel.title.clone());
    }

    if let Some(section) = app.selected_section() {
        parts.push(section.title.clone());
    }

    parts.join(" / ")
}

fn navigation_rows(app: &AppState) -> Vec<NavigationRow> {
    let mut rows = Vec::new();

    for (content_index, content) in app.registry.contents.iter().enumerate() {
        let content_selected = app.selection.content_index == content_index
            && app.selection.panel_index.is_none()
            && app.selection.section_index.is_none();

        rows.push(NavigationRow {
            depth: 0,
            label: content.title.clone(),
            selected: content_selected,
        });

        if app.selection.content_index != content_index {
            continue;
        }

        for (panel_index, panel) in content.panels.iter().enumerate() {
            let panel_selected = app.selection.panel_index == Some(panel_index)
                && app.selection.section_index.is_none();

            rows.push(NavigationRow {
                depth: 1,
                label: panel.title.clone(),
                selected: panel_selected,
            });

            if app.selection.panel_index != Some(panel_index) {
                continue;
            }

            for (section_index, section) in panel.sections.iter().enumerate() {
                let section_selected = app.selection.section_index == Some(section_index);

                rows.push(NavigationRow {
                    depth: 2,
                    label: section.title.clone(),
                    selected: section_selected,
                });
            }
        }
    }

    rows
}
