mod check;
mod company;
mod competency;
mod config;
mod domain;
mod fit;
mod job;
mod llm;
mod metro;
mod note;
mod pipeline;
mod profile;
mod prompts;
mod sanitize;
mod scraper;
mod secrets;
mod watcher;
mod worker;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // The durable pipeline queue lives in the app data dir (never the vault).
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let queue = pipeline::queue::SqliteQueue::open(&dir.join("queue.db"))?;
            app.manage(worker::PipelineState {
                queue: std::sync::Arc::new(queue),
                cancelled: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            });
            // The vault file-watcher is started by the frontend (it owns the vault path) via
            // `start_vault_watcher`; park an empty slot for its handle here.
            app.manage(watcher::WatcherState::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            company::list_companies,
            company::update_company_field,
            company::set_company_list_field,
            company::set_company_notes,
            company::create_company,
            company::set_company_status,
            domain::list_domains,
            job::list_jobs,
            check::list_checks,
            check::get_check,
            secrets::set_secret,
            secrets::secret_present,
            config::get_config,
            config::set_config,
            worker::fetch_jobs_for_company,
            worker::cancel_run,
            watcher::start_vault_watcher
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
