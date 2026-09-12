use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::paths::AlpnestPaths;

struct Resource {
    path: &'static str,
    contents: &'static [u8],
}

const STARTER_CONTENT: &[Resource] = &[
    Resource {
        path: "today/.today.cfg",
        contents: include_bytes!("../defaults/contents/today/.today.cfg"),
    },
    Resource {
        path: "today/overview.md",
        contents: include_bytes!("../defaults/contents/today/overview.md"),
    },
    Resource {
        path: "today/context.md",
        contents: include_bytes!("../defaults/contents/today/context.md"),
    },
    Resource {
        path: "projects/.projects.cfg",
        contents: include_bytes!("../defaults/contents/projects/.projects.cfg"),
    },
    Resource {
        path: "projects/overview.md",
        contents: include_bytes!("../defaults/contents/projects/overview.md"),
    },
    Resource {
        path: "projects/context.md",
        contents: include_bytes!("../defaults/contents/projects/context.md"),
    },
    Resource {
        path: "projects/inbox/.panel.cfg",
        contents: include_bytes!("../defaults/contents/projects/inbox/.panel.cfg"),
    },
    Resource {
        path: "projects/inbox/.prompt.md",
        contents: include_bytes!("../defaults/contents/projects/inbox/.prompt.md"),
    },
    Resource {
        path: "projects/inbox/overview.md",
        contents: include_bytes!("../defaults/contents/projects/inbox/overview.md"),
    },
    Resource {
        path: "projects/inbox/overview.context.md",
        contents: include_bytes!("../defaults/contents/projects/inbox/overview.context.md"),
    },
    Resource {
        path: "projects/inbox/first-steps.md",
        contents: include_bytes!("../defaults/contents/projects/inbox/first-steps.md"),
    },
    Resource {
        path: "projects/inbox/first-steps.context.md",
        contents: include_bytes!("../defaults/contents/projects/inbox/first-steps.context.md"),
    },
    Resource {
        path: "mail/.mail.cfg",
        contents: include_bytes!("../defaults/contents/mail/.mail.cfg"),
    },
    Resource {
        path: "mail/overview.md",
        contents: include_bytes!("../defaults/contents/mail/overview.md"),
    },
];

const RUNTIME_RESOURCES: &[Resource] = &[
    Resource {
        path: "scripts/sync_mail_imap.py",
        contents: include_bytes!("../scripts/sync_mail_imap.py"),
    },
    Resource {
        path: "scripts/sync_mail_apple.py",
        contents: include_bytes!("../scripts/sync_mail_apple.py"),
    },
    Resource {
        path: "scripts/summarize_mail_local.py",
        contents: include_bytes!("../scripts/summarize_mail_local.py"),
    },
    Resource {
        path: "scripts/summarizer_backend.py",
        contents: include_bytes!("../scripts/summarizer_backend.py"),
    },
    Resource {
        path: "scripts/mail_accounts.py",
        contents: include_bytes!("../scripts/mail_accounts.py"),
    },
    Resource {
        path: "scripts/mail_errors.py",
        contents: include_bytes!("../scripts/mail_errors.py"),
    },
    Resource {
        path: "scripts/paths.py",
        contents: include_bytes!("../scripts/paths.py"),
    },
    Resource {
        path: "scripts/mail_filters.cfg",
        contents: include_bytes!("../scripts/mail_filters.cfg"),
    },
    Resource {
        path: "scripts/suggest_section_local.py",
        contents: include_bytes!("../scripts/suggest_section_local.py"),
    },
    Resource {
        path: "prompts/qwen/mail_summarizer/system.md",
        contents: include_bytes!("../prompts/qwen/mail_summarizer/system.md"),
    },
    Resource {
        path: "prompts/qwen/mail_summarizer/context.md",
        contents: include_bytes!("../prompts/qwen/mail_summarizer/context.md"),
    },
    Resource {
        path: "prompts/qwen/mail_summarizer/task.md",
        contents: include_bytes!("../prompts/qwen/mail_summarizer/task.md"),
    },
    Resource {
        path: "prompts/qwen/mail_summarizer/output_schema.md",
        contents: include_bytes!("../prompts/qwen/mail_summarizer/output_schema.md"),
    },
    Resource {
        path: "prompts/qwen/mail_summarizer/rubric.md",
        contents: include_bytes!("../prompts/qwen/mail_summarizer/rubric.md"),
    },
    Resource {
        path: "prompts/qwen/mail_summarizer/examples.md",
        contents: include_bytes!("../prompts/qwen/mail_summarizer/examples.md"),
    },
    Resource {
        path: "prompts/qwen/mail_summarizer/failure_modes.md",
        contents: include_bytes!("../prompts/qwen/mail_summarizer/failure_modes.md"),
    },
];

