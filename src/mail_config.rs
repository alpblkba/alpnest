//! Configure mail view state.
//!
//! This view owns the account list on the left and a connection form on the
//! right. It mutates an in-memory draft; nothing touches the filesystem until
//! the user saves, and nothing touches a secret at all — credential rows only
//! report Keychain state and hand back a command for the runtime to spawn.

use crate::content_editor::slugify;
use crate::mail::account::{
    MailAccount, MailAccountRegistry, MailProvider, SummarizerSettings,
};
use crate::mail::keychain::{self, CredentialState};
use crate::wizard_log::{LogEntry, LogLevel, trim};

const MAX_LOGS: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailConfigFocus {
    Accounts,
    Form,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MailConfigField {
    DisplayName,
    Provider,
    Email,
    Username,
    ImapHost,
    ImapPort,
    Security,
    Mailboxes,
    InitialLimit,
    BatchLimit,
    Enabled,
    Credential,
    SummarizerProvider,
    SummarizerModel,
    SummarizerKey,
    TestConnection,
    SyncAccount,
    Save,
    Remove,
}

impl MailConfigField {
    pub const ORDER: [MailConfigField; 19] = [
        Self::DisplayName,
        Self::Provider,
        Self::Email,
        Self::Username,
        Self::ImapHost,
        Self::ImapPort,
        Self::Security,
        Self::Mailboxes,
        Self::InitialLimit,
        Self::BatchLimit,
        Self::Enabled,
        Self::Credential,
        Self::SummarizerProvider,
        Self::SummarizerModel,
        Self::SummarizerKey,
        Self::TestConnection,
        Self::SyncAccount,
        Self::Save,
        Self::Remove,
    ];

    fn is_text(self) -> bool {
        matches!(
            self,
            Self::DisplayName
                | Self::Email
                | Self::Username
                | Self::ImapHost
                | Self::ImapPort
                | Self::Mailboxes
                | Self::InitialLimit
                | Self::BatchLimit
                | Self::SummarizerModel
        )
    }

    fn is_numeric(self) -> bool {
        matches!(self, Self::ImapPort | Self::InitialLimit | Self::BatchLimit)
    }
}

/// Work the runtime has to perform on the wizard's behalf, because it needs
/// the embedded terminal or a subprocess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailConfigAction {
    None,
    RunCommand { title: String, argv: Vec<String> },
    Reload,
}

#[derive(Debug, Clone)]
pub struct MailConfigState {
    pub registry: MailAccountRegistry,
    pub focus: MailConfigFocus,
    pub selected_account: usize,
    pub selected_field: MailConfigField,
    pub draft: MailAccount,
    pub draft_is_new: bool,
    pub dirty: bool,
    pub editing_text: bool,
    pub edit_buffer: String,
    pub confirm_remove: bool,
    pub logs: Vec<LogEntry>,
}

impl Default for MailConfigState {
    fn default() -> Self {
        Self {
            registry: MailAccountRegistry::default(),
            focus: MailConfigFocus::Accounts,
            selected_account: 0,
            selected_field: MailConfigField::DisplayName,
            draft: MailAccount::default(),
            draft_is_new: true,
            dirty: false,
            editing_text: false,
            edit_buffer: String::new(),
            confirm_remove: false,
            logs: Vec::new(),
        }
    }
}

impl MailConfigState {
    pub fn load() -> Self {
        let mut state = Self::default();

        match MailAccountRegistry::load() {
            Ok(registry) => {
                state.registry = registry;

                if state.registry.is_empty() {
                    state.log_note("no mail accounts configured yet — press n to add one");
                    state.start_new_account();
                } else {
                    state.select_account(0);
                    state.log_note(format!("{} account(s) loaded", state.registry.len()));
                }
            }
            Err(err) => {
                state.log_error(format!("failed to load accounts: {err}"));
                state.start_new_account();
            }
        }

        state
    }

    pub fn credential_state(&self) -> CredentialState {
        if !self.draft.provider.needs_network() {
            return CredentialState::NotApplicable;
        }

        if self.draft.id.trim().is_empty() {
            return CredentialState::Missing;
        }

        keychain::credential_state(&self.draft.id)
    }

