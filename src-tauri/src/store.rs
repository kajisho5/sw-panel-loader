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
    /// 設定の形の版。2未満なら起動ロゴを「そのまま」に戻す（0.1.0で初期値が危なかったため）
    #[serde(default)]
    pub version: u32,
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
            boot_logo: BootLogo::Keep,
            last_dir: None,
            version: 2,
        }
    }
}

fn file(dir: &PathBuf) -> PathBuf {
    dir.join("settings.json")
}

pub fn load(dir: &PathBuf) -> Saved {
    let mut s: Saved = std::fs::read_to_string(file(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if s.version < 2 {
        s.boot_logo = BootLogo::Keep;
        s.version = 2;
    }
    s
}

pub fn save(dir: &PathBuf, s: &Saved) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(file(dir), serde_json::to_string_pretty(s)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_prefs_reset_boot_logo() {
        let dir = std::env::temp_dir().join(format!("swpl-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.json"), r#"{"bootLogo":"same","rotation":180}"#).unwrap();
        let s = load(&dir);
        assert_eq!(s.boot_logo, BootLogo::Keep);
        assert_eq!(s.rotation, 180);
    }
}
