//! Filesystem side of the cook section wizard.
//!
//! Every operation here is scoped to a single panel directory. Sections are
//! flat files inside that directory, so the guards below exist to make sure a
//! slug can never escape the panel or collide with the panel's own reserved
//! files (`context.md`, `.prompt.md`, `.panel.cfg`).

use std::fs;
use std::io;
use std::path::Path;

use crate::content_editor::slugify;
use crate::section_wizard::{SectionOperation, SectionWizardState};
use crate::wizard_log::{LogEntry, LogLevel};

/// Names that belong to the panel itself, not to any section.
const RESERVED_SECTION_IDS: [&str; 3] = ["context", "prompt", "panel"];

pub fn cook_sections_from_wizard(wizard: &SectionWizardState) -> io::Result<Vec<LogEntry>> {
    let panel_dir = validated_panel_dir(wizard)?;
    let mut logs = Vec::new();

    for (index, raw_name) in wizard.section_names.iter().enumerate() {
        let title = raw_name.trim();

        if title.is_empty() {
            logs.push(log(
                LogLevel::Warning,
                format!("section {} has an empty name; skipped", index + 1),
            ));
            continue;
        }

        let slug = slugify(title);

        if let Err(reason) = validate_slug(&slug) {
            logs.push(log(
                LogLevel::Error,
                format!("section {}: {reason}", index + 1),
            ));
            continue;
        }

        let defaults = wizard.defaults_for_section(index);

        if !defaults.create_body && !defaults.create_context {
            logs.push(log(
                LogLevel::Warning,
                format!("{slug}: nothing selected to create; skipped"),
            ));
            continue;
        }

        let body_path = panel_dir.join(format!("{slug}.md"));
        let context_path = panel_dir.join(format!("{slug}.context.md"));

        if defaults.create_body && body_path.exists() {
            logs.push(log(
                LogLevel::Warning,
                format!("{slug}.md already exists; skipped"),
            ));
            continue;
        }

        let mut created = Vec::new();

        if defaults.create_body {
            fs::write(&body_path, defaults.template.body(title))?;
            created.push(format!("{slug}.md"));
        }

        if defaults.create_context && !context_path.exists() {
            fs::write(&context_path, defaults.template.context(title))?;
            created.push(format!("{slug}.context.md"));
        }

        let deadline_days = defaults.deadline_days.parse::<u64>().unwrap_or(0);

        if deadline_days > 0 {
            record_section_deadline(&panel_dir, &slug, deadline_days)?;
            created.push(format!("deadline {deadline_days}d"));
        }

        logs.push(log(
            LogLevel::Info,
            format!("cooked {} ({})", slug, created.join(", ")),
        ));
    }

    if logs.is_empty() {
        logs.push(log(LogLevel::Note, "no sections were cooked"));
    }

    Ok(logs)
}

pub fn rename_section_from_wizard(wizard: &SectionWizardState) -> io::Result<Vec<LogEntry>> {
    let panel_dir = validated_panel_dir(wizard)?;

    let Some(current) = wizard.selected_section_name() else {
        return Ok(vec![log(LogLevel::Error, "no section selected to rename")]);
    };

    let current = current.to_string();
    let new_slug = slugify(&wizard.rename_buffer);

    if let Err(reason) = validate_slug(&new_slug) {
        return Ok(vec![log(LogLevel::Error, reason)]);
    }

    if new_slug == current {
        return Ok(vec![log(LogLevel::Note, "section name is unchanged")]);
    }

    let old_body = panel_dir.join(format!("{current}.md"));
    let new_body = panel_dir.join(format!("{new_slug}.md"));

    if !old_body.exists() {
        return Ok(vec![log(
            LogLevel::Error,
            format!("{current}.md does not exist"),
        )]);
    }

    if new_body.exists() {
        return Ok(vec![log(
            LogLevel::Error,
            format!("{new_slug}.md already exists"),
        )]);
    }

    fs::rename(&old_body, &new_body)?;
    let mut moved = vec![format!("{current}.md -> {new_slug}.md")];

    let old_context = panel_dir.join(format!("{current}.context.md"));
    let new_context = panel_dir.join(format!("{new_slug}.context.md"));

    if old_context.exists() && !new_context.exists() {
        fs::rename(&old_context, &new_context)?;
        moved.push(format!("{current}.context.md -> {new_slug}.context.md"));
    }

    Ok(vec![log(LogLevel::Info, format!("renamed {}", moved.join(", ")))])
}

