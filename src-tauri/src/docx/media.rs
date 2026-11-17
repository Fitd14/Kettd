//! 图片读取与尺寸换算（kb-export-word D2 / task 3.4）。
//!
//! 图片在数据目录里是内容寻址的 `assets/<sha256>.<ext>`：块模型只带相对路径，
//! 真正读字节、算 EMU 尺寸都在这里做。
//!
//! 边界：
//! - 只读、不改、不复制原图；字节直接塞进 `word/media/`；
//! - 单图上限 5 MB、正文可用宽度 15 cm（超出按比例缩）；
//! - 读不到 / 超限不报错中断，返回占位文案（导出继续，缺图不影响其余正文）。

use std::path::{Path, PathBuf};

/// 单图字节上限：再大就不嵌入（Word 打开慢，导出包也没必要背着）
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
/// 正文可用宽度 15 cm，1 cm = 360000 EMU
pub const MAX_WIDTH_EMU: u64 = 15 * 360_000;
/// 96 dpi 下 1 px = 9525 EMU
pub const EMU_PER_PX: u64 = 9_525;
/// 读不出尺寸时的兜底（4:3 常见截图比例）
const FALLBACK_PX: (u64, u64) = (600, 450);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaImage {
  pub bytes: Vec<u8>,
  /// 扩展名（小写、不含点），用于 `word/media/image{n}.{ext}`
  pub ext: String,
  pub width_emu: u64,
  pub height_emu: u64,
}

/// 从 `assets/` 目录读图并算好尺寸；失败时 Err 里是可直接写进正文的占位文案。
pub fn load(assets_dir: &Path, asset: &str, alt: Option<&str>) -> Result<MediaImage, String> {
  let missing = || match alt.map(str::trim).filter(|text| !text.is_empty()) {
    Some(text) => format!("[图片缺失：{text}]"),
    None => "[图片缺失]".to_string(),
  };

  let Some(name) = Path::new(asset).file_name().and_then(|value| value.to_str()) else {
    return Err(missing());
  };
  // 只认 assets/ 下的文件名；路径穿越在取 file_name 时已被削掉
  let path: PathBuf = assets_dir.join(name);
  let bytes = match std::fs::read(&path) {
    Ok(bytes) => bytes,
    Err(_) => return Err(missing()),
  };
  if bytes.len() > MAX_IMAGE_BYTES {
    return Err(match alt.map(str::trim).filter(|text| !text.is_empty()) {
      Some(text) => format!("[图片过大未嵌入：{text}]"),
      None => "[图片过大未嵌入]".to_string(),
    });
  }
  let ext = path
    .extension()
    .and_then(|value| value.to_str())
    .map(|value| value.to_ascii_lowercase())
    .filter(|value| matches!(value.as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp"))
    .unwrap_or_else(|| "png".to_string());

  let (width_px, height_px) = dimensions(&bytes).unwrap_or(FALLBACK_PX);
  let (width_emu, height_emu) = fit(width_px, height_px);
  Ok(MediaImage {
    bytes,
    ext,
    width_emu,
    height_emu,
  })
}

/// 按 96 dpi 把像素换算成 EMU，并把宽度限制在正文可用宽度内（等比）。
pub fn fit(width_px: u64, height_px: u64) -> (u64, u64) {
  let width_px = width_px.max(1);
  let height_px = height_px.max(1);
  let raw_width = width_px.saturating_mul(EMU_PER_PX);
  let raw_height = height_px.saturating_mul(EMU_PER_PX);
  if raw_width <= MAX_WIDTH_EMU {
    return (raw_width.max(1), raw_height.max(1));
  }
  let height = (raw_height as u128 * MAX_WIDTH_EMU as u128 / raw_width as u128) as u64;
  (MAX_WIDTH_EMU, height.max(1))
}

/// 从文件头读像素尺寸：支持 PNG / JPEG / GIF / BMP；WebP 等读不出就走兜底。
pub fn dimensions(bytes: &[u8]) -> Option<(u64, u64)> {
  if bytes.len() >= 24 && bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    return Some((u64::from(width), u64::from(height)));
  }
  if bytes.len() >= 10 && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
    let width = u16::from_le_bytes([bytes[6], bytes[7]]);
    let height = u16::from_le_bytes([bytes[8], bytes[9]]);
    return Some((u64::from(width), u64::from(height)));
  }
  if bytes.len() >= 26 && bytes.starts_with(b"BM") {
    let width = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]).unsigned_abs();
    let height = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]).unsigned_abs();
    return Some((u64::from(width), u64::from(height)));
  }
  jpeg_dimensions(bytes)
}

