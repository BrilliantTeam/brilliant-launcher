use crate::config::{now_secs, Account, Settings};
use crate::err::R;
use crate::instance::Instance;
use crate::mc::{assets, java, libs, rules::Features, version};
use crate::net::{self, DlFile};
use crate::state::{AppState, RunningGame};
use crate::{modinject, paths};
use crate::bail;
use anyhow::Context;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};

const LAUNCHER_NAME: &str = "BrilliantLauncher";
const CLASSPATH_SEP: &str = if cfg!(target_os = "windows") { ";" } else { ":" };
const DEFAULT_PORT: u16 = 25565;

#[derive(Serialize, Clone)]
pub struct LogLine {
    pub instance: String,
    pub line: String,
    pub stderr: bool,
}

#[derive(Serialize, Clone)]
pub struct ExitInfo {
    pub instance: String,
    pub code: i32,
    pub seconds: i64,
}

fn subst(template: &str, vars: &BTreeMap<&str, String>) -> String {
    let mut out = template.to_string();
    for (key, value) in vars {
        let needle = format!("${{{key}}}");
        if out.contains(&needle) {
            out = out.replace(&needle, value);
        }
    }
    out
}

fn expand(items: &[Value], features: &Features, vars: &BTreeMap<&str, String>) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        match item {
            Value::String(s) => out.push(subst(s, vars)),
            Value::Object(obj) => {
                let rules: Vec<crate::mc::rules::Rule> = obj
                    .get("rules")
                    .and_then(|r| serde_json::from_value(r.clone()).ok())
                    .unwrap_or_default();
                if !crate::mc::rules::allowed(&rules, features) {
                    continue;
                }
                match obj.get("value") {
                    Some(Value::String(s)) => out.push(subst(s, vars)),
                    Some(Value::Array(list)) => out.extend(
                        list.iter().filter_map(|v| v.as_str()).map(|s| subst(s, vars)),
                    ),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

fn split_host(target: &str) -> (String, u16) {
    match target.rsplit_once(':') {
        Some((host, port)) => (host.to_string(), port.parse().unwrap_or(DEFAULT_PORT)),
        None => (target.to_string(), DEFAULT_PORT),
    }
}

async fn open_log(file: &Path) -> Option<tokio::fs::File> {
    tokio::fs::OpenOptions::new().create(true).append(true).open(file).await.ok()
}

async fn append_log(file: &Path, line: &str) {
    use tokio::io::AsyncWriteExt;
    if let Some(mut f) = open_log(file).await {
        let _ = f.write_all(format!("{line}\n").as_bytes()).await;
    }
}

async fn prepare(
    app: &AppHandle,
    instance: &Instance,
    settings: &Settings,
    account: Option<&Account>,
    files_only: bool,
) -> R<Option<(PathBuf, Vec<String>, PathBuf, Vec<String>)>> {
    let mirror = settings.use_mirror;
    let game_dir = paths::game_dir(&instance.id);
    tokio::fs::create_dir_all(&game_dir).await?;

    net::emit_stage(app, "version", &format!("解析版本 {}", instance.version_id));
    let resolved = version::resolve(&instance.version_id, mirror).await?;

    let (width, height, fullscreen) = instance.resolution(settings);
    let quick_join = settings.quick_join.trim().to_string();
    let has_quick_play = serde_json::to_string(&resolved.arguments.game)
        .map(|s| s.contains("quickPlayMultiplayer"))
        .unwrap_or(false);

    let mut features = Features::new();
    features.insert("is_demo_user".into(), false);
    features.insert("has_custom_resolution".into(), !fullscreen);
    features.insert("has_quick_plays_support".into(), false);
    features.insert("is_quick_play_singleplayer".into(), false);
    features.insert("is_quick_play_realms".into(), false);
    features.insert(
        "is_quick_play_multiplayer".into(),
        has_quick_play && !quick_join.is_empty(),
    );

    let plan = libs::plan(&resolved, &features);
    let client_jar = paths::version_jar(&resolved.base_id);
    let mut jobs = plan.downloads.clone();
    if let Some(client) = &resolved.client {
        jobs.push(
            DlFile::new(client.url.clone(), client_jar.clone())
                .sha1(client.sha1.clone())
                .size(client.size),
        );
    } else if !client_jar.is_file() {
        bail!("{} 沒有提供 client.jar 下載位址", resolved.base_id);
    }
    let asset_index_id = resolved.asset_index.as_ref().map(|a| a.id.clone()).unwrap_or_default();
    let natives_dir = paths::natives(&resolved.id);
    let files = async {
        net::download_all(app, "libraries", jobs, mirror).await?;
        let index = assets::ensure_index(&resolved, mirror).await?;
        net::download_all(app, "assets", assets::jobs(&index), mirror).await?;
        let legacy_dir = assets::materialise_legacy(app, &index, &asset_index_id, &game_dir).await?;
        tokio::fs::create_dir_all(&natives_dir).await?;
        for (jar, exclude) in &plan.natives {
            let stamp = natives_dir.join(format!(".{}.ok", jar.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
            if stamp.is_file() && !net::DEEP_VERIFY.load(std::sync::atomic::Ordering::Relaxed) {
                continue;
            }
            libs::extract_natives(jar, &natives_dir, exclude)?;
            let _ = std::fs::write(stamp, b"");
        }
        Ok::<_, crate::err::Error>(legacy_dir)
    };

    let (java_component, java_major) = if instance.java_major > 0 && instance.java_major != resolved.java_version.major_version {
        (crate::advanced::java_component(instance.java_major), instance.java_major)
    } else {
        (resolved.java_version.component.clone(), resolved.java_version.major_version)
    };
    let java = java::ensure(app, &java_component, java_major, instance.java(settings), mirror, settings.java_auto_update);
    let mods = async {
        if files_only || !instance.wants_rich_presence(settings) {
            return Ok(Vec::new());
        }
        net::emit_stage(app, "mods", "同步內建模組");
        modinject::inject(app, instance).await
    };
    let packs = async {
        if !files_only && crate::boost::wants_packs(instance, settings) {
            if let Err(e) = crate::boost::sync_server_packs(app, instance).await {
                eprintln!("[respack] 預先下載伺服器資源包失敗：{:#}", e.0);
            }
        }
        Ok::<_, crate::err::Error>(())
    };
    let (legacy_dir, java_exe, notes, ()) = tokio::try_join!(files, java, mods, packs)?;
    let Some(account) = account else {
        return Ok(None);
    };

    let mut classpath = plan.classpath.clone();
    classpath.push(client_jar);
    let classpath_str = classpath
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(CLASSPATH_SEP);

    let mut vars: BTreeMap<&str, String> = BTreeMap::new();
    vars.insert("auth_player_name", account.name.clone());
    vars.insert("version_name", resolved.id.clone());
    vars.insert("game_directory", game_dir.to_string_lossy().to_string());
    vars.insert("assets_root", paths::assets().to_string_lossy().to_string());
    vars.insert("assets_index_name", asset_index_id.clone());
    vars.insert("auth_uuid", account.uuid.clone());
    vars.insert("auth_access_token", account.access_token.clone());
    vars.insert("auth_session", format!("token:{}:{}", account.access_token, account.uuid));
    vars.insert("clientid", crate::config::MSA_CLIENT_ID.to_string());
    vars.insert("auth_xuid", account.xuid.clone());
    vars.insert(
        "user_type",
        if account.kind == "msa" { "msa".into() } else { "legacy".to_string() },
    );
    vars.insert("user_properties", "{}".into());
    vars.insert("version_type", resolved.kind.clone());
    vars.insert("resolution_width", width.to_string());
    vars.insert("resolution_height", height.to_string());
    vars.insert("natives_directory", natives_dir.to_string_lossy().to_string());
    vars.insert("library_directory", paths::libraries().to_string_lossy().to_string());
    vars.insert("classpath", classpath_str);
    vars.insert("classpath_separator", CLASSPATH_SEP.into());
    vars.insert("launcher_name", LAUNCHER_NAME.into());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").into());
    vars.insert(
        "game_assets",
        legacy_dir
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| paths::assets().to_string_lossy().to_string()),
    );
    if !quick_join.is_empty() {
        vars.insert("quickPlayMultiplayer", quick_join.clone());
    }
    vars.insert("quickPlayPath", String::new());
    vars.insert("quickPlaySingleplayer", String::new());
    vars.insert("quickPlayRealms", String::new());

    let mut args: Vec<String> = Vec::new();
    let max_memory = instance.memory(settings);
    args.push(format!("-Xmx{max_memory}M"));
    if settings.memory_min_mb > 0 {
        args.push(format!("-Xms{}M", settings.memory_min_mb.min(max_memory)));
    }
    if settings.debug_mode {
        args.push("-Dfabric.log.level=debug".into());
        args.push("-Dmixin.debug.verbose=true".into());
    }
    args.extend(crate::advanced::proxy_props(&settings.proxy));
    args.push("-Dfile.encoding=UTF-8".into());
    args.push("-Dstdout.encoding=UTF-8".into());
    args.push("-Dstderr.encoding=UTF-8".into());
    args.push(format!("-Dbrilliant.launcher={}", env!("CARGO_PKG_VERSION")));
    if cfg!(target_os = "macos") {
        args.push("-XstartOnFirstThread".into());
    }
    args.extend(
        instance
            .jvm(settings)
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    );
    args.extend(expand(&resolved.arguments.jvm, &features, &vars));
    args.push(resolved.main_class.clone());
    let mut game_args = expand(&resolved.arguments.game, &features, &vars);

    if !fullscreen && !game_args.iter().any(|a| a == "--width") {
        game_args.extend(["--width".into(), width.to_string(), "--height".into(), height.to_string()]);
    }
    if fullscreen && !game_args.iter().any(|a| a == "--fullscreen") {
        game_args.push("--fullscreen".into());
    }
    if !quick_join.is_empty() && !game_args.iter().any(|a| a.starts_with("--quickPlay")) {
        let (host, port) = split_host(&quick_join);
        game_args.extend(["--server".into(), host, "--port".into(), port.to_string()]);
    }
    args.extend(game_args);

    Ok(Some((java_exe, args, game_dir, notes)))
}

pub async fn prefetch(app: &AppHandle, instance_id: &str) -> R<()> {
    let state = app.state::<AppState>();
    if state.is_running(instance_id).await {
        return Ok(());
    }
    let Ok(_guard) = state.prep_lock.try_lock() else {
        return Ok(());
    };
    let settings = crate::config::load_settings().await;
    let instance = crate::instance::get(instance_id).await?;
    prepare(app, &instance, &settings, None, false).await?;
    Ok(())
}

pub async fn prefetch_upgrade(app: &AppHandle, instance_id: &str, mc: &str) -> R<()> {
    let state = app.state::<AppState>();
    if state.is_running(instance_id).await {
        return Ok(());
    }
    let Ok(_guard) = state.prep_lock.try_lock() else {
        return Ok(());
    };
    let settings = crate::config::load_settings().await;
    let mut draft = crate::instance::get(instance_id).await?;
    draft.mc_version = mc.to_string();
    draft.loader_version.clear();
    crate::commands::resolve_version(&mut draft, &settings).await?;
    prepare(app, &draft, &settings, None, true).await?;
    if draft.wants_rich_presence(&settings) {
        let _ = crate::modinject::brilliant_mod(mc, settings.mod_update == "keep").await;
    }
    Ok(())
}

pub async fn repair(app: &AppHandle, instance_id: &str) -> R<()> {
    let state = app.state::<AppState>();
    if state.is_running(instance_id).await {
        bail!("請先關閉遊戲再修復");
    }
    let _guard = state.prep_lock.lock().await;
    let settings = crate::config::load_settings().await;
    let instance = crate::instance::get(instance_id).await?;
    crate::modinject::forget_sync();
    net::DEEP_VERIFY.store(true, std::sync::atomic::Ordering::Relaxed);
    let result = prepare(app, &instance, &settings, None, false).await;
    net::DEEP_VERIFY.store(false, std::sync::atomic::Ordering::Relaxed);
    result.map(|_| ())
}

pub async fn launch(app: &AppHandle, instance_id: &str, join: Option<String>) -> R<Vec<String>> {
    let state = app.state::<AppState>();
    let Some(_claim) = state.claim(instance_id).await else {
        bail!("這個實例已經在執行中或正在準備啟動，同一個實例一次只能開一個遊戲");
    };

    let mut settings = crate::config::load_settings().await;
    if let Some(host) = join {
        settings.quick_join = host;
        settings.clean();
    }
    let started = std::time::Instant::now();
    let mut instance = crate::instance::get(instance_id).await?;
    let account = crate::commands::account_for(&settings, &instance.account).await?;
    let stale_login = account.kind == "msa" && account.expires_at <= now_secs();

    if settings.backup_on_launch {
        net::emit_stage(app, "backup", "備份存檔");
        let (id, keep) = (instance.id.clone(), settings.backup_keep);
        tokio::task::spawn_blocking(move || crate::extras::backup_saves(&id, "auto", keep))
            .await
            .context("備份工作中斷")??;
    }

    let mut early_notes = Vec::new();
    if stale_login {
        early_notes.push("目前沒有網路，用上次的登入狀態開始遊戲：單人遊戲可以正常玩，連線伺服器要等網路恢復。".to_string());
    }
    if settings.auto_update_mods && !instance.freeze_mods && instance.loader == "fabric" {
        net::emit_stage(app, "mods", "檢查模組更新");
        let mods_kind = crate::modrinth::Kind::Mod;
        match crate::modrinth::installed(&instance.id, mods_kind, &instance.mc_version, &instance.pinned, instance.wants_rich_presence(&settings), instance.channel(&settings), false).await {
            Ok(list) => {
                let mut updated = 0;
                for entry in list.into_iter().filter(|e| !e.pinned && !e.managed) {
                    if let Some(update) = entry.update {
                        if crate::modrinth::replace(app, &instance.id, mods_kind, &entry.file, update).await.is_ok() {
                            updated += 1;
                        }
                    }
                }
                if updated > 0 {
                    early_notes.push(format!("啟動前已自動更新 {updated} 個模組"));
                }
            }
            Err(e) => early_notes.push(format!("檢查模組更新失敗，這次照原本的版本啟動（{:#}）", e.0)),
        }
    }
    if !instance.general {
        let id = instance.id.clone();
        let risky = tokio::task::spawn_blocking(move || crate::insight::risky_mods(&id, &[])).await.unwrap_or_default();
        if !risky.is_empty() {
            let body = serde_json::json!({
                "name": account.name,
                "uuid": account.uuid,
                "offline": account.kind != "msa",
                "mc": instance.mc_version,
                "mods": risky.iter().map(|m| serde_json::json!({ "id": m.id, "name": m.name, "version": m.version })).collect::<Vec<_>>(),
            });
            tokio::spawn(async move {
                let _: R<serde_json::Value> = net::post_json("https://www.brilliantw.net/api/launcher/risky", &body).await;
            });
        }
        for m in risky {
            early_notes.push(format!("「{}」可能違反輝煌服規 1-1（{}），在輝煌伺服器使用可能會被停權。", m.name, m.reason));
        }
    }
    {
        let (inst, s) = (instance.clone(), settings.clone());
        let shared = tokio::task::spawn_blocking(move || crate::advanced::sync_shared(&inst, &s))
            .await
            .context("共用資料夾工作中斷")??;
        early_notes.extend(shared);
    }

    let prepared = {
        let _guard = state.prep_lock.lock().await;
        prepare(app, &instance, &settings, Some(&account), false).await?
    };
    let (java_exe, args, game_dir, mut notes) = prepared.context("準備啟動參數失敗")?;
    early_notes.append(&mut notes);
    let notes = early_notes;

    if !settings.pre_launch_cmd.trim().is_empty() {
        net::emit_stage(app, "hook", "執行啟動前指令");
        let mut hook = crate::extras::hook_command(&settings.pre_launch_cmd, &instance)
            .kill_on_drop(true)
            .spawn()
            .context("無法執行啟動前指令")?;
        let status = match tokio::time::timeout(std::time::Duration::from_secs(60), hook.wait()).await {
            Ok(status) => status.context("無法執行啟動前指令")?,
            Err(_) => {
                let _ = hook.kill().await;
                bail!("啟動前指令超過 60 秒還沒結束，已取消啟動。要順便開啟會一直執行的程式（例如 OBS），Windows 請在指令前面加上 start，例如 start obs64.exe");
            }
        };
        if !status.success() {
            bail!("啟動前指令失敗（結束代碼 {}），已取消啟動", status.code().unwrap_or(-1));
        }
    }

    net::emit_stage(app, "launch", "啟動遊戲");
    let log_file = paths::logs().join(format!("{instance_id}-{}.log", now_secs()));
    tokio::fs::create_dir_all(paths::logs()).await?;
    let wrapper = instance.wrapper_cmd(&settings);
    append_log(&log_file, &format!("[輝煌啟動器] 啟動前準備耗時 {} 毫秒", started.elapsed().as_millis())).await;
    append_log(&log_file, &crate::advanced::redact(&format!("$ {} {} {}", wrapper.join(" "), java_exe.display(), args.join(" ")))).await;

    let mut cmd = match wrapper.split_first() {
        Some((program, rest)) => {
            let mut c = tokio::process::Command::new(program);
            c.args(rest).arg(&java_exe);
            c
        }
        None => tokio::process::Command::new(&java_exe),
    };
    for (key, value) in instance.env(&settings) {
        cmd.env(key, value);
    }
    if settings.prefer_dgpu {
        crate::advanced::prefer_dgpu(&mut cmd, &java_exe);
    }
    cmd.args(&args)
        .current_dir(&game_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null());
    java::hide_console(&mut cmd);

    let mut child = cmd
        .spawn()
        .with_context(|| match wrapper.first() {
            Some(program) => format!("無法執行包裝指令「{program}」，請確認它已安裝並在 PATH 中"),
            None => format!("無法執行 {}", java_exe.display()),
        })?;

    let stdout = child.stdout.take().context("取不到遊戲輸出")?;
    let stderr = child.stderr.take().context("取不到遊戲錯誤輸出")?;
    for (reader, is_err) in [
        (Box::new(stdout) as Box<dyn tokio::io::AsyncRead + Unpin + Send>, false),
        (Box::new(stderr), true),
    ] {
        let app = app.clone();
        let id = instance_id.to_string();
        let log_file = log_file.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let mut lines = BufReader::new(reader).lines();
            let mut out = open_log(&log_file).await;
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(f) = out.as_mut() {
                    let _ = f.write_all(format!("{line}\n").as_bytes()).await;
                }
                let _ = app.emit(
                    "launcher://log",
                    LogLine { instance: id.clone(), line, stderr: is_err },
                );
            }
            if let Some(f) = out.as_mut() {
                let _ = f.flush().await;
            }
        });
    }

    let started = now_secs();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    state
        .running
        .lock()
        .await
        .insert(instance_id.to_string(), RunningGame { started_at: started, stop: Some(stop_tx) });
    if let Some(pid) = child.id() {
        save_record(instance_id, &RunRecord { pid, started, game_dir: game_dir.to_string_lossy().to_string() });
    }

    let app_handle = app.clone();
    let hook_instance = instance.clone();
    tokio::spawn(async move {
        let mut asked_to_stop = false;
        let code = tokio::select! {
            status = child.wait() => status.ok().and_then(|s| s.code()).unwrap_or(-1),
            _ = stop_rx => { asked_to_stop = true; -1 }
        };
        if asked_to_stop {
            let _ = child.kill().await;
        }
        finish_game(&app_handle, &hook_instance, started, code, asked_to_stop).await;
    });

    instance.last_played = started;
    crate::instance::save(&instance).await?;
    Ok(notes)
}

async fn finish_game(app: &AppHandle, instance: &Instance, started: i64, code: i32, asked_to_stop: bool) {
    let id = instance.id.clone();
    let settings = crate::config::load_settings().await;
    let seconds = now_secs() - started;
    app.state::<AppState>().running.lock().await.remove(&id);
    drop_record(&id);
    if let Ok(mut inst) = crate::instance::get(&id).await {
        inst.play_seconds += seconds.max(0);
        inst.last_played = now_secs();
        let _ = crate::instance::save(&inst).await;
    }
    crate::insight::record_session(crate::insight::Session { instance: id.clone(), name: instance.name.clone(), start: started, seconds, code }).await;
    if code != 0 && !asked_to_stop && settings.notifications {
        crate::boost::notify(app, "遊戲異常結束", &format!("「{}」結束代碼 {code}，打開啟動器查看可能的原因。", instance.name));
    }
    let post_exit = settings.post_exit_cmd.trim().to_string();
    if !post_exit.is_empty() {
        let _ = crate::extras::hook_command(&post_exit, instance).env("EXIT_CODE", code.to_string()).spawn();
    }
    let _ = app.emit("launcher://exit", ExitInfo { instance: id, code, seconds });
}

#[derive(Serialize, serde::Deserialize)]
struct RunRecord {
    pid: u32,
    started: i64,
    game_dir: String,
}

fn record_path(id: &str) -> PathBuf {
    paths::root().join("running").join(format!("{id}.json"))
}

fn save_record(id: &str, record: &RunRecord) {
    let path = record_path(id);
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
    if let Ok(json) = serde_json::to_string(record) {
        let _ = std::fs::write(path, json);
    }
}

fn drop_record(id: &str) {
    let _ = std::fs::remove_file(record_path(id));
}

#[cfg(windows)]
mod proc {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, STILL_ACTIVE, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, GetProcessTimes, OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    };

    pub struct Handle(isize);

    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0 as _) };
        }
    }

    pub fn open(pid: u32, started: i64, _game_dir: &str) -> Option<Handle> {
        unsafe {
            let raw = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_TERMINATE, 0, pid);
            if raw.is_null() {
                return None;
            }
            let handle = Handle(raw as isize);
            let mut code = 0u32;
            let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
            let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
            if GetExitCodeProcess(raw, &mut code) == 0 || code != STILL_ACTIVE as u32 || GetProcessTimes(raw, &mut created, &mut exited, &mut kernel, &mut user) == 0 {
                return None;
            }
            let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
            let secs = (ticks / 10_000_000) as i64 - 11_644_473_600;
            ((started - secs).abs() <= 120).then_some(handle)
        }
    }

    pub fn wait(handle: &Handle, millis: u32) -> Option<i32> {
        unsafe {
            if WaitForSingleObject(handle.0 as _, millis) != WAIT_OBJECT_0 {
                return None;
            }
            let mut code = 0u32;
            GetExitCodeProcess(handle.0 as _, &mut code);
            Some(code as i32)
        }
    }

    pub fn kill(handle: &Handle) {
        unsafe { TerminateProcess(handle.0 as _, 1) };
    }
}

