pub mod config;
use tauri::Manager;
pub mod adapter;
mod commands;
pub mod db;
pub mod fs;
pub mod model;
pub mod net;
pub mod permissions;
pub mod provider;
pub mod workspace;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let store = workspace::Store::open(&data_dir).map_err(std::io::Error::other)?;
            let _ = config::pipeline::recover(&store);
            app.manage(store);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::read_text_file,
            commands::write_text_file,
            commands::http_request,
            workspace::load_workspace,
            workspace::save_provider,
            workspace::copy_provider,
            workspace::delete_provider,
            model::commands::save_model,
            model::commands::delete_model,
            model::commands::import_models,
            provider::fetch::fetch_models,
            config::path::set_target_path,
            provider::connection::test_model,
            config::cli::detect_droid,
            config::pipeline::reload_configuration,
            config::pipeline::preview_configuration,
            config::pipeline::apply_configuration,
            config::pipeline::discard_draft,
            config::pipeline::export_draft,
            config::pipeline::resolve_recovery,
            config::takeover::takeover_model,
            config::takeover::release_binding,
            config::backups::list_backups,
            config::backups::preview_restore,
            config::backups::restore_backup
        ])
        .run(tauri::generate_context!())
        .expect("Droid Switch failed to start");
}
