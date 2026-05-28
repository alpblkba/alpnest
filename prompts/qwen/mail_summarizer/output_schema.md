Return exactly one JSON object with this shape:

```json
{
  "display_sender": "string",
  "display_subject": "string",
  "summary": "string",
  "category": "school|admin|meeting|assignment|exam|lab|seminar|research|opportunity|newsletter|social|noise|unknown",
  "action_required": false,
  "action": null,
  "deadline": null,
  "date_or_time": null,
  "language": "tr|en|de|mixed|unknown",
  "noise": false,
  "needs_human_review": false,
  "confidence": 0.0
}
Field rules:

* display_sender: readable sender name. Do not invent an organization if unclear.
* display_subject: cleaned subject. Preserve important course/task names.
* summary: 1-2 short sentences. Must be faithful and action-preserving.
* category: choose exactly one allowed category.
* action_required: true only if the mail explicitly asks Alp to do something or clearly implies an action.
* action: string if action_required is true, otherwise null.
* deadline: explicit deadline only. If no explicit deadline exists, null.
* date_or_time: any explicit relevant date/time, even if it is not a deadline. Otherwise null.
* language: language of the original mail or mixed/unknown.
* noise: true for low-value newsletters, social updates, onboarding reminders, promotions, and obvious non-work distractions.
* needs_human_review: true if the message may matter but the payload is incomplete, ambiguous, long, official, or action-related.
* confidence: number from 0.0 to 1.0. Use lower confidence for metadata-only or ambiguous messages.

Never output extra keys.
Never output markdown.
Never output explanations.
