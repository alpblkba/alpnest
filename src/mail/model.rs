use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Stable mail identity lives here. Generated files such as `mail0.md` are
/// disposable feed projections and must not be treated as long-term identity.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Mail {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub account: Option<String>,
    #[serde(default)]
    pub account_name: Option<String>,
    #[serde(default)]
    pub mailbox: Option<String>,
    #[serde(default)]
    pub mailbox_name: Option<String>,
    #[serde(default)]
    pub apple_mail_id: Option<String>,
    #[serde(default)]
    pub sender: Option<String>,
    #[serde(default)]
    pub sender_key: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub subject_key: Option<String>,
    #[serde(default)]
    pub normalized_subject: Option<String>,
    #[serde(default)]
    pub received_at: Option<String>,
    #[serde(default)]
    pub is_read: Option<bool>,
    #[serde(default)]
    pub unread: Option<bool>,
    #[serde(default)]
    pub recipients: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    #[serde(default)]
    pub snippet: Option<String>,
    #[serde(default)]
    pub body_path: Option<PathBuf>,
    #[serde(default)]
    pub body_length: Option<u64>,
    #[serde(default)]
    pub body_truncated: Option<bool>,
    #[serde(default)]
    pub body_status: Option<String>,
    #[serde(default)]
    pub body_sync_policy: Option<String>,
    #[serde(default)]
    pub payload_hash: Option<String>,
    #[serde(default)]
    pub has_attachment: Option<bool>,
    #[serde(default)]
    pub attachments: Vec<Value>,
}

impl Mail {
    pub fn account_or_unknown(&self) -> String {
        nonempty(self.account.as_deref())
            .unwrap_or("unknown")
            .to_string()
    }

    pub fn sender_or_unknown(&self) -> String {
        nonempty(self.sender.as_deref())
            .or_else(|| nonempty(self.sender_key.as_deref()))
            .unwrap_or("unknown sender")
            .to_string()
    }

    pub fn subject_or_unknown(&self) -> String {
        nonempty(self.subject.as_deref())
            .or_else(|| nonempty(self.normalized_subject.as_deref()))
            .or_else(|| nonempty(self.subject_key.as_deref()))
            .unwrap_or("unknown subject")
            .to_string()
    }

    pub fn normalized_subject_or_subject(&self) -> String {
        nonempty(self.normalized_subject.as_deref())
            .or_else(|| nonempty(self.subject.as_deref()))
            .or_else(|| nonempty(self.subject_key.as_deref()))
            .unwrap_or("unknown subject")
            .to_string()
    }

    pub fn is_unread(&self) -> bool {
        self.unread.unwrap_or_else(|| !self.is_read.unwrap_or(true))
    }

    pub fn has_fetched_body(&self) -> bool {
        self.body_path.is_some()
            || self
                .body_status
                .as_deref()
                .is_some_and(|status| status == "fetched")
    }
}

/// Stable thread identity lives in `MailThread.id`; slots point at threads.
#[derive(Clone, Debug, Default)]
pub struct MailThread {
    pub id: String,
    pub account: String,
    pub display_sender: String,
    pub display_subject: String,
    pub normalized_subject: String,
    pub participants: Vec<String>,
    pub message_ids: Vec<String>,
    pub mails: Vec<Mail>,
    pub latest_mail_id: Option<String>,
    pub latest_received_at: Option<String>,
    pub latest_sort_key: i64,
    pub unread_count: usize,
    pub category_guess: Option<String>,
    pub noise_guess: bool,
    pub action_required: bool,
    pub action: Option<String>,
    pub deadline: Option<String>,
    pub date_or_time: Option<String>,
    pub summary_local: Option<String>,
    pub summary: Option<String>,
    pub summary_source: Option<String>,
    pub summary_confidence: Option<f64>,
    pub status: Option<String>,
}

impl MailThread {
    pub fn has_unread(&self) -> bool {
        self.unread_count > 0
    }

    pub fn has_deadline_or_date(&self) -> bool {
        has_text(self.deadline.as_deref()) || has_text(self.date_or_time.as_deref())
    }

    pub fn display_summary(&self) -> String {
        nonempty(self.summary_local.as_deref())
            .or_else(|| nonempty(self.summary.as_deref()))
            .unwrap_or("No readable summary yet.")
            .to_string()
    }
}

/// A current projection slot. `mail0` can point at a different thread after the
/// next refresh; resolve through `feed_index.json` instead of storing slot IDs.
#[derive(Clone, Debug)]
pub struct MailFeedSlot {
    pub index: usize,
    pub slot_id: String,
    pub thread: MailThread,
    pub detail_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct MailFeed {
    pub capacity: usize,
    pub slots: Vec<MailFeedSlot>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct EventStream {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account: Option<String>,
    #[serde(default)]
    pub sender_key: Option<String>,
    #[serde(default)]
    pub subject_key: Option<String>,
    #[serde(default)]
    pub message_ids: Vec<String>,
    #[serde(default)]
    pub latest_at: Option<String>,
    #[serde(default)]
    pub category_guess: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub display_sender: Option<String>,
    #[serde(default)]
    pub display_subject: Option<String>,
    #[serde(default)]
    pub summary_local: Option<String>,
    #[serde(default)]
    pub action_required: Option<bool>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub deadline: Option<String>,
    #[serde(default)]
    pub date_or_time: Option<String>,
    #[serde(default)]
    pub noise_guess: Option<bool>,
    #[serde(default)]
    pub summary_source: Option<String>,
    #[serde(default)]
    pub summary_confidence: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FeedIndexEntry {
    pub slot_id: String,
    pub index: usize,
    pub thread_id: String,
    pub message_ids: Vec<String>,
    pub account: String,
    pub sender: String,
    pub subject: String,
    pub latest_received_at: Option<String>,
    pub unread_count: usize,
    pub detail_path: String,
}

pub fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

pub fn has_text(value: Option<&str>) -> bool {
    nonempty(value).is_some()
}
