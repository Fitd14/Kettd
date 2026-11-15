//! 极小的 base64 编解码。
//!
//! 只为一件事存在：字体文件（1–3 MB）要经 IPC 走一趟，用 JSON 数字数组会膨胀到
//! 十倍且解析慢；为这一处新增 base64 依赖又不划算。实现只覆盖标准字母表 + `=` 填充，
//! 解码时容忍换行与空格（前端 `FileReader` 可能带行宽）。

const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode(bytes: &[u8]) -> String {
  let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
  for chunk in bytes.chunks(3) {
    let b0 = chunk[0] as u32;
    let b1 = *chunk.get(1).unwrap_or(&0) as u32;
    let b2 = *chunk.get(2).unwrap_or(&0) as u32;
    let triple = (b0 << 16) | (b1 << 8) | b2;
    out.push(TABLE[(triple >> 18) as usize & 63] as char);
    out.push(TABLE[(triple >> 12) as usize & 63] as char);
    if chunk.len() > 1 {
      out.push(TABLE[(triple >> 6) as usize & 63] as char);
    } else {
      out.push('=');
    }
    if chunk.len() > 2 {
      out.push(TABLE[triple as usize & 63] as char);
    } else {
      out.push('=');
    }
  }
  out
}

pub fn decode(text: &str) -> Result<Vec<u8>, String> {
  let mut out = Vec::with_capacity(text.len() / 4 * 3);
  let mut buffer: u32 = 0;
  let mut bits: u32 = 0;
  for byte in text.bytes() {
    let value = match byte {
      b'A'..=b'Z' => byte - b'A',
      b'a'..=b'z' => byte - b'a' + 26,
      b'0'..=b'9' => byte - b'0' + 52,
      b'+' => 62,
      b'/' => 63,
      b'=' | b'\n' | b'\r' | b' ' | b'\t' => continue,
      _ => return Err("字体数据编码无效".to_string()),
    } as u32;
    buffer = (buffer << 6) | value;
    bits += 6;
    if bits >= 8 {
      bits -= 8;
      out.push((buffer >> bits) as u8);
      if bits == 0 {
        buffer = 0;
      } else {
        buffer &= (1u32 << bits) - 1;
      }
    }
  }
  Ok(out)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn roundtrip_every_padding_case() {
    for len in 0..=6usize {
      let bytes: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
      let text = encode(&bytes);
      assert_eq!(decode(&text).unwrap(), bytes, "长度 {len} 往返失败");
    }
  }

  #[test]
  fn matches_known_vectors() {
    assert_eq!(encode(b""), "");
    assert_eq!(encode(b"f"), "Zg==");
    assert_eq!(encode(b"fo"), "Zm8=");
    assert_eq!(encode(b"foo"), "Zm9v");
    assert_eq!(encode(b"foobar"), "Zm9vYmFy");
  }

  #[test]
  fn decode_tolerates_wrapping_and_rejects_garbage() {
    assert_eq!(decode("Zm9v\nYmFy").unwrap(), b"foobar".to_vec());
    assert_eq!(decode("Zm9v YmFy").unwrap(), b"foobar".to_vec());
    assert!(decode("Zm9v$mFy").is_err());
  }

  #[test]
  fn large_buffer_roundtrip() {
    let bytes: Vec<u8> = (0..300_000).map(|i| (i % 251) as u8).collect();
    assert_eq!(decode(&encode(&bytes)).unwrap(), bytes);
  }
}