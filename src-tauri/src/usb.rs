//! USBで画面とやり取りする。
//! 1回の操作ごとに開いて閉じる（ずっと握らないので、公式ソフトとも取り合いになりにくい）。

use std::time::Duration;

use rusb::{Direction, GlobalContext, TransferType};

use crate::logln;
use crate::protocol::{self, cmd, DeviceSettings, Storage, PACKET_LEN, PRODUCT_ID_88, VENDOR_ID};

const TIMEOUT: Duration = Duration::from_millis(3000);
const TIMEOUT_BIG: Duration = Duration::from_millis(15000);

#[derive(Debug, thiserror::Error)]
pub enum UsbError {
    #[error("画面が見つかりません。USBケーブルを挿し直してください")]
    NotFound,
    #[error("画面は見つかりましたが開けませんでした。公式のTURZXソフトを閉じてから、もう一度試してください（{0}）")]
    Busy(String),
    #[error("画面が応答しません。USB（電源）を抜いて10秒待ってから挿し直してください（{0}）")]
    Io(String),
}

/// 画面を使う処理は1つずつ（書き込み中に状態確認が割り込まないように）
static USB_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub struct Panel {
    // 先に宣言したものから先に片付くので、handle を閉じてから鍵を外す
    handle: rusb::DeviceHandle<GlobalContext>,
    ep_out: u8,
    ep_in: u8,
    iface: u8,
    _guard: std::sync::MutexGuard<'static, ()>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    pub connected: bool,
    /// 8.8インチ以外の TURZX 画面が挿さっているとき
    pub other_model: Option<String>,
}

/// 挿さっているかだけを見る（開かない）
pub fn presence() -> Presence {
    let mut p = Presence { connected: false, other_model: None };
    if let Ok(list) = rusb::devices() {
        for d in list.iter() {
            if let Ok(desc) = d.device_descriptor() {
                if desc.vendor_id() == VENDOR_ID {
                    if desc.product_id() == PRODUCT_ID_88 {
                        p.connected = true;
                        return p;
                    }
                    p.other_model = Some(format!("{:04X}", desc.product_id()));
                }
            }
        }
    }
    p
}

