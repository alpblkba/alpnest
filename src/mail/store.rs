use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

use chrono::{NaiveDateTime, TimeZone, Utc};
use color_eyre::{Result, eyre::eyre};

use super::model::{EventStream, Mail, MailThread, nonempty};

#[derive(Clone, Debug)]
pub struct MailStore {
    pub data_home: PathBuf,
    pub messages: Vec<Mail>,
    pub eventstreams: Vec<EventStream>,
}

impl MailStore {
    pub fn load_default() -> Result<Self> {
        Self::load(alpnest_data_home())
    }

    pub fn load(data_home: PathBuf) -> Result<Self> {
        let store_dir = data_home.join("store");
        let messages_path = store_dir.join("messages.json");
        let eventstreams_path = store_dir.join("eventstreams.json");

        Ok(Self {
            data_home,
            messages: read_json_list(&messages_path, "mail messages")?,
            eventstreams: read_json_list(&eventstreams_path, "mail event streams")?,
        })
    }

    pub fn build_threads(&self) -> Vec<MailThread> {
        let messages_by_id = deduplicated_messages_by_original_id(&self.messages);

        self.eventstreams
            .iter()
            .filter_map(|stream| thread_from_stream(stream, &messages_by_id))
            .collect()
    }
}

pub fn alpnest_data_home() -> PathBuf {
    if let Ok(value) = env::var("ALPNEST_DATA_HOME") {
        return PathBuf::from(value);
    }

    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share/alpnest")
}

fn read_json_list<T>(path: &Path, label: &str) -> Result<Vec<T>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    if !path.exists() {
        return Err(eyre!(
            "missing {label} store: {}. Run the mail sync script first, or set ALPNEST_DATA_HOME to the alpnest data directory.",
            path.display()
        ));
    }

    let raw = fs::read_to_string(path).map_err(|error| {
        eyre!(
            "failed to read {label} store at {}: {error}",
            path.display()
        )
    })?;

    serde_json::from_str(&raw).map_err(|error| {
        eyre!(
            "failed to parse {label} store at {}: {error}",
            path.display()
        )
    })
}

fn thread_from_stream(
    stream: &EventStream,
    messages_by_id: &HashMap<String, Mail>,
) -> Option<MailThread> {
    let mut mails: Vec<Mail> = Vec::new();

    for id in &stream.message_ids {
        let Some(mail) = messages_by_id.get(id) else {
            continue;
        };

        if !mails.iter().any(|existing| existing.id == mail.id) {
            mails.push(mail.clone());
        }
    }

    if stream.id.is_empty() && mails.is_empty() {
        return None;
    }

    let latest = latest_mail(&mails);
    let account = nonempty(stream.account.as_deref())
        .map(str::to_string)
        .or_else(|| latest.map(Mail::account_or_unknown))
        .unwrap_or_else(|| "unknown".to_string());

    let display_sender = nonempty(stream.display_sender.as_deref())
        .map(str::to_string)
        .or_else(|| latest.map(Mail::sender_or_unknown))
        .or_else(|| nonempty(stream.sender_key.as_deref()).map(str::to_string))
        .unwrap_or_else(|| "unknown sender".to_string());

    let display_subject = nonempty(stream.display_subject.as_deref())
        .map(str::to_string)
        .or_else(|| latest.map(Mail::subject_or_unknown))
        .or_else(|| nonempty(stream.subject_key.as_deref()).map(str::to_string))
        .unwrap_or_else(|| "unknown subject".to_string());

    let normalized_subject = latest
        .map(Mail::normalized_subject_or_subject)
        .or_else(|| nonempty(stream.subject_key.as_deref()).map(str::to_string))
        .unwrap_or_else(|| display_subject.clone());

    let latest_received_at = latest
        .and_then(|mail| mail.received_at.clone())
        .or_else(|| stream.latest_at.clone());
    let latest_sort_key = latest_received_at
        .as_deref()
        .and_then(parse_mail_time)
        .unwrap_or_default();
    let latest_mail_id = latest.map(|mail| mail.id.clone());
    let participants = participants(&mails);
    let message_ids = mails.iter().map(|mail| mail.id.clone()).collect();
    let unread_count = mails.iter().filter(|mail| mail.is_unread()).count();

    Some(MailThread {
        id: if stream.id.is_empty() {
            fallback_thread_id(&account, &display_sender, &display_subject)
        } else {
            stream.id.clone()
        },
        account,
        display_sender,
        display_subject,
        normalized_subject,
        participants,
        message_ids,
        mails,
        latest_mail_id,
        latest_received_at,
        latest_sort_key,
        unread_count,
        category_guess: stream.category_guess.clone(),
        noise_guess: stream.noise_guess.unwrap_or_else(|| {
            stream
                .category_guess
                .as_deref()
                .is_some_and(|value| value == "noise")
        }),
        action_required: stream.action_required.unwrap_or(false),
        action: stream.action.clone(),
        deadline: stream.deadline.clone(),
        date_or_time: stream.date_or_time.clone(),
        attention: stream
            .attention
            .clone()
            .or_else(|| stream.attention_guess.clone()),
        importance: stream
            .importance
            .clone()
            .or_else(|| stream.importance_guess.clone()),
        retention_hint: stream.retention_hint.clone(),
        source_language: stream.source_language.clone(),
        summary_language: stream.summary_language.clone(),
        summary_local: stream.summary_local.clone(),
        summary: stream.summary.clone(),
        summary_source: stream.summary_source.clone(),
        summary_confidence: stream.summary_confidence,
        status: stream.status.clone(),
    })
}

