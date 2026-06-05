use std::{
    fs,
    io::{self, Read},
    path::Path,
};

use super::model::{Mail, MailThread, nonempty};

const DEFAULT_FILTER_PATH: &str = "scripts/mail_filters.cfg";
const BODY_MATCH_LIMIT_BYTES: u64 = 16 * 1024;

#[derive(Clone, Debug, Default)]
pub struct MailFilters {
    rules: Vec<FilterRule>,
}

#[derive(Clone, Debug)]
struct FilterRule {
    kind: FilterKind,
    needle: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilterKind {
    Sender,
    Subject,
    Body,
    Summary,
    Category,
    Account,
}

impl MailFilters {
    pub fn load_default() -> Self {
        Self::load(DEFAULT_FILTER_PATH)
    }

    pub fn load(path: impl AsRef<Path>) -> Self {
        fs::read_to_string(path)
            .map(|text| Self::from_text(&text))
            .unwrap_or_default()
    }

    pub fn from_text(text: &str) -> Self {
        let mut rules = Vec::new();
        let mut legacy_section: Option<&str> = None;
        let mut legacy_patterns_section: Option<&str> = None;

        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                legacy_section = Some(line.trim_matches(&['[', ']'][..]));
                legacy_patterns_section = None;
                continue;
            }

            if raw_line.starts_with(char::is_whitespace) {
                if let Some(kind) = legacy_patterns_section.and_then(legacy_section_kind) {
                    push_rule(&mut rules, kind, line);
                }
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();

            if key == "patterns" {
                legacy_patterns_section = legacy_section;
                if let Some(kind) = legacy_section.and_then(legacy_section_kind) {
                    for pattern in value.lines().map(str::trim) {
                        push_rule(&mut rules, kind, pattern);
                    }
                }
                continue;
            }

            legacy_patterns_section = None;

            let kind = match key.as_str() {
                "sender_contains" => Some(FilterKind::Sender),
                "subject_contains" => Some(FilterKind::Subject),
                "body_contains" => Some(FilterKind::Body),
                "summary_contains" => Some(FilterKind::Summary),
                "category_contains" => Some(FilterKind::Category),
                "account" | "account_contains" => Some(FilterKind::Account),
                _ => None,
            };

            if let Some(kind) = kind {
                push_rule(&mut rules, kind, value);
            }
        }

        Self { rules }
    }

    pub fn matches_thread(&self, thread: &MailThread) -> bool {
        self.rules.iter().any(|rule| {
            rule.matches_thread(thread) || thread.mails.iter().any(|mail| rule.matches_mail(mail))
        })
    }

    pub fn visible_threads(&self, threads: Vec<MailThread>) -> Vec<MailThread> {
        threads
            .into_iter()
            .filter(|thread| !self.matches_thread(thread))
            .collect()
    }
}

impl FilterRule {
    fn matches_thread(&self, thread: &MailThread) -> bool {
        match self.kind {
            FilterKind::Sender => contains(&thread.display_sender, &self.needle),
            FilterKind::Subject => {
                contains(&thread.display_subject, &self.needle)
                    || contains(&thread.normalized_subject, &self.needle)
            }
            FilterKind::Body => false,
            FilterKind::Summary => contains(&thread.display_summary(), &self.needle),
            FilterKind::Category => {
                contains_optional(thread.category_guess.as_deref(), &self.needle)
            }
            FilterKind::Account => contains(&thread.account, &self.needle),
        }
    }

    fn matches_mail(&self, mail: &Mail) -> bool {
        match self.kind {
            FilterKind::Sender => {
                contains_optional(mail.sender.as_deref(), &self.needle)
                    || contains_optional(mail.sender_key.as_deref(), &self.needle)
            }
            FilterKind::Subject => {
                contains_optional(mail.subject.as_deref(), &self.needle)
                    || contains_optional(mail.normalized_subject.as_deref(), &self.needle)
                    || contains_optional(mail.subject_key.as_deref(), &self.needle)
            }
            FilterKind::Body => {
                contains_optional(mail.snippet.as_deref(), &self.needle)
                    || mail
                        .body_path
                        .as_deref()
                        .and_then(read_body_prefix)
                        .is_some_and(|body| contains(&body, &self.needle))
            }
            FilterKind::Summary => false,
            FilterKind::Category => false,
            FilterKind::Account => {
                contains_optional(mail.account.as_deref(), &self.needle)
                    || contains_optional(mail.account_name.as_deref(), &self.needle)
            }
        }
    }
}

