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
    if is_dismissed(thread) {
        return false;
    }

    match normalized_attention(thread).as_deref() {
        Some("hidden" | "account_only") => return false,
        Some("overview") => return true,
        _ => {}
    }

    if is_noise_category(thread) {
        return false;
    }

    if thread.action_required || thread.has_deadline_or_date() {
        return true;
    }

    if has_security_or_account_signal(thread) {
        return true;
    }

    if has_school_signal(thread)
        || has_work_or_application_signal(thread)
        || has_project_signal(thread)
    {
        return true;
    }

    if is_meaningful_category(thread) && has_actionable_context(thread) {
        return true;
    }

    false
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

fn normalized_attention(thread: &MailThread) -> Option<String> {
    nonempty(thread.attention.as_deref()).map(|attention| attention.trim().to_ascii_lowercase())
}

fn is_meaningful_category(thread: &MailThread) -> bool {
    matches!(
        normalized_category(thread).as_str(),
        "academic"
            | "admin"
            | "assignment"
            | "application"
            | "calendar"
            | "career"
            | "exam"
            | "finance"
            | "github"
            | "lab"
            | "meeting"
            | "project"
            | "research"
            | "school"
            | "security"
            | "seminar"
            | "work"
    )
}

fn is_noise_category(thread: &MailThread) -> bool {
    if thread.noise_guess {
        return true;
    }

    matches!(
        normalized_category(thread).as_str(),
        "advertising"
            | "marketing"
            | "newsletter"
            | "noise"
            | "promo"
            | "promotion"
            | "shopping"
            | "social"
    )
}

fn has_actionable_context(thread: &MailThread) -> bool {
    has_school_signal(thread)
        || has_work_or_application_signal(thread)
        || has_project_signal(thread)
        || has_security_or_account_signal(thread)
        || haystack_contains_any(thread, ACTIONABLE_TERMS)
}

fn has_school_signal(thread: &MailThread) -> bool {
    thread.account == "kit"
        || haystack_contains_any(thread, SCHOOL_TERMS)
        || thread
            .participants
            .iter()
            .any(|participant| contains_any(participant, SCHOOL_TERMS))
}

fn has_work_or_application_signal(thread: &MailThread) -> bool {
    matches!(normalized_category(thread).as_str(), "research" | "work")
        || haystack_contains_any(thread, WORK_TERMS)
}

fn has_project_signal(thread: &MailThread) -> bool {
    let haystack = thread_haystack(thread);

    contains_any(&haystack, PROJECT_DOMAIN_TERMS)
        || (contains_any(&haystack, &["github.com", "notifications@github.com"])
            && contains_any(&haystack, GITHUB_WORK_TERMS))
}

fn has_security_or_account_signal(thread: &MailThread) -> bool {
    haystack_contains_any(thread, SECURITY_TERMS)
}

fn haystack_contains_any(thread: &MailThread, needles: &[&str]) -> bool {
    contains_any(thread_haystack(thread).as_str(), needles)
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let value = value.to_ascii_lowercase();
    needles.iter().any(|needle| value.contains(needle))
}

fn thread_haystack(thread: &MailThread) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        thread.account,
        thread.display_sender,
        thread.display_subject,
        thread.normalized_subject,
        thread.display_summary(),
        thread.category_guess.as_deref().unwrap_or("")
    )
    .to_ascii_lowercase()
}