#[cfg(unix)]
mod proc {
    pub struct Handle {
        pid: u32,
        game_dir: String,
    }

    fn command_line(pid: u32) -> String {
        if cfg!(target_os = "linux") {
            std::fs::read(format!("/proc/{pid}/cmdline")).map(|b| String::from_utf8_lossy(&b).replace('\0', " ")).unwrap_or_default()
        } else {
            std::process::Command::new("ps")
                .args(["-p", &pid.to_string(), "-o", "command="])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_default()
        }
    }

    pub fn open(pid: u32, _started: i64, game_dir: &str) -> Option<Handle> {
        command_line(pid).contains(game_dir).then(|| Handle { pid, game_dir: game_dir.to_string() })
    }

    pub fn wait(handle: &Handle, millis: u32) -> Option<i32> {
        if command_line(handle.pid).contains(&handle.game_dir) {
            std::thread::sleep(std::time::Duration::from_millis(u64::from(millis)));
            None
        } else {
            Some(0)
        }
    }

    pub fn kill(handle: &Handle) {
        let _ = std::process::Command::new("kill").arg(handle.pid.to_string()).status();
    }
}

fn crashed_since(game_dir: &Path, started: i64) -> bool {
    std::fs::read_dir(game_dir.join("crash-reports"))
        .map(|d| {
            d.flatten().any(|e| {
                e.metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .is_some_and(|t| t.as_secs() as i64 >= started)
            })
        })
        .unwrap_or(false)
}