/// JPEG 的尺寸在各帧的 SOF 段里：逐段跳，遇到 SOF0–SOF15（不含 DHT/DAC 等）取高宽。
fn jpeg_dimensions(bytes: &[u8]) -> Option<(u64, u64)> {
  if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
    return None;
  }
  let mut at = 2usize;
  while at + 9 < bytes.len() {
    if bytes[at] != 0xFF {
      at += 1;
      continue;
    }
    let marker = bytes[at + 1];
    // 无长度字段的标记
    if (0xD0..=0xD9).contains(&marker) || marker == 0x01 {
      at += 2;
      continue;
    }
    let length = usize::from(u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]));
    if length < 2 {
      return None;
    }
    let is_sof = (0xC0..=0xC3).contains(&marker)
      || (0xC5..=0xC7).contains(&marker)
      || (0xC9..=0xCB).contains(&marker)
      || (0xCD..=0xCF).contains(&marker);
    if is_sof && at + 9 < bytes.len() {
      let height = u16::from_be_bytes([bytes[at + 5], bytes[at + 6]]);
      let width = u16::from_be_bytes([bytes[at + 7], bytes[at + 8]]);
      return Some((u64::from(width), u64::from(height)));
    }
    at += 2 + length;
  }
  None
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;

  /// 最小合法 PNG 头：签名 + IHDR（宽高按参数写），足够尺寸解析用
  fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    bytes.extend_from_slice(&13u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
  }

  fn temp_dir(tag: &str) -> PathBuf {
    crate::docx::test_dir(&format!("media-{tag}"))
  }

  #[test]
  fn reads_png_dimensions() {
    assert_eq!(dimensions(&png_header(800, 600)), Some((800, 600)));
  }

  #[test]
  fn reads_gif_and_bmp_dimensions() {
    let mut gif = b"GIF89a".to_vec();
    gif.extend_from_slice(&320u16.to_le_bytes());
    gif.extend_from_slice(&240u16.to_le_bytes());
    assert_eq!(dimensions(&gif), Some((320, 240)));

    let mut bmp = b"BM".to_vec();
    bmp.extend_from_slice(&[0u8; 16]);
    bmp.extend_from_slice(&128i32.to_le_bytes());
    bmp.extend_from_slice(&64i32.to_le_bytes());
    assert_eq!(dimensions(&bmp), Some((128, 64)));
  }

  #[test]
  fn fit_scales_down_to_text_width() {
    // 1200px 宽 → 超过 15cm，等比缩放后宽度正好等于上限
    let (width, height) = fit(1200, 600);
    assert_eq!(width, MAX_WIDTH_EMU);
    assert_eq!(height, MAX_WIDTH_EMU / 2);

    // 小图不放大
    let (small_width, _) = fit(96, 96);
    assert_eq!(small_width, 96 * EMU_PER_PX);
  }

  #[test]
  fn loads_existing_image() {
    let dir = temp_dir("ok");
    fs::write(dir.join("abc.png"), png_header(400, 300)).unwrap();
    let image = load(&dir, "assets/abc.png", Some("示意图")).expect("应能读到");
    assert_eq!(image.ext, "png");
    assert_eq!(image.width_emu, 400 * EMU_PER_PX);
    assert_eq!(image.height_emu, 300 * EMU_PER_PX);
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn missing_image_returns_placeholder_text() {
    let dir = temp_dir("missing");
    let err = load(&dir, "assets/none.png", Some("配图")).expect_err("读不到应返回占位");
    assert_eq!(err, "[图片缺失：配图]");
    let err_without_alt = load(&dir, "assets/none.png", None).expect_err("读不到应返回占位");
    assert_eq!(err_without_alt, "[图片缺失]");
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn oversized_image_returns_placeholder_text() {
    let dir = temp_dir("big");
    let mut bytes = png_header(10, 10);
    bytes.resize(MAX_IMAGE_BYTES + 1, 0);
    fs::write(dir.join("big.png"), &bytes).unwrap();
    let err = load(&dir, "assets/big.png", None).expect_err("超限应返回占位");
    assert_eq!(err, "[图片过大未嵌入]");
    let _ = fs::remove_dir_all(&dir);
  }
}