//! Threadwell desktop backend. The frontend talks only to the typed commands in
//! `commands`; storage, validation and file access live behind them.

mod commands;
mod db;
mod error;
mod markdown;
mod pages;
mod sample;
mod search;
mod tasks;
mod transfer;
mod util;
mod workspace;

use commands::AppState;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            app.manage(AppState::new(config_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_status,
            commands::create_workspace,
            commands::open_workspace,
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
