Good summarization target: 9/10.

A 9/10 summary is:

- faithful: does not add information not present in the mail

- compact: readable inside a terminal dashboard

- action-preserving: keeps required actions visible

- date-preserving: keeps deadlines, dates, times, and locations visible

- context-preserving: keeps course, lab, assignment, exam, seminar, sender, and account context when relevant

- uncertainty-aware: says null/unknown or needs_human_review instead of guessing

- dashboard-friendly: helps Alp decide whether to inspect the mail later

A 5/10 summary is too generic:

- "This email is about homework."

- "This is a school message."

- "This is an announcement."

A 2/10 summary is dangerous:

- invents a deadline

- invents an action

- turns feedback into a new assignment

- marks a newsletter as urgent

- hides an explicit deadline

- hides a meeting time

- claims body content that was not provided

- treats metadata-only mail as fully understood

When in doubt:

- preserve visible facts

- avoid conclusions

- set needs_human_review to true

- reduce confidence


Confidence policy:

The confidence field is a rough self-assessment, not a calibrated probability.

Use high confidence only when:
- the body is fetched
- the relevant action/date/location facts are explicit
- the mail is short or structurally clear
- the language is understood clearly

Use lower confidence when:
- body_status is not_fetched
- the body is truncated
- the mail is long
- the mail is mixed-language
- action/deadline interpretation is ambiguous
- the message is official or school-related but lacks body content

Guidelines:
- metadata-only official/school mail should usually be 0.65 or lower
- clear fetched event announcements can be 0.80-0.90
- clear fetched deadline/action mails can be 0.90+
- long or politically/socially nuanced messages should not get extreme confidence unless all concrete details are preserved