fn normalized_category(thread: &MailThread) -> String {
    match nonempty(thread.category_guess.as_deref())
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

const SCHOOL_TERMS: &[&str] = &[
    "kit.edu",
    "studium.kit.edu",
    "lists.kit.edu",
    "ilias",
    "kit-ilias",
    "course",
    "lecture",
    "task",
    "homework",
    "assignment",
    "hand-in",
    "feedback",
    "exercise",
    "seminar",
    "exam",
    "praktikum",
    "hiwi",
    "chair",
    "tutor",
    "professor",
    "supervisor",
];

const WORK_TERMS: &[&str] = &[
    "application",
    "interview",
    "student assistant",
    "working student",
    "internship",
    "recruiting",
    "career",
    "fraunhofer",
    "job",
    "jobs",
    "position",
    "rejection",
    "accepted",
    "approval",
    "registration",
    "hackathon",
    "research",
];

const PROJECT_DOMAIN_TERMS: &[&str] = &[
    "github.com",
    "notifications@github.com",
    "sregym",
    "rfc",
    "hls",
    "iot lab",
    "hardware security",
];

const GITHUB_WORK_TERMS: &[&str] = &[
    "pull request",
    " pr ",
    "issue",
    "comment",
    "mentioned",
    "review",
];

const SECURITY_TERMS: &[&str] = &[
    "verify",
    "verification",
    "security",
    "account",
    "password",
    "2fa",
    "two-factor",
    "two factor",
    "login",
    "sign in",
    "cloud account",
];

const ACTIONABLE_TERMS: &[&str] = &[
    "deadline",
    "due",
    "appointment",
    "meeting",
    "register",
    "registration",
    "approval",
    "submit",
    "upload",
    "payment",
    "invoice",
    "check-in",
    "boarding",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn thread(sender: &str, subject: &str) -> MailThread {
        MailThread {
            id: format!("{sender}-{subject}"),
            account: "gmail".to_string(),
            display_sender: sender.to_string(),
            display_subject: subject.to_string(),
            normalized_subject: subject.to_string(),
            latest_sort_key: 1,
            ..MailThread::default()
        }
    }

    #[test]
    fn deterministic_noise_examples_do_not_enter_overview() {
        let mut promo = thread("Shop <news@example.com>", "Weekly newsletter");
        promo.category_guess = Some("newsletter".to_string());
        assert!(!should_enter_main_feed(&promo));

        let mut social = thread("Facebook <notification@facebookmail.com>", "Friend update");
        social.noise_guess = true;
        assert!(!should_enter_main_feed(&social));
    }

    #[test]
    fn positive_school_and_work_examples_enter_overview() {
        let kit = thread(
            "KIT-ILIAS <noreply-ilias@studium.kit.edu>",
            "[KIT-ILIAS] Hand-in Task 1",
        );
        assert!(should_enter_main_feed(&kit));

        let supervisor = thread("Simon Pankner <simon.pankner@kit.edu>", "Lab feedback");
        assert!(should_enter_main_feed(&supervisor));

        let job = thread(
            "Fraunhofer <recruiting@example.com>",
            "Your Application - Student Assistant",
        );
        assert!(should_enter_main_feed(&job));
    }

    #[test]
    fn github_security_enters_overview() {
        let github = thread(
            "GitHub <noreply@github.com>",
            "[GitHub] Please verify your email address",
        );

        assert!(should_enter_main_feed(&github));
    }

    #[test]
    fn explicit_attention_overview_enters_overview() {
        let mut thread = thread("Sender", "Plain old subject");
        thread.attention = Some("overview".to_string());

        assert!(should_enter_main_feed(&thread));
    }

    #[test]
    fn explicit_attention_account_only_does_not_enter_overview() {
        let mut thread = thread("KIT <person@kit.edu>", "Course announcement");
        thread.attention = Some("account_only".to_string());

        assert!(!should_enter_main_feed(&thread));
    }

    #[test]
    fn explicit_attention_hidden_does_not_enter_overview() {
        let mut thread = thread("KIT <person@kit.edu>", "Course announcement");
        thread.attention = Some("hidden".to_string());

        assert!(!should_enter_main_feed(&thread));
    }

    #[test]
    fn unknown_without_strong_signal_does_not_enter_overview() {
        let unknown = thread("Brain.fm", "5 days left.");
        assert!(!should_enter_main_feed(&unknown));
    }

    #[test]
    fn opportunity_category_alone_does_not_enter_overview() {
        let mut commercial_opportunity = thread(
            "Tobi from Kittl",
            "Your first design is closer than you think",
        );
        commercial_opportunity.category_guess = Some("opportunity".to_string());

        assert!(!should_enter_main_feed(&commercial_opportunity));
    }
}