fn deduplicated_messages_by_original_id(messages: &[Mail]) -> HashMap<String, Mail> {
    let mut best_by_key: HashMap<String, (usize, Mail)> = HashMap::new();

    for (index, mail) in messages.iter().enumerate() {
        let key = stable_message_key(mail);
        let should_replace = best_by_key
            .get(&key)
            .map(|(best_index, best_mail)| mail_is_better(mail, index, best_mail, *best_index))
            .unwrap_or(true);

        if should_replace {
            best_by_key.insert(key, (index, mail.clone()));
        }
    }

    let mut messages_by_original_id = HashMap::new();
    for mail in messages {
        if mail.id.is_empty() {
            continue;
        }

        if let Some((_, best)) = best_by_key.get(&stable_message_key(mail)) {
            messages_by_original_id.insert(mail.id.clone(), best.clone());
        }
    }

    messages_by_original_id
}

fn stable_message_key(mail: &Mail) -> String {
    match (
        nonempty(mail.apple_mail_id.as_deref()),
        nonempty(mail.account.as_deref()),
        nonempty(mail.mailbox.as_deref()),
    ) {
        (Some(apple_mail_id), Some(account), Some(mailbox)) => {
            format!("apple:{account}:{mailbox}:{apple_mail_id}")
        }
        _ if !mail.id.is_empty() => format!("id:{}", mail.id),
        _ => format!(
            "fallback:{}:{}:{}",
            nonempty(mail.sender.as_deref()).unwrap_or(""),
            nonempty(mail.subject.as_deref()).unwrap_or(""),
            nonempty(mail.received_at.as_deref()).unwrap_or("")
        ),
    }
}

fn mail_is_better(
    candidate: &Mail,
    candidate_index: usize,
    current: &Mail,
    current_index: usize,
) -> bool {
    (
        candidate.has_fetched_body(),
        candidate.body_length.unwrap_or_default(),
        candidate_index,
    ) > (
        current.has_fetched_body(),
        current.body_length.unwrap_or_default(),
        current_index,
    )
}

fn latest_mail(mails: &[Mail]) -> Option<&Mail> {
    mails.iter().max_by_key(|mail| {
        (
            mail.received_at
                .as_deref()
                .and_then(parse_mail_time)
                .unwrap_or_default(),
            mail.has_fetched_body(),
            mail.body_length.unwrap_or_default(),
        )
    })
}

fn participants(mails: &[Mail]) -> Vec<String> {
    let mut values = Vec::new();

    for mail in mails {
        push_unique(&mut values, mail.sender.as_deref());
        for recipient in &mail.recipients {
            push_unique(&mut values, Some(recipient));
        }
        for recipient in &mail.cc {
            push_unique(&mut values, Some(recipient));
        }
        for recipient in &mail.bcc {
            push_unique(&mut values, Some(recipient));
        }
    }

    values
}

fn push_unique(values: &mut Vec<String>, candidate: Option<&str>) {
    let Some(candidate) = nonempty(candidate) else {
        return;
    };

    if !values.iter().any(|value| value == candidate) {
        values.push(candidate.to_string());
    }
}

fn fallback_thread_id(account: &str, sender: &str, subject: &str) -> String {
    format!(
        "stream_{}_{}_{}",
        compact_key(account),
        compact_key(sender),
        compact_key(subject)
    )
}

fn compact_key(value: &str) -> String {
    let mut key = String::new();
    let mut last_dash = false;

    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            key.push(ch);
            last_dash = false;
        } else if !last_dash {
            key.push('-');
            last_dash = true;
        }
    }

    key.trim_matches('-').to_string()
}

fn parse_mail_time(value: &str) -> Option<i64> {
    let raw = value.trim();
    if raw.is_empty() {
        return None;
    }

    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(parsed.timestamp());
    }

    let cleaned = raw.replace(" at ", " ");
    for format in ["%A, %d. %B %Y %H:%M:%S", "%d. %B %Y %H:%M:%S"] {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(&cleaned, format) {
            return Utc
                .from_local_datetime(&parsed)
                .single()
                .map(|dt| dt.timestamp());
        }
    }

    None
}
