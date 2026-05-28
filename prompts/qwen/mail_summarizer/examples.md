Example 1: KIT ILIAS feedback, no immediate action

Input:
account: kit
sender: KIT-ILIAS <no-reply@ilias.kit.edu>
subject: [KIT-ILIAS] New feedback file for Homework 01
body_status: fetched
payload: A tutor uploaded a new feedback file for your submission in Low Power Design.

Output:
{
  "display_sender": "KIT-ILIAS",
  "display_subject": "New feedback file for Homework 01",
  "summary": "Low Power Design feedback for Homework 01 is available on ILIAS; no immediate action is required unless Alp wants to review it.",
  "category": "school",
  "action_required": false,
  "action": null,
  "deadline": null,
  "date_or_time": null,
  "language": "en",
  "noise": false,
  "needs_human_review": false,
  "confidence": 0.9
}

Example 2: explicit assignment deadline

Input:
account: kit
sender: Hardware Security Lab <tutor@kit.edu>
subject: Task 1 submission deadline
body_status: fetched
payload: Please submit Task 1 by Friday, 31 May, 23:59. Upload your solution to ILIAS.

Output:
{
  "display_sender": "Hardware Security Lab",
  "display_subject": "Task 1 submission deadline",
  "summary": "Hardware Security Lab Task 1 must be submitted on ILIAS by Friday, 31 May, 23:59.",
  "category": "lab",
  "action_required": true,
  "action": "Submit Task 1 to ILIAS.",
  "deadline": "Friday, 31 May, 23:59",
  "date_or_time": "Friday, 31 May, 23:59",
  "language": "en",
  "noise": false,
  "needs_human_review": false,
  "confidence": 0.96
}

Example 3: German school/admin mail, summarize in English

Input:
account: kit
sender: Studienbüro <studium@kit.edu>
subject: Änderung der Sprechstunde
body_status: fetched
payload: Die Sprechstunde am Mittwoch entfällt. Der Ersatztermin findet am Freitag um 10:00 Uhr statt.

Output:
{
  "display_sender": "Studienbüro",
  "display_subject": "Office hour change",
  "summary": "The Wednesday office hour is cancelled; the replacement appointment is on Friday at 10:00.",
  "category": "admin",
  "action_required": false,
  "action": null,
  "deadline": null,
  "date_or_time": "Friday 10:00",
  "language": "de",
  "noise": false,
  "needs_human_review": false,
  "confidence": 0.92
}

Example 4: Turkish personal mail, summarize in Turkish

Input:
account: gmail
sender: Ahmet <ahmet@example.com>
subject: Yarınki toplantı
body_status: fetched
payload: Yarın 15:00 yerine 16:00'da buluşalım mı? Benim önce başka bir işim çıktı.

Output:
{
  "display_sender": "Ahmet",
  "display_subject": "Yarınki toplantı",
  "summary": "Ahmet yarınki buluşmayı 15:00 yerine 16:00'ya almak istiyor.",
  "category": "meeting",
  "action_required": true,
  "action": "Yarınki buluşma saatinin 16:00 olup olmayacağını yanıtla.",
  "deadline": null,
  "date_or_time": "yarın 16:00",
  "language": "tr",
  "noise": false,
  "needs_human_review": false,
  "confidence": 0.93
}

Example 5: newsletter/noise

Input:
account: gmail
sender: Events <event.st.com>
subject: Webinar: secure device identity
body_status: fetched
payload: Join our upcoming webinar about device identity, authentication, and embedded security products.

Output:
{
  "display_sender": "Events",
  "display_subject": "Webinar: secure device identity",
  "summary": "This is a webinar invitation about device identity and embedded security products.",
  "category": "newsletter",
  "action_required": false,
  "action": null,
  "deadline": null,
  "date_or_time": null,
  "language": "en",
  "noise": true,
  "needs_human_review": false,
  "confidence": 0.86
}

Example 6: metadata-only possibly important school mail

Input:
account: kit
sender: KIT-ILIAS <no-reply@ilias.kit.edu>
subject: [KIT-ILIAS] Important instructions for Practical Introduction to Hardware Security
body_status: not_fetched
payload: metadata only; body not fetched

Output:
{
  "display_sender": "KIT-ILIAS",
  "display_subject": "Important instructions for Practical Introduction to Hardware Security",
  "summary": "Metadata-only KIT ILIAS message about important instructions for Practical Introduction to Hardware Security; the body should be reviewed before deciding on actions.",
  "category": "school",
  "action_required": false,
  "action": null,
  "deadline": null,
  "date_or_time": null,
  "language": "en",
  "noise": false,
  "needs_human_review": true,
  "confidence": 0.55
}

Example 7: German university mailing-list civic event announcement

Input:
account: kit
sender: Presse AStA KIT via students Mailing List <students@lists.kit.edu>
subject: [KIT-Student] Einladung: gemeinsames Basteln für die Demonstration gegen Kulturstreichungen
received_at: 2025-11-27 14:18
body_status: fetched
payload: Liebe Studis, die Stadt plant Kürzungen: keine Nextbikes mehr, die S-Bahn fährt nur noch halb so oft, Kultureinrichtungen müssen dicht machen, Kita-Gebühren steigen. Gleichzeitig ist aber Geld für Prestigeprojekte da. Gemeinsam mit dem Bündnis Solidarische Stadt ruft der AStA zur Demo am 06.12. um 12 Uhr am Kronenplatz auf. Damit ihr nicht unvorbereitet seid, könnt ihr am 02.12. von 18-20 Uhr im AStA-Container vorbeikommen und mit uns Demoschilder basteln. Material ist da.

Output:
{
  "display_sender": "Presse AStA KIT",
  "display_subject": "Invitation: sign-making for the demonstration against cultural budget cuts",
  "summary": "AStA invites students to a demonstration against planned city budget cuts on 06.12 at 12:00 at Kronenplatz, with a sign-making preparation session on 02.12 from 18:00-20:00 at the AStA-Container.",
  "category": "social",
  "action_required": false,
  "action": null,
  "deadline": null,
  "date_or_time": "02.12 18:00-20:00; 06.12 12:00",
  "language": "de",
  "noise": false,
  "needs_human_review": false,
  "confidence": 0.88
}
