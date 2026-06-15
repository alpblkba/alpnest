use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use crate::settings::{AlpnestSettings, TerminalLayout};

pub fn uses_builtin_editor(settings: &AlpnestSettings) -> bool {
    settings.terminal_layout == TerminalLayout::BuiltInRightPane
}

pub fn open_markdown_file(path: &Path, settings: &AlpnestSettings) -> io::Result<()> {
    let editor = settings.text_editor.command();
    let backend = resolved_backend(settings.terminal_layout);

    match backend {
        TerminalLayout::BuiltInRightPane => Ok(()),
        TerminalLayout::Suspend => open_suspend(path, editor),
        TerminalLayout::TmuxRightPane => open_tmux(path, editor),
        TerminalLayout::ZellijRightPane => open_zellij(path, editor),
        TerminalLayout::Auto => {
            unreachable!("auto backend should be resolved before opening editor")
        }
    }
}

fn resolved_backend(layout: TerminalLayout) -> TerminalLayout {
    match layout {
        TerminalLayout::Auto => {
            if env::var_os("ZELLIJ").is_some() {
                TerminalLayout::ZellijRightPane
            } else if env::var_os("TMUX").is_some() {
                TerminalLayout::TmuxRightPane
            } else {
                TerminalLayout::Suspend
            }
        }
        other => other,
    }
}

fn open_suspend(path: &Path, editor: &str) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    let status = Command::new(editor).arg(path).status();

    execute!(io::stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;

    let status = status?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{editor} exited with status {status}"),
        ))
    }
}

fn open_tmux(path: &Path, editor: &str) -> io::Result<()> {
    let marker = marker_path("tmux");
    let script = format!(
        "{} {}; touch {}",
        shell_quote(editor),
        shell_quote_path(path),
        shell_quote_path(&marker)
    );

    let status = Command::new("tmux")
        .args(["split-window", "-h", "sh", "-lc", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("tmux split-window failed with status {status}"),
        ));
    }

    wait_for_marker(&marker)
}

fn open_zellij(path: &Path, editor: &str) -> io::Result<()> {
    let marker = marker_path("zellij");
    let script = format!(
        "{} {}; touch {}",
        shell_quote(editor),
        shell_quote_path(path),
        shell_quote_path(&marker)
    );

    let status = Command::new("zellij")
        .args([
            "action",
            "new-pane",
            "--direction",
            "right",
            "--",
            "sh",
            "-lc",
            &script,
        ])
        .status()?;

    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("zellij new-pane failed with status {status}"),
        ));
    }

    wait_for_marker(&marker)
}

fn marker_path(prefix: &str) -> std::path::PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    env::temp_dir().join(format!("alpnest-{prefix}-editor-done-{millis}"))
}

fn wait_for_marker(marker: &Path) -> io::Result<()> {
    loop {
        if marker.exists() {
            let _ = fs::remove_file(marker);
            return Ok(());
        }

        thread::sleep(Duration::from_millis(120));
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn shell_quote_path(path: &Path) -> String {
    shell_quote(&path.display().to_string())
}
