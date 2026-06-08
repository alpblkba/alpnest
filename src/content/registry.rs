use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::model::{Content, ContentType, Panel, Section};

#[derive(Debug, Clone, Default)]
pub struct ContentRegistry {
    pub contents: Vec<Content>,
}

impl ContentRegistry {
    pub fn load_from_data_dir(path: impl AsRef<Path>) -> io::Result<Self> {
        let data_dir = path.as_ref();
        let contents_dir = data_dir.join("contents");

        if contents_dir.exists() {
            Self::load_from_contents_dir(contents_dir)
        } else {
            Ok(Self::default())
        }
    }

    pub fn load_from_contents_dir(path: impl AsRef<Path>) -> io::Result<Self> {
        let mut contents = Vec::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() || is_hidden_path(&path) {
                continue;
            }

            contents.push(load_content_dir(path)?);
        }

        contents.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.title.cmp(&b.title)));

        Ok(Self { contents })
    }
}

fn load_content_dir(path: PathBuf) -> io::Result<Content> {
    let id = clean_id_from_path(&path);
    let title = title_from_id(&id);

    let body_path = first_existing(&path, &["overview.md"]);
    let context_path = first_existing(&path, &["context.md", "overview.context.md"]);

    let content_type = infer_content_type(&id);

    let panels = if matches!(content_type, ContentType::Minimal) {
        Vec::new()
    } else {
        load_panels(&path)?
    };

    Ok(Content {
        id,
        title,
        content_type,
        path,
        body_path,
        context_path,
        panels,
        order: 0,
        hidden: false,
    })
}

fn load_panels(path: &Path) -> io::Result<Vec<Panel>> {
    let mut panels = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let panel_path = entry.path();

        if !panel_path.is_dir() || is_hidden_path(&panel_path) {
            continue;
        }

        let id = clean_id_from_path(&panel_path);
        let title = title_from_id(&id);
        let prompt_path = first_existing(&panel_path, &[".prompt.md", "prompt.md"]);
        let sections = load_sections(&panel_path)?;

        panels.push(Panel {
            id,
            title,
            path: panel_path,
            prompt_path,
            sections,
            order: 0,
            hidden: false,
        });
    }

    panels.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.title.cmp(&b.title)));

    Ok(panels)
}

fn load_sections(path: &Path) -> io::Result<Vec<Section>> {
    let mut sections = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let section_path = entry.path();

        if !section_path.is_file() || is_hidden_path(&section_path) {
            continue;
        }

        let Some(file_name) = section_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if !file_name.ends_with(".md")
            || file_name == "context.md"
            || file_name == "prompt.md"
            || file_name == ".prompt.md"
            || file_name.ends_with(".context.md")
        {
            continue;
        }

        let id = file_name.trim_end_matches(".md").to_string();
        let title = title_from_id(&id);
        let context_name = format!("{id}.context.md");
        let context_path = first_existing(path, &[context_name.as_str()]);

        sections.push(Section {
            id,
            title,
            body_path: section_path,
            context_path,
            order: 0,
            hidden: false,
        });
    }

    sections.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.title.cmp(&b.title)));

    Ok(sections)
}

fn infer_content_type(id: &str) -> ContentType {
    match id {
        "today" => ContentType::Minimal,
        "mail" => ContentType::Mail,
        "calendar" => ContentType::Calendar,
        _ => ContentType::Unknown,
    }
}

fn first_existing(path: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| path.join(name))
        .find(|candidate| candidate.exists())
}

fn clean_id_from_path(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    name.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-')
        .to_string()
}

fn title_from_id(id: &str) -> String {
    id.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_hidden_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with('.'))
        .unwrap_or(false)
}
