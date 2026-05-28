Avoid these failure modes:

1. Generic summaries

Bad:
"This email is about school."

Better:
"KIT ILIAS announced that feedback for Homework 01 is available."

2. Invented action

Bad:
"Alp must submit Homework 01."

If the mail only says feedback was uploaded, there is no submission action.

3. Invented deadline

Bad:
"The assignment is due soon."

Only output a deadline when it is explicitly present.

4. Overconfident metadata-only summary

Bad:
"The email explains the full instructions for the lab."

If body_status is not_fetched, say the body was not reviewed and set needs_human_review if it may matter.

5. Wrong language

Bad:
Summarizing a German official mail in German.

German mails should be summarized in English.

6. Losing important details

Bad:
"There is a meeting update."

Better:
"The meeting is moved to Friday at 10:00."

7. Treating noise as action

Bad:
"Attend the webinar."

A promotional webinar invitation is usually newsletter/noise unless the mail explicitly requires action from Alp.

8. Adding project context not in the email

Bad:
"This relates to Alp's Hardware Security repo."

Only say that if the mail explicitly says it.

9. Treating optional civic/university events as mandatory tasks

Bad:
"Alp must attend the demonstration."

Better:
"The mail invites students to a demonstration on 06.12 at 12:00 at Kronenplatz and a sign-making preparation session on 02.12 from 18:00-20:00 at the AStA-Container."

10. Marking meaningful university mailing-list events as pure noise

Bad:
"This is mailing-list noise."

Better:
"This is an optional university/civic event announcement. It is not mandatory, but it contains concrete event details."

11. Losing secondary event details

Bad:
"Students are invited to a demonstration."

Better:
"Students are invited to the demonstration on 06.12 at 12:00 at Kronenplatz; there is also a sign-making session on 02.12 from 18:00-20:00 at the AStA-Container."