    pub fn start_new_account(&mut self) {
        let id = self.registry.unique_id("account");

        self.draft = MailAccount::new(id, MailProvider::Gmail);
        self.draft.display_name = "New account".to_string();
        self.draft_is_new = true;
        self.dirty = false;
        self.confirm_remove = false;
        self.stop_editing();
        self.focus = MailConfigFocus::Form;
        self.selected_field = MailConfigField::DisplayName;
        self.log_note("new account draft started");
    }

    pub fn select_account(&mut self, index: usize) {
        let Some(account) = self.registry.get(index) else {
            return;
        };

        self.selected_account = index;
        self.draft = account.clone();
        self.draft_is_new = false;
        self.dirty = false;
        self.confirm_remove = false;
        self.stop_editing();
    }

    pub fn focus_accounts(&mut self) {
        self.stop_editing();
        self.focus = MailConfigFocus::Accounts;
        self.confirm_remove = false;
    }

    pub fn focus_form(&mut self) {
        self.stop_editing();
        self.focus = MailConfigFocus::Form;
        self.confirm_remove = false;
    }

    pub fn move_next(&mut self) {
        if self.editing_text {
            return;
        }

        match self.focus {
            MailConfigFocus::Accounts => {
                if self.registry.is_empty() {
                    return;
                }

                let next = (self.selected_account + 1).min(self.registry.len() - 1);
                self.select_account(next);
            }
            MailConfigFocus::Form => {
                let current = self.field_index();
                let next = (current + 1) % MailConfigField::ORDER.len();
                self.selected_field = MailConfigField::ORDER[next];
                self.confirm_remove = false;
            }
        }
    }

    pub fn move_prev(&mut self) {
        if self.editing_text {
            return;
        }

        match self.focus {
            MailConfigFocus::Accounts => {
                if self.registry.is_empty() {
                    return;
                }

                let prev = self.selected_account.saturating_sub(1);
                self.select_account(prev);
            }
            MailConfigFocus::Form => {
                let current = self.field_index();
                let prev = if current == 0 {
                    MailConfigField::ORDER.len() - 1
                } else {
                    current - 1
                };
                self.selected_field = MailConfigField::ORDER[prev];
                self.confirm_remove = false;
            }
        }
    }

    /// Enter/space on the selected row. Returns any work for the runtime.
    pub fn activate(&mut self) -> MailConfigAction {
        if self.focus == MailConfigFocus::Accounts {
            self.focus_form();
            return MailConfigAction::None;
        }

        if self.editing_text {
            self.commit_text_edit();
            return MailConfigAction::None;
        }

        match self.selected_field {
            field if field.is_text() => {
                self.start_text_edit();
                MailConfigAction::None
            }
            MailConfigField::Provider => {
                let was_default_host =
                    self.draft.imap_host == self.draft.provider.default_host()
                        || self.draft.imap_host.is_empty();

                self.draft.provider = self.draft.provider.next();

                if was_default_host {
                    self.draft.apply_provider_defaults();
                }

                self.dirty = true;
                MailConfigAction::None
            }
            MailConfigField::Security => {
                self.draft.security = self.draft.security.next();
                self.dirty = true;
                MailConfigAction::None
            }
            MailConfigField::Enabled => {
                self.draft.enabled = !self.draft.enabled;
                self.dirty = true;
                MailConfigAction::None
            }
            MailConfigField::Credential => self.credential_action(),
            MailConfigField::SummarizerProvider => {
                let summarizer = &mut self.registry.summarizer;
                summarizer.provider = summarizer.provider.next();
                summarizer.model = summarizer.provider.default_model().to_string();

                match self.registry.save() {
                    Ok(_) => self.log_info(format!(
                        "summarizer set to {}",
                        self.registry.summarizer.provider.label()
                    )),
                    Err(err) => self.log_error(format!("could not save summarizer: {err}")),
                }

                MailConfigAction::None
            }
            MailConfigField::SummarizerKey => self.summarizer_key_action(),
            MailConfigField::TestConnection => self.sync_action(true),
            MailConfigField::SyncAccount => self.sync_action(false),
            MailConfigField::Save => self.save(),
            MailConfigField::Remove => self.remove(),
            _ => MailConfigAction::None,
        }
    }

