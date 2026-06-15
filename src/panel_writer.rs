use std::fs;
use std::io;
use std::path::Path;

use crate::content_editor::slugify;
use crate::panel_wizard::{PanelDefaults, PanelLogEntry, PanelLogLevel, PanelWizardState};

pub fn build_panels_from_wizard(wizard: &PanelWizardState) -> io::Result<Vec<PanelLogEntry>> {
    let mut logs = Vec::new();

    fs::create_dir_all(&wizard.target_content_path)?;

    for (index, raw_name) in wizard.panel_names.iter().enumerate() {
        let title = raw_name.trim();

        if title.is_empty() {
            logs.push(log(
                PanelLogLevel::Warning,
                format!("panel {} has an empty title; skipped", index + 1),
            ));
            continue;
        }

        let slug = slugify(title);

        if slug.is_empty() {
            logs.push(log(
                PanelLogLevel::Error,
                format!("panel {} could not produce a valid slug", index + 1),
            ));
            continue;
        }

        let defaults = wizard.defaults_for_panel(index);
        let panel_dir = wizard.target_content_path.join(&slug);

        if panel_dir.exists() {
            logs.push(log(
                PanelLogLevel::Warning,
                format!("{slug} was already added; skipped"),
            ));
            continue;
        }

        fs::create_dir_all(&panel_dir)?;

        write_panel_manifest(&panel_dir, title, &slug, &defaults)?;

        if defaults.create_overview {
            write_file_if_missing(panel_dir.join("overview.md"), &format!("# {title}\n\n"))?;
        }

        if defaults.create_overview_context {
            write_file_if_missing(
                panel_dir.join("overview.context.md"),
                &format!("# {title} context\n\n"),
            )?;
        }

        if defaults.create_prompt {
            write_file_if_missing(
                panel_dir.join(".prompt.md"),
                &format!("# {title} panel prompt\n\n"),
            )?;
        }

        if defaults.create_notes {
            write_file_if_missing(panel_dir.join("notes.md"), &format!("# {title} notes\n\n"))?;
        }

        if defaults.create_notes_context {
            write_file_if_missing(
                panel_dir.join("notes.context.md"),
                &format!("# {title} notes context\n\n"),
            )?;
        }

        logs.push(log(
            PanelLogLevel::Info,
            format!("panel {} {slug} added", index + 1),
        ));
    }

    if logs.is_empty() {
        logs.push(log(PanelLogLevel::Note, "no panel changes were made"));
    }

    Ok(logs)
}

pub fn destroy_selected_panel_from_wizard(
    wizard: &PanelWizardState,
) -> io::Result<Vec<PanelLogEntry>> {
    let Some(title) = wizard.selected_panel_title() else {
        return Ok(vec![log(
            PanelLogLevel::Error,
            "no panel selected for destroy",
        )]);
    };

    let slug = slugify(title);
    let panel_dir = wizard.target_content_path.join(&slug);

    if !panel_dir.exists() {
        return Ok(vec![log(
            PanelLogLevel::Error,
            format!("panel path does not exist: {}", panel_dir.display()),
        )]);
    }

    if panel_dir.parent() != Some(wizard.target_content_path.as_path()) {
        return Ok(vec![log(
            PanelLogLevel::Error,
            "refusing to destroy path outside target content",
        )]);
    }

    fs::remove_dir_all(&panel_dir)?;

    Ok(vec![log(
        PanelLogLevel::Info,
        format!("destroyed panel {title}"),
    )])
}

fn write_panel_manifest(
    panel_dir: &Path,
    title: &str,
    slug: &str,
    defaults: &PanelDefaults,
) -> io::Result<()> {
    let deadline_days = defaults.deadline_days.parse::<u64>().unwrap_or(0);
    let deadline_enabled = deadline_days > 0;

    let raw = format!(
        "schema_version = 1\nid = \"{}\"\ntitle = \"{}\"\nkind = \"panel\"\nhidden = false\norder = 0\nprompting_enabled = {}\ndeadline_enabled = {}\ndeadline_days = {}\n",
        toml_escape(slug),
        toml_escape(title),
        defaults.create_prompt,
        deadline_enabled,
        deadline_days,
    );

    fs::write(panel_dir.join(".panel.cfg"), raw)
}

fn write_file_if_missing(path: impl AsRef<Path>, content: &str) -> io::Result<()> {
    let path = path.as_ref();

    if path.exists() {
        return Ok(());
    }

    fs::write(path, content)
}

fn log(level: PanelLogLevel, message: impl Into<String>) -> PanelLogEntry {
    PanelLogEntry {
        level,
        message: message.into(),
    }
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
