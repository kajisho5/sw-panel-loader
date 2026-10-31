//! 画像を画面ぴったり（縦480×横1920）に整える。
//! 画面に表示されるものとプレビューを同じ処理から作り、見た目と結果をそろえる。

use std::io::Cursor;
use std::path::Path;

use image::{imageops::FilterType, DynamicImage, ImageDecoder, ImageReader, Rgb, RgbImage};
use serde::{Deserialize, Serialize};

use crate::protocol::{PANEL_H, PANEL_W};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// 全体を表示（余白あり）
    Contain,
    /// いっぱいに切り抜き
    Cover,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub source_path: String,
    /// 時計回り 0 / 90 / 180 / 270
    pub rotation: u16,
    pub fit: Fit,
    /// "#rrggbb"
    pub pad_color: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub file_name: String,
}

pub fn parse_color(s: &str) -> Rgb<u8> {
    let h = s.trim().trim_start_matches('#');
    if h.len() == 6 {
        if let Ok(v) = u32::from_str_radix(h, 16) {
            return Rgb([(v >> 16) as u8, (v >> 8) as u8, v as u8]);
        }
    }
    Rgb([0, 0, 0])
}

/// 写真の向き情報（EXIF）も反映して読み込む
pub fn load(path: &Path) -> anyhow::Result<DynamicImage> {
    let reader = ImageReader::open(path)
        .map_err(|e| anyhow::anyhow!("画像を開けませんでした：{e}"))?
        .with_guessed_format()?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| anyhow::anyhow!("この形式の画像は読めません（PNG・JPEG・BMP・WebPに対応）"))?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder)
        .map_err(|e| anyhow::anyhow!("画像を読み込めませんでした：{e}"))?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Ok(img)
}

pub fn source_info(path: &Path) -> anyhow::Result<SourceInfo> {
    let img = load(path)?;
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    Ok(SourceInfo {
        width: img.width(),
        height: img.height(),
        bytes,
        file_name: path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
    })
}

fn rotate(img: DynamicImage, deg: u16) -> DynamicImage {
    match deg % 360 {
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => img,
    }
}

/// 画面ぴったりの画像を作る
pub fn compose(img: DynamicImage, rotation: u16, fit: Fit, pad: Rgb<u8>) -> RgbImage {
    let img = rotate(img, rotation).to_rgb8();
    let (sw, sh) = (img.width().max(1) as f64, img.height().max(1) as f64);
    let (pw, ph) = (PANEL_W as f64, PANEL_H as f64);
    let scale = match fit {
        Fit::Contain => (pw / sw).min(ph / sh),
        Fit::Cover => (pw / sw).max(ph / sh),
    };
    let nw = ((sw * scale).round() as u32).max(1);
    let nh = ((sh * scale).round() as u32).max(1);
    let resized = image::imageops::resize(&img, nw, nh, FilterType::Lanczos3);
    let mut canvas = RgbImage::from_pixel(PANEL_W, PANEL_H, pad);
    let x = (PANEL_W as i64 - nw as i64) / 2;
    let y = (PANEL_H as i64 - nh as i64) / 2;
    image::imageops::overlay(&mut canvas, &resized, x, y);
    canvas
}

pub fn render_job(job: &Job) -> anyhow::Result<RgbImage> {
    let img = load(Path::new(&job.source_path))?;
    Ok(compose(img, job.rotation, job.fit, parse_color(&job.pad_color)))
}

pub fn encode_png(img: &RgbImage) -> anyhow::Result<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)?;
    Ok(out.into_inner())
}

/// 画面に送る用のPNG（公開ソフトと同じ 8bit RGBA。透明度は不透明で埋める）
pub fn encode_panel_png(img: &RgbImage) -> anyhow::Result<Vec<u8>> {
    let rgba = DynamicImage::ImageRgb8(img.clone()).to_rgba8();
    let mut out = Cursor::new(Vec::new());
    rgba.write_to(&mut out, image::ImageFormat::Png)?;
    Ok(out.into_inner())
}

pub fn encode_jpeg(img: &RgbImage, quality: u8) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    enc.encode_image(img)?;
    Ok(out)
}

/// 公式ソフトと同じく、指定の大きさ以下になるまで画質を下げてJPEGにする
pub fn encode_jpeg_under(img: &RgbImage, max_bytes: usize) -> anyhow::Result<Vec<u8>> {
    let mut q = 95u8;
    loop {
        let j = encode_jpeg(img, q)?;
        if j.len() <= max_bytes || q <= 40 {
            anyhow::ensure!(j.len() <= max_bytes, "起動ロゴを{}KB以下にできませんでした", max_bytes / 1024);
            return Ok(j);
        }
        q -= 5;
    }
}

/// 起動ロゴ用：真っ黒
pub fn black_panel() -> RgbImage {
    RgbImage::from_pixel(PANEL_W, PANEL_H, Rgb([0, 0, 0]))
}

/// 画面のプレビュー用（縮小PNGをdata URLで返す）
pub fn preview_data_url(img: &RgbImage) -> anyhow::Result<String> {
    use base64::Engine;
    let small = image::imageops::resize(img, PANEL_W / 2, PANEL_H / 2, FilterType::Triangle);
    let png = encode_png(&small)?;
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([255, 255, 255])))
    }

    #[test]
    fn always_panel_size() {
        for (w, h) in [(1080, 4320), (1920, 1080), (480, 1680), (10, 10)] {
            for fit in [Fit::Contain, Fit::Cover] {
                for r in [0, 90, 180, 270] {
                    let c = compose(solid(w, h), r, fit, Rgb([0, 0, 0]));
                    assert_eq!((c.width(), c.height()), (480, 1920));
                }
            }
        }
    }

    #[test]
    fn contain_leaves_padding_for_short_image() {
        // 480×1680 の画像は上下に余白が出る（＝起動ロゴが透けない）
        let c = compose(solid(480, 1680), 0, Fit::Contain, Rgb([0, 0, 0]));
        assert_eq!(c.get_pixel(240, 5), &Rgb([0, 0, 0]));
        assert_eq!(c.get_pixel(240, 960), &Rgb([255, 255, 255]));
    }

    #[test]
    fn cover_fills_everything() {
        let c = compose(solid(1920, 1080), 0, Fit::Cover, Rgb([0, 0, 0]));
        assert_eq!(c.get_pixel(0, 0), &Rgb([255, 255, 255]));
        assert_eq!(c.get_pixel(479, 1919), &Rgb([255, 255, 255]));
    }

    #[test]
    fn landscape_rotated_fills_portrait() {
        // 横長 1920×480 を90°回すとぴったり
        let c = compose(solid(1920, 480), 90, Fit::Contain, Rgb([0, 0, 0]));
        assert_eq!(c.get_pixel(0, 0), &Rgb([255, 255, 255]));
    }

    #[test]
    fn panel_png_is_rgba() {
        let png = encode_panel_png(&black_panel()).unwrap();
        // IHDR: 幅480・高さ1920・8bit・カラータイプ6(RGBA)
        assert_eq!(&png[16..24], &[0, 0, 1, 0xe0, 0, 0, 7, 0x80]);
        assert_eq!(png[24], 8);
        assert_eq!(png[25], 6);
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#ff8000"), Rgb([255, 128, 0]));
        assert_eq!(parse_color("bad"), Rgb([0, 0, 0]));
    }
}
