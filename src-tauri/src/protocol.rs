//! TURZX（Turing）USB画面の命令の組み立て。
//!
//! 命令の形は公開ソフト turing-smart-screen-python（GPL-3.0）と、
//! 公式ソフトの動きを照らし合わせて確認したもの。
//! - 500バイトの命令を DES-CBC（鍵・IVとも "slv3tuzx"）で暗号化 → 504バイト
//! - 512バイトに詰め、末尾2バイトに 161, 26 を置く
//! - ファイル本体などは512バイトの後ろにそのまま続ける

use cbc::cipher::{block_padding::NoPadding, BlockModeEncrypt, KeyIvInit};

pub const VENDOR_ID: u16 = 0x1cbe;
/// 8.8インチ（縦480×横1920）
pub const PRODUCT_ID_88: u16 = 0x0088;

pub const PANEL_W: u32 = 480;
pub const PANEL_H: u32 = 1920;

const DES_KEY: &[u8; 8] = b"slv3tuzx";
pub const PACKET_LEN: usize = 512;
const HEADER_LEN: usize = 500;

/// 1回で送れる最大のデータ量（1MB）
pub const MAX_CHUNK: usize = 1024 * 1024;
/// これより小さいファイルは1回の命令で送る（公式ソフトと同じ境目）
pub const SMALL_FILE_LIMIT: usize = 100 * 1024;

/// 命令番号
pub mod cmd {
    pub const SYNC: u8 = 10;
    pub const RESTART: u8 = 11;
    pub const BRIGHTNESS: u8 = 14;
    pub const CREATE_FILE: u8 = 38;
    pub const WRITE_CHUNK: u8 = 39;
    pub const WRITE_SMALL_FILE: u8 = 40;
    pub const DELETE: u8 = 42;
    pub const LIST_DIR: u8 = 99;
    pub const STORAGE: u8 = 100;
    pub const PLAY_IMAGE: u8 = 113;
    pub const SAVE_SETTINGS: u8 = 125;
}

/// 画面の中の保存場所
pub mod path {
    pub const SD_IMG: &str = "/tmp/sdcard/mmcblk0p1/img/";
    pub const INTERNAL_IMG: &str = "/usr/data/img/";
    pub const BOOT_LOGO: &str = "/usr/data/boot.jpg";
}

/// 起動時の動き（公式の設定ファイル startMode の値）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartMode {
    Default = 0,
    Image = 1,
    Video = 2,
}

/// 命令125で保存する設定
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceSettings {
    /// 0〜100
    pub brightness: u8,
    pub start_mode: StartMode,
    /// 画面側の回転（0のまま使い、回転は画像側で行う）
    pub rotation: u8,
    /// スリープまでの時間。0 = スリープしない
    pub sleep_delay: u8,
    /// PCなしで表示する（起動時表示を使うとき 1）
    pub offline: u8,
}

fn millis_since_local_midnight() -> u32 {
    use chrono::Timelike;
    let now = chrono::Local::now();
    (now.num_seconds_from_midnight() * 1000 + now.timestamp_subsec_millis()) as u32
}

/// 命令の素の500バイト（暗号化前）
pub fn header(command: u8) -> [u8; HEADER_LEN] {
    header_at(command, millis_since_local_midnight())
}

pub fn header_at(command: u8, millis: u32) -> [u8; HEADER_LEN] {
    let mut p = [0u8; HEADER_LEN];
    p[0] = command;
    p[2] = 0x1A;
    p[3] = 0x6D;
    p[4..8].copy_from_slice(&millis.to_le_bytes());
    p
}

/// 500バイトを暗号化して512バイトの送信単位にする
pub fn seal(plain: &[u8; HEADER_LEN]) -> [u8; PACKET_LEN] {
    type DesCbc = cbc::Encryptor<des::Des>;
    let mut buf = [0u8; 504];
    buf[..HEADER_LEN].copy_from_slice(plain);
    let enc = DesCbc::new(DES_KEY.into(), DES_KEY.into());
    enc.encrypt_padded::<NoPadding>(&mut buf, 504)
        .expect("504は8の倍数");
    let mut out = [0u8; PACKET_LEN];
    out[..504].copy_from_slice(&buf);
    out[510] = 161;
    out[511] = 26;
    out
}

fn put_u32_be(p: &mut [u8], at: usize, v: u32) {
    p[at..at + 4].copy_from_slice(&v.to_be_bytes());
}

/// 引数なしの命令
pub fn simple(command: u8) -> Vec<u8> {
    seal(&header(command)).to_vec()
}

pub fn brightness(level_0_100: u8) -> Vec<u8> {
    let mut h = header(cmd::BRIGHTNESS);
    h[8] = level_0_100.min(100);
    seal(&h).to_vec()
}

pub fn save_settings(s: &DeviceSettings) -> Vec<u8> {
    let mut h = header(cmd::SAVE_SETTINGS);
    h[8] = s.brightness.min(100);
    h[9] = s.start_mode as u8;
    h[10] = 0;
    h[11] = s.rotation;
    h[12] = s.sleep_delay;
    h[13] = s.offline;
    h[14] = 0; // 時計の向き（使わない）
    seal(&h).to_vec()
}

