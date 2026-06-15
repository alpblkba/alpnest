use std::env;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedTerminalKind {
    Editor,
    Shell,
}

pub struct EmbeddedTerminal {
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    parser: vt100::Parser,
    rx: Receiver<Vec<u8>>,
    pub active_path: PathBuf,
    pub kind: EmbeddedTerminalKind,
}

impl EmbeddedTerminal {
    pub fn spawn_editor(editor: &str, path: &Path, cols: u16, rows: u16) -> io::Result<Self> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(to_io_error)?;

        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut command = CommandBuilder::new(shell);

        let editor_command = editor_launch_command(editor, path);
        command.arg("-lc");
        command.arg(editor_command);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("ALPNEST_EMBEDDED_TERMINAL", "1");

        let child = pair.slave.spawn_command(command).map_err(to_io_error)?;
        let writer = pair.master.take_writer().map_err(to_io_error)?;
        let mut reader = pair.master.try_clone_reader().map_err(to_io_error)?;

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut buf = [0_u8; 8192];

            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            writer,
            child,
            parser: vt100::Parser::new(rows, cols, 2000),
            rx,
            active_path: path.to_path_buf(),
            kind: EmbeddedTerminalKind::Editor,
        })
    }

    pub fn spawn_shell(cwd: &Path, cols: u16, rows: u16) -> io::Result<Self> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(to_io_error)?;

        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut command = CommandBuilder::new(&shell);

        let shell_command = format!(
            "cd {} && exec {} -l",
            shell_quote_path(cwd),
            shell_quote(&shell)
        );
        command.arg("-lc");
        command.arg(shell_command);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("ALPNEST_EMBEDDED_TERMINAL", "1");

        let child = pair.slave.spawn_command(command).map_err(to_io_error)?;
        let writer = pair.master.take_writer().map_err(to_io_error)?;
        let mut reader = pair.master.try_clone_reader().map_err(to_io_error)?;

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut buf = [0_u8; 8192];

            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            writer,
            child,
            parser: vt100::Parser::new(rows, cols, 2000),
            rx,
            active_path: cwd.to_path_buf(),
            kind: EmbeddedTerminalKind::Shell,
        })
    }

    pub fn drain_output(&mut self) {
        while let Ok(bytes) = self.rx.try_recv() {
            self.parser.process(&bytes);
        }
    }

    pub fn is_finished(&mut self) -> io::Result<bool> {
        self.drain_output();

        match self.child.try_wait().map_err(to_io_error)? {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }

    pub fn terminate(&mut self) -> io::Result<()> {
        self.child.kill().map_err(to_io_error)
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }

    pub fn formatted_bytes(&mut self) -> Vec<u8> {
        self.drain_output();

        let bytes = self.parser.screen().contents_formatted();

        if bytes.is_empty() {
            b"embedded terminal is starting...".to_vec()
        } else {
            bytes
        }
    }
}

fn editor_launch_command(editor: &str, path: &Path) -> String {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| path.to_str().unwrap_or("file.md"));

    let cd_prefix = format!("cd {} && ", shell_quote_path(dir));
    let file = shell_quote(file_name);

    match editor {
        "vim" => {
            cd_prefix
                + "if [ -f \"$HOME/.vimrc\" ]; then exec vim -N -u \"$HOME/.vimrc\" "
                + "-c 'set showtabline=0' "
                + "-c 'set laststatus=0' "
                + "-c 'set noruler' "
                + "-c 'set noshowcmd' "
                + "-c 'set noshowmode' "
                + "-c 'silent! let g:airline#extensions#tabline#enabled = 0' "
                + &file
                + "; else exec vim -N "
                + "-c 'set showtabline=0' "
                + "-c 'set laststatus=0' "
                + "-c 'set noruler' "
                + "-c 'set noshowcmd' "
                + "-c 'set noshowmode' "
                + &file
                + "; fi"
        }
        "nvim" | "neovim" => {
            cd_prefix
                + "exec nvim "
                + "-c 'set showtabline=0' "
                + "-c 'set laststatus=0' "
                + "-c 'set noruler' "
                + "-c 'set noshowcmd' "
                + "-c 'set noshowmode' "
                + &file
        }
        _ => cd_prefix + &format!("exec {} {}", shell_quote(editor), file),
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn shell_quote_path(path: &Path) -> String {
    shell_quote(&path.display().to_string())
}

fn to_io_error<E>(error: E) -> io::Error
where
    E: std::fmt::Display,
{
    io::Error::new(io::ErrorKind::Other, error.to_string())
}
