# Sample files

These files are for trying Threadwell without your own data. They describe a fictional website relaunch.

| File                     | Use it for                                                                                                                     |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| `import-project-plan.md` | Settings → Import a Markdown file. Creates a page titled "Website Relaunch Plan" with headings, lists, checkboxes and a table. |
| `meeting-kickoff.txt`    | Meetings → Import file. A plain transcript with `[hh:mm:ss] Speaker: text` lines.                                              |
| `meeting-kickoff.vtt`    | Meetings → Import file. The same meeting as WebVTT cues. Use one or the other, not both.                                       |

## Suggested first session (about 15 minutes)

1. Create a workspace in an empty folder. Leave "Include the sample project" ticked if you want more examples.
2. Import `import-project-plan.md` from Settings. Open the page and make a small edit. Close the app and reopen it.
3. Meetings → Import file → choose `meeting-kickoff.txt`. Then choose **Extract notes and actions**.
   Check that each extracted line shows evidence timestamps, and that no due date appears unless the transcript states one.
4. Open the assistant (right side). With a model installed, ask: "What did we decide about the launch date?"
   The answer should cite the meeting or the plan. If it cites nothing, treat the answer as unverified.
5. Create a recipe under Recipes: manual schedule, instructions "Summarize the open tasks in three bullet points."
   Choose **Run now**, then review the draft it creates.

The assistant needs a local model. The Word guide explains setup. Without a model, every other feature still works.