    fn credential_action(&mut self) -> MailConfigAction {
        if !self.draft.provider.needs_network() {
            self.log_note("this provider reads the local Mail store; no credential needed");
            return MailConfigAction::None;
        }

        if self.draft.id.trim().is_empty() {
            self.log_error("account needs an id before a credential can be stored");
            return MailConfigAction::None;
        }

        if self.draft_is_new || self.dirty {
            self.log_warning("save the account first so the credential binds to a stored id");
        }

        self.log_info("macOS will prompt for the secret; Alpnest never sees it");

        MailConfigAction::RunCommand {
            title: format!("keychain: {}", self.draft.id),
            argv: keychain::store_command(&self.draft.id, &self.draft.email),
        }
    }

    /// API keys go into the Keychain the same way mail passwords do: macOS
    /// prompts, Alpnest never sees the value.
    fn summarizer_key_action(&mut self) -> MailConfigAction {
        let provider = self.registry.summarizer.provider;

        if !provider.needs_api_key() {
            self.log_note("the local model needs no API key");
            return MailConfigAction::None;
        }

        let account = provider.keychain_account();
        self.log_info("macOS will prompt for the key; Alpnest never sees it");

        MailConfigAction::RunCommand {
            title: format!("keychain: {account}"),
            argv: keychain::store_command(&account, provider.config_value()),
        }
    }

    fn sync_action(&mut self, test_only: bool) -> MailConfigAction {
        if let Err(reason) = self.draft.validate() {
            self.log_error(reason);
            return MailConfigAction::None;
        }

        if self.draft_is_new || self.dirty {
            self.log_warning("unsaved changes; syncing the last saved configuration");
        }

        let Some(script) = sync_script_path() else {
            self.log_error("mail sync helper is missing; run `alpnest doctor`");
            return MailConfigAction::None;
        };

        let mut argv = vec![
            "python3".to_string(),
            script,
            "--account".to_string(),
            self.draft.id.clone(),
        ];

        if test_only {
            argv.push("--test".to_string());
        }

        let title = if test_only {
            format!("test connection: {}", self.draft.id)
        } else {
            format!("sync: {}", self.draft.id)
        };

        self.log_info(title.clone());

        MailConfigAction::RunCommand { title, argv }
    }

    pub fn sync_all_action(&mut self) -> MailConfigAction {
        if self.registry.is_empty() {
            self.log_warning("no accounts to sync");
            return MailConfigAction::None;
        }

        let Some(script) = sync_script_path() else {
            self.log_error("mail sync helper is missing; run `alpnest doctor`");
            return MailConfigAction::None;
        };

        self.log_info("syncing every enabled account");

        MailConfigAction::RunCommand {
            title: "sync all accounts".to_string(),
            argv: vec!["python3".to_string(), script, "--all".to_string()],
        }
    }

    fn save(&mut self) -> MailConfigAction {
        if let Err(reason) = self.draft.validate() {
            self.log_error(reason);
            return MailConfigAction::None;
        }

        if self.draft_is_new {
            let base = slugify(&self.draft.display_name);
            let base = if base.is_empty() { "account" } else { &base };
            self.draft.id = self.registry.unique_id(base);
            self.registry.accounts.push(self.draft.clone());
            self.selected_account = self.registry.len() - 1;
        } else if let Some(existing) = self.registry.get_mut(self.selected_account) {
            *existing = self.draft.clone();
        }

        match self.registry.save() {
            Ok(path) => {
                self.draft_is_new = false;
                self.dirty = false;
                self.log_info(format!("saved {} accounts to {}", self.registry.len(), path.display()));
                MailConfigAction::Reload
            }
            Err(err) => {
                self.log_error(format!("save failed: {err}"));
                MailConfigAction::None
            }
        }
    }

