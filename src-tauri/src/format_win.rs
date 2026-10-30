//! TFカードの作り直し（Windows専用）。
//! - 一覧には「取り外しできる」ドライブ（SDカードリーダー・USB）だけを出す
//! - 実行は管理者の許可を取って、このアプリ自身を別に起動して行う
//! - 手順：区画を全部消す → 主区画を1つ（種類 0C）→ FAT32

use std::ffi::c_void;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
    FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceProperty, FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME,
    FSCTL_UNLOCK_VOLUME, GET_LENGTH_INFORMATION, IOCTL_DISK_GET_LENGTH_INFO,
    IOCTL_STORAGE_GET_DEVICE_NUMBER, IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_DEVICE_DESCRIPTOR,
    STORAGE_DEVICE_NUMBER, STORAGE_PROPERTY_QUERY,
};
use windows::Win32::System::IO::DeviceIoControl;

const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
// STORAGE_BUS_TYPE
const BUS_USB: i32 = 7;
const BUS_SD: i32 = 0xC;
const BUS_MMC: i32 = 0xD;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Removable {
    pub letter: String,
    pub label: String,
    pub fs: String,
    pub size_bytes: u64,
    pub bus: String,
    pub disk_number: u32,
}

fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

fn open_query(path: &str) -> Option<HANDLE> {
    let w = wide(path);
    unsafe {
        CreateFileW(
            PCWSTR(w.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
        .ok()
    }
}

fn ioctl<T: Default>(h: HANDLE, code: u32, input: Option<(*const c_void, u32)>) -> Option<T> {
    let mut out = T::default();
    let mut ret = 0u32;
    let (ip, il) = input.map(|(p, l)| (Some(p), l)).unwrap_or((None, 0));
    unsafe {
        DeviceIoControl(
            h,
            code,
            ip,
            il,
            Some(&mut out as *mut T as *mut c_void),
            std::mem::size_of::<T>() as u32,
            Some(&mut ret),
            None,
        )
        .ok()?;
    }
    Some(out)
}

fn disk_number(letter: char) -> Option<u32> {
    let h = open_query(&format!(r"\\.\{letter}:"))?;
    let n: Option<STORAGE_DEVICE_NUMBER> = ioctl(h, IOCTL_STORAGE_GET_DEVICE_NUMBER, None);
    unsafe {
        let _ = CloseHandle(h);
    }
    n.map(|n| n.DeviceNumber)
}

fn bus_type(letter: char) -> Option<i32> {
    let h = open_query(&format!(r"\\.\{letter}:"))?;
    let q = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        ..Default::default()
    };
    let d: Option<STORAGE_DEVICE_DESCRIPTOR> = ioctl(
        h,
        IOCTL_STORAGE_QUERY_PROPERTY,
        Some((&q as *const _ as *const c_void, std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32)),
    );
    unsafe {
        let _ = CloseHandle(h);
    }
    d.map(|d| d.BusType.0)
}

fn system_letter() -> char {
    std::env::var("SystemDrive")
        .ok()
        .and_then(|s| s.chars().next())
        .unwrap_or('C')
        .to_ascii_uppercase()
}

/// 作り直してよいドライブの一覧
pub fn list_removable() -> Vec<Removable> {
    let mut out = Vec::new();
    let mask = unsafe { GetLogicalDrives() };
    let sys = system_letter();
    let sys_disk = disk_number(sys);
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        if letter == sys || letter < 'C' {
            continue;
        }
        let root = wide(&format!("{letter}:\\"));
        let dtype = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
        let bus = bus_type(letter);
        let bus_ok = matches!(bus, Some(BUS_USB) | Some(BUS_SD) | Some(BUS_MMC));
        // 取り外しできるもの、または USB/SD 接続のものだけ
        if !(dtype == DRIVE_REMOVABLE || (dtype == DRIVE_FIXED && bus_ok)) {
            continue;
        }
        let Some(dn) = disk_number(letter) else { continue };
        if Some(dn) == sys_disk {
            continue;
        }
        let mut name = [0u16; 64];
        let mut fsname = [0u16; 32];
        unsafe {
            let _ = GetVolumeInformationW(
                PCWSTR(root.as_ptr()),
                Some(&mut name),
                None,
                None,
                None,
                Some(&mut fsname),
            );
        }
        let mut total = 0u64;
        unsafe {
            let _ = GetDiskFreeSpaceExW(PCWSTR(root.as_ptr()), None, Some(&mut total), None);
        }
        let s = |b: &[u16]| String::from_utf16_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]);
        out.push(Removable {
            letter: letter.to_string(),
            label: s(&name),
            fs: s(&fsname),
            size_bytes: total,
            bus: match bus {
                Some(BUS_SD) | Some(BUS_MMC) => "SD".into(),
                Some(BUS_USB) => "USB".into(),
                _ => "取り外し可能".into(),
            },
            disk_number: dn,
        });
    }
    out
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HelperResult {
    pub ok: bool,
    pub message: String,
    pub log: Vec<String>,
}

