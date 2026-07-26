//! Mail account registry.
//!
//! Accounts are described in `$ALPNEST_HOME/config/mail/accounts.cfg`. That
//! file holds connection metadata only — hostnames, ports, mailbox names. It
//! never holds a password, an app password, or an OAuth token; those live in
//! the macOS Keychain and are looked up by account id at sync time. See
//! [`crate::mail::keychain`].

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::paths::AlpnestPaths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailProvider {
    Gmail,
    Microsoft,
    Icloud,
    Yahoo,
    GenericImap,
    AppleMailLocal,
}

impl MailProvider {
    pub const ALL: [MailProvider; 6] = [
        MailProvider::Gmail,
        MailProvider::Microsoft,
        MailProvider::Icloud,
        MailProvider::Yahoo,
        MailProvider::GenericImap,
        MailProvider::AppleMailLocal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Gmail => "Gmail",
            Self::Microsoft => "Microsoft 365 / Exchange Online",
            Self::Icloud => "iCloud Mail",
            Self::Yahoo => "Yahoo Mail",
            Self::GenericImap => "Generic IMAP",
            Self::AppleMailLocal => "Apple Mail (local, read-only)",
        }
    }

    pub fn config_value(self) -> &'static str {
        match self {
            Self::Gmail => "gmail",
            Self::Microsoft => "microsoft",
            Self::Icloud => "icloud",
            Self::Yahoo => "yahoo",
            Self::GenericImap => "imap",
            Self::AppleMailLocal => "apple_local",
        }
    }

    pub fn from_config(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "gmail" | "google" => Self::Gmail,
            "microsoft" | "outlook" | "office365" | "exchange" => Self::Microsoft,
            "icloud" | "apple" => Self::Icloud,
            "yahoo" => Self::Yahoo,
            "apple_local" | "local" => Self::AppleMailLocal,
            _ => Self::GenericImap,
        }
    }

    pub fn next(self) -> Self {
        let current = Self::ALL
            .iter()
            .position(|provider| *provider == self)
            .unwrap_or(0);

        Self::ALL[(current + 1) % Self::ALL.len()]
    }

    pub fn default_host(self) -> &'static str {
        match self {
            Self::Gmail => "imap.gmail.com",
            Self::Microsoft => "outlook.office365.com",
            Self::Icloud => "imap.mail.me.com",
            Self::Yahoo => "imap.mail.yahoo.com",
            Self::GenericImap | Self::AppleMailLocal => "",
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            Self::AppleMailLocal => 0,
            _ => 993,
        }
    }

    /// What the user has to put in the Keychain for this provider to work.
    pub fn credential_hint(self) -> &'static str {
        match self {
            Self::Gmail => "app password (Google account → 2FA → App passwords)",
            Self::Microsoft => "app password, or an OAuth2 token if your tenant blocks basic auth",
            Self::Icloud => "app-specific password (appleid.apple.com → Sign-In and Security)",
            Self::Yahoo => "app password (Yahoo account security)",
            Self::GenericImap => "IMAP password or app password for this server",
            Self::AppleMailLocal => "none — reads the local Mail.app store",
        }
    }

    pub fn needs_network(self) -> bool {
        self != Self::AppleMailLocal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImapSecurity {
    Ssl,
    StartTls,
}

impl ImapSecurity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ssl => "SSL/TLS",
            Self::StartTls => "STARTTLS",
        }
    }

    pub fn config_value(self) -> &'static str {
        match self {
            Self::Ssl => "ssl",
            Self::StartTls => "starttls",
        }
    }

    pub fn from_config(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "starttls" | "tls" => Self::StartTls,
            _ => Self::Ssl,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Ssl => Self::StartTls,
            Self::StartTls => Self::Ssl,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailAccount {
    pub id: String,
    pub display_name: String,
    pub provider: MailProvider,
    pub email: String,
    /// IMAP login name when it differs from the address. University servers
    /// commonly authenticate with an account short name (KIT's u-Kürzel)
    /// rather than the full address. Empty means "use the email".
    pub username: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub security: ImapSecurity,
    pub mailboxes: Vec<String>,
    /// How many messages the very first sync of a mailbox pulls.
    pub initial_limit: u32,
    /// Ceiling per later pass. The agent runs every minute, so this bounds how
    /// much work one tick can create for the summarizer.
    pub batch_limit: u32,
    pub enabled: bool,
}

impl Default for MailAccount {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            provider: MailProvider::Gmail,
            email: String::new(),
            username: String::new(),
            imap_host: MailProvider::Gmail.default_host().to_string(),
            imap_port: 993,
            security: ImapSecurity::Ssl,
            mailboxes: vec!["INBOX".to_string()],
            initial_limit: 20,
            batch_limit: 10,
            enabled: true,
        }
    }
}

