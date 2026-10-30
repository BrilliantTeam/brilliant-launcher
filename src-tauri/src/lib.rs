pub mod auth;
pub mod boost;
pub mod importer;
pub mod commands;
pub mod config;
pub mod err;
pub mod extras;
pub mod fabric;
pub mod forge;
pub mod instance;
pub mod launch;
pub mod leftovers;
pub mod logfiles;
pub mod mc;
pub mod modinject;
pub mod modrinth;
pub mod mrpack;
pub mod curseforge;
pub mod net;
pub mod paths;
pub mod ping;
pub mod skins;
pub mod state;
pub mod worlds;
pub mod advanced;
pub mod insight;
pub mod trust;
pub mod templates;
pub mod gameopts;
pub mod gamewin;
pub mod snapshots;
pub mod diskuse;

use tauri::{Listener, Manager};

fn install_panic_log() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let place = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let message = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let line = format!("[{}] v{} {} thread={thread} at {place}: {message}\n", config::now_secs(), env!("CARGO_PKG_VERSION"), std::env::consts::OS);
        let _ = std::fs::create_dir_all(paths::logs());
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(paths::logs().join("panic.log")) {
            use std::io::Write;
            let _ = f.write_all(line.as_bytes());
        }
        default(info);
    }));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_panic_log();
    #[cfg(target_os = "linux")]
    if std::path::Path::new("/proc/driver/nvidia").exists() && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _| {
            extras::reveal(app);
            advanced::handle_cli(app, &argv);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(state::AppState::default())
        .setup(|app| {
            let startup = tauri::async_runtime::block_on(config::load_settings());
            net::set_concurrency(startup.download_threads);
            if let Err(e) = net::set_proxy(&startup.proxy) {
                eprintln!("[setup] 代理設定無效，改用直接連線：{:#}", e.0);
            }
            advanced::handle_cli(app.handle(), &std::env::args().collect::<Vec<_>>());
            extras::setup_tray(app)?;
            std::thread::spawn(commands::cleanup_updates);
            std::thread::spawn(|| paths::purge_trash(paths::TRASH_DAYS));
            tauri::async_runtime::spawn(launch::adopt(app.handle().clone()));
            if let Err(e) = app.asset_protocol_scope().allow_directory(paths::root(), true) {
                eprintln!("[setup] 開放資料夾給介面讀取失敗：{e}");
            }
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
            commands::cancel_login,
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
            commands::add_content,
            commands::create_shortcut,
            commands::export_diagnostics,
            commands::export_config,
            commands::import_config,
            commands::launch_game,
            commands::stop_game,
            commands::ping_server,
            commands::site_json,
            commands::content_search,
            commands::content_detail,
            commands::content_install,
            commands::content_list,
            commands::content_versions,
            commands::content_categories,
            commands::content_icons,
            commands::set_content_source,
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
            commands::update_platform,
            commands::net_status,
            commands::check_update,
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
            extras::remove_media,
            extras::keep_image,
            templates::list_templates,
            templates::save_template,
            templates::create_from_template,
            templates::delete_template,
            gamewin::list_screens,
            gamewin::forget_window,
            snapshots::list_mod_snapshots,
            snapshots::restore_mod_snapshot,
            diskuse::disk_usage,
            diskuse::clean_disk,
            extras::import_image,
            extras::save_image,
            extras::export_look,
            extras::inspect_look,
            extras::install_look,
            extras::read_svg,
            extras::save_svg,
            leftovers::mod_leftovers,
            leftovers::remove_leftovers,
            logfiles::log_files,
            logfiles::read_log,
            logfiles::delete_logs,
            logfiles::upload_log,
            advanced::shared_folder_path,
            advanced::apply_shared,
            advanced::shared_status,
            advanced::shared_items,
            advanced::shared_add,
            advanced::shared_remove,
            advanced::shared_discard,
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
            commands::prefetch_upgrade,
            commands::report_post,
            skins::share_screenshot,
            boost::system_info,
            boost::notify_user,
            importer::import_sources,
            importer::import_into,
            importer::import_inspect,
            insight::crash_context,
            insight::collect_report,
            insight::mod_risks,
            insight::perf_report,
            insight::apply_perf,
            insight::play_history,
            insight::list_all_screenshots,
            insight::delete_screenshot,
            insight::save_png,
            insight::save_shot_thumb,
            extras::dir_stamp,
        ])
        .run(tauri::generate_context!())
        .expect("啟動 Tauri 失敗");
}