    fn remove(&mut self) -> MailConfigAction {
        if self.draft_is_new {
            self.log_note("draft discarded");
            self.reset_to_registry();
            return MailConfigAction::None;
        }

        if !self.confirm_remove {
            self.confirm_remove = true;
            self.log_warning(format!(
                "press enter again to remove {} (its keychain entry is kept)",
                self.draft.id
            ));
            return MailConfigAction::None;
        }

        self.confirm_remove = false;

        if self.selected_account >= self.registry.len() {
            self.log_error("nothing to remove");
            return MailConfigAction::None;
        }

        let removed = self.registry.accounts.remove(self.selected_account);

        match self.registry.save() {
            Ok(_) => {
                self.log_info(format!(
                    "removed {}; run `security delete-generic-password -a {} -s alpnest-mail` to drop its secret",
                    removed.id, removed.id
                ));
                self.reset_to_registry();
                MailConfigAction::Reload
            }
            Err(err) => {
                self.registry.accounts.insert(self.selected_account, removed);
                self.log_error(format!("remove failed: {err}"));
                MailConfigAction::None
            }
        }
    }

    fn reset_to_registry(&mut self) {
        if self.registry.is_empty() {
            self.start_new_account();
        } else {
            let index = self.selected_account.min(self.registry.len() - 1);
            self.select_account(index);
        }
    }

    pub fn start_text_edit(&mut self) {
        if !self.selected_field.is_text() {
            return;
        }

        self.editing_text = true;
        self.edit_buffer = self.current_text_value();
    }

    pub fn commit_text_edit(&mut self) {
        if !self.editing_text {
            return;
        }

        let value = self.edit_buffer.trim().to_string();

        match self.selected_field {
            MailConfigField::DisplayName => {
                if !value.is_empty() {
                    self.draft.display_name = value;
                }
            }
            MailConfigField::Email => self.draft.email = value,
            MailConfigField::Username => self.draft.username = value,
            MailConfigField::SummarizerModel => {
                if !value.is_empty() {
                    self.registry.summarizer.model = value;
                    let _ = self.registry.save();
                }
            }
            MailConfigField::ImapHost => self.draft.imap_host = value,
            MailConfigField::ImapPort => {
                self.draft.imap_port = value.parse().unwrap_or(self.draft.imap_port);
            }
            MailConfigField::Mailboxes => self.draft.set_mailboxes_from_display(&value),
            MailConfigField::InitialLimit => {
                self.draft.initial_limit = value.parse().unwrap_or(self.draft.initial_limit);
            }
            MailConfigField::BatchLimit => {
                self.draft.batch_limit = value.parse().unwrap_or(self.draft.batch_limit);
            }
            _ => {}
        }

        self.editing_text = false;
        self.edit_buffer.clear();
        self.dirty = true;
    }

    pub fn cancel_text_edit(&mut self) {
        self.editing_text = false;
        self.edit_buffer.clear();
    }

    pub fn push_char(&mut self, c: char) {
        if !self.editing_text {
            if self.focus == MailConfigFocus::Form && self.selected_field.is_text() {
                self.start_text_edit();
                self.edit_buffer.clear();
            } else {
                return;
            }
        }

        if self.selected_field.is_numeric() && !c.is_ascii_digit() {
            return;
        }

        self.edit_buffer.push(c);
    }

    pub fn backspace(&mut self) {
        if self.editing_text {
            self.edit_buffer.pop();
        }
    }

    pub fn account_rows(&self) -> Vec<(String, bool)> {
        if self.registry.is_empty() {
            return vec![("no accounts yet".to_string(), false)];
        }

        self.registry
            .accounts
            .iter()
            .enumerate()
            .map(|(index, account)| {
                let marker = if account.enabled { "●" } else { "○" };
                (
                    format!("{marker} {}  ({})", account.display_name, account.id),
                    self.focus == MailConfigFocus::Accounts && self.selected_account == index,
                )
            })
            .collect()
    }

