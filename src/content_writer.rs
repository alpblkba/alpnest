use std::fs;
use std::io;
use std::path::Path;

use crate::content_editor::{ContentEditorKind, ContentEditorState, slugify};
use crate::paths::AlpnestPaths;

pub fn remove_content_from_draft(draft: &ContentEditorState) -> io::Result<String> {
    let title = draft
        .existing_content_titles
        .get(draft.selected_existing_content)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no existing content selected"))?;

    let slug = slugify(title);
    if slug.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "selected content has an empty slug",
        ));
    }

    let paths = AlpnestPaths::resolve()?;
    let content_dir = paths.contents_dir.join(&slug);

    if !content_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "content directory does not exist: {}",
                content_dir.display()
            ),
        ));
    }

    fs::remove_dir_all(&content_dir)?;

    Ok(format!("removed content: {title}"))
}

pub fn create_content_from_draft(draft: &ContentEditorState) -> io::Result<String> {
    draft
        .validate_for_create()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    let slug = draft.slug();
    let paths = AlpnestPaths::resolve()?;
    fs::create_dir_all(&paths.contents_dir)?;
    let content_dir = paths.contents_dir.join(&slug);

    if content_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("content already exists: {}", content_dir.display()),
        ));
    }

    fs::create_dir_all(&content_dir)?;

    match draft.content_kind {
        ContentEditorKind::Minimal => {
            write_file(content_dir.join("overview.md"), &draft.overview_buffer)?;
            write_file(content_dir.join("context.md"), &draft.context_buffer)?;
        }
        ContentEditorKind::Task => {
            write_file(content_dir.join("context.md"), &draft.context_buffer)?;

            if draft.auto_generate_panel_section {
                create_panel1_section1(&content_dir)?;
            }
        }
        ContentEditorKind::Project => {
            write_file(content_dir.join("context.md"), &draft.context_buffer)?;

            if draft.auto_generate_panel_section {
                create_panel1_section1(&content_dir)?;
            }
        }
    }

    write_file(
        content_dir.join(format!(".{slug}.cfg")),
        &manifest_for(draft),
    )?;

    Ok(format!("created content: {slug}"))
}

fn create_panel1_section1(content_dir: &Path) -> io::Result<()> {
    let panel_dir = content_dir.join("panel1");
    fs::create_dir_all(&panel_dir)?;

    write_file(panel_dir.join("overview.md"), "# panel1\n\n")?;
    write_file(
        panel_dir.join("overview.context.md"),
        "# panel1 context\n\n",
    )?;
    write_file(panel_dir.join("section1.md"), "# section1\n\n")?;
    write_file(
        panel_dir.join("section1.context.md"),
        "# section1 context\n\n",
    )?;
    write_file(panel_dir.join(".prompt.md"), "# panel-local prompt\n\n")?;

    Ok(())
}

fn manifest_for(draft: &ContentEditorState) -> String {
    let slug = draft.slug();
    let title = toml_escape(draft.content_name.trim());
    let kind = draft.content_kind.label();

    let mut manifest = format!(
        "schema_version = 1\nid = \"{slug}\"\ntitle = \"{title}\"\ncontent_type = \"{kind}\"\nhidden = false\norder = 0\n"
    );

    match draft.content_kind {
        ContentEditorKind::Minimal | ContentEditorKind::Task => {
            if draft.deadline_enabled {
                manifest.push_str("deadline_enabled = true\n");
                manifest.push_str(&format!(
                    "deadline_minutes = {}\n",
                    draft.deadline_minutes.trim()
                ));
            } else {
                manifest.push_str("deadline_enabled = false\n");
            }
        }
        ContentEditorKind::Project => {
            manifest.push_str("deadline_enabled = false\n");
            manifest.push_str(&format!(
                "project_base_dir = \"{}\"\n",
                toml_escape(draft.project_base_dir.trim())
            ));
        }
    }

    manifest
}

fn write_file(path: impl AsRef<Path>, content: &str) -> io::Result<()> {
    fs::write(path, content)
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