pub fn remove_section_from_wizard(wizard: &SectionWizardState) -> io::Result<Vec<LogEntry>> {
    let panel_dir = validated_panel_dir(wizard)?;

    let Some(current) = wizard.selected_section_name() else {
        return Ok(vec![log(LogLevel::Error, "no section selected to remove")]);
    };

    let current = current.to_string();

    if let Err(reason) = validate_slug(&current) {
        return Ok(vec![log(LogLevel::Error, reason)]);
    }

    let body_path = panel_dir.join(format!("{current}.md"));
    let context_path = panel_dir.join(format!("{current}.context.md"));

    if !is_direct_child(&panel_dir, &body_path) {
        return Ok(vec![log(
            LogLevel::Error,
            "refusing to remove a path outside the target panel",
        )]);
    }

    let mut removed = Vec::new();

    if body_path.exists() {
        fs::remove_file(&body_path)?;
        removed.push(format!("{current}.md"));
    }

    if context_path.exists() {
        fs::remove_file(&context_path)?;
        removed.push(format!("{current}.context.md"));
    }

    if removed.is_empty() {
        return Ok(vec![log(
            LogLevel::Warning,
            format!("{current} had no files to remove"),
        )]);
    }

    Ok(vec![log(
        LogLevel::Info,
        format!("removed {}", removed.join(", ")),
    )])
}

pub fn apply_wizard(wizard: &SectionWizardState) -> io::Result<Vec<LogEntry>> {
    match wizard.operation {
        SectionOperation::Cook => cook_sections_from_wizard(wizard),
        SectionOperation::Edit => rename_section_from_wizard(wizard),
        SectionOperation::Remove => remove_section_from_wizard(wizard),
    }
}

/// Section deadlines live in the panel manifest so the registry can read them
/// back later without opening every markdown file.
fn record_section_deadline(panel_dir: &Path, slug: &str, days: u64) -> io::Result<()> {
    let manifest_path = panel_dir.join(".panel.cfg");
    let existing = fs::read_to_string(&manifest_path).unwrap_or_default();
    let key = format!("section_deadline_days.{slug}");

    let mut lines = existing
        .lines()
        .filter(|line| !line.trim_start().starts_with(&key))
        .map(str::to_string)
        .collect::<Vec<_>>();

    lines.push(format!("{key} = {days}"));

    let mut raw = lines.join("\n");
    raw.push('\n');

    fs::write(manifest_path, raw)
}

fn validated_panel_dir(wizard: &SectionWizardState) -> io::Result<std::path::PathBuf> {
    if !wizard.has_target {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no writable panel is selected",
        ));
    }

    let panel_dir = wizard.target_panel_path.clone();

    if panel_dir.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "target panel path is empty",
        ));
    }

    if !panel_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("panel directory does not exist: {}", panel_dir.display()),
        ));
    }

    Ok(panel_dir)
}

fn validate_slug(slug: &str) -> Result<(), String> {
    if slug.is_empty() {
        return Err("section name could not produce a valid slug".to_string());
    }

    if slug.starts_with('.') {
        return Err("section names cannot start with a dot".to_string());
    }

    if slug.ends_with(".context") {
        return Err("section names cannot end with .context".to_string());
    }

    if RESERVED_SECTION_IDS.contains(&slug) {
        return Err(format!("{slug} is reserved by the panel itself"));
    }

    Ok(())
}

fn is_direct_child(dir: &Path, candidate: &Path) -> bool {
    candidate.parent() == Some(dir)
}

