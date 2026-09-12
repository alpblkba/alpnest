use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::model::{Content, ContentManifest, ContentType, Panel, Section};
use crate::paths::AlpnestPaths;

#[derive(Debug, Clone, Default)]
pub struct ContentRegistry {
    pub contents: Vec<Content>,
}

impl ContentRegistry {
    pub fn load_default() -> io::Result<Self> {
        let paths = AlpnestPaths::resolve()?;
        crate::bootstrap::ensure_initialized(&paths)?;
        Self::load_from_contents_dir(paths.contents_dir)
    }

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

            let content = load_content_dir(path)?;
            if content.is_navigable() {
                contents.push(content);
            }
        }

        contents.sort_by(|a, b| {
            a.order.cmp(&b.order).then_with(|| {
                a.title
                    .to_ascii_lowercase()
                    .cmp(&b.title.to_ascii_lowercase())
            })
        });

        Ok(Self { contents })
    }

    pub fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    pub fn len(&self) -> usize {
        self.contents.len()
    }

    pub fn content(&self, index: usize) -> Option<&Content> {
        self.contents.get(index)
    }
}

fn load_content_dir(path: PathBuf) -> io::Result<Content> {
    let manifest = load_manifest(&path)?;
    let fallback_id = clean_id_from_path(&path);
    let id = manifest.id.clone().unwrap_or(fallback_id);
    let title = manifest.title.clone().unwrap_or_else(|| title_from_id(&id));
    let content_type = manifest
        .content_type
        .clone()
        .unwrap_or_else(|| infer_content_type(&id));

    let body_path = first_existing(&path, &["overview.md"]);
    let context_path = first_existing(&path, &["context.md", "overview.context.md"]);

    let panels = match content_type {
        ContentType::Minimal => Vec::new(),
        ContentType::Mail => load_mail_panels(&path)?,
        ContentType::Calendar => load_special_root_panels(&path, &["daily", "weekly"])?,
        _ => load_directory_panels(&path)?,
    };

    Ok(Content {
        id,
        title,
        content_type,
        path,
        body_path,
        context_path,
        panels,
        order: manifest.order,
        hidden: manifest.hidden,
    })
}

fn load_directory_panels(path: &Path) -> io::Result<Vec<Panel>> {
    let mut panels = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let panel_path = entry.path();

        if !panel_path.is_dir() || is_hidden_path(&panel_path) {
            continue;
        }

        let manifest = load_manifest(&panel_path)?;
        let fallback_id = clean_id_from_path(&panel_path);
        let id = manifest.id.clone().unwrap_or(fallback_id);
        let title = manifest.title.clone().unwrap_or_else(|| title_from_id(&id));
        let prompt_path = first_existing(&panel_path, &[".prompt.md", "prompt.md"]);
        let sections = load_sections(&panel_path)?;

        let panel = Panel {
            id,
            title,
            path: panel_path,
            prompt_path,
            sections,
            order: manifest.order,
            hidden: manifest.hidden,
            synthetic: false,
        };

        if panel.is_navigable() {
            panels.push(panel);
        }
    }

    panels.sort_by(|a, b| {
        a.order.cmp(&b.order).then_with(|| {
            a.title
                .to_ascii_lowercase()
                .cmp(&b.title.to_ascii_lowercase())
        })
    });

    Ok(panels)
}

/// Mail panels are exactly the connected accounts — one panel per domain, and
/// nothing else.
///
/// Loose markdown at the mail root is deliberately *not* promoted to a panel.
/// The combined cross-account digest lives in `overview.md`, which the content
/// loader picks up as this content's own body, so selecting `Mail` shows the
/// digest without it appearing as a fake account beside the real ones.
fn load_mail_panels(path: &Path) -> io::Result<Vec<Panel>> {
    load_directory_panels(path)
}

fn load_special_root_panels(path: &Path, names: &[&str]) -> io::Result<Vec<Panel>> {
    let mut panels = Vec::new();

    for name in names {
        let body_path = path.join(format!("{name}.md"));

        if !body_path.exists() {
            continue;
        }

        let context_path = first_existing(path, &[format!("{name}.context.md").as_str()]);
        let section = Section {
            id: (*name).to_string(),
            title: title_from_id(name),
            body_path,
            context_path,
            order: 0,
            hidden: false,
        };

        panels.push(Panel {
            id: (*name).to_string(),
            title: title_from_id(name),
            path: path.to_path_buf(),
            prompt_path: None,
            sections: vec![section],
            order: 0,
            hidden: false,
            synthetic: true,
        });
    }

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

        if !is_visible_markdown_section(file_name) {
            continue;
        }

        let id = file_name.trim_end_matches(".md").to_string();
        let context_name = format!("{id}.context.md");
        let context_path = first_existing(path, &[context_name.as_str()]);

        // A leading `NN-` sets sort order and is stripped from the display
        // title, matching how top-level contents already use `00-today`.
        let (order, display_id) = split_order_prefix(&id);

        // A generated mail section is named for ordering, not for reading, so
        // prefer the document's own `# heading` when it has one.
        let title = heading_title(&section_path).unwrap_or_else(|| title_from_id(display_id));

        let section = Section {
            id,
            title,
            body_path: section_path,
            context_path,
            order,
            hidden: false,
        };

        if section.is_navigable() {
            sections.push(section);
        }
    }

    sections.sort_by(|a, b| {
        a.order.cmp(&b.order).then_with(|| {
            a.title
                .to_ascii_lowercase()
                .cmp(&b.title.to_ascii_lowercase())
        })
    });

    Ok(sections)
}

