//! 画面とのやり取りの記録。アプリ内の「詳しい記録」に出し、コピーして送れる。

use std::sync::Mutex;

static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static FILE: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

/// 記録をファイルにも残す（アプリを閉じても後から見られるように）
pub fn init_file(path: std::path::PathBuf) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = FILE.lock() {
        *f = Some(path);
    }
}
const MAX_LINES: usize = 600;

pub fn push(msg: impl AsRef<str>) {
    let line = format!("{} {}", chrono::Local::now().format("%H:%M:%S%.3f"), msg.as_ref());
    if let Ok(f) = FILE.lock() {
        if let Some(p) = f.as_ref() {
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
                let _ = writeln!(file, "{line}");
            }
        }
    }
    if let Ok(mut l) = LINES.lock() {
        l.push(line);
        let n = l.len();
        if n > MAX_LINES {
            l.drain(0..n - MAX_LINES);
        }
    }
}

pub fn all() -> Vec<String> {
    LINES.lock().map(|l| l.clone()).unwrap_or_default()
}

/// 返事の先頭を16進で短く書く
pub fn hex_head(b: &[u8], n: usize) -> String {
    b.iter().take(n).map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

#[macro_export]
macro_rules! logln {
    ($($arg:tt)*) => { $crate::log::push(format!($($arg)*)) };
}
