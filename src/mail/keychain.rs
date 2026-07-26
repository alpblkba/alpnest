//! Keychain-backed mail credentials.
//!
//! Alpnest deliberately never reads, prompts for, or stores a mail password
//! itself. It only ever asks the macOS Keychain two things: "do you have a
//! secret for this account id?" and "please prompt the user to set one". The
//! secret is handed straight from the Keychain to the sync helper at fetch
//! time, so it is never held in Alpnest's memory or written to any config file.

use std::io;
use std::process::{Command, Stdio};

/// Keychain service name shared by the TUI and the Python sync helpers.
pub const SERVICE: &str = "alpnest-mail";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialState {
    Stored,
    Missing,
    NotApplicable,
    Unsupported,
}

impl CredentialState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stored => "keychain: stored",
            Self::Missing => "keychain: missing",
            Self::NotApplicable => "keychain: not needed",
            Self::Unsupported => "keychain: unsupported platform",
        }
    }
}

/// Existence check only. Note the absence of `-w`: without it `security`
/// prints item metadata and never the secret itself.
pub fn credential_state(account_id: &str) -> CredentialState {
    if !cfg!(target_os = "macos") {
        return CredentialState::Unsupported;
    }

    if account_id.trim().is_empty() {
        return CredentialState::Missing;
    }

    let status = Command::new("security")
        .args(["find-generic-password", "-a", account_id, "-s", SERVICE])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match status {
        Ok(status) if status.success() => CredentialState::Stored,
        Ok(_) => CredentialState::Missing,
        Err(_) => CredentialState::Unsupported,
    }
}

/// The command Alpnest spawns in the right-hand terminal so that macOS — not
/// Alpnest — prompts for the secret. `-w` with no value makes `security` read
/// the password interactively.
pub fn store_command(account_id: &str, email: &str) -> Vec<String> {
    let label = if email.trim().is_empty() {
        format!("alpnest mail: {account_id}")
    } else {
        format!("alpnest mail: {email}")
    };

    vec![
        "security".to_string(),
        "add-generic-password".to_string(),
        "-U".to_string(),
        "-a".to_string(),
        account_id.to_string(),
        "-s".to_string(),
        SERVICE.to_string(),
        "-l".to_string(),
        label,
        "-w".to_string(),
    ]
}

/// Shell-ready form of [`store_command`], shown in the UI so the user can also
/// run it themselves outside Alpnest.
pub fn store_command_display(account_id: &str, email: &str) -> String {
    store_command(account_id, email)
        .into_iter()
        .map(|part| {
            if part.contains(' ') {
                format!("\"{part}\"")
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn delete_credential(account_id: &str) -> io::Result<bool> {
    if !cfg!(target_os = "macos") {
        return Ok(false);
    }

    let status = Command::new("security")
        .args(["delete-generic-password", "-a", account_id, "-s", SERVICE])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    Ok(status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_command_is_interactive_and_carries_no_secret() {
        let argv = store_command("work", "someone@example.com");

        assert_eq!(argv[0], "security");
        assert!(argv.contains(&"-U".to_string()));
        assert!(argv.contains(&SERVICE.to_string()));

        // `-w` must be the final argument: a trailing value would mean the
        // secret was passed on the command line, which is exactly what this
        // module exists to avoid.
        assert_eq!(argv.last().map(String::as_str), Some("-w"));
    }

    #[test]
    fn store_command_labels_fall_back_to_the_account_id() {
        let display = store_command_display("work", "");
        assert!(display.contains("alpnest mail: work"));
    }

    #[test]
    fn empty_account_id_is_reported_missing() {
        if cfg!(target_os = "macos") {
            assert_eq!(credential_state("  "), CredentialState::Missing);
        }
    }
}