    /// `(label, value, selected, is_action)`
    pub fn form_rows(&self) -> Vec<(String, String, bool, bool)> {
        let credential = self.credential_state();

        MailConfigField::ORDER
            .into_iter()
            .map(|field| {
                let selected =
                    self.focus == MailConfigFocus::Form && self.selected_field == field;

                let editing = selected && self.editing_text;

                let (label, value, is_action) = match field {
                    MailConfigField::DisplayName => (
                        "display name",
                        self.text_or_buffer(&self.draft.display_name, editing),
                        false,
                    ),
                    MailConfigField::Provider => {
                        ("provider", self.draft.provider.label().to_string(), false)
                    }
                    MailConfigField::Email => (
                        "email",
                        self.text_or_buffer(&self.draft.email, editing),
                        false,
                    ),
                    MailConfigField::Username => (
                        "login name",
                        if self.draft.username.is_empty() && !editing {
                            format!("<same as email: {}>", self.draft.email)
                        } else {
                            self.text_or_buffer(&self.draft.username, editing)
                        },
                        false,
                    ),
                    MailConfigField::ImapHost => (
                        "imap host",
                        self.text_or_buffer(&self.draft.imap_host, editing),
                        false,
                    ),
                    MailConfigField::ImapPort => (
                        "imap port",
                        self.text_or_buffer(&self.draft.imap_port.to_string(), editing),
                        false,
                    ),
                    MailConfigField::Security => {
                        ("security", self.draft.security.label().to_string(), false)
                    }
                    MailConfigField::Mailboxes => (
                        "mailboxes",
                        self.text_or_buffer(&self.draft.mailboxes_display(), editing),
                        false,
                    ),
                    MailConfigField::InitialLimit => (
                        "first sync",
                        self.text_or_buffer(&self.draft.initial_limit.to_string(), editing),
                        false,
                    ),
                    MailConfigField::BatchLimit => (
                        "per sync",
                        self.text_or_buffer(&self.draft.batch_limit.to_string(), editing),
                        false,
                    ),
                    MailConfigField::Enabled => (
                        "enabled",
                        if self.draft.enabled { "yes" } else { "no" }.to_string(),
                        false,
                    ),
                    MailConfigField::Credential => {
                        ("credential", credential.label().to_string(), false)
                    }
                    MailConfigField::SummarizerProvider => (
                        "summarizer",
                        self.registry.summarizer.provider.label().to_string(),
                        false,
                    ),
                    MailConfigField::SummarizerModel => (
                        "model",
                        self.text_or_buffer(&self.registry.summarizer.model, editing),
                        false,
                    ),
                    MailConfigField::SummarizerKey => (
                        "api key",
                        if self.registry.summarizer.provider.needs_api_key() {
                            summarizer_key_state(&self.registry.summarizer).to_string()
                        } else {
                            "not needed (local model)".to_string()
                        },
                        false,
                    ),
                    MailConfigField::TestConnection => {
                        ("action", "test connection".to_string(), true)
                    }
                    MailConfigField::SyncAccount => ("action", "sync this account".to_string(), true),
                    MailConfigField::Save => (
                        "action",
                        if self.draft_is_new {
                            "create account".to_string()
                        } else {
                            "save changes".to_string()
                        },
                        true,
                    ),
                    MailConfigField::Remove => (
                        "action",
                        if self.draft_is_new {
                            "discard draft".to_string()
                        } else if self.confirm_remove {
                            "confirm remove account".to_string()
                        } else {
                            "remove account".to_string()
                        },
                        true,
                    ),
                };

                (label.to_string(), value, selected, is_action)
            })
            .collect()
    }

