//! Sample workspace content. It is clearly labelled as sample material and includes
//! related notes, a conflicting pair of decisions, tasks, and a pasted transcript so
//! that later milestones (grounded answers, proposals, meeting extraction) have
//! realistic material to work against.

use rusqlite::Connection;

use crate::error::AppResult;
use crate::markdown::{self, PAGE_LINK_PREFIX};
use crate::pages;
use crate::tasks::{self, NewTask};

pub fn seed(conn: &Connection, ws: &str) -> AppResult<()> {
    let welcome = pages::create(conn, ws, "Welcome to Threadwell (sample)", None)?;
    pages::set_favorite(conn, ws, &welcome.id, true)?;

    let atlas = pages::create(conn, ws, "Atlas Mobile", None)?;
    let auth = pages::create(conn, ws, "Authentication decisions", Some(&atlas.id))?;
    let planning = pages::create(conn, ws, "Planning notes", Some(&atlas.id))?;
    let kickoff = pages::create(conn, ws, "Meeting: Atlas kickoff transcript", Some(&atlas.id))?;

    let link = |id: &str| format!("{PAGE_LINK_PREFIX}{id}");

    let auth_md = format!(
        "## Decisions\n\n\
         - **2026-03-03 (kickoff):** Use magic-link sign-in for every account.\n\
         - **2026-04-12 (security review):** Replace magic links with passkeys, keep email codes as a fallback.\n\n\
         > Note: the two entries disagree about the primary sign-in method. The April entry does not say it supersedes March.\n\n\
         Related: [Planning notes]({}) and [kickoff transcript]({}).\n",
        link(&planning.id),
        link(&kickoff.id),
    );
    pages::update(conn, ws, &auth.id, "Authentication decisions", &markdown::from_markdown(&auth_md), auth.revision)?;

    let planning_md = "## Planning\n\n\
        - Offline editing is required for the beta.\n\
        - Sync is out of scope until after launch.\n\
        - The beta date has not been decided. Confirm it with the product lead.\n\
        - Onboarding needs a short checklist for first-time workspace setup.\n";
    pages::update(conn, ws, &planning.id, "Planning notes", &markdown::from_markdown(planning_md), planning.revision)?;

    let transcript_md = "## Transcript (pasted sample)\n\n\
        [00:00:12] Ana: Thanks for joining. Goal today is to agree on sign-in for the beta.\n\n\
        [00:02:40] Ben: I still prefer magic links. They are simple and nobody forgets a password.\n\n\
        [00:05:31] Ana: Security wants passkeys. We should check whether the fallback is acceptable.\n\n\
        [00:09:05] Chris: Let's write up the passkey fallback spec. Ana, can you own it?\n\n\
        [00:09:44] Ana: Yes. I'll have a draft before the next review.\n\n\
        [00:11:02] Ben: Who decides the beta date? We never settled it.\n\n\
        [00:11:20] Chris: Not today. Ask the product lead.\n";
    pages::update(conn, ws, &kickoff.id, "Meeting: Atlas kickoff transcript", &markdown::from_markdown(transcript_md), kickoff.revision)?;

    let overview_md = format!(
        "# Atlas Mobile\n\nSample project. Start with [Authentication decisions]({}), then the [planning notes]({}).\n",
        link(&auth.id),
        link(&planning.id),
    );
    let atlas_doc = markdown::from_markdown(&overview_md);
    pages::update(conn, ws, &atlas.id, "Atlas Mobile", &atlas_doc, atlas.revision)?;

    let welcome_md = "# Welcome to Threadwell\n\n\
        This is a sample workspace. Everything here is ordinary data you can edit or delete.\n\n\
        - Your notes are saved in this workspace folder on this computer.\n\
        - Use Ctrl+K to open pages and views, and type / in a page for block commands.\n\
        - Tasks can be viewed as a table or a board, and exported as CSV.\n\
        - AI assistance is not part of this build yet.\n";
    pages::update(conn, ws, &welcome.id, "Welcome to Threadwell (sample)", &markdown::from_markdown(welcome_md), welcome.revision)?;

    let project = tasks::create_project(conn, ws, "Atlas Mobile")?;
    tasks::create_task(
        conn,
        ws,
        NewTask {
            title: "Write passkey fallback spec".into(),
            description: Some("Cover email-code fallback and account recovery. Source: kickoff transcript.".into()),
            status: Some("doing".into()),
            priority: Some("high".into()),
            project_id: Some(project.id.clone()),
            source_page_id: Some(kickoff.id.clone()),
            ..Default::default()
        },
    )?;
    tasks::create_task(
        conn,
        ws,
        NewTask {
            title: "Confirm beta date with product lead".into(),
            description: Some("No deadline is set in any note. Leave the due date empty until confirmed.".into()),
            project_id: Some(project.id.clone()),
            source_page_id: Some(planning.id.clone()),
            ..Default::default()
        },
    )?;
    tasks::create_task(
        conn,
        ws,
        NewTask {
            title: "Draft onboarding checklist".into(),
            priority: Some("low".into()),
            project_id: Some(project.id.clone()),
            source_page_id: Some(planning.id.clone()),
            ..Default::default()
        },
    )?;
    tasks::create_task(
        conn,
        ws,
        NewTask {
            title: "Share kickoff notes with the team".into(),
            status: Some("done".into()),
            project_id: Some(project.id),
            source_page_id: Some(kickoff.id),
            ..Default::default()
        },
    )?;

    let personal = tasks::create_project(conn, ws, "Personal")?;
    tasks::create_task(
        conn,
        ws,
        NewTask {
            title: "Try a keyboard-only workflow for a day".into(),
            project_id: Some(personal.id),
            ..Default::default()
        },
    )?;
    Ok(())
}
