//! 画面とのやり取りの記録。アプリ内の「詳しい記録」に出し、コピーして送れる。

use std::sync::Mutex;

static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());
const MAX_LINES: usize = 600;

pub fn push(msg: impl AsRef<str>) {
    let line = format!("{} {}", chrono::Local::now().format("%H:%M:%S%.3f"), msg.as_ref());
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