    pub fn hint_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("provider: {}", self.draft.provider.label()),
            format!("credential: {}", self.draft.provider.credential_hint()),
            String::new(),
        ];

        if self.draft.provider.needs_network() {
            lines.push("Alpnest stores no secret itself. Selecting the credential".to_string());
            lines.push("row runs this command so macOS does the prompting:".to_string());
            lines.push(String::new());
            lines.push(keychain::store_command_display(
                &self.draft.id,
                &self.draft.email,
            ));
        } else {
            lines.push("This provider reads the local Mail.app store and needs".to_string());
            lines.push("no network credential.".to_string());
        }

        lines.push(String::new());
        lines.push(format!("accounts file: {}", account_file_display()));

        lines
    }

    fn text_or_buffer(&self, current: &str, editing: bool) -> String {
        if editing {
            format!("{}_", self.edit_buffer)
        } else if current.is_empty() {
            "<empty>".to_string()
        } else {
            current.to_string()
        }
    }

    fn current_text_value(&self) -> String {
        match self.selected_field {
            MailConfigField::DisplayName => self.draft.display_name.clone(),
            MailConfigField::Email => self.draft.email.clone(),
            MailConfigField::Username => self.draft.username.clone(),
            MailConfigField::SummarizerModel => self.registry.summarizer.model.clone(),
            MailConfigField::ImapHost => self.draft.imap_host.clone(),
            MailConfigField::ImapPort => self.draft.imap_port.to_string(),
            MailConfigField::Mailboxes => self.draft.mailboxes_display(),
            MailConfigField::InitialLimit => self.draft.initial_limit.to_string(),
            MailConfigField::BatchLimit => self.draft.batch_limit.to_string(),
            _ => String::new(),
        }
    }

    fn field_index(&self) -> usize {
        MailConfigField::ORDER
            .iter()
            .position(|field| *field == self.selected_field)
            .unwrap_or(0)
    }

    fn stop_editing(&mut self) {
        self.editing_text = false;
        self.edit_buffer.clear();
    }

    pub fn log_note(&mut self, message: impl Into<String>) {
        self.push_log(LogLevel::Note, message);
    }

    pub fn log_info(&mut self, message: impl Into<String>) {
        self.push_log(LogLevel::Info, message);
    }

    pub fn log_warning(&mut self, message: impl Into<String>) {
        self.push_log(LogLevel::Warning, message);
    }

    pub fn log_error(&mut self, message: impl Into<String>) {
        self.push_log(LogLevel::Error, message);
    }

    fn push_log(&mut self, level: LogLevel, message: impl Into<String>) {
        self.logs.push(LogEntry::new(level, message));
        trim(&mut self.logs, MAX_LOGS);
    }
}

/// Resolve the sync helper to an absolute path.
///
/// Alpnest is usually launched from somewhere other than its own checkout
/// (`~/.cargo/bin/alpnest`), so a relative `scripts/...` path would not
/// resolve. An explicit checkout wins for development; installed builds use
/// the helper bundle extracted under `ALPNEST_HOME/runtime` at startup.
pub fn sync_script_path() -> Option<String> {
    const SCRIPT: &str = "scripts/sync_mail_imap.py";

    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    if let Some(repo) = std::env::var_os("ALPNEST_REPO") {
        candidates.push(std::path::PathBuf::from(repo).join(SCRIPT));
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(SCRIPT));
    }

    // target/debug/alpnest and target/release/alpnest both sit three levels
    // below the checkout root.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(root) = exe.parent().and_then(|p| p.parent()).and_then(|p| p.parent()) {
            candidates.push(root.join(SCRIPT));
        }
    }

    if let Ok(paths) = crate::paths::AlpnestPaths::resolve()
        && let Some(path) = crate::bootstrap::runtime_script_path(&paths, "sync_mail_imap.py")
    {
        candidates.push(path);
    }

    candidates
        .into_iter()
        .find(|path| path.is_file())
        .map(|path| path.display().to_string())
}

fn summarizer_key_state(summarizer: &SummarizerSettings) -> &'static str {
    match keychain::credential_state(&summarizer.provider.keychain_account()) {
        CredentialState::Stored => "keychain: stored",
        CredentialState::Missing => "keychain: missing",
        CredentialState::NotApplicable => "not needed",
        CredentialState::Unsupported => "keychain: unsupported platform",
    }
}