fn log(level: LogLevel, message: impl Into<String>) -> LogEntry {
    LogEntry::new(level, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::section_wizard::{SectionOperation, SectionWizardState};
    use std::path::PathBuf;

    fn temp_panel(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("alpnest-section-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp panel");
        dir
    }

    fn wizard_for(dir: &Path) -> SectionWizardState {
        SectionWizardState {
            has_target: true,
            target_panel_path: dir.to_path_buf(),
            ..SectionWizardState::default()
        }
    }

    #[test]
    fn reserved_and_malformed_slugs_are_rejected() {
        assert!(validate_slug("").is_err());
        assert!(validate_slug("context").is_err());
        assert!(validate_slug("prompt").is_err());
        assert!(validate_slug("notes.context").is_err());
        assert!(validate_slug("notes").is_ok());
    }

    #[test]
    fn cook_writes_body_and_context_pair() {
        let dir = temp_panel("cook-pair");
        let mut wizard = wizard_for(&dir);
        wizard.section_names = vec!["Deep Notes".to_string()];
        wizard.section_defaults = vec![Default::default()];

        let logs = cook_sections_from_wizard(&wizard).expect("cook");

        assert!(dir.join("deep-notes.md").exists());
        assert!(dir.join("deep-notes.context.md").exists());
        assert!(logs.iter().any(|entry| entry.level == LogLevel::Info));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cook_skips_an_existing_section() {
        let dir = temp_panel("cook-existing");
        fs::write(dir.join("notes.md"), "# notes\n").expect("seed");

        let mut wizard = wizard_for(&dir);
        wizard.section_names = vec!["notes".to_string()];
        wizard.section_defaults = vec![Default::default()];

        let logs = cook_sections_from_wizard(&wizard).expect("cook");

        assert!(logs.iter().any(|entry| entry.level == LogLevel::Warning));
        assert_eq!(fs::read_to_string(dir.join("notes.md")).unwrap(), "# notes\n");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_moves_both_files() {
        let dir = temp_panel("rename-pair");
        fs::write(dir.join("old.md"), "# old\n").expect("seed body");
        fs::write(dir.join("old.context.md"), "# old context\n").expect("seed context");

        let mut wizard = wizard_for(&dir);
        wizard.operation = SectionOperation::Edit;
        wizard.existing_sections = vec!["old".to_string()];
        wizard.rename_buffer = "new".to_string();

        rename_section_from_wizard(&wizard).expect("rename");

        assert!(dir.join("new.md").exists());
        assert!(dir.join("new.context.md").exists());
        assert!(!dir.join("old.md").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_refuses_to_overwrite() {
        let dir = temp_panel("rename-collision");
        fs::write(dir.join("old.md"), "# old\n").expect("seed");
        fs::write(dir.join("new.md"), "# new\n").expect("seed");

        let mut wizard = wizard_for(&dir);
        wizard.operation = SectionOperation::Edit;
        wizard.existing_sections = vec!["old".to_string()];
        wizard.rename_buffer = "new".to_string();

        let logs = rename_section_from_wizard(&wizard).expect("rename");

        assert!(logs.iter().any(|entry| entry.level == LogLevel::Error));
        assert!(dir.join("old.md").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_deletes_body_and_context() {
        let dir = temp_panel("remove-pair");
        fs::write(dir.join("gone.md"), "# gone\n").expect("seed body");
        fs::write(dir.join("gone.context.md"), "# gone context\n").expect("seed context");

        let mut wizard = wizard_for(&dir);
        wizard.operation = SectionOperation::Remove;
        wizard.existing_sections = vec!["gone".to_string()];

        remove_section_from_wizard(&wizard).expect("remove");

        assert!(!dir.join("gone.md").exists());
        assert!(!dir.join("gone.context.md").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn deadline_is_recorded_in_the_panel_manifest() {
        let dir = temp_panel("deadline");
        let mut wizard = wizard_for(&dir);
        wizard.section_names = vec!["exam".to_string()];
        wizard.section_defaults = vec![crate::section_wizard::SectionDefaults {
            deadline_days: "14".to_string(),
            ..Default::default()
        }];

        cook_sections_from_wizard(&wizard).expect("cook");

        let manifest = fs::read_to_string(dir.join(".panel.cfg")).expect("manifest");
        assert!(manifest.contains("section_deadline_days.exam = 14"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_target_is_an_error_not_a_panic() {
        let wizard = SectionWizardState::default();
        assert!(cook_sections_from_wizard(&wizard).is_err());
    }
}
