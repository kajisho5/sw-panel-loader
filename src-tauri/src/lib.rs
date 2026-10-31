pub mod imaging;
pub mod log;
pub mod protocol;
pub mod sector_io;
pub mod store;
pub mod usb;
#[cfg(windows)]
pub mod format_win;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::imaging::Job;
use crate::protocol::{path as dpath, DeviceSettings, StartMode, Storage};
use crate::store::{BootLogo, Saved};

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    let s = e.to_string();
    logln!("エラー：{s}");
    s
}

fn config_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path().app_config_dir().unwrap_or_else(|_| std::env::temp_dir().join("sw-panel-loader"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceStatus {
    connected: bool,
    other_model: Option<String>,
    storage: Option<Storage>,
    error: Option<String>,
    /// ほかのソフトが画面を使っていて開けない
    busy: bool,
}

#[tauri::command]
fn device_presence() -> usb::Presence {
    usb::presence()
}

/// 接続を確かめて、カードの容量も読む
#[tauri::command]
async fn device_status() -> CmdResult<DeviceStatus> {
    tauri::async_runtime::spawn_blocking(|| {
        let p = usb::presence();
        if !p.connected {
            return DeviceStatus { connected: false, other_model: p.other_model, storage: None, error: None, busy: false };
        }
        match usb::Panel::open().and_then(|panel| {
            panel.sync()?;
            panel.storage()
        }) {
            Ok(s) => DeviceStatus { connected: true, other_model: None, storage: Some(s), error: None, busy: false },
            Err(e) => {
                let busy = matches!(e, usb::UsbError::Busy(_));
                DeviceStatus { connected: true, other_model: None, storage: None, error: Some(err(e)), busy }
            }
        }
    })
    .await
    .map_err(err)
}

#[tauri::command]
async fn inspect_image(path: String) -> CmdResult<imaging::SourceInfo> {
    tauri::async_runtime::spawn_blocking(move || imaging::source_info(Path::new(&path)).map_err(err))
        .await
        .map_err(err)?
}

#[tauri::command]
async fn preview(job: Job) -> CmdResult<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let img = imaging::render_job(&job).map_err(err)?;
        imaging::preview_data_url(&img).map_err(err)
    })
    .await
    .map_err(err)?
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct WriteOptions {
    boot_show_image: bool,
    no_sleep: bool,
    brightness: u8,
    remove_others: bool,
    boot_logo: BootLogo,
    restart: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    stage: &'static str,
    label: String,
    sent: usize,
    total: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WriteResult {
    device_path: String,
    played_now: bool,
    removed: Vec<String>,
    used_internal: bool,
}

#[tauri::command]
async fn write_panel(app: tauri::AppHandle, job: Job, opts: WriteOptions) -> CmdResult<WriteResult> {
    tauri::async_runtime::spawn_blocking(move || write_panel_inner(&app, &job, &opts).map_err(err))
        .await
        .map_err(err)?
}

fn write_panel_inner(app: &tauri::AppHandle, job: &Job, o: &WriteOptions) -> anyhow::Result<WriteResult> {
    logln!("書き込み {job:?} {o:?}");
    let emit = |stage: &'static str, label: &str, sent: usize, total: usize| {
        let _ = app.emit("progress", Progress { stage, label: label.into(), sent, total });
    };
    emit("prepare", "画像を整えています", 0, 1);
    let img = imaging::render_job(job)?;
    // 画面はJPEGが本命（表示の命令名が PlayLocalJpg）。PNGは起動時に固まることがあった
    let png = imaging::encode_jpeg_under(&img, 900 * 1024)?;
    logln!("画像 {} バイト（JPEG）", png.len());

    emit("prepare", "画面に接続しています", 0, 1);
    let panel = usb::Panel::open()?;
    panel.sync()?;
    let storage = panel.storage().ok();
    let has_card = storage.map(|s| s.total_kb > 0).unwrap_or(false);
    let dir = if has_card { dpath::SD_IMG } else { dpath::INTERNAL_IMG };
    let stem = Path::new(&job.source_path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = protocol::device_name_with_stamp(&stem, &chrono::Local::now().format("%m%d%H%M%S").to_string(), "jpg");
    let device_path = format!("{dir}{name}");

    // 先に新しい画像を入れてから古いものを消す（途中で失敗しても画像が空にならないように）
    panel.upload(&device_path, &png, |s, t| emit("upload", "画像を送っています", s, t))?;

    let mut removed = Vec::new();
    if o.remove_others {
        emit("prepare", "前の画像を片付けています", 0, 1);
        if let Ok(names) = panel.list_dir(dir.trim_end_matches('/')) {
            for n in names {
                if n == name {
                    continue;
                }
                let lower = n.to_ascii_lowercase();
                if [".png", ".jpg", ".jpeg", ".bmp"].iter().any(|e| lower.ends_with(e))
                    && panel.delete(&format!("{dir}{n}")).is_ok()
                {
                    removed.push(n);
                }
            }
        }
    }

    match o.boot_logo {
        BootLogo::Keep => {}
        BootLogo::Same | BootLogo::Black => {
            emit("upload", "起動ロゴを入れ替えています", 0, 1);
            let logo = if o.boot_logo == BootLogo::Same { img.clone() } else { imaging::black_panel() };
            let jpg = imaging::encode_jpeg_under(&logo, 300 * 1024)?;
            logln!("起動ロゴ {} バイト", jpg.len());
            panel.upload(dpath::BOOT_LOGO, &jpg, |_, _| {})?;
        }
    }

    emit("save", "設定を保存しています", 0, 1);
    let played_now = panel.play_image(&device_path).unwrap_or(false);
    panel.brightness(o.brightness)?;
    panel.save_settings(&DeviceSettings {
        brightness: o.brightness,
        start_mode: if o.boot_show_image { StartMode::Image } else { StartMode::Default },
        rotation: 0,
        sleep_delay: if o.no_sleep { 0 } else { 30 },
        offline: u8::from(o.boot_show_image),
    })?;

    if o.restart {
        emit("restart", "画面を再起動しています", 0, 1);
        panel.restart()?;
    }
    drop(panel);
    emit("done", "完了しました", 1, 1);
    logln!("完了 {device_path}");
    Ok(WriteResult { device_path, played_now, removed, used_internal: !has_card })
}

/// 起動時の設定を初期（公式ソフトの標準）に戻す。必要なら起動ロゴを黒にする
#[tauri::command]
async fn reset_device(black_logo: bool) -> CmdResult<()> {
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<()> {
        logln!("初期に戻す（起動ロゴを黒に: {black_logo}）");
        let p = usb::Panel::open()?;
        p.sync()?;
        if black_logo {
            let jpg = imaging::encode_jpeg_under(&imaging::black_panel(), 300 * 1024)?;
            p.upload(dpath::BOOT_LOGO, &jpg, |_, _| {})?;
        }
        p.save_settings(&DeviceSettings {
            brightness: 75,
            start_mode: StartMode::Default,
            rotation: 0,
            sleep_delay: 0,
            offline: 0,
        })?;
        Ok(())
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[tauri::command]
async fn restart_device() -> CmdResult<()> {
    tauri::async_runtime::spawn_blocking(|| {
        let p = usb::Panel::open()?;
        p.restart()
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[tauri::command]
fn load_prefs(app: tauri::AppHandle) -> Saved {
    store::load(&config_dir(&app))
}

#[tauri::command]
fn save_prefs(app: tauri::AppHandle, prefs: Saved) -> CmdResult<()> {
    store::save(&config_dir(&app), &prefs).map_err(err)
}

#[tauri::command]
fn get_log() -> Vec<String> {
    log::all()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CardDrive {
    pub letter: String,
    pub label: String,
    pub fs: String,
    pub size_bytes: u64,
    pub bus: String,
    pub disk_number: u32,
}

#[tauri::command]
async fn list_cards() -> CmdResult<Vec<CardDrive>> {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(|| {
            format_win::list_removable()
                .into_iter()
                .map(|r| CardDrive {
                    letter: r.letter,
                    label: r.label,
                    fs: r.fs,
                    size_bytes: r.size_bytes,
                    bus: r.bus,
                    disk_number: r.disk_number,
                })
                .collect()
        })
        .await
        .map_err(err)
    }
    #[cfg(not(windows))]
    {
        Ok(vec![])
    }
}

#[derive(Serialize)]
struct FormatResult {
    ok: bool,
    message: String,
}

#[tauri::command]
async fn format_card(letter: String, disk_number: u32) -> CmdResult<FormatResult> {
    logln!("作り直し {letter}: ディスク{disk_number}");
    #[cfg(windows)]
    {
        let r = tauri::async_runtime::spawn_blocking(move || format_win::format_elevated(&letter, disk_number))
            .await
            .map_err(err)?
            .map_err(err)?;
        for l in &r.log {
            logln!("{l}");
        }
        Ok(FormatResult { ok: r.ok, message: r.message })
    }
    #[cfg(not(windows))]
    {
        let _ = (letter, disk_number);
        Err("この機能はWindows専用です".into())
    }
}

/// 管理者で起動されたときの入口（Windowsのみ）。該当すれば終了コードを返す
pub fn maybe_run_helper() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s == "--format-card").unwrap_or(false) {
        #[cfg(windows)]
        {
            return Some(format_win::run_helper(&args[2..]));
        }
        #[cfg(not(windows))]
        {
            return Some(1);
        }
    }
    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logln!("SW PANEL LOADER {} 起動", env!("CARGO_PKG_VERSION"));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = config_dir(app.handle());
            log::init_file(dir.join("記録.log"));
            logln!("記録の保存先 {}", dir.display());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            device_presence,
            device_status,
            inspect_image,
            preview,
            write_panel,
            restart_device,
            reset_device,
            load_prefs,
            save_prefs,
            get_log,
            list_cards,
            format_card
        ])
        .run(tauri::generate_context!())
        .expect("アプリを起動できませんでした");
}