fn account_file_display() -> String {
    MailAccountRegistry::config_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "$ALPNEST_HOME/config/mail/accounts.cfg".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_accounts() -> MailConfigState {
        let registry = MailAccountRegistry {
            summarizer: SummarizerSettings::default(),
            accounts: vec![
                {
                    let mut account = MailAccount::new("work", MailProvider::Microsoft);
                    account.email = "someone@example.com".to_string();
                    account
                },
                {
                    let mut account = MailAccount::new("home", MailProvider::Gmail);
                    account.email = "someone@gmail.com".to_string();
                    account
                },
            ],
        };

        let mut state = MailConfigState {
            registry,
            ..MailConfigState::default()
        };
        state.select_account(0);
        state
    }

    #[test]
    fn selecting_an_account_loads_it_into_the_draft() {
        let mut state = state_with_accounts();

        assert_eq!(state.draft.id, "work");
        assert!(!state.draft_is_new);

        state.selected_account = 1;
        state.select_account(1);
        assert_eq!(state.draft.id, "home");
    }

    #[test]
    fn form_navigation_wraps_through_every_field() {
        let mut state = state_with_accounts();
        state.focus_form();

        for _ in 0..MailConfigField::ORDER.len() {
            state.move_next();
        }

        assert_eq!(state.selected_field, MailConfigField::DisplayName);
    }

    #[test]
    fn cycling_provider_updates_default_host() {
        let mut state = state_with_accounts();
        state.focus_form();
        state.selected_field = MailConfigField::Provider;

        let before = state.draft.provider;
        state.activate();

        assert_ne!(state.draft.provider, before);
        assert_eq!(state.draft.imap_host, state.draft.provider.default_host());
        assert!(state.dirty);
    }

    #[test]
    fn editing_a_numeric_field_rejects_letters() {
        let mut state = state_with_accounts();
        state.focus_form();
        state.selected_field = MailConfigField::ImapPort;
        state.start_text_edit();
        state.edit_buffer.clear();

        state.push_char('9');
        state.push_char('x');
        state.push_char('3');
        state.commit_text_edit();

        assert_eq!(state.draft.imap_port, 93);
    }

    #[test]
    fn removing_requires_a_confirmation_pass() {
        let mut state = state_with_accounts();
        state.focus_form();
        state.selected_field = MailConfigField::Remove;

        state.activate();
        assert!(state.confirm_remove);
        assert_eq!(state.registry.len(), 2);
    }

    #[test]
    fn invalid_account_does_not_produce_a_sync_command() {
        let mut state = state_with_accounts();
        state.focus_form();
        state.draft.email.clear();
        state.selected_field = MailConfigField::SyncAccount;

        assert_eq!(state.activate(), MailConfigAction::None);
        assert!(state.logs.iter().any(|entry| entry.level == LogLevel::Error));
    }

    #[test]
    fn credential_action_returns_an_interactive_keychain_command() {
        let mut state = state_with_accounts();
        state.focus_form();
        state.selected_field = MailConfigField::Credential;

        match state.activate() {
            MailConfigAction::RunCommand { argv, .. } => {
                assert_eq!(argv[0], "security");
                assert_eq!(argv.last().map(String::as_str), Some("-w"));
            }
            other => panic!("expected a keychain command, got {other:?}"),
        }
    }

    #[test]
    fn sync_script_resolves_to_an_absolute_path() {
        // `cargo test` runs with the crate root as the working directory, so
        // the checkout copy must be found.
        let resolved = sync_script_path().expect("sync helper should resolve");

        assert!(resolved.ends_with("scripts/sync_mail_imap.py"));
        assert!(std::path::Path::new(&resolved).is_absolute());
    }

    #[test]
    fn valid_account_produces_a_sync_command() {
        let mut state = state_with_accounts();
        state.focus_form();
        state.selected_field = MailConfigField::SyncAccount;

        match state.activate() {
            MailConfigAction::RunCommand { argv, .. } => {
                assert_eq!(argv[0], "python3");
                assert!(argv.contains(&"--account".to_string()));
                assert!(argv.contains(&"work".to_string()));
            }
            other => panic!("expected a sync command, got {other:?}"),
        }
    }

    #[test]
    fn local_provider_needs_no_credential() {
        let mut state = state_with_accounts();
        state.draft.provider = MailProvider::AppleMailLocal;

        assert_eq!(state.credential_state(), CredentialState::NotApplicable);
    }
}