/// Prepare a fresh runtime without overwriting any user-authored content.
/// Product-owned helper resources are refreshed when the installed version changes.
pub fn ensure_initialized(paths: &AlpnestPaths) -> io::Result<bool> {
    fs::create_dir_all(&paths.home)?;
    fs::create_dir_all(&paths.config_dir)?;
    fs::create_dir_all(&paths.contents_dir)?;

    let should_seed = !fs::read_dir(&paths.contents_dir)?
        .filter_map(Result::ok)
        .any(|entry| entry.path().is_dir());

    if should_seed {
        write_resources(&paths.contents_dir, STARTER_CONTENT, false)?;
    }

    write_resources(&runtime_dir(paths), RUNTIME_RESOURCES, true)?;
    Ok(should_seed)
}

pub fn runtime_dir(paths: &AlpnestPaths) -> PathBuf {
    paths.home.join("runtime")
}

pub fn runtime_script_path(paths: &AlpnestPaths, name: &str) -> Option<PathBuf> {
    let path = runtime_dir(paths).join("scripts").join(name);
    path.is_file().then_some(path)
}

fn write_resources(root: &Path, resources: &[Resource], replace_changed: bool) -> io::Result<()> {
    for resource in resources {
        let path = root.join(resource.path);

        let unchanged = fs::read(&path)
            .map(|contents| contents == resource.contents)
            .unwrap_or(false);

        if path.exists() && (!replace_changed || unchanged) {
            continue;
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, resource.contents)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(name: &str) -> AlpnestPaths {
        let home =
            std::env::temp_dir().join(format!("alpnest-bootstrap-{name}-{}", std::process::id()));

        let _ = fs::remove_dir_all(&home);

        AlpnestPaths {
            contents_dir: home.join("contents"),
            config_dir: home.join("config"),
            config_file: home.join("config/alpnest.toml"),
            home,
        }
    }

    #[test]
    fn fresh_home_gets_starter_content_and_runtime_helpers() {
        let paths = test_paths("fresh");

        assert!(ensure_initialized(&paths).expect("initialize"));
        assert!(paths.contents_dir.join("today/overview.md").is_file());
        assert!(
            paths
                .contents_dir
                .join("projects/inbox/first-steps.md")
                .is_file()
        );
        assert!(runtime_script_path(&paths, "suggest_section_local.py").is_some());
        assert!(
            runtime_dir(&paths)
                .join("prompts/qwen/mail_summarizer/system.md")
                .is_file()
        );

        let _ = fs::remove_dir_all(paths.home);
    }

    #[test]
    fn repeat_initialization_preserves_user_content() {
        let paths = test_paths("preserve");
        ensure_initialized(&paths).expect("initialize");

        let today = paths.contents_dir.join("today/overview.md");
        fs::write(&today, "# My day\n").expect("customize");

        assert!(!ensure_initialized(&paths).expect("initialize again"));
        assert_eq!(fs::read_to_string(today).unwrap(), "# My day\n");

        let _ = fs::remove_dir_all(paths.home);
    }
}
