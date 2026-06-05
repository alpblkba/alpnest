use std::{fs, path::Path};

use color_eyre::{Result, eyre::eyre};

use super::{
    feed::{MAIL_FEED_CAPACITY, sort_threads},
    model::{FeedIndexEntry, Mail, MailFeed, MailFeedSlot, MailThread, nonempty},
};

pub fn render_mail_feed(
    feed: &MailFeed,
    all_threads: &[MailThread],
    generated_mail_dir: &Path,
) -> Result<()> {
    let feed_dir = generated_mail_dir.join("feed");
    fs::create_dir_all(&feed_dir).map_err(|error| {
        eyre!(
            "failed to create mail feed directory {}: {error}",
            feed_dir.display()
        )
    })?;

    for index in 0..MAIL_FEED_CAPACITY {
        let path = feed_dir.join(format!("mail{index}.md"));
        let text = feed
            .slots
            .iter()
            .find(|slot| slot.index == index)
            .map(render_slot_detail)
            .transpose()?
            .unwrap_or_else(render_empty_slot);
        fs::write(&path, text)
            .map_err(|error| eyre!("failed to write mail slot {}: {error}", path.display()))?;
    }

    let index_entries: Vec<FeedIndexEntry> = feed.slots.iter().map(index_entry).collect();
    let index_path = generated_mail_dir.join("feed_index.json");
    let index_json = serde_json::to_string_pretty(&index_entries)?;
    fs::write(&index_path, format!("{index_json}\n")).map_err(|error| {
        eyre!(
            "failed to write mail feed index {}: {error}",
            index_path.display()
        )
    })?;

    let overview_path = generated_mail_dir
        .parent()
        .unwrap_or(generated_mail_dir)
        .join("mail_feed.md");
    fs::write(&overview_path, render_compact_feed(feed)).map_err(|error| {
        eyre!(
            "failed to write compact mail feed {}: {error}",
            overview_path.display()
        )
    })?;

    let generated_dir = generated_mail_dir.parent().unwrap_or(generated_mail_dir);
    for account in ["kit", "gmail"] {
        render_account_projection(generated_mail_dir, all_threads, account)?;

        let account_path = generated_dir.join(format!("mail_{account}.md"));
        let account_slots = account_slots(all_threads, account);
        fs::write(
            &account_path,
            render_compact_account_feed(account, &account_slots),
        )
        .map_err(|error| {
            eyre!(
                "failed to write account mail feed {}: {error}",
                account_path.display()
            )
        })?;
    }

    Ok(())
}

#[derive(Clone, Debug)]
struct AccountMailSlot {
    index: usize,
    slot_id: String,
    thread: MailThread,
}

fn account_slots(all_threads: &[MailThread], account: &str) -> Vec<AccountMailSlot> {
    let mut visible: Vec<MailThread> = all_threads
        .iter()
        .filter(|thread| thread.account == account)
        .filter(|thread| account_thread_is_visible(thread))
        .cloned()
        .collect();
    sort_threads(&mut visible);

    visible
        .into_iter()
        .take(MAIL_FEED_CAPACITY)
        .enumerate()
        .map(|(index, thread)| AccountMailSlot {
            index,
            slot_id: format!("{account}{index}"),
            thread,
        })
        .collect()
}

fn account_thread_is_visible(thread: &MailThread) -> bool {
    let attention = thread
        .attention
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if attention == "hidden" {
        return false;
    }

    if thread.noise_guess {
        return false;
    }

    !matches!(
        normalized_category(thread.category_guess.as_deref()).as_str(),
        "noise" | "social" | "promotion" | "shopping"
    )
}

fn render_account_projection(
    generated_mail_dir: &Path,
    all_threads: &[MailThread],
    account: &str,
) -> Result<()> {
    let account_dir = generated_mail_dir.join(account);
    fs::create_dir_all(&account_dir).map_err(|error| {
        eyre!(
            "failed to create account mail directory {}: {error}",
            account_dir.display()
        )
    })?;

    let slots = account_slots(all_threads, account);
    for index in 0..MAIL_FEED_CAPACITY {
        let path = account_dir.join(format!("{account}{index}.md"));
        let text = slots
            .iter()
            .find(|slot| slot.index == index)
            .map(|slot| render_thread_detail(&slot.thread))
            .transpose()?
            .unwrap_or_else(render_empty_slot);
        fs::write(&path, text).map_err(|error| {
            eyre!(
                "failed to write account mail slot {}: {error}",
                path.display()
            )
        })?;
    }

    Ok(())
}