fn load_manifest(path: &Path) -> io::Result<ContentManifest> {
    let Some(manifest_path) = find_cfg_file(path)? else {
        return Ok(ContentManifest::default());
    };

    let raw = fs::read_to_string(manifest_path)?;
    Ok(parse_manifest_stub(&raw))
}

fn find_cfg_file(path: &Path) -> io::Result<Option<PathBuf>> {
    let mut candidates = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let cfg_path = entry.path();

        if !cfg_path.is_file() {
            continue;
        }

        let Some(file_name) = cfg_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if file_name.starts_with('.') && file_name.ends_with(".cfg") {
            candidates.push(cfg_path);
        }
    }

    candidates.sort();
    Ok(candidates.into_iter().next())
}

fn parse_manifest_stub(raw: &str) -> ContentManifest {
    let mut manifest = ContentManifest::default();

    for line in raw.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let key = key.trim();
        let value = strip_toml_string(value.trim());

        match key {
            "schema_version" => {
                manifest.schema_version = value.parse().unwrap_or(1);
            }
            "id" => {
                manifest.id = Some(value.to_string());
            }
            "title" => {
                manifest.title = Some(value.to_string());
            }
            "content_type" => {
                manifest.content_type = Some(ContentType::from_manifest_value(value));
            }
            "hidden" => {
                manifest.hidden = matches!(value, "true" | "True" | "TRUE");
            }
            "order" => {
                manifest.order = value.parse().unwrap_or(0);
            }
            _ => {}
        }
    }

    manifest
}

fn strip_toml_string(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
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

/// First `# heading` in the opening lines of a markdown file, if any.
///
/// Only the head of the file is inspected so this stays cheap when a panel
/// holds dozens of sections with full mail bodies.
fn heading_title(path: &Path) -> Option<String> {
    use std::io::{BufRead, BufReader};

    const MAX_LINES: usize = 12;
    const MAX_TITLE: usize = 72;

    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(MAX_LINES) {
        let line = line.ok()?;
        let trimmed = line.trim();

        if let Some(heading) = trimmed.strip_prefix("# ") {
            let heading = heading.trim();

            if heading.is_empty() {
                return None;
            }

            if heading.chars().count() > MAX_TITLE {
                let clipped: String = heading.chars().take(MAX_TITLE - 1).collect();
                return Some(format!("{}…", clipped.trim_end()));
            }

            return Some(heading.to_string());
        }
    }

    None
}

/// Splits a `NN-rest` prefix into an explicit sort order plus the remainder.
/// Without a numeric prefix the order is 0 and the id is returned unchanged.
fn split_order_prefix(id: &str) -> (i32, &str) {
    let digits: String = id.chars().take_while(char::is_ascii_digit).collect();

    if digits.is_empty() {
        return (0, id);
    }

    let rest = &id[digits.len()..];

    match rest.strip_prefix('-') {
        Some(remainder) if !remainder.is_empty() => {
            (digits.parse().unwrap_or(0), remainder)
        }
        _ => (0, id),
    }
}

fn title_from_id(id: &str) -> String {
    id.split('-')
        .filter(|part| !part.is_empty())
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();

    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn is_hidden_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with('.'))
        .unwrap_or(false)
}

fn is_visible_markdown_section(file_name: &str) -> bool {
    file_name.ends_with(".md")
        && !file_name.starts_with('.')
        && file_name != "context.md"
        && file_name != "prompt.md"
        && !file_name.ends_with(".context.md")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_prefix_sets_sort_order_and_strips_the_title() {
        assert_eq!(split_order_prefix("00-overview"), (0, "overview"));
        assert_eq!(split_order_prefix("07-2026-07-25-exam"), (7, "2026-07-25-exam"));
        assert_eq!(split_order_prefix("notes"), (0, "notes"));
    }

    #[test]
    fn a_bare_numeric_id_keeps_its_name() {
        // "2026" is a plausible section name; it must not become an empty title.
        assert_eq!(split_order_prefix("2026"), (0, "2026"));
        assert_eq!(split_order_prefix("12-"), (0, "12-"));
    }

    #[test]
    fn context_and_prompt_files_are_not_sections() {
        assert!(is_visible_markdown_section("notes.md"));
        assert!(!is_visible_markdown_section("notes.context.md"));
        assert!(!is_visible_markdown_section("context.md"));
        assert!(!is_visible_markdown_section(".prompt.md"));
    }
}
