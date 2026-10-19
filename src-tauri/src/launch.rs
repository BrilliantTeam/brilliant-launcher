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

async fn append_log(file: &Path, line: &str) {
    use tokio::io::AsyncWriteExt;
    if let Ok(mut f) = tokio::fs::OpenOptions::new().create(true).append(true).open(file).await {
        let _ = f.write_all(line.as_bytes()).await;
        let _ = f.write_all(b"\n").await;
    }
}

async fn prepare(
    app: &AppHandle,
    instance: &Instance,
    settings: &Settings,
    account: Option<&Account>,
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
    net::download_all(app, "libraries", jobs, mirror).await?;

    let index = assets::ensure_index(&resolved, mirror).await?;
    net::download_all(app, "assets", assets::jobs(&index), mirror).await?;
    let asset_index_id = resolved.asset_index.as_ref().map(|a| a.id.clone()).unwrap_or_default();
    let legacy_dir =
        assets::materialise_legacy(app, &index, &asset_index_id, &game_dir).await?;

    let natives_dir = paths::natives(&resolved.id);
    tokio::fs::create_dir_all(&natives_dir).await?;
    for (jar, exclude) in &plan.natives {
        let stamp = natives_dir.join(format!(".{}.ok", jar.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
        if stamp.is_file() {
            continue;
        }
        libs::extract_natives(jar, &natives_dir, exclude)?;
        let _ = std::fs::write(stamp, b"");
    }

    let java_exe = java::ensure(
        app,
        &resolved.java_version.component,
        resolved.java_version.major_version,
        instance.java(settings),
        mirror,
    )
    .await?;

    let mut notes = Vec::new();
    if instance.wants_rich_presence(settings) {
        net::emit_stage(app, "mods", "同步內建模組");
        notes.extend(modinject::inject(app, instance).await?);
    }
    if crate::boost::wants_packs(instance, settings) {
        if let Err(e) = crate::boost::sync_server_packs(app, instance).await {
            eprintln!("[respack] 預先下載伺服器資源包失敗：{:#}", e.0);
        }
    }
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
    args.push(format!("-Xmx{}M", instance.memory(settings)));
    args.push("-Dfile.encoding=UTF-8".into());
    args.push("-Dstdout.encoding=UTF-8".into());
    args.push("-Dstderr.encoding=UTF-8".into());
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
    prepare(app, &instance, &settings, None).await?;
    Ok(())
}

pub async fn launch(app: &AppHandle, instance_id: &str) -> R<Vec<String>> {
    let state = app.state::<AppState>();
    if state.is_running(instance_id).await {
        bail!("這個實例已經在執行中了");
    }

    let settings = crate::config::load_settings().await;
    let mut instance = crate::instance::get(instance_id).await?;
    let account = crate::commands::active_account(&settings).await?;

    if settings.backup_on_launch {
        net::emit_stage(app, "backup", "備份存檔");
        let (id, keep) = (instance.id.clone(), settings.backup_keep);
        tokio::task::spawn_blocking(move || crate::extras::backup_saves(&id, "auto", keep))
            .await
            .context("備份工作中斷")??;
    }

    let prepared = {
        let _guard = state.prep_lock.lock().await;
        prepare(app, &instance, &settings, Some(&account)).await?
    };
    let (java_exe, args, game_dir, notes) = prepared.context("準備啟動參數失敗")?;

    if !settings.pre_launch_cmd.trim().is_empty() {
        net::emit_stage(app, "hook", "執行啟動前指令");
        let status = crate::extras::hook_command(&settings.pre_launch_cmd, &instance)
            .status()
            .await
            .context("無法執行啟動前指令")?;
        if !status.success() {
            bail!("啟動前指令失敗（結束代碼 {}），已取消啟動", status.code().unwrap_or(-1));
        }
    }

    net::emit_stage(app, "launch", "啟動遊戲");
    let log_file = paths::logs().join(format!("{instance_id}-{}.log", now_secs()));
    tokio::fs::create_dir_all(paths::logs()).await?;
    append_log(&log_file, &format!("$ {} {}", java_exe.display(), args.join(" "))).await;

    let mut cmd = tokio::process::Command::new(&java_exe);
    cmd.args(&args)
        .current_dir(&game_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null());
    java::hide_console(&mut cmd);

    let mut child = cmd
        .spawn()
        .with_context(|| format!("無法執行 {}", java_exe.display()))?;

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
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                append_log(&log_file, &line).await;
                let _ = app.emit(
                    "launcher://log",
                    LogLine { instance: id.clone(), line, stderr: is_err },
                );
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

    let app_handle = app.clone();
    let id = instance_id.to_string();
    let post_exit = settings.post_exit_cmd.trim().to_string();
    let notify_crash = settings.notifications;
    let instance_name = instance.name.clone();
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
        let seconds = now_secs() - started;
        let state = app_handle.state::<AppState>();
        state.running.lock().await.remove(&id);

        if let Ok(mut inst) = crate::instance::get(&id).await {
            inst.play_seconds += seconds.max(0);
            inst.last_played = now_secs();
            let _ = crate::instance::save(&inst).await;
        }
        if code != 0 && !asked_to_stop && notify_crash {
            crate::boost::notify(&app_handle, "遊戲異常結束", &format!("「{instance_name}」結束代碼 {code}，打開啟動器查看可能的原因。"));
        }
        if !post_exit.is_empty() {
            let _ = crate::extras::hook_command(&post_exit, &hook_instance).env("EXIT_CODE", code.to_string()).spawn();
        }
        let _ = app_handle.emit("launcher://exit", ExitInfo { instance: id, code, seconds });
    });

    instance.last_played = started;
    crate::instance::save(&instance).await?;
    Ok(notes)
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