fn index_entry(slot: &MailFeedSlot) -> FeedIndexEntry {
    FeedIndexEntry {
        slot_id: slot.slot_id.clone(),
        index: slot.index,
        thread_id: slot.thread.id.clone(),
        message_ids: slot.thread.message_ids.clone(),
        account: slot.thread.account.clone(),
        sender: slot.thread.display_sender.clone(),
        subject: slot.thread.display_subject.clone(),
        latest_received_at: slot.thread.latest_received_at.clone(),
        unread_count: slot.thread.unread_count,
        detail_path: slot.detail_path.display().to_string(),
    }
}

fn render_slot_detail(slot: &MailFeedSlot) -> Result<String> {
    render_thread_detail(&slot.thread)
}

fn render_thread_detail(thread: &MailThread) -> Result<String> {
    let mut text = String::new();
    let latest = thread.mails.iter().find(|mail| {
        thread
            .latest_mail_id
            .as_deref()
            .is_some_and(|id| id == mail.id)
    });

    push_line(&mut text, format!("# {}", thread.display_subject));
    push_line(&mut text, "");
    push_line(
        &mut text,
        format!(
            "from: {}",
            latest
                .map(Mail::sender_or_unknown)
                .unwrap_or_else(|| thread.display_sender.clone())
        ),
    );
    if let Some(date) = nonempty(thread.latest_received_at.as_deref()) {
        push_line(&mut text, format!("date: {date}"));
    }
    push_line(&mut text, format!("account: {}", thread.account));
    if is_meaningful_category(thread.category_guess.as_deref()) {
        push_line(
            &mut text,
            format!("category: {}", thread.category_guess.as_deref().unwrap()),
        );
    }
    if let Some(deadline) = nonempty(thread.deadline.as_deref()) {
        push_line(&mut text, format!("deadline: {deadline}"));
    }
    if let Some(date_or_time) = nonempty(thread.date_or_time.as_deref()) {
        push_line(&mut text, format!("date/time: {date_or_time}"));
    }
    if let Some(action) = nonempty(thread.action.as_deref()) {
        push_line(&mut text, format!("action: {action}"));
    }
    push_line(&mut text, "");

    for (index, mail) in thread.mails.iter().enumerate() {
        let title = if thread.mails.len() == 1 {
            "".to_string()
        } else {
            format!("## message {}", index + 1)
        };
        render_message(&mut text, title.as_str(), mail)?;
    }

    if thread.mails.is_empty() {
        push_line(&mut text, "## messages");
        push_line(&mut text, "");
        push_line(&mut text, "No local messages were found for this thread.");
    }

    Ok(text)
}

fn render_message(text: &mut String, title: &str, mail: &Mail) -> Result<()> {
    if !title.is_empty() {
        push_line(text, title);
        push_line(text, "");
    }

    push_line(
        text,
        format!("from: {}", optional_or_dash(mail.sender.as_deref())),
    );
    if !mail.recipients.is_empty() {
        push_line(text, format!("to: {}", list_or_dash(&mail.recipients)));
    }
    if !mail.cc.is_empty() {
        push_line(text, format!("cc: {}", list_or_dash(&mail.cc)));
    }
    if !mail.bcc.is_empty() {
        push_line(text, format!("bcc: {}", list_or_dash(&mail.bcc)));
    }
    if let Some(date) = nonempty(mail.received_at.as_deref()) {
        push_line(text, format!("date: {date}"));
    }
    push_line(
        text,
        format!("read: {}", if mail.is_unread() { "no" } else { "yes" }),
    );
    push_line(text, "");

    if let Some(body_path) = &mail.body_path {
        match fs::read_to_string(body_path) {
            Ok(body) if !body.trim().is_empty() => push_line(text, body),
            Ok(_) => push_line(
                text,
                "body not fetched. run sync_mail_apple.py with --include-body to fetch it.",
            ),
            Err(_) => push_line(
                text,
                "body not fetched. run sync_mail_apple.py with --include-body to fetch it.",
            ),
        }
    } else {
        push_line(
            text,
            "body not fetched. run sync_mail_apple.py with --include-body to fetch it.",
        );
    }

    push_line(text, "");
    Ok(())
}

fn render_empty_slot() -> String {
    "# empty mail slot\n\nno mail assigned.\n".to_string()
}

fn render_compact_feed(feed: &MailFeed) -> String {
    render_compact_slots("# mail", feed.slots.iter())
}

fn render_compact_account_feed(account: &str, slots: &[AccountMailSlot]) -> String {
    let heading = match account {
        "kit" => "# KIT mail".to_string(),
        "gmail" => "# Gmail mail".to_string(),
        _ => format!("# {account} mail"),
    };
    render_compact_account_slots(heading.as_str(), slots.iter())
}

