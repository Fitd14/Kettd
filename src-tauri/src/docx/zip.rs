//! 最小 ZIP 写入器（kb-export-word D4）。
//!
//! 只做 `.docx` 需要的那一小撮：store（不压缩）+ UTF-8 文件名 + 中央目录 + EOCD。
//! 不引第三方 crate（依赖冻结线 + crates.io 不可达），CRC32 用查表实现。
//!
//! 边界：
//! - 不写 ZIP64：单文件与总长都按 32 位算，`.docx` 文档不会接近 4 GB；
//! - 不做压缩：store-only 体积换实现稳定，Word 对未压缩条目完全接受；
//! - 条目名一律 `/` 分隔、不以 `/` 开头（OOXML 约定）。

use std::sync::OnceLock;

const LOCAL_SIG: u32 = 0x0403_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const EOCD_SIG: u32 = 0x0605_4b50;
/// 通用位标记第 11 位：文件名按 UTF-8 解释（Word 与解压工具都认）
const FLAG_UTF8: u16 = 0x0800;
/// DOS 时间戳固定 1980-01-01 00:00：导出的文档不需要按写入时刻排序
const DOS_TIME: u16 = 0;
const DOS_DATE: u16 = 0x0021;

struct Entry {
  name: String,
  crc: u32,
  size: u32,
  offset: u32,
}

fn crc_table() -> &'static [u32; 256] {
  static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
  TABLE.get_or_init(|| {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
      let mut value = i as u32;
      for _ in 0..8 {
        value = if value & 1 != 0 {
          (value >> 1) ^ 0xEDB8_8320
        } else {
          value >> 1
        };
      }
      *slot = value;
    }
    table
  })
}

/// 标准 CRC32（IEEE 802.3，ZIP 用）。
pub fn crc32(bytes: &[u8]) -> u32 {
  let table = crc_table();
  let mut crc = 0xFFFF_FFFFu32;
  for byte in bytes {
    let index = ((crc ^ u32::from(*byte)) & 0xFF) as usize;
    crc = (crc >> 8) ^ table[index];
  }
  crc ^ 0xFFFF_FFFF
}

pub struct ZipWriter {
  entries: Vec<Entry>,
  body: Vec<u8>,
}

impl Default for ZipWriter {
  fn default() -> Self {
    Self::new()
  }
}

impl ZipWriter {
  pub fn new() -> Self {
    Self {
      entries: Vec::new(),
      body: Vec::new(),
    }
  }

  /// 追加一个文件条目：名字非法（空 / 超 255 字节 / 以 `/` 开头 / 重名）返回中文错误。
  pub fn add_file(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
    if name.is_empty() {
      return Err("导出条目名不能为空".into());
    }
    if name.starts_with('/') {
      return Err("导出条目名不能以斜杠开头".into());
    }
    if name.len() > u16::MAX as usize {
      return Err("导出条目名过长".into());
    }
    if self.entries.iter().any(|entry| entry.name == name) {
      return Err(format!("导出条目重复：{name}"));
    }
    let offset = u32::try_from(self.body.len()).map_err(|_| "导出内容过大".to_string())?;
    let size = u32::try_from(bytes.len()).map_err(|_| "导出内容过大".to_string())?;
    let crc = crc32(bytes);
    let name_bytes = name.as_bytes();

    self.body.extend_from_slice(&LOCAL_SIG.to_le_bytes());
    self.body.extend_from_slice(&20u16.to_le_bytes()); // version needed
    self.body.extend_from_slice(&FLAG_UTF8.to_le_bytes());
    self.body.extend_from_slice(&0u16.to_le_bytes()); // method = store
    self.body.extend_from_slice(&DOS_TIME.to_le_bytes());
    self.body.extend_from_slice(&DOS_DATE.to_le_bytes());
    self.body.extend_from_slice(&crc.to_le_bytes());
    self.body.extend_from_slice(&size.to_le_bytes()); // compressed
    self.body.extend_from_slice(&size.to_le_bytes()); // uncompressed
    self.body.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    self.body.extend_from_slice(&0u16.to_le_bytes()); // extra len
    self.body.extend_from_slice(name_bytes);
    self.body.extend_from_slice(bytes);

    self.entries.push(Entry {
      name: name.to_string(),
      crc,
      size,
      offset,
    });
    Ok(())
  }

