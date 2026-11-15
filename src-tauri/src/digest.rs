#![allow(dead_code)]

//! 无依赖摘要 / 编码工具（kb-storage-backup 1.1）。
//!
//! 边界：
//! - **不引入新 crate**：SHA-256、base64 均为仓内纯 Rust 实现；只用于资源内容寻址
//!   （`assets/<sha256>.<ext>`）与备份内容指纹，**不用于安全加密**。
//! - 全部纯函数、不碰 IO；错误为中文短句，不含路径与堆栈。
//! - 输出口径：`sha256_hex` 固定 64 位小写十六进制；`base64` 标准 alphabet + padding。

// ---------------------------------------------------------------- SHA-256

/// 前 64 个素数立方根小数部分取前 32 位（SHA-256 标准常量表）
const SHA256_K: [u32; 64] = [
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// 初始哈希值：前 8 个素数平方根小数部分取前 32 位（SHA-256 标准常量）
const SHA256_H0: [u32; 8] = [
  0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

const HEX_LOWER: [u8; 16] = *b"0123456789abcdef";

/// 标准 SHA-256（FIPS 180-4），返回 64 位小写十六进制。
pub fn sha256_hex(bytes: &[u8]) -> String {
  let digest = sha256_digest(bytes);
  let mut out = String::with_capacity(64);
  for byte in digest {
    out.push(HEX_LOWER[(byte >> 4) as usize] as char);
    out.push(HEX_LOWER[(byte & 0x0f) as usize] as char);
  }
  out
}

/// 原始 32 字节摘要（内部实现；测试与后续批次的交叉校验用）。
fn sha256_digest(bytes: &[u8]) -> [u8; 32] {
  let mut h = SHA256_H0;
  // padding：0x80 + 若干 0x00，使 (len + 1 + zeros) % 64 == 56，再补 8 字节大端位长
  let mut message = Vec::with_capacity(bytes.len() + 72);
  message.extend_from_slice(bytes);
  message.push(0x80);
  while message.len() % 64 != 56 {
    message.push(0x00);
  }
  let bit_len = (bytes.len() as u64).wrapping_mul(8);
  message.extend_from_slice(&bit_len.to_be_bytes());

  for chunk in message.chunks_exact(64) {
    let mut w = [0u32; 64];
    for i in 0..16 {
      w[i] = u32::from_be_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
    }
    for i in 16..64 {
      let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
      let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
      w[i] = w[i - 16]
        .wrapping_add(s0)
        .wrapping_add(w[i - 7])
        .wrapping_add(s1);
    }

    let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
      (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
    for i in 0..64 {
      let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
      let ch = (e & f) ^ ((!e) & g);
      let temp1 = hh
        .wrapping_add(s1)
        .wrapping_add(ch)
        .wrapping_add(SHA256_K[i])
        .wrapping_add(w[i]);
      let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
      let maj = (a & b) ^ (a & c) ^ (b & c);
      let temp2 = s0.wrapping_add(maj);
      hh = g;
      g = f;
      f = e;
      e = d.wrapping_add(temp1);
      d = c;
      c = b;
      b = a;
      a = temp1.wrapping_add(temp2);
    }

    h[0] = h[0].wrapping_add(a);
    h[1] = h[1].wrapping_add(b);
    h[2] = h[2].wrapping_add(c);
    h[3] = h[3].wrapping_add(d);
    h[4] = h[4].wrapping_add(e);
    h[5] = h[5].wrapping_add(f);
    h[6] = h[6].wrapping_add(g);
    h[7] = h[7].wrapping_add(hh);
  }

  let mut out = [0u8; 32];
  for (i, word) in h.iter().enumerate() {
    out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
  }
  out
}

// ---------------------------------------------------------------- base64

const B64_ALPHABET: [u8; 64] =
  *b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// 标准 base64 编码（RFC 4648 alphabet，带 `=` padding）。
pub fn base64_encode(bytes: &[u8]) -> String {
  let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
  for chunk in bytes.chunks(3) {
    let b0 = chunk[0] as u32;
    let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
    let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
    let triple = (b0 << 16) | (b1 << 8) | b2;
    out.push(B64_ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
    out.push(B64_ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
    if chunk.len() > 1 {
      out.push(B64_ALPHABET[((triple >> 6) & 0x3f) as usize] as char);
    } else {
      out.push('=');
    }
    if chunk.len() > 2 {
      out.push(B64_ALPHABET[(triple & 0x3f) as usize] as char);
    } else {
      out.push('=');
    }
  }
  out
}

fn base64_value(byte: u8) -> Option<u8> {
  match byte {
    b'A'..=b'Z' => Some(byte - b'A'),
    b'a'..=b'z' => Some(byte - b'a' + 26),
    b'0'..=b'9' => Some(byte - b'0' + 52),
    b'+' => Some(62),
    b'/' => Some(63),
    _ => None,
  }
}

/// 标准 base64 解码：兼容带 / 不带 padding，容忍首尾空白；非法字符或非法长度返回中文错误。
pub fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
  let text = input.trim();
  let bytes = text.as_bytes();

  // 尾部 padding（0/1/2 个 '='；超过 2 个必然非法）
  let mut pad = 0;
  while pad < bytes.len() && bytes[bytes.len() - 1 - pad] == b'=' {
    pad += 1;
  }
  if pad > 2 {
    return Err("Base64 长度不合法".to_string());
  }
  let body = &bytes[..bytes.len() - pad];
  // 先查非法字符（含内嵌 '='），再查长度：两者同时出错时优先报「非法字符」，提示更准确
  if body.iter().any(|byte| base64_value(*byte).is_none()) {
    return Err("Base64 内容包含非法字符".to_string());
  }
  // 合法无填充长度余数为 0/2/3；余数 1 永远无法解码
  if body.len() % 4 == 1 {
    return Err("Base64 长度不合法".to_string());
  }
  if pad > 0 && bytes.len() % 4 != 0 {
    return Err("Base64 长度不合法".to_string());
  }

  let mut out = Vec::with_capacity((body.len() + 3) / 4 * 3);
  let mut buffer: u32 = 0;
  let mut bits: u32 = 0;
  for &byte in body {
    let value = base64_value(byte).ok_or_else(|| "Base64 内容包含非法字符".to_string())?;
    buffer = (buffer << 6) | value as u32;
    bits += 6;
    if bits >= 8 {
      bits -= 8;
      out.push(((buffer >> bits) & 0xff) as u8);
      buffer &= (1u32 << bits) - 1;
    }
  }
  Ok(out)
}

// ---------------------------------------------------------------- data URL

/// `data:image/<mime>;base64,<payload>` 的解析结果。
///
/// `mime` 保存完整媒体类型并归一为小写（如 `image/png`、`image/webp`），
/// 后续批次据此映射扩展名；`bytes` 为解码后的原始字节。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataUrl {
  pub mime: String,
  pub bytes: Vec<u8>,
}

/// 解析 `data:image/<mime>;base64,<payload>`。
///
/// 非 data URL、非 base64 编码、空负载、非 `image/*` 一律返回中文错误（不含路径/堆栈）。
/// 解码失败同样报错——调用方若能接受「保留原内联数据」需自行降级，本函数不静默吞错。
pub fn parse_data_url(url: &str) -> Result<DataUrl, String> {
  let rest = url
    .get(..5)
    .filter(|prefix| prefix.eq_ignore_ascii_case("data:"))
    .map(|_| &url[5..])
    .ok_or_else(|| "不是有效的图片数据地址".to_string())?;
  let comma = rest
    .find(',')
    .ok_or_else(|| "不是有效的图片数据地址".to_string())?;
  let header = &rest[..comma];
  let payload = &rest[comma + 1..];

  let header_lower = header.to_ascii_lowercase();
  let media = if header_lower.ends_with(";base64") {
    header[..header.len() - ";base64".len()].trim()
  } else {
    return Err("仅支持 base64 编码的图片数据".to_string());
  };
  let media_lower = media.to_ascii_lowercase();
  if !media_lower.starts_with("image/") {
    return Err("仅支持图片类型的数据".to_string());
  }
  let subtype = &media_lower["image/".len()..];
  if subtype.is_empty() || subtype.contains(';') || subtype.contains(',') {
    return Err("图片类型无法识别".to_string());
  }

  if payload.is_empty() {
    return Err("图片数据为空".to_string());
  }
  let bytes = base64_decode(payload).map_err(|_| "图片数据无法解析".to_string())?;
  if bytes.is_empty() {
    return Err("图片数据为空".to_string());
  }
  Ok(DataUrl {
    mime: media_lower,
    bytes,
  })
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
  use super::*;

  // 标准向量来源：FIPS 180-4 / RFC 4648 / 常见公开测试集
  #[test]
  fn sha256_matches_standard_vectors() {
    assert_eq!(
      sha256_hex(b""),
      "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
      sha256_hex(b"abc"),
      "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
      sha256_hex(b"The quick brown fox jumps over the lazy dog"),
      "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"
    );
  }

  #[test]
  fn sha256_is_64_lowercase_hex_and_crosses_block_boundary() {
    // 55 / 56 / 64 / 1000 字节：覆盖 padding 的边界与多分块
    for len in [55usize, 56, 63, 64, 65, 1000] {
      let data = vec![0x61u8; len];
      let hex = sha256_hex(&data);
      assert_eq!(hex.len(), 64, "{len} 字节摘要应为 64 位");
      assert!(hex.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
    }
    // 标准长串向量（> 1 个分块）
    let long = "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    assert_eq!(
      sha256_hex(long.as_bytes()),
      "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
  }

  #[test]
  fn base64_encodes_rfc4648_vectors() {
    let cases: [(&[u8], &str); 7] = [
      (b"", ""),
      (b"f", "Zg=="),
      (b"fo", "Zm8="),
      (b"foo", "Zm9v"),
      (b"foob", "Zm9vYg=="),
      (b"fooba", "Zm9vYmE="),
      (b"foobar", "Zm9vYmFy"),
    ];
    for (input, expected) in cases {
      assert_eq!(base64_encode(input), expected);
      assert_eq!(base64_decode(expected).unwrap(), input, "带 padding 解码往返");
    }
  }

  #[test]
  fn base64_decodes_without_padding() {
    assert_eq!(base64_decode("Zg").unwrap(), b"f");
    assert_eq!(base64_decode("Zm8").unwrap(), b"fo");
    assert_eq!(base64_decode("Zm9vYg").unwrap(), b"foob");
    assert_eq!(base64_decode("  Zm9vYmFy  ").unwrap(), b"foobar", "首尾空白容忍");
  }

  #[test]
  fn base64_rejects_invalid_chars_and_lengths() {
    let bad_char = base64_decode("Zm9v!").unwrap_err();
    assert!(bad_char.contains("非法字符"), "实际: {bad_char}");
    let bad_char_interior = base64_decode("Zm9v Yg==").unwrap_err();
    assert!(bad_char_interior.contains("非法字符"), "实际: {bad_char_interior}");
    let bad_eq_inside = base64_decode("Z=g=").unwrap_err();
    assert!(bad_eq_inside.contains("非法字符"), "实际: {bad_eq_inside}");

    for bad in ["Z", "Zg=", "Zm9vY", "Zg==="] {
      let err = base64_decode(bad).unwrap_err();
      assert!(err.contains("长度不合法"), "{bad} 实际: {err}");
    }
  }

  #[test]
  fn base64_roundtrips_all_byte_values() {
    let bytes: Vec<u8> = (0u8..=255).collect();
    let encoded = base64_encode(&bytes);
    assert_eq!(base64_decode(&encoded).unwrap(), bytes);
  }

  #[test]
  fn parse_data_url_extracts_mime_and_bytes() {
    let parsed = parse_data_url("data:image/png;base64,aGVsbG8=").unwrap();
    assert_eq!(parsed.mime, "image/png");
    assert_eq!(parsed.bytes, b"hello");

    let webp = parse_data_url("data:image/webp;base64,aGVsbG8=").unwrap();
    assert_eq!(webp.mime, "image/webp");

    let upper = parse_data_url("DATA:IMAGE/JPEG;BASE64,aGVsbG8=").unwrap();
    assert_eq!(upper.mime, "image/jpeg", "scheme/media/base64 大小写归一");
  }

  #[test]
  fn parse_data_url_rejects_invalid_inputs() {
    let cases: [(&str, &str); 8] = [
      ("https://example.com/a.png", "不是有效的"),
      ("data:image/png;base64", "不是有效的"),
      ("data:image/png,abc", "仅支持 base64"),
      ("data:image/png;utf8,abc", "仅支持 base64"),
      ("data:text/plain;base64,aGVsbG8=", "图片类型"),
      ("data:image/;base64,aGVsbG8=", "无法识别"),
      ("data:image/png;base64,", "图片数据为空"),
      ("data:image/png;base64,!!!!", "无法解析"),
    ];
    for (input, expected) in cases {
      let err = parse_data_url(input).unwrap_err();
      assert!(err.contains(expected), "{input} 实际: {err}");
      assert!(!err.contains('/') && !err.contains('\\'), "错误不含路径片段: {err}");
    }
  }
}