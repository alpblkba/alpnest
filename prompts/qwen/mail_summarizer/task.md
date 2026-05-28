For each email or mail event stream, produce a faithful JSON digest.

You must:

1. Clean the sender into a human-readable display_sender.
2. Clean the subject into a human-readable display_subject.
3. Summarize the message in 1-2 short sentences.
4. Preserve explicit actions.
5. Preserve explicit deadlines, dates, times, locations, course names, assignment names, exam names, lab names, and seminar names.
6. Classify the message into one category.
7. Detect obvious noise such as newsletters, social updates, onboarding reminders, and promotional messages.
8. Mark needs_human_review when the message may matter but the available payload is insufficient.
9. Avoid adding assumptions.
10. Return only the JSON object.

Language policy:

- If the original mail is Turkish, summarize in Turkish.
- If the original mail is English, summarize in English.
- If the original mail is German, summarize in English.
- If the original mail is mixed or unclear, summarize in English.

Metadata-only policy:

If the body is not fetched and only sender/subject/metadata are available:
- do not pretend to know the full content
- create a limited summary from the visible metadata
- set needs_human_review to true if the message may be school/admin/action-related
- set confidence lower than usual

Long-mail policy:

If the mail contains multiple topics or appears too long to safely compress:
- preserve the most action-relevant facts
- set needs_human_review to true
- do not hide deadlines or tasks behind a generic summary

Optional event / civic announcement policy:

Some university mailing-list messages announce optional events, demonstrations, workshops, political/civic actions, or social activities.

For these messages:
- do not mark action_required true unless the mail explicitly requires Alp to respond, register, submit, or attend
- preserve concrete event dates, times, locations, and preparation sessions
- category should usually be admin, opportunity, social, or unknown depending on content
- noise should be false if the event is institutionally relevant or culturally/socially meaningful
- needs_human_review can be false if the event details are clear and no obligation is implied
- summary should clearly say it is an invitation/announcement, not an obligation

German and bilingual mail policy:

- German mails must be summarized in English.
- Preserve German organization names, locations, course names, and event names when useful.
- Translate the meaning, not proper nouns.
- If a German mail contains concrete dates/times/locations, preserve them exactly.
- If a mail contains German and English versions of the same content, summarize the repeated content once in English.
- If the German and English sections contain different facts, preserve unique actions, dates, times, and locations from both sections.
- For mixed or bilingual mail, set language to "mixed".
- If unsure whether bilingual sections are duplicates or contain unique facts, set needs_human_review to true.
