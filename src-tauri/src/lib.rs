pub mod codex;
pub mod config;
pub mod import;
pub mod switch;
pub mod usage;
use tauri::Manager;
pub mod adapter;
pub mod commands;
pub mod db;
pub mod fs;
pub mod model;
pub mod net;
pub mod permissions;
pub mod provider;
pub mod proxy;
pub mod workspace;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let store = workspace::Store::open(&data_dir).map_err(std::io::Error::other)?;
            let _ = config::pipeline::recover(&store);

            // Initialize and start proxy server
            let registry = store.proxy_registry.clone();
            if let Ok(workspace) = store.load() {
                if let Err(e) = registry.load_from_workspace(&workspace) {
                    eprintln!("代理注册表加载失败: {}", e);
                }
            }
            proxy::start_proxy_server(registry.clone());
            app.manage(registry);

            let binary = app
                .path()
                .resource_dir()?
                .join("resources/codex/cli-proxy-api");
            #[cfg(debug_assertions)]
            let binary = if binary.exists() {
                binary
            } else {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("resources/codex/cli-proxy-api")
            };
            app.manage(codex::CodexService::new(
                data_dir.join("codex-proxy"),
                binary,
            ));
            app.manage(store);
            codex::monitor(app.handle().clone());
            usage::commands::start_auto_scan(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            codex::codex_action,
            workspace::load_workspace,
            workspace::save_provider,
            workspace::copy_provider,
            workspace::delete_provider,
            model::naming::suggest_model_names,
            model::naming::apply_model_names,
            model::commands::save_model,
            model::commands::delete_model,
            model::commands::import_models,
            provider::fetch::fetch_models,
            config::path::set_target_path,
            provider::connection::test_model,
            provider::connection::latest_tests,
            provider::connection::validate_workspace,
            config::cli::detect_droid,
            config::pipeline::reload_configuration,
            config::pipeline::preview_configuration,
            config::pipeline::apply_configuration,
            config::pipeline::discard_draft,
            config::pipeline::export_draft,
            config::pipeline::resolve_recovery,
            config::reconcile::inspect_configuration,
            config::reconcile::resolve_binding,
            config::takeover::takeover_model,
            config::takeover::release_binding,
            config::backups::list_backups,
            config::backups::preview_restore,
            config::backups::restore_backup,
            config::backups::set_backup_retention,
            config::backups::prune_backups,
            config::backups::open_backup_directory,
            switch::catalog::discover_models,
            switch::scheme::save_scheme,
            switch::catalog::open_factory_login,
            import::mapping::preview_ccswitch,
            import::mapping::import_ccswitch,
            usage::pricing::save_price,
            usage::pricing::preview_usage_reprice,
            usage::pricing::apply_usage_reprice,
            usage::commands::save_usage_settings,
            usage::commands::scan_usage,
            usage::commands::load_usage,
            usage::commands::export_usage,
            usage::commands::clear_usage,
            usage::commands::delete_price,
            usage::commands::usage_prices,
            codex::quota::codex_quota
        ])
        .build(tauri::generate_context!())
        .expect("Droid Switch failed to start")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<codex::CodexService>().shutdown();
            }
        });
}