impl Panel {
    pub fn open() -> Result<Panel, UsbError> {
        let guard = USB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let list = rusb::devices().map_err(|e| UsbError::Io(e.to_string()))?;
        let dev = list
            .iter()
            .find(|d| {
                d.device_descriptor()
                    .map(|x| x.vendor_id() == VENDOR_ID && x.product_id() == PRODUCT_ID_88)
                    .unwrap_or(false)
            })
            .ok_or(UsbError::NotFound)?;
        let handle = dev.open().map_err(|e| UsbError::Busy(e.to_string()))?;
        let cfg = dev
            .active_config_descriptor()
            .or_else(|_| dev.config_descriptor(0))
            .map_err(|e| UsbError::Io(e.to_string()))?;
        let mut found = None;
        'outer: for iface in cfg.interfaces() {
            for alt in iface.descriptors() {
                let mut out = None;
                let mut inn = None;
                for ep in alt.endpoint_descriptors() {
                    if ep.transfer_type() != TransferType::Bulk {
                        continue;
                    }
                    match ep.direction() {
                        Direction::Out => out = out.or(Some(ep.address())),
                        Direction::In => inn = inn.or(Some(ep.address())),
                    }
                }
                if let (Some(o), Some(i)) = (out, inn) {
                    found = Some((alt.interface_number(), o, i));
                    break 'outer;
                }
            }
        }
        let (iface, ep_out, ep_in) = found.ok_or_else(|| UsbError::Io("送受信の口が見つかりません".into()))?;
        #[cfg(target_os = "linux")]
        {
            let _ = handle.set_auto_detach_kernel_driver(true);
        }
        handle.claim_interface(iface).map_err(|e| UsbError::Busy(e.to_string()))?;
        logln!("接続しました（口 {iface}, 出 {ep_out:#04x}, 入 {ep_in:#04x}）");
        Ok(Panel { handle, ep_out, ep_in, iface, _guard: guard })
    }

    fn flush_in(&self) {
        let mut buf = [0u8; PACKET_LEN];
        for _ in 0..5 {
            if self.handle.read_bulk(self.ep_in, &mut buf, Duration::from_millis(80)).is_err() {
                break;
            }
        }
    }

    /// 送って、512バイトの返事を受け取る
    pub fn exchange(&self, data: &[u8], big: bool) -> Result<Vec<u8>, UsbError> {
        let t = if big { TIMEOUT_BIG } else { TIMEOUT };
        let mut off = 0;
        while off < data.len() {
            let n = self
                .handle
                .write_bulk(self.ep_out, &data[off..], t)
                .map_err(|e| UsbError::Io(format!("送信 {e}")))?;
            if n == 0 {
                return Err(UsbError::Io("送信できませんでした".into()));
            }
            off += n;
        }
        let mut buf = vec![0u8; PACKET_LEN];
        let n = self
            .handle
            .read_bulk(self.ep_in, &mut buf, t)
            .map_err(|e| UsbError::Io(format!("受信 {e}")))?;
        buf.truncate(n);
        self.flush_in();
        Ok(buf)
    }

    fn run(&self, name: &str, data: Vec<u8>) -> Result<Vec<u8>, UsbError> {
        let r = self.exchange(&data, false)?;
        logln!("{name} → {}", crate::log::hex_head(&r, 20));
        Ok(r)
    }

    pub fn sync(&self) -> Result<(), UsbError> {
        self.run("同期", protocol::simple(cmd::SYNC)).map(|_| ())
    }

    pub fn storage(&self) -> Result<Storage, UsbError> {
        let r = self.run("容量", protocol::simple(cmd::STORAGE))?;
        protocol::parse_storage(&r).ok_or_else(|| UsbError::Io("容量の返事が短すぎます".into()))
    }

    pub fn list_dir(&self, dir: &str) -> Result<Vec<String>, UsbError> {
        let pkt = protocol::with_path(cmd::LIST_DIR, dir).map_err(|e| UsbError::Io(e.to_string()))?;
        let mut all = Vec::new();
        // 公式ソフトと同じく、最大20回に分けて受け取る
        let mut prev: Option<Vec<u8>> = None;
        for i in 0..20 {
            let r = self.exchange(&pkt, false)?;
            if i == 0 {
                logln!("一覧 {dir} → {}", crate::log::hex_head(&r, 24));
            }
            // 空、または前回と同じ中身（＝続きがない）なら終わり
            let same = prev.as_ref().map(|p| p.get(8..) == r.get(8..)).unwrap_or(false);
            if r.iter().all(|&b| b == 0) || same {
                break;
            }
            all.extend_from_slice(&r);
            prev = Some(r);
        }
        let names = protocol::parse_dir_listing(&all);
        logln!("一覧 {dir}：{} 件 {:?}", names.len(), names);
        Ok(names)
    }

    pub fn delete(&self, device_path: &str) -> Result<(), UsbError> {
        let pkt = protocol::with_path(cmd::DELETE, device_path).map_err(|e| UsbError::Io(e.to_string()))?;
        self.run(&format!("削除 {device_path}"), pkt).map(|_| ())
    }

    /// ファイルを書き込む。進み具合は on_progress(送った量, 全体)
    pub fn upload(
        &self,
        device_path: &str,
        data: &[u8],
        mut on_progress: impl FnMut(usize, usize),
    ) -> Result<(), UsbError> {
        let io = |e: anyhow::Error| UsbError::Io(e.to_string());
        logln!("書き込み開始 {device_path}（{} バイト）", data.len());
        if data.len() < protocol::SMALL_FILE_LIMIT {
            let pkt = protocol::write_small_file(device_path, data).map_err(io)?;
            let r = self.exchange(&pkt, true)?;
            logln!("小ファイル → {}", crate::log::hex_head(&r, 20));
            on_progress(data.len(), data.len());
            return Ok(());
        }
        let r = self.exchange(&protocol::with_path(cmd::CREATE_FILE, device_path).map_err(io)?, false)?;
        logln!("ファイル作成 → {}", crate::log::hex_head(&r, 20));
        let mut sent = 0;
        for chunk in data.chunks(protocol::MAX_CHUNK) {
            let r = self.exchange(&protocol::write_chunk(chunk), true)?;
            sent += chunk.len();
            logln!("送信 {sent}/{} → {}", data.len(), crate::log::hex_head(&r, 12));
            on_progress(sent, data.len());
        }
        Ok(())
    }

    /// 画像をすぐ表示する。返事の2バイト目が200なら成功
    pub fn play_image(&self, device_path: &str) -> Result<bool, UsbError> {
        let pkt = protocol::with_path(cmd::PLAY_IMAGE, device_path).map_err(|e| UsbError::Io(e.to_string()))?;
        let r = self.run(&format!("表示 {device_path}"), pkt)?;
        Ok(r.get(1) == Some(&200))
    }

    pub fn brightness(&self, level: u8) -> Result<(), UsbError> {
        self.run(&format!("明るさ {level}"), protocol::brightness(level)).map(|_| ())
    }

    pub fn save_settings(&self, s: &DeviceSettings) -> Result<(), UsbError> {
        self.run(&format!("設定保存 {s:?}"), protocol::save_settings(s)).map(|_| ())
    }

    /// 再起動（返事が来ないことがあるので、送れたら成功とみなす）
    pub fn restart(&self) -> Result<(), UsbError> {
        let pkt = protocol::simple(cmd::RESTART);
        self.handle
            .write_bulk(self.ep_out, &pkt, TIMEOUT)
            .map_err(|e| UsbError::Io(format!("再起動 {e}")))?;
        logln!("再起動を送りました");
        Ok(())
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(self.iface);
    }
}