fn render_compact_slots<'a>(
    heading: &str,
    slots: impl Iterator<Item = &'a MailFeedSlot>,
) -> String {
    let mut text = String::new();
    push_line(&mut text, heading);
    push_line(&mut text, "");

    let mut wrote_any = false;
    for slot in slots {
        wrote_any = true;
        push_line(&mut text, compact_slot_line(slot));
        push_line(&mut text, compact_summary_line(&slot.thread));
        push_line(&mut text, "");
    }

    if !wrote_any {
        push_line(&mut text, "No active mail slots.");
    }

    text
}

fn compact_slot_line(slot: &MailFeedSlot) -> String {
    compact_projection_line(slot.slot_id.as_str(), &slot.thread, true)
}

fn render_compact_account_slots<'a>(
    heading: &str,
    slots: impl Iterator<Item = &'a AccountMailSlot>,
) -> String {
    let mut text = String::new();
    push_line(&mut text, heading);
    push_line(&mut text, "");

    let mut wrote_any = false;
    for slot in slots {
        wrote_any = true;
        push_line(
            &mut text,
            compact_projection_line(slot.slot_id.as_str(), &slot.thread, false),
        );
        push_line(&mut text, compact_summary_line(&slot.thread));
        push_line(&mut text, "");
    }

    if !wrote_any {
        push_line(&mut text, "No active mail slots.");
    }

    text
}

fn compact_projection_line(slot_id: &str, thread: &MailThread, overview: bool) -> String {
    let date = optional_or_dash(thread.latest_received_at.as_deref());
    let labels = compact_labels(thread, overview);
    let read_state = if thread.has_unread() {
        "unread"
    } else {
        "read"
    };
    let label_text = if labels.is_empty() {
        read_state.to_string()
    } else {
        format!("{}, {read_state}", labels.join(", "))
    };

    format!(
        "- {} · {}: {} ({label_text}) [{slot_id}]",
        date, thread.display_sender, thread.display_subject
    )
}

fn compact_labels(thread: &MailThread, overview: bool) -> Vec<String> {
    let mut labels = Vec::new();

    let category = display_category(thread);
    if category != "unknown" {
        labels.push(category);
    }

    if overview {
        if let Some(importance) = display_importance(thread) {
            labels.push(importance);
        }
    }

    labels
}

fn compact_summary_line(thread: &MailThread) -> String {
    format!("  {}", one_line_summary(thread.display_summary().as_str()))
}

fn one_line_summary(summary: &str) -> String {
    summary.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn push_line(text: &mut String, line: impl AsRef<str>) {
    text.push_str(line.as_ref());
    text.push('\n');
}

fn optional_or_dash(value: Option<&str>) -> &str {
    nonempty(value).unwrap_or("-")
}

fn list_or_dash(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}

fn is_meaningful_category(value: Option<&str>) -> bool {
    nonempty(value).is_some_and(|category| {
        !matches!(
            category,
            "unknown" | "uncategorized" | "noise" | "newsletter" | "social"
        )
    })
}

fn display_category(thread: &MailThread) -> String {
    let category = normalized_category(thread.category_guess.as_deref());
    if !matches!(category.as_str(), "" | "unknown" | "uncategorized") {
        return category;
    }

    infer_category(thread).unwrap_or("unknown").to_string()
}

fn display_importance(thread: &MailThread) -> Option<String> {
    let importance = thread.importance.as_deref()?.trim().to_ascii_lowercase();
    if matches!(importance.as_str(), "high" | "medium" | "low") {
        Some(importance)
    } else {
        None
    }
}

fn normalized_category(value: Option<&str>) -> String {
    match value
        .unwrap_or("unknown")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "uncategorized" => "unknown".to_string(),
        "opportunity" => "career".to_string(),
        "promo" => "promotion".to_string(),
        "academic" => "school".to_string(),
        value => value.to_string(),
    }
}