fn legacy_section_kind(section: &str) -> Option<FilterKind> {
    match section {
        "ignore_senders" => Some(FilterKind::Sender),
        "ignore_subjects" => Some(FilterKind::Subject),
        "ignore_bodies" => Some(FilterKind::Body),
        "ignore_summaries" => Some(FilterKind::Summary),
        "ignore_categories" => Some(FilterKind::Category),
        "ignore_accounts" => Some(FilterKind::Account),
        _ => None,
    }
}

fn push_rule(rules: &mut Vec<FilterRule>, kind: FilterKind, value: &str) {
    let needle = value.trim().to_ascii_lowercase();
    if !needle.is_empty() {
        rules.push(FilterRule { kind, needle });
    }
}

fn contains_optional(value: Option<&str>, needle: &str) -> bool {
    nonempty(value).is_some_and(|value| contains(value, needle))
}

fn contains(value: &str, needle: &str) -> bool {
    value.to_ascii_lowercase().contains(needle)
}

fn read_body_prefix(path: &Path) -> Option<String> {
    read_body_prefix_result(path).ok()
}

fn read_body_prefix_result(path: &Path) -> io::Result<String> {
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(BODY_MATCH_LIMIT_BYTES).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::feed::build_mail_feed;

    fn thread(sender: &str, subject: &str) -> MailThread {
        MailThread {
            id: format!("{sender}-{subject}"),
            account: "gmail".to_string(),
            display_sender: sender.to_string(),
            display_subject: subject.to_string(),
            normalized_subject: subject.to_string(),
            latest_sort_key: 1,
            mails: vec![Mail {
                sender: Some(sender.to_string()),
                subject: Some(subject.to_string()),
                ..Mail::default()
            }],
            ..MailThread::default()
        }
    }

    #[test]
    fn sender_contains_hides_matching_thread() {
        let filters = MailFilters::from_text("sender_contains = facebookmail.com");
        assert!(filters.matches_thread(&thread(
            "Facebook Anilar <memories@facebookmail.com>",
            "Memories"
        )));
    }

    #[test]
    fn subject_contains_hides_matching_thread() {
        let filters = MailFilters::from_text("subject_contains = % off");
        assert!(filters.matches_thread(&thread(
            "Pegasus <pegasus@example.com>",
            "Your exclusive 40% off"
        )));
    }

    #[test]
    fn body_contains_hides_matching_body_file() {
        let path = std::env::temp_dir().join(format!(
            "alpnest-filter-body-{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, "View in browser\n\nunsubscribe from this list").unwrap();

        let mut thread = thread("Sender", "Subject");
        thread.mails[0].body_path = Some(path);
        let filters = MailFilters::from_text("body_contains = unsubscribe");

        assert!(filters.matches_thread(&thread));
    }

    #[test]
    fn missing_filter_file_matches_nothing() {
        let filters = MailFilters::load(std::env::temp_dir().join(format!(
            "missing-alpnest-mail-filters-{}.cfg",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));

        assert!(!filters.matches_thread(&thread("Sender", "Subject")));
    }

    #[test]
    fn filtered_threads_do_not_enter_main_feed() {
        let filters = MailFilters::from_text("sender_contains = facebookmail.com");
        let threads = filters.visible_threads(vec![thread(
            "Facebook Anilar <memories@facebookmail.com>",
            "Memories",
        )]);
        let feed = build_mail_feed(threads, &std::env::temp_dir());

        assert!(feed.slots.is_empty());
    }

    #[test]
    fn hard_filter_beats_attention_overview() {
        let filters = MailFilters::from_text("sender_contains = facebookmail.com");
        let mut thread = thread(
            "Facebook Anilar <memories@facebookmail.com>",
            "Security alert",
        );
        thread.attention = Some("overview".to_string());

        assert!(filters.visible_threads(vec![thread]).is_empty());
    }

    #[test]
    fn legacy_ignore_sender_section_is_supported() {
        let filters = MailFilters::from_text(
            "[ignore_senders]\npatterns =\n    facebookmail.com\n\n[school_hints]\npatterns =\n    kit.edu\n",
        );

        assert!(filters.matches_thread(&thread(
            "Facebook Anilar <memories@facebookmail.com>",
            "Memories"
        )));
        assert!(!filters.matches_thread(&thread("Tutor <person@kit.edu>", "Lab")));
    }
}
