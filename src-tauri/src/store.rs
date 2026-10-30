//! 前回の設定を覚えておく（アプリ用フォルダの settings.json）

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::imaging::Fit;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Saved {
    pub rotation: u16,
    pub fit: Fit,
    pub pad_color: String,
    pub boot_show_image: bool,
    pub no_sleep: bool,
    pub brightness: u8,
    pub remove_others: bool,
    pub boot_logo: BootLogo,
    pub last_dir: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BootLogo {
    /// 触らない
    Keep,
    /// 入れる画像と同じにする
    Same,
    /// 真っ黒にする
    Black,
}

impl Default for Saved {
    fn default() -> Self {
        Saved {
            rotation: 0,
            fit: Fit::Contain,
            pad_color: "#000000".into(),
            boot_show_image: true,
            no_sleep: true,
            brightness: 75,
            remove_others: true,
            boot_logo: BootLogo::Same,
            last_dir: None,
        }
    }
}

fn file(dir: &PathBuf) -> PathBuf {
    dir.join("settings.json")
}

pub fn load(dir: &PathBuf) -> Saved {
    std::fs::read_to_string(file(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(dir: &PathBuf, s: &Saved) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(file(dir), serde_json::to_string_pretty(s)?)?;
    Ok(())
}