async fn tail_latest_log(app: AppHandle, id: String, file: PathBuf, done: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    use std::io::{Read, Seek, SeekFrom};
    let mut offset = std::fs::metadata(&file).map(|m| m.len().saturating_sub(64 << 10)).unwrap_or(0);
    let mut first = true;
    let mut carry = String::new();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    while !done.load(std::sync::atomic::Ordering::Relaxed) {
        let len = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        if len < offset {
            offset = 0;
        }
        if len > offset {
            if let Ok(mut f) = std::fs::File::open(&file) {
                let mut bytes = Vec::new();
                if f.seek(SeekFrom::Start(offset)).is_ok() && f.take(len - offset).read_to_end(&mut bytes).is_ok() {
                    offset += bytes.len() as u64;
                    carry.push_str(&String::from_utf8_lossy(&bytes));
                    let mut lines: Vec<String> = carry.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect();
                    carry = lines.pop().unwrap_or_default();
                    if first && offset > bytes.len() as u64 && !lines.is_empty() {
                        lines.remove(0);
                    }
                    first = false;
                    for line in lines {
                        let _ = app.emit("launcher://log", LogLine { instance: id.clone(), line, stderr: false });
                    }
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

pub async fn adopt(app: AppHandle) {
    let Ok(entries) = std::fs::read_dir(paths::root().join("running")) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(id) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else { continue };
        let record: Option<RunRecord> = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok());
        let instance = crate::instance::get(&id).await.ok();
        let (Some(record), Some(instance)) = (record, instance) else {
            let _ = std::fs::remove_file(&path);
            continue;
        };
        let Some(handle) = proc::open(record.pid, record.started, &record.game_dir) else {
            let _ = std::fs::remove_file(&path);
            continue;
        };
        let state = app.state::<AppState>();
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
        state.running.lock().await.insert(id.clone(), RunningGame { started_at: record.started, stop: Some(stop_tx) });
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let game_dir = PathBuf::from(&record.game_dir);
        tokio::spawn(tail_latest_log(app.clone(), id.clone(), game_dir.join("logs").join("latest.log"), done.clone()));
        let app_handle = app.clone();
        tokio::spawn(async move {
            let handle = std::sync::Arc::new(handle);
            let mut asked_to_stop = false;
            let code = loop {
                let waiter = handle.clone();
                let poll = tokio::task::spawn_blocking(move || proc::wait(&waiter, 1000));
                if let Ok(Some(code)) = poll.await {
                    break code;
                }
                if !asked_to_stop && stop_rx.try_recv().is_ok() {
                    asked_to_stop = true;
                    proc::kill(&handle);
                }
            };
            done.store(true, std::sync::atomic::Ordering::Relaxed);
            let code = if cfg!(unix) && code == 0 && crashed_since(&game_dir, record.started) { 1 } else { code };
            finish_game(&app_handle, &instance, record.started, code, asked_to_stop).await;
        });
        let _ = app.emit("launcher://adopted", id);
    }
}

pub async fn stop(app: &AppHandle, instance_id: &str) -> R<()> {
    let state = app.state::<AppState>();
    let mut running = state.running.lock().await;
    match running.get_mut(instance_id).and_then(|g| g.stop.take()) {
        Some(tx) => {
            let _ = tx.send(());
            Ok(())
        }
        None => bail!("這個實例沒有在執行"),
    }
}

#[cfg(all(test, windows))]
mod adopt_tests {
    use super::*;

    #[test]
    fn finds_live_process_and_exit_code() {
        let mut child = std::process::Command::new("cmd").args(["/C", "ping -n 3 127.0.0.1 >NUL & exit 7"]).spawn().unwrap();
        let pid = child.id();
        assert!(proc::open(pid, now_secs() - 3600, "").is_none(), "建立時間對不上時不能認領");
        let handle = proc::open(pid, now_secs(), "").expect("要認得剛啟動的程序");
        let code = loop {
            if let Some(code) = proc::wait(&handle, 500) {
                break code;
            }
        };
        assert_eq!(code, 7);
        let _ = child.wait();
        assert!(proc::open(pid, now_secs(), "").is_none(), "結束後不能再認領");
    }
}
