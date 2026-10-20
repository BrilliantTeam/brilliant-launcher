pub mod auth;
pub mod boost;
pub mod importer;
pub mod commands;
pub mod config;
pub mod err;
pub mod extras;
pub mod fabric;
pub mod instance;
pub mod launch;
pub mod mc;
pub mod modinject;
pub mod modrinth;
pub mod mrpack;
pub mod net;
pub mod paths;
pub mod ping;
pub mod skins;
pub mod state;
pub mod worlds;

use tauri::{Listener, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    if std::path::Path::new("/proc/driver/nvidia").exists() && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| extras::reveal(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(state::AppState::default())
        .setup(|app| {
            net::set_concurrency(tauri::async_runtime::block_on(config::load_settings()).download_threads);
            extras::setup_tray(app)?;
            for dir in [paths::instances(), paths::shared(), paths::logs()] {
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    eprintln!("[setup] 建立 {} 失敗：{e}", dir.display());
                }
            }
            let handle = app.handle().clone();
            app.listen("launcher://exit", move |_| {
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::launcher_paths,
            commands::list_accounts,
            commands::login_microsoft,
            commands::add_offline_account,
            commands::remove_account,
            commands::select_account,
            commands::list_versions,
            commands::list_fabric_loaders,
            commands::list_java,
            commands::list_instances,
            commands::create_instance,
            commands::update_instance,
            commands::delete_instance,
            commands::duplicate_instance,
            commands::select_instance,
            commands::set_mod_enabled,
            commands::add_mods,
            commands::launch_game,
            commands::stop_game,
            commands::ping_server,
            commands::site_json,
            commands::content_search,
            commands::content_detail,
            commands::content_install,
            commands::content_list,
            commands::content_versions,
            commands::content_pin,
            commands::set_mod_profiles,
            mrpack::profile_export,
            mrpack::mrpack_import,
            worlds::list_worlds,
            worlds::world_export,
            worlds::world_copy,
            worlds::world_delete,
            worlds::world_import,
            worlds::pack_order,
            worlds::set_pack_order,
            commands::content_update,
            commands::content_remove,
            commands::launcher_info,
            commands::download_update,
            commands::apply_update,
            commands::open_path,
            commands::open_instance_folder,
            commands::hide_window,
            commands::show_window,
            commands::exit_launcher,
            extras::list_backups,
            extras::backup_now,
            extras::restore_backup,
            extras::delete_backup,
            extras::list_screenshots,
            extras::clean_cache,
            extras::set_media,
            commands::set_instance_image,
            skins::account_profile,
            skins::skin_library,
            skins::skin_add,
            skins::skin_save_current,
            skins::skin_remove,
            skins::skin_data,
            skins::skin_rename,
            skins::skin_apply,
            skins::skin_reset,
            skins::cape_set,
            skins::name_status,
            skins::name_change,
            skins::refresh_sessions,
            commands::prefetch,
            commands::repair_instance,
            commands::send_report,
            skins::share_screenshot,
            boost::system_info,
            boost::notify_user,
            importer::import_sources,
            importer::import_into,
            importer::import_inspect,
        ])
        .run(tauri::generate_context!())
        .expect("啟動 Tauri 失敗");
}
