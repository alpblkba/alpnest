use std::path::Path;

use super::model::{MailFeed, MailFeedSlot, MailThread, nonempty};

pub const MAIL_FEED_CAPACITY: usize = 20;

pub fn build_mail_feed(threads: Vec<MailThread>, generated_mail_dir: &Path) -> MailFeed {
    let mut visible: Vec<MailThread> = threads
        .into_iter()
        .filter(|thread| !is_dismissed(thread))
        .collect();

    visible.retain(should_enter_main_feed);

    sort_threads(&mut visible);

    let slots = visible
        .into_iter()
        .take(MAIL_FEED_CAPACITY)
        .enumerate()
        .map(|(index, thread)| {
            let slot_id = format!("mail{index}");
            let detail_path = generated_mail_dir
                .join("feed")
                .join(format!("{slot_id}.md"));

            MailFeedSlot {
                index,
                slot_id,
                thread,
                detail_path,
            }
        })
        .collect();

    MailFeed {
        capacity: MAIL_FEED_CAPACITY,
        slots,
    }
}

pub fn should_enter_main_feed(thread: &MailThread) -> bool {
    if is_dismissed(thread) || is_obvious_noise(thread) {
        return false;
    }

    if thread.action_required || thread.has_deadline_or_date() {
        return true;
    }

    if thread.has_unread() {
        return true;
    }

    if is_meaningful_category(thread) {
        return true;
    }

    if thread.mails.iter().any(|mail| mail.has_fetched_body()) {
        return true;
    }

    true
}

pub fn sort_threads(threads: &mut [MailThread]) {
    threads.sort_by(|left, right| sort_key(right).cmp(&sort_key(left)));
}

pub fn is_dismissed(thread: &MailThread) -> bool {
    thread
        .status
        .as_deref()
        .is_some_and(|status| status.eq_ignore_ascii_case("dismissed"))
}

fn sort_key(thread: &MailThread) -> (u8, u8, u8, u8, i64, &str) {
    (
        bool_rank(thread.has_unread()),
        bool_rank(thread.action_required),
        bool_rank(thread.has_deadline_or_date()),
        bool_rank(!thread.noise_guess),
        thread.latest_sort_key,
        thread.id.as_str(),
    )
}

fn bool_rank(value: bool) -> u8 {
    u8::from(value)
}

fn is_meaningful_category(thread: &MailThread) -> bool {
    matches!(
        normalized_category(thread).as_str(),
        "academic"
            | "admin"
            | "assignment"
            | "exam"
            | "lab"
            | "meeting"
            | "opportunity"
            | "project"
            | "research"
            | "school"
            | "seminar"
            | "work"
    )
}

fn is_obvious_noise(thread: &MailThread) -> bool {
    if thread.noise_guess {
        return true;
    }

    if matches!(
        normalized_category(thread).as_str(),
        "advertising" | "marketing" | "newsletter" | "noise" | "promo" | "promotion" | "social"
    ) {
        return true;
    }

    let haystack = format!(
        "{} {} {} {}",
        thread.display_sender,
        thread.display_subject,
        thread.display_summary(),
        thread.category_guess.as_deref().unwrap_or("")
    )
    .to_ascii_lowercase();

    [
        "advertisement",
        "black friday",
        "deal",
        "discount",
        "marketing",
        "newsletter",
        "onboarding tasks",
        "promo",
        "promotion",
        "sale",
        "webinar",
        " 30% off",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

fn normalized_category(thread: &MailThread) -> String {
    nonempty(thread.category_guess.as_deref())
        .unwrap_or("unknown")
        .trim()
        .to_ascii_lowercase()
}