/// 管理者の許可を取って自分自身を起動し、作り直しを行う（終わるまで待つ）
pub fn format_elevated(letter: &str, disk_number: u32) -> anyhow::Result<HelperResult> {
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let exe = std::env::current_exe()?;
    let result_path = std::env::temp_dir().join(format!("sw-panel-loader-format-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&result_path);
    let args = format!(
        "--format-card {} {} \"{}\"",
        letter,
        disk_number,
        result_path.display()
    );
    let verb = wide("runas");
    let file = wide(&exe.to_string_lossy());
    let params = wide(&args);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe {
        ShellExecuteExW(&mut info).map_err(|_| anyhow::anyhow!("管理者の許可が得られなかったので、作り直しを中止しました"))?;
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 0u32;
        let _ = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);
    }
    let text = std::fs::read_to_string(&result_path)
        .map_err(|_| anyhow::anyhow!("作り直しの結果が読めませんでした"))?;
    let _ = std::fs::remove_file(&result_path);
    Ok(serde_json::from_str(&text)?)
}

/// 管理者で起動されたときの本体。戻り値はプロセスの終了コード
pub fn run_helper(args: &[String]) -> i32 {
    let mut log = Vec::new();
    let (ok, message) = match helper_inner(args, &mut log) {
        Ok(()) => (true, "FAT32で作り直しました".to_string()),
        Err(e) => (false, e.to_string()),
    };
    if let Some(p) = args.get(2) {
        let _ = std::fs::write(
            PathBuf::from(p),
            serde_json::to_string(&HelperResult { ok, message, log }).unwrap_or_default(),
        );
    }
    if ok { 0 } else { 1 }
}

fn helper_inner(args: &[String], log: &mut Vec<String>) -> anyhow::Result<()> {
    let letter = args
        .first()
        .and_then(|s| s.chars().next())
        .map(|c| c.to_ascii_uppercase())
        .ok_or_else(|| anyhow::anyhow!("ドライブ名がありません"))?;
    let expected_disk: u32 = args.get(1).and_then(|s| s.parse().ok()).ok_or_else(|| anyhow::anyhow!("ディスク番号がありません"))?;

    // もう一度、安全か確かめる
    let target = list_removable()
        .into_iter()
        .find(|r| r.letter.starts_with(letter))
        .ok_or_else(|| anyhow::anyhow!("{letter}: は取り外しできるドライブとして見つかりません"))?;
    anyhow::ensure!(
        target.disk_number == expected_disk,
        "{letter}: のディスク番号が変わりました。カードを挿し直して、もう一度選んでください"
    );
    log.push(format!("対象 {letter}: ディスク{} {} バイト", target.disk_number, target.size_bytes));

    // 1. 区画を作り直す
    let script = format!(
        "select disk {d}\r\nclean\r\ncreate partition primary\r\nset id=0c override noerr\r\nassign letter={letter} noerr\r\nexit\r\n",
        d = target.disk_number
    );
    let script_path = std::env::temp_dir().join("sw-panel-loader-diskpart.txt");
    std::fs::write(&script_path, script)?;
    let out = std::process::Command::new("diskpart")
        .arg("/s")
        .arg(&script_path)
        .output()?;
    let _ = std::fs::remove_file(&script_path);
    log.push(String::from_utf8_lossy(&out.stdout).trim().to_string());
    anyhow::ensure!(out.status.success(), "区画の作り直しに失敗しました（diskpart）");

    // 2. 新しい区画が見えるまで待つ
    let raw_path = format!(r"\\.\{letter}:");
    let start = Instant::now();
    let file = loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(&raw_path) {
            Ok(f) => break f,
            Err(e) if start.elapsed() < Duration::from_secs(20) => {
                log.push(format!("待機中 {e}"));
                std::thread::sleep(Duration::from_millis(700));
            }
            Err(e) => anyhow::bail!("新しい区画を開けませんでした：{e}"),
        }
    };
    let h = HANDLE(file.as_raw_handle() as *mut c_void);
    let len: GET_LENGTH_INFORMATION =
        ioctl(h, IOCTL_DISK_GET_LENGTH_INFO, None).ok_or_else(|| anyhow::anyhow!("区画の大きさが分かりません"))?;
    let len = len.Length as u64;
    log.push(format!("区画 {len} バイト"));
    let _: Option<u32> = ioctl(h, FSCTL_LOCK_VOLUME, None);

    // 3. FAT32を書く
    crate::sector_io::format_fat32(&file, len, "TURZX")?;
    (&file).flush()?;
    let _: Option<u32> = ioctl(h, FSCTL_DISMOUNT_VOLUME, None);
    let _: Option<u32> = ioctl(h, FSCTL_UNLOCK_VOLUME, None);
    drop(file);
    log.push("FAT32 書き込み完了".into());
    Ok(())
}