  /// 收口：写中央目录与 EOCD，返回完整字节。
  pub fn finish(self) -> Vec<u8> {
    let cd_start = self.body.len();
    let mut out = self.body;
    for entry in &self.entries {
      let name_bytes = entry.name.as_bytes();
      out.extend_from_slice(&CENTRAL_SIG.to_le_bytes());
      out.extend_from_slice(&20u16.to_le_bytes()); // version made by
      out.extend_from_slice(&20u16.to_le_bytes()); // version needed
      out.extend_from_slice(&FLAG_UTF8.to_le_bytes());
      out.extend_from_slice(&0u16.to_le_bytes()); // method
      out.extend_from_slice(&DOS_TIME.to_le_bytes());
      out.extend_from_slice(&DOS_DATE.to_le_bytes());
      out.extend_from_slice(&entry.crc.to_le_bytes());
      out.extend_from_slice(&entry.size.to_le_bytes());
      out.extend_from_slice(&entry.size.to_le_bytes());
      out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
      out.extend_from_slice(&0u16.to_le_bytes()); // extra
      out.extend_from_slice(&0u16.to_le_bytes()); // comment
      out.extend_from_slice(&0u16.to_le_bytes()); // disk
      out.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
      out.extend_from_slice(&0u32.to_le_bytes()); // external attrs
      out.extend_from_slice(&entry.offset.to_le_bytes());
      out.extend_from_slice(name_bytes);
    }
    let cd_size = out.len() - cd_start;
    let count = self.entries.len() as u16;

    out.extend_from_slice(&EOCD_SIG.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // disk
    out.extend_from_slice(&0u16.to_le_bytes()); // cd disk
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&(cd_size as u32).to_le_bytes());
    out.extend_from_slice(&(cd_start as u32).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment len
    out
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use std::path::PathBuf;

  fn temp_path(name: &str) -> PathBuf {
    let dir = crate::docx::test_dir("zip");
    dir.join(name)
  }

  fn read_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
  }

  fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
  }

  #[test]
  fn crc32_matches_known_vectors() {
    assert_eq!(crc32(b""), 0x0000_0000);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(b"hello"), 0x3610_A686);
  }

  #[test]
  fn empty_zip_is_valid() {
    let bytes = ZipWriter::new().finish();
    assert_eq!(bytes.len(), 22);
    assert_eq!(read_u32(&bytes, 0), EOCD_SIG);
    assert_eq!(read_u16(&bytes, 10), 0);
    assert_eq!(read_u16(&bytes, 8), 0);
  }

  #[test]
  fn writes_entries_and_central_directory() {
    let mut zip = ZipWriter::new();
    zip.add_file("[Content_Types].xml", b"<Types/>").unwrap();
    zip.add_file("word/document.xml", b"<w:document/>").unwrap();
    let bytes = zip.finish();

    fs::write(temp_path("docx.zip"), &bytes).unwrap();

    // 第一条 local header 在偏移 0，CRC 与自算一致
    assert_eq!(read_u32(&bytes, 0), LOCAL_SIG);
    let crc_at = 14;
    let first_payload = b"<Types/>";
    assert_eq!(read_u32(&bytes, crc_at), crc32(first_payload));
    // 偏移：签名 0、版本 4、标志 6、方法 8、时间 10、日期 12、CRC 14
    assert_eq!(read_u16(&bytes, 6), FLAG_UTF8);
    assert_eq!(read_u16(&bytes, 8), 0, "method 必须是 store");

    // EOCD 在末尾，声明 2 条 + 中央目录偏移正确
    let eocd = bytes.len() - 22;
    assert_eq!(read_u32(&bytes, eocd), EOCD_SIG);
    assert_eq!(read_u16(&bytes, eocd + 10), 2);
    let cd_offset = read_u32(&bytes, eocd + 16) as usize;
    assert_eq!(read_u32(&bytes, cd_offset), CENTRAL_SIG);

    // 中央目录第一条能读回文件名
    let name_len = read_u16(&bytes, cd_offset + 28) as usize;
    let name_at = cd_offset + 46;
    assert_eq!(&bytes[name_at..name_at + name_len], b"[Content_Types].xml");

    let _ = fs::remove_file(temp_path("docx.zip"));
  }

  #[test]
  fn rejects_bad_names() {
    let mut zip = ZipWriter::new();
    assert!(zip.add_file("", b"x").is_err());
    assert!(zip.add_file("/abs.xml", b"x").is_err());
    zip.add_file("word/document.xml", b"x").unwrap();
    assert!(zip.add_file("word/document.xml", b"y").is_err(), "重名必须拒绝");
  }
}