fn infer_category(thread: &MailThread) -> Option<&'static str> {
    let haystack = format!(
        "{}\n{}\n{}\n{}",
        thread.account,
        thread.display_sender,
        thread.display_subject,
        thread.display_summary()
    )
    .to_ascii_lowercase();

    if contains_any(&haystack, &["facebookmail.com", "reels", "memories"]) {
        return Some("social");
    }

    if contains_any(
        &haystack,
        &[
            "assignment",
            "homework",
            "hand-in",
            "feedback",
            "exercise",
            "task",
        ],
    ) {
        return Some("assignment");
    }

    if contains_any(
        &haystack,
        &[
            "kit.edu",
            "studium.kit.edu",
            "lists.kit.edu",
            "ilias",
            "course",
            "lecture",
            "seminar",
            "exam",
        ],
    ) {
        return Some("school");
    }

    if contains_any(
        &haystack,
        &[
            "verify",
            "verification",
            "password",
            "2fa",
            "two-factor",
            "two factor",
            "cloud account",
            "security event",
        ],
    ) {
        return Some("security");
    }

    if contains_any(
        &haystack,
        &[
            "application",
            "student assistant",
            "internship",
            "working student",
            "fraunhofer",
            "career",
            "recruiting",
        ],
    ) {
        return Some("application");
    }

    if contains_any(
        &haystack,
        &[
            "github.com",
            "sregym",
            "rfc",
            "pull request",
            "hls",
            "iot lab",
        ],
    ) {
        return Some("project");
    }

    if contains_any(
        &haystack,
        &["hackathon", "registration", "approval", "opportunity"],
    ) {
        return Some("event");
    }

    if contains_any(
        &haystack,
        &[
            "newsletter",
            "unsubscribe",
            "discount",
            "% off",
            "free shipping",
            "promotion",
            "marketing",
            "sale",
        ],
    ) {
        return Some("promotion");
    }

    None
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::filters::MailFilters;

    fn thread(account: &str, sender: &str, subject: &str, sort_key: i64) -> MailThread {
        MailThread {
            id: format!("{account}-{sender}-{subject}"),
            account: account.to_string(),
            display_sender: sender.to_string(),
            display_subject: subject.to_string(),
            normalized_subject: subject.to_string(),
            latest_sort_key: sort_key,
            mails: vec![Mail {
                account: Some(account.to_string()),
                sender: Some(sender.to_string()),
                subject: Some(subject.to_string()),
                ..Mail::default()
            }],
            ..MailThread::default()
        }
    }

    #[test]
    fn filtered_threads_do_not_enter_account_slots() {
        let filters = MailFilters::from_text("sender_contains = facebookmail.com");
        let threads = filters.visible_threads(vec![
            thread(
                "gmail",
                "Facebook <notification@facebookmail.com>",
                "Noise",
                2,
            ),
            thread("gmail", "Human <person@example.com>", "Useful", 1),
        ]);
        let slots = account_slots(&threads, "gmail");

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].thread.display_subject, "Useful");
        assert_eq!(slots[0].slot_id, "gmail0");
    }

    #[test]
    fn account_slots_include_account_only_threads() {
        let mut thread = thread(
            "gmail",
            "Useful <person@example.com>",
            "Readable but not urgent",
            1,
        );
        thread.attention = Some("account_only".to_string());
        let slots = account_slots(&[thread], "gmail");

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].slot_id, "gmail0");
    }

    #[test]
    fn account_slots_include_overview_threads() {
        let mut thread = thread(
            "gmail",
            "Security <security@example.com>",
            "Verify your account",
            1,
        );
        thread.attention = Some("overview".to_string());
        let slots = account_slots(&[thread], "gmail");

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].slot_id, "gmail0");
    }

    #[test]
    fn account_slots_exclude_hidden_threads() {
        let mut hidden = thread("gmail", "Noise <noise@example.com>", "Hidden promo", 2);
        hidden.attention = Some("hidden".to_string());
        hidden.category_guess = Some("promotion".to_string());

        let visible = thread("gmail", "Human <person@example.com>", "Readable", 1);
        let slots = account_slots(&[hidden, visible], "gmail");

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].thread.display_subject, "Readable");
    }

    #[test]
    fn hidden_newsletter_promotion_does_not_render_in_account_feed() {
        let mut hidden = thread("gmail", "Brain.fm", "5 days left.", 2);
        hidden.attention = Some("hidden".to_string());
        hidden.category_guess = Some("newsletter".to_string());

        let mut visible = thread("gmail", "Human <person@example.com>", "Readable", 1);
        visible.attention = Some("account_only".to_string());

        let slots = account_slots(&[hidden, visible], "gmail");
        let text = render_compact_account_slots("# Gmail mail", slots.iter());

        assert!(!text.contains("Brain.fm"));
        assert!(!text.contains("newsletter"));
        assert!(text.contains("Readable"));
    }

    #[test]
    fn compact_rows_do_not_show_uncategorized() {
        let mut thread = thread("gmail", "Sender", "Plain note", 1);
        thread.category_guess = Some("uncategorized".to_string());
        let line = compact_projection_line("gmail0", &thread, false);

        assert!(!line.contains("uncategorized"));
        assert!(line.contains("(read)"));
    }
}