impl MailAccount {
    pub fn new(id: impl Into<String>, provider: MailProvider) -> Self {
        let id = id.into();

        Self {
            display_name: id.clone(),
            id,
            provider,
            imap_host: provider.default_host().to_string(),
            imap_port: provider.default_port(),
            ..Self::default()
        }
    }

    /// Re-apply provider defaults to the host/port pair, used when the user
    /// cycles the provider on a fresh account.
    pub fn apply_provider_defaults(&mut self) {
        self.imap_host = self.provider.default_host().to_string();
        self.imap_port = self.provider.default_port();
    }

    /// The name to send in the IMAP LOGIN command.
    pub fn login_name(&self) -> &str {
        if self.username.trim().is_empty() {
            &self.email
        } else {
            &self.username
        }
    }

    pub fn mailboxes_display(&self) -> String {
        if self.mailboxes.is_empty() {
            "INBOX".to_string()
        } else {
            self.mailboxes.join(",")
        }
    }

    pub fn set_mailboxes_from_display(&mut self, value: &str) {
        self.mailboxes = value
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect();

        if self.mailboxes.is_empty() {
            self.mailboxes.push("INBOX".to_string());
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("account id cannot be empty".to_string());
        }

        if self.display_name.trim().is_empty() {
            return Err("display name cannot be empty".to_string());
        }

        if !self.provider.needs_network() {
            return Ok(());
        }

        if self.email.trim().is_empty() {
            return Err("email address cannot be empty".to_string());
        }

        if !self.email.contains('@') {
            return Err("email address must contain @".to_string());
        }

        if self.imap_host.trim().is_empty() {
            return Err("imap host cannot be empty".to_string());
        }

        if self.imap_port == 0 {
            return Err("imap port must be greater than zero".to_string());
        }

        Ok(())
    }

    fn to_config_block(&self) -> String {
        let mut block = format!("[account.{}]\n", self.id);
        block.push_str(&format!(
            "display_name = \"{}\"\n",
            escape(&self.display_name)
        ));
        block.push_str(&format!("provider = \"{}\"\n", self.provider.config_value()));
        block.push_str(&format!("email = \"{}\"\n", escape(&self.email)));
        block.push_str(&format!("username = \"{}\"\n", escape(&self.username)));
        block.push_str(&format!("imap_host = \"{}\"\n", escape(&self.imap_host)));
        block.push_str(&format!("imap_port = {}\n", self.imap_port));
        block.push_str(&format!("security = \"{}\"\n", self.security.config_value()));
        block.push_str(&format!(
            "mailboxes = \"{}\"\n",
            escape(&self.mailboxes_display())
        ));
        block.push_str(&format!("initial_limit = {}\n", self.initial_limit));
        block.push_str(&format!("batch_limit = {}\n", self.batch_limit));
        block.push_str(&format!("enabled = {}\n", self.enabled));
        block
    }
}

/// Which model writes the summaries.
///
/// The backend is selectable but the *behaviour* is fixed: every provider gets
/// the same prompt pack, is pinned to temperature 0, and must return the same
/// JSON schema, which Alpnest then normalises identically. Swapping models can
/// change how well a summary reads; it must never change the data shape or the
/// allowed triage vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummarizerProvider {
    Ollama,
    Anthropic,
    OpenAi,
}

impl SummarizerProvider {
    pub const ALL: [SummarizerProvider; 3] = [Self::Ollama, Self::Anthropic, Self::OpenAi];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ollama => "local (ollama)",
            Self::Anthropic => "Anthropic API",
            Self::OpenAi => "OpenAI API",
        }
    }

    pub fn config_value(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
        }
    }

    pub fn from_config(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "anthropic" => Self::Anthropic,
            "openai" => Self::OpenAi,
            _ => Self::Ollama,
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Self::Ollama => "qwen3:8b",
            Self::Anthropic => "claude-sonnet-5",
            Self::OpenAi => "gpt-4o-mini",
        }
    }

    pub fn needs_api_key(self) -> bool {
        self != Self::Ollama
    }

    pub fn keychain_account(self) -> String {
        format!("summarizer:{}", self.config_value())
    }

    pub fn next(self) -> Self {
        let current = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(current + 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummarizerSettings {
    pub provider: SummarizerProvider,
    pub model: String,
}

impl Default for SummarizerSettings {
    fn default() -> Self {
        Self {
            provider: SummarizerProvider::Ollama,
            model: SummarizerProvider::Ollama.default_model().to_string(),
        }
    }
}

impl SummarizerSettings {
    fn to_config_block(&self) -> String {
        format!(
            "[summarizer]\nprovider = \"{}\"\nmodel = \"{}\"\n",
            self.provider.config_value(),
            escape(&self.model)
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct MailAccountRegistry {
    pub accounts: Vec<MailAccount>,
    pub summarizer: SummarizerSettings,
}

impl MailAccountRegistry {
    pub fn config_path() -> io::Result<PathBuf> {
        let paths = AlpnestPaths::resolve()?;
        Ok(paths.config_dir.join("mail").join("accounts.cfg"))
    }

    pub fn load() -> io::Result<Self> {
        let path = Self::config_path()?;

        let Ok(raw) = fs::read_to_string(&path) else {
            return Ok(Self::default());
        };

        Ok(Self::parse(&raw))
    }

    pub fn parse(raw: &str) -> Self {
        let mut blocks: Vec<(String, BTreeMap<String, String>)> = Vec::new();
        let mut current: Option<(String, BTreeMap<String, String>)> = None;
        let mut summarizer = SummarizerSettings::default();
        let mut in_summarizer = false;

        for line in raw.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                if let Some(block) = current.take() {
                    blocks.push(block);
                }

                in_summarizer = header.trim() == "summarizer";

                if let Some(id) = header.strip_prefix("account.") {
                    current = Some((id.trim().to_string(), BTreeMap::new()));
                }

                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            let key = key.trim();
            let value = unquote(value.trim());

            if in_summarizer {
                match key {
                    "provider" => summarizer.provider = SummarizerProvider::from_config(value),
                    "model" => summarizer.model = value.to_string(),
                    _ => {}
                }

                continue;
            }

            if let Some((_, map)) = current.as_mut() {
                map.insert(key.to_string(), value.to_string());
            }
        }

        if summarizer.model.trim().is_empty() {
            summarizer.model = summarizer.provider.default_model().to_string();
        }

        if let Some(block) = current.take() {
            blocks.push(block);
        }

        let accounts = blocks
            .into_iter()
            .filter(|(id, _)| !id.is_empty())
            .map(|(id, map)| {
                let provider = map
                    .get("provider")
                    .map(|value| MailProvider::from_config(value))
                    .unwrap_or(MailProvider::GenericImap);

                let mut account = MailAccount {
                    display_name: map.get("display_name").cloned().unwrap_or_else(|| id.clone()),
                    id,
                    provider,
                    email: map.get("email").cloned().unwrap_or_default(),
                    username: map.get("username").cloned().unwrap_or_default(),
                    imap_host: map
                        .get("imap_host")
                        .cloned()
                        .unwrap_or_else(|| provider.default_host().to_string()),
                    imap_port: map
                        .get("imap_port")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_else(|| provider.default_port()),
                    security: map
                        .get("security")
                        .map(|value| ImapSecurity::from_config(value))
                        .unwrap_or(ImapSecurity::Ssl),
                    mailboxes: Vec::new(),
                    // `sync_limit` is the pre-split key; honour it as the
                    // initial size so older configs keep working.
                    initial_limit: map
                        .get("initial_limit")
                        .or_else(|| map.get("sync_limit"))
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(20),
                    batch_limit: map
                        .get("batch_limit")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(10),
                    enabled: map
                        .get("enabled")
                        .map(|value| matches!(value.as_str(), "true" | "yes" | "1"))
                        .unwrap_or(true),
                };

                account.set_mailboxes_from_display(
                    map.get("mailboxes").map(String::as_str).unwrap_or("INBOX"),
                );

                account
            })
            .collect();

        Self {
            accounts,
            summarizer,
        }
    }

    pub fn save(&self) -> io::Result<PathBuf> {
        let path = Self::config_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut raw = String::from(
            "# Alpnest mail accounts.\n\
             #\n\
             # Connection metadata only. Passwords, app passwords and OAuth tokens\n\
             # are stored in the macOS Keychain under the service \"alpnest-mail\"\n\
             # and are never written to this file.\n\
             schema_version = 1\n\n",
        );

        raw.push_str(&self.summarizer.to_config_block());
        raw.push('\n');

        for account in &self.accounts {
            raw.push_str(&account.to_config_block());
            raw.push('\n');
        }

        fs::write(&path, raw)?;
        Ok(path)
    }

    pub fn get(&self, index: usize) -> Option<&MailAccount> {
        self.accounts.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut MailAccount> {
        self.accounts.get_mut(index)
    }

    pub fn contains_id(&self, id: &str) -> bool {
        self.accounts.iter().any(|account| account.id == id)
    }

    /// Appends `-2`, `-3`, ... until the id is free.
    pub fn unique_id(&self, base: &str) -> String {
        let base = if base.trim().is_empty() {
            "account"
        } else {
            base
        };

        if !self.contains_id(base) {
            return base.to_string();
        }

        let mut suffix = 2;

        loop {
            let candidate = format!("{base}-{suffix}");

            if !self.contains_id(&candidate) {
                return candidate;
            }

            suffix += 1;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    pub fn len(&self) -> usize {
        self.accounts.len()
    }
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_account_blocks() {
        let raw = r#"
schema_version = 1

[account.work]
display_name = "Work"
provider = "microsoft"
email = "someone@example.com"
imap_port = 993
mailboxes = "INBOX,Archive"
enabled = true

[account.personal]
display_name = "Personal"
provider = "gmail"
email = "someone@gmail.com"
enabled = false
"#;

        let registry = MailAccountRegistry::parse(raw);

        assert_eq!(registry.len(), 2);
        assert_eq!(registry.accounts[0].id, "work");
        assert_eq!(registry.accounts[0].provider, MailProvider::Microsoft);
        assert_eq!(
            registry.accounts[0].mailboxes,
            vec!["INBOX".to_string(), "Archive".to_string()]
        );
        assert!(!registry.accounts[1].enabled);
        assert_eq!(registry.accounts[1].imap_host, "imap.gmail.com");
    }

    #[test]
    fn round_trips_through_config_text() {
        let mut account = MailAccount::new("work", MailProvider::Icloud);
        account.email = "someone@icloud.com".to_string();
        account.initial_limit = 50;
        account.set_mailboxes_from_display("INBOX, Sent");

        let registry = MailAccountRegistry {
            summarizer: SummarizerSettings::default(),
            accounts: vec![account.clone()],
        };

        let raw = registry
            .accounts
            .iter()
            .map(|account| account.to_config_block())
            .collect::<String>();

        let reparsed = MailAccountRegistry::parse(&raw);
        assert_eq!(reparsed.accounts, vec![account]);
    }

    #[test]
    fn unique_id_avoids_collisions() {
        let registry = MailAccountRegistry {
            summarizer: SummarizerSettings::default(),
            accounts: vec![
                MailAccount::new("work", MailProvider::Gmail),
                MailAccount::new("work-2", MailProvider::Gmail),
            ],
        };

        assert_eq!(registry.unique_id("work"), "work-3");
        assert_eq!(registry.unique_id("home"), "home");
    }

    #[test]
    fn validation_requires_network_fields_only_for_remote_providers() {
        let mut local = MailAccount::new("local", MailProvider::AppleMailLocal);
        local.email.clear();
        assert!(local.validate().is_ok());

        let mut remote = MailAccount::new("remote", MailProvider::Gmail);
        assert!(remote.validate().is_err());

        remote.email = "someone@gmail.com".to_string();
        assert!(remote.validate().is_ok());
    }

    #[test]
    fn provider_cycle_visits_every_provider() {
        let mut provider = MailProvider::Gmail;

        for _ in 0..MailProvider::ALL.len() {
            provider = provider.next();
        }

        assert_eq!(provider, MailProvider::Gmail);
    }

    #[test]
    fn saved_config_never_contains_secret_keys() {
        let mut account = MailAccount::new("work", MailProvider::Gmail);
        account.email = "someone@gmail.com".to_string();

        let block = account.to_config_block();

        assert!(!block.contains("password"));
        assert!(!block.contains("token"));
        assert!(!block.contains("secret"));
    }
}
