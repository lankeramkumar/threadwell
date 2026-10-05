//! Threadwell desktop backend. The frontend talks only to the typed commands in
//! `commands`; storage, validation and file access live behind them.

mod ai;
mod attachments;
mod commands;
mod db;
mod documents;
mod error;
mod knowledge;
mod local_import;
mod markdown;
mod meetings;
mod recipes;
mod pages;
mod sample;
mod search;
mod tasks;
mod telemetry;
mod transfer;
mod util;
mod workspace;
mod workspaces;

use commands::AppState;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            telemetry::init(&config_dir);
            app.manage(AppState::new(config_dir));
            let state = app.state::<AppState>();
            recipes::start_scheduler(app.handle().clone(), state.active.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ai::commands::ai_get_status,
            ai::commands::ai_save_config,
            ai::commands::ai_chat_send,
            ai::commands::ai_page_action,
            ai::commands::ai_cancel,
            ai::commands::ai_list_conversations,
            ai::commands::ai_get_conversation,
            ai::commands::ai_list_runs,
            ai::commands::ai_run_trace,
            ai::commands::ai_list_proposals,
            ai::commands::ai_apply_proposal,
            ai::commands::ai_reject_proposal,
            ai::commands::ai_undo_proposal,
            ai::commands::ai_save_retrieval,
            ai::commands::ai_set_page_excluded,
            ai::commands::ai_index_status,
            ai::commands::ai_index_start,
            ai::history::ai_search_conversations,
            ai::history::ai_delete_run,
            telemetry::telemetry_get_settings,
            telemetry::telemetry_set_settings,
            telemetry::telemetry_delete_traces,
            attachments::attachments_list,
            attachments::attachment_add,
            attachments::attachment_remove,
            attachments::attachment_reveal,
            local_import::local_scan_folder,
            local_import::local_import_folder,
            meetings::meetings_list,
            meetings::meetings_get,
            meetings::meetings_import_text,
            meetings::meetings_import_file,
            meetings::meetings_import_audio,
            meetings::audio_settings_get,
            meetings::audio_settings_save,
            local_import::local_watched_folders,
            local_import::local_set_watched,
            local_import::local_sync_now,
            meetings::meetings_process,
            recipes::recipes_list,
            recipes::recipes_create,
            recipes::recipes_update,
            recipes::recipes_delete,
            recipes::recipe_runs_list,
            recipes::recipe_run_now,
            commands::app_status,
            commands::create_workspace,
            commands::open_workspace,
            workspaces::workspaces_list,
            workspaces::workspace_switch,
            workspaces::workspace_rename,
            workspaces::workspace_forget,
            commands::list_pages,
            commands::list_trash,
            commands::get_page,
            commands::create_page,
            commands::save_page,
            commands::move_page,
            commands::set_page_favorite,
            commands::trash_page,
            commands::restore_page,
            commands::backlinks,
            commands::list_projects,
            commands::create_project,
            commands::list_tasks,
            commands::create_task,
            commands::update_task,
            commands::delete_task,
            commands::search_workspace,
            commands::rebuild_search_index,
            commands::get_settings,
            commands::set_setting,
            commands::export_page_markdown,
            commands::export_all_markdown,
            commands::export_tasks_csv,
            commands::import_markdown,
            commands::create_backup,
            commands::restore_backup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Threadwell");
}