/// [8..12]=パスの長さ, [16..]=パス の形の命令（作成・削除・一覧・画像表示）
pub fn with_path(command: u8, device_path: &str) -> anyhow::Result<Vec<u8>> {
    let bytes = device_path.as_bytes();
    anyhow::ensure!(device_path.is_ascii(), "画面側の名前は英数字だけにしてください");
    anyhow::ensure!(bytes.len() <= HEADER_LEN - 16, "パスが長すぎます");
    let mut h = header(command);
    put_u32_be(&mut h, 8, bytes.len() as u32);
    h[16..16 + bytes.len()].copy_from_slice(bytes);
    Ok(seal(&h).to_vec())
}

/// 100KB未満のファイルを1回で書く（命令40）
pub fn write_small_file(device_path: &str, data: &[u8]) -> anyhow::Result<Vec<u8>> {
    let bytes = device_path.as_bytes();
    anyhow::ensure!(device_path.is_ascii(), "画面側の名前は英数字だけにしてください");
    anyhow::ensure!(data.len() < SMALL_FILE_LIMIT, "小さいファイル用の命令です");
    let mut h = header(cmd::WRITE_SMALL_FILE);
    put_u32_be(&mut h, 8, bytes.len() as u32);
    put_u32_be(&mut h, 12, data.len() as u32);
    h[16..16 + bytes.len()].copy_from_slice(bytes);
    let mut out = seal(&h).to_vec();
    out.extend_from_slice(data);
    Ok(out)
}

/// 大きいファイルの1かたまり（命令39）。先に CREATE_FILE を送っておく
pub fn write_chunk(chunk: &[u8]) -> Vec<u8> {
    assert!(chunk.len() <= MAX_CHUNK);
    let mut h = header(cmd::WRITE_CHUNK);
    put_u32_be(&mut h, 8, MAX_CHUNK as u32);
    put_u32_be(&mut h, 12, chunk.len() as u32);
    let mut out = seal(&h).to_vec();
    out.extend_from_slice(chunk);
    out
}

/// 空き容量（命令100の返事）。単位はKB
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    pub total_kb: u32,
    pub used_kb: u32,
    pub free_kb: u32,
}

pub fn parse_storage(resp: &[u8]) -> Option<Storage> {
    if resp.len() < 20 {
        return None;
    }
    let r = |i: usize| u32::from_le_bytes([resp[i], resp[i + 1], resp[i + 2], resp[i + 3]]);
    Some(Storage { total_kb: r(8), used_kb: r(12), free_kb: r(16) })
}

/// 一覧の返事（"…file:a.png/b.png/…"）からファイル名を取り出す
pub fn parse_dir_listing(raw: &[u8]) -> Vec<String> {
    let text: String = String::from_utf8_lossy(raw).replace('\0', "");
    let Some(idx) = text.find("file:") else { return vec![] };
    let rest = &text[idx + 5..];
    let mut out: Vec<String> = Vec::new();
    for name in rest.split('/') {
        let name = name.trim();
        if name.is_empty() || name.contains("file:") {
            continue;
        }
        if !name.chars().all(|c| c.is_ascii_graphic()) {
            continue;
        }
        if !out.iter().any(|n| n == name) {
            out.push(name.to_string());
        }
    }
    out
}

/// 画面の中で使うファイル名にする（英数字と - _ . だけ）
pub fn safe_device_name(stem: &str, ext: &str) -> String {
    let mut s: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    let s = s.trim_matches('_');
    let s = if s.is_empty() { "panel" } else { s };
    let s: String = s.chars().take(40).collect();
    format!("{s}.{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_layout() {
        let h = header_at(125, 0x0102_0304);
        assert_eq!(h[0], 125);
        assert_eq!(h[2], 0x1A);
        assert_eq!(h[3], 0x6D);
        assert_eq!(&h[4..8], &[4, 3, 2, 1]);
    }

    #[test]
    fn seal_trailer() {
        let p = seal(&header_at(10, 0));
        assert_eq!(p.len(), 512);
        assert_eq!(p[510], 161);
        assert_eq!(p[511], 26);
        assert_eq!(&p[504..510], &[0; 6]);
    }

    /// 参照実装（Python + pycryptodome相当）で作った値と一致するか
    #[test]
    fn seal_matches_reference() {
        let p = seal(&header_at(10, 12345678));
        let expect = include_str!("../tests/sync_12345678.hex").trim();
        assert_eq!(hex(&p), expect);
    }

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn listing() {
        let raw = b"\x00\x00ok file:a.png/b_c.png/\x00\x00\x00";
        assert_eq!(parse_dir_listing(raw), vec!["a.png", "b_c.png"]);
        assert!(parse_dir_listing(b"nodir-createdone").is_empty());
    }

    #[test]
    fn names() {
        assert_eq!(safe_device_name("受付 サンプル 01", "png"), "01.png");
        assert_eq!(safe_device_name("次座長席", "png"), "panel.png");
        assert_eq!(safe_device_name("hall-b_sign", "png"), "hall-b_sign.png");
    }

    #[test]
    fn storage() {
        let mut r = vec![0u8; 512];
        r[8..12].copy_from_slice(&15_000_000u32.to_le_bytes());
        r[12..16].copy_from_slice(&600_000u32.to_le_bytes());
        r[16..20].copy_from_slice(&14_400_000u32.to_le_bytes());
        let s = parse_storage(&r).unwrap();
        assert_eq!(s.total_kb, 15_000_000);
        assert_eq!(s.free_kb, 14_400_000);
    }

    #[test]
    fn small_file_packet() {
        let p = write_small_file("/usr/data/boot.jpg", &[1, 2, 3]).unwrap();
        assert_eq!(p.len(), 515);
        assert_eq!(&p[512..], &[1, 2, 3]);
    }
}
