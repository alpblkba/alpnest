Return exactly one JSON object with this shape:

```json
{
  "display_sender": "string",
  "display_subject": "string",
  "summary": "string",
  "category": "school|admin|assignment|exam|lab|seminar|research|project|work|career|application|meeting|event|calendar|security|finance|travel|github|tool|newsletter|promotion|shopping|social|noise|unknown",
  "attention": "overview|account_only|hidden",
  "importance": "high|medium|low",
  "action_required": false,
  "action": null,
  "deadline": null,
  "date_or_time": null,
  "retention_hint": "24h|3d|7d|until_deadline|keep|hidden",
  "source_language": "tr|en|de|mixed|unknown",
  "summary_language": "en",
  "noise": false,
  "needs_human_review": false,
  "confidence": 0.0
}
```

Field rules:

* display_sender: readable sender name. Do not invent an organization if unclear.
* display_subject: cleaned subject. Preserve important course/task names.
* summary: 1-2 short English sentences. Must be faithful and action-preserving.
* category: what the mail is about. Choose exactly one allowed category.
* attention: overview for main attention-feed mail, account_only for browsable account mail, hidden for noise/promotions/social/spam.
* importance: high for required action, security, deadlines, exams, and urgent account/work/school items; medium for relevant non-urgent mail; low for account-only or hidden mail.
* action_required: true only if the mail explicitly asks Alp to do something or clearly implies an action.
* action: string if action_required is true, otherwise null.
* deadline: explicit deadline only. If no explicit deadline exists, null.
* date_or_time: any explicit relevant date/time, even if it is not a deadline. Otherwise null.
* retention_hint: hidden for hidden mail; until_deadline for explicit deadlines; keep for durable important records; otherwise 24h, 3d, or 7d.
* source_language: language of the original mail or mixed/unknown.
* summary_language: always "en".
* noise: true for low-value newsletters, social updates, onboarding reminders, promotions, shopping mail, and obvious non-work distractions.
* needs_human_review: true if the message may matter but the payload is incomplete, ambiguous, long, official, or action-related.
* confidence: number from 0.0 to 1.0. Use lower confidence for metadata-only or ambiguous messages.

Never output extra keys.
Never output markdown.
Never output explanations.
