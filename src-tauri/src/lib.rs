mod company;
mod domain;
mod job;
mod note;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            company::list_companies,
            company::update_company_field,
            company::set_company_notes,
            company::create_company,
            company::set_company_status,
            domain::list_domains,
            job::list_jobs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
