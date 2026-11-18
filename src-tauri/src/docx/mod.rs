//! 知识库文档 → `.docx`（知识库导出能力，任务 3.5 / 4.1 的落点）。
//!
//! 分工：
//! - `model.rs` 反序列化前端块模型；
//! - `ooxml.rs` 产出正文 XML 与包内各部件；
//! - `zip.rs` 只做 store-only 打包（无 deflate，零依赖）；
//! - 本文件负责遍历块、收集图片、按 OOXML 部件顺序组包，并提供目标路径解析与落盘。

pub mod media;
pub mod model;
pub mod ooxml;
pub mod zip;

use std::path::{Path, PathBuf};

use model::ExportBlock;
use ooxml::Writer;

/// 块模型 → 完整 `.docx` 字节。
///
/// `assets_dir` 是 `store.assets_dir()` 的绝对路径，图片从这里读；读不到时写占位段落，
/// 不中断导出（spec「图片缺失仍可导出」）。
pub fn build(blocks: &[ExportBlock], assets_dir: &str) -> Result<Vec<u8>, String> {
  let mut writer = Writer::new();
  let mut image_bytes: Vec<Vec<u8>> = Vec::new();
  let root = Path::new(assets_dir);
  walk(blocks, &mut writer, root, &mut image_bytes);

  let media = writer.media().to_vec();
  if media.len() != image_bytes.len() {
    // 理论上不会发生：每成功嵌入一张图就同步压一份字节
    return Err("图片打包失败，请重试".to_string());
  }
  let extensions: Vec<String> = media.iter().map(|item| item.ext.clone()).collect();
  // finish() 会消费 writer，超链接目标必须先取出来
  let hyperlinks = writer.hyperlink_targets().to_vec();
  let document_xml = writer.finish();

  let mut archive = zip::ZipWriter::new();
  archive.add_file("[Content_Types].xml", ooxml::content_types(&extensions).as_bytes())?;
  archive.add_file("_rels/.rels", ooxml::root_rels().as_bytes())?;
  archive.add_file("word/document.xml", document_xml.as_bytes())?;
  archive.add_file(
    "word/_rels/document.xml.rels",
    ooxml::document_rels(&media, &hyperlinks).as_bytes(),
  )?;
  archive.add_file("word/styles.xml", ooxml::styles_xml().as_bytes())?;
  archive.add_file("word/numbering.xml", ooxml::numbering_xml().as_bytes())?;
  for (index, media_ref) in media.iter().enumerate() {
    archive.add_file(
      &format!("word/media/{}", media_ref.file_name),
      &image_bytes[index],
    )?;
  }
  Ok(archive.finish())
}

fn walk(blocks: &[ExportBlock], writer: &mut Writer, assets_dir: &Path, image_bytes: &mut Vec<Vec<u8>>) {
  for block in blocks {
    match block {
      ExportBlock::Heading { level, runs } => writer.heading(*level, runs),
      ExportBlock::Paragraph { runs } => writer.paragraph(runs),
      ExportBlock::Bullets { items } => writer.list(items, false),
      ExportBlock::Numbers { items } => writer.list(items, true),
      ExportBlock::Quote { blocks } => writer.quote(blocks, 0),
      ExportBlock::Code { text, .. } => writer.code(text),
      ExportBlock::Table { header, rows } => writer.table(*header, rows),
      ExportBlock::Image { asset, alt } => match media::load(assets_dir, asset, alt.as_deref()) {
        Ok(image) => {
          writer.image(&image, alt.as_deref());
          image_bytes.push(image.bytes);
        }
        Err(placeholder) => {
          // media::load 的错误就是给用户看的占位文案
          writer.paragraph(&[model::Run::plain(placeholder)]);
        }
      },
    }
  }
}

/// 命令层逻辑（任务 4.1）：条目存在性、空正文、路径校验、生成、落盘。
///
/// 抽成纯函数是为了让命令保持「锁内只做三件事」，也便于单测覆盖四条失败路径。
pub fn export_document(
  item_exists: bool,
  blocks: &[ExportBlock],
  assets_dir: &str,
  target_path: &str,
) -> Result<String, String> {
  if !item_exists {
    return Err("这篇文档已经不存在了".to_string());
  }
  if blocks.is_empty() {
    return Err("正文为空，没有可导出的内容".to_string());
  }
  let target = resolve_target_path(target_path)?;
  let bytes = build(blocks, assets_dir)?;
  write_bytes(&target, &bytes)?;
  Ok(target.to_string_lossy().to_string())
}

/// 校验并补全目标路径（任务 4.1 的前半段，单独放这里以便单测）。
///
/// 规则：绝对路径、父目录存在、扩展名不是 `.docx` 就补上（大小写不敏感）。
pub(crate) fn resolve_target_path(raw: &str) -> Result<PathBuf, String> {
  resolve_target_path_with(raw, "docx")
}

/// 同上，但扩展名由调用方给出：PDF 导出复用同一套路径校验，只换扩展名。
pub(crate) fn resolve_target_path_with(raw: &str, extension: &str) -> Result<PathBuf, String> {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return Err("没有选择导出位置".to_string());
  }
  let candidate = PathBuf::from(trimmed);
  if !candidate.is_absolute() {
    return Err("导出位置必须是完整路径".to_string());
  }
  let parent = candidate
    .parent()
    .ok_or_else(|| "导出位置不合法".to_string())?;
  if !parent.is_dir() {
    return Err("目标文件夹不存在".to_string());
  }
  let has_expected_ext = candidate
    .extension()
    .map(|ext| ext.to_string_lossy().eq_ignore_ascii_case(extension))
    .unwrap_or(false);
  if has_expected_ext {
    Ok(candidate)
  } else {
    let mut with_ext = candidate.into_os_string();
    with_ext.push(".");
    with_ext.push(extension);
    Ok(PathBuf::from(with_ext))
  }
}

/// 同目录临时文件 + rename，失败时删临时文件（不留下半截 `.docx`）。
pub(crate) fn write_bytes(target: &Path, bytes: &[u8]) -> Result<(), String> {
  let parent = target
    .parent()
    .ok_or_else(|| "导出位置不合法".to_string())?;
  let name = target
    .file_name()
    .map(|value| value.to_string_lossy().to_string())
    .unwrap_or_else(|| "导出.docx".to_string());
  let temp = parent.join(format!(".{name}.tmp"));
  if let Err(error) = std::fs::write(&temp, bytes) {
    let _ = std::fs::remove_file(&temp);
    return Err(write_error_text(error.kind()));
  }
  // Windows 上 rename 不覆盖已存在文件：先把旧文件挪成 .bak，成功后再删；
  // 若换名失败则把旧文件挪回去，保证「写入失败不截断原文件」。
  let backup = parent.join(format!(".{name}.bak"));
  let had_existing = target.exists();
  if had_existing {
    let _ = std::fs::remove_file(&backup);
    if let Err(error) = std::fs::rename(target, &backup) {
      let _ = std::fs::remove_file(&temp);
      return Err(write_error_text(error.kind()));
    }
  }
  if let Err(error) = std::fs::rename(&temp, target) {
    let _ = std::fs::remove_file(&temp);
    if had_existing {
      let _ = std::fs::rename(&backup, target);
    }
    return Err(write_error_text(error.kind()));
  }
  if had_existing {
    let _ = std::fs::remove_file(&backup);
  }
  Ok(())
}

fn write_error_text(kind: std::io::ErrorKind) -> String {
  match kind {
    std::io::ErrorKind::PermissionDenied => "没有写入权限，请换一个位置".to_string(),
    std::io::ErrorKind::NotFound => "目标文件夹不存在".to_string(),
    _ => "写入失败，请换一个位置再试".to_string(),
  }
}

/// 单测临时目录：先试 `%TEMP%`，不可写时退到 `target/docx-test-tmp`。
///
/// 门禁与沙箱下 `%TEMP%` 的 ACL 不一定允许当前令牌写入（曾出现
/// `Os { code: 5, kind: PermissionDenied }`），退到构建目录里始终可写且被 git 忽略。
#[cfg(test)]
pub(crate) fn test_dir(tag: &str) -> PathBuf {
  let name = format!("kettd-docx-{tag}-{}", std::process::id());
  let mut candidates: Vec<PathBuf> = Vec::new();
  if let Some(base) = std::env::var_os("KETTD_TEST_TMP") {
    candidates.push(PathBuf::from(base).join(&name));
  }
  candidates.push(std::env::temp_dir().join(&name));
  candidates.push(
    std::env::current_dir()
      .unwrap_or_else(|_| PathBuf::from("."))
      .join("target")
      .join("docx-test-tmp")
      .join(&name),
  );
  for candidate in candidates {
    let _ = std::fs::remove_dir_all(&candidate);
    if std::fs::create_dir_all(&candidate).is_ok() {
      return candidate;
    }
  }
  panic!("没有可写的临时目录（%TEMP% 与 target/ 都失败）");
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::docx::model::{ExportBlock, Run};

  /// 1×1 透明 PNG（67 字节），用于验证图片嵌入
  const PNG_1X1: [u8; 67] = [
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
  ];

  fn temp_dir(tag: &str) -> PathBuf {
    super::test_dir(tag)
  }

  #[test]
  fn build_writes_required_parts_and_body_text() {
    let dir = temp_dir("build");
    std::fs::write(dir.join("abc.png"), PNG_1X1).expect("写测试图");
    let blocks = vec![
      ExportBlock::Heading { level: 1, runs: vec![Run::plain("标题")] },
      ExportBlock::Paragraph { runs: vec![Run::plain("正文内容")] },
      ExportBlock::Code { lang: Some("rust".into()), text: "fn main() {}".into() },
      ExportBlock::Image { asset: "abc.png".into(), alt: Some("示意图".into()) },
    ];
    let bytes = build(&blocks, &dir.to_string_lossy()).expect("生成 docx");
    assert_eq!(&bytes[0..4], b"PK\x03\x04", "必须是 ZIP 头");
    let text = String::from_utf8_lossy(&bytes);
    for part in [
      "[Content_Types].xml",
      "_rels/.rels",
      "word/document.xml",
      "word/_rels/document.xml.rels",
      "word/styles.xml",
      "word/numbering.xml",
      "word/media/image1.png",
    ] {
      assert!(text.contains(part), "缺少部件 {part}");
    }
    assert!(text.contains("正文内容"));
    assert!(text.contains(r#"r:embed="rIdM1""#));
    // EOCD 签名必须在末尾 22 字节内
    let tail = &bytes[bytes.len() - 22..];
    assert_eq!(&tail[0..4], b"PK\x05\x06");
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn build_covers_every_block_kind_with_valid_relationships() {
    use crate::docx::model::ListItem;
    let dir = temp_dir("rich");
    std::fs::write(dir.join("pic.png"), PNG_1X1).expect("写测试图");
    let blocks = vec![
      ExportBlock::Heading { level: 1, runs: vec![Run::plain("项目说明")] },
      ExportBlock::Heading { level: 2, runs: vec![Run::plain("背景")] },
      ExportBlock::Paragraph {
        runs: vec![
          Run { text: "加粗".into(), bold: true, ..Run::default() },
          Run::plain("与"),
          Run { text: "链接".into(), link: Some("https://example.com/doc".into()), ..Run::default() },
          Run { text: "代码".into(), code: true, ..Run::default() },
        ],
      },
      ExportBlock::Bullets {
        items: vec![
          ListItem { depth: 0, runs: vec![Run::plain("一级")] },
          ListItem { depth: 1, runs: vec![Run::plain("二级")] },
        ],
      },
      ExportBlock::Numbers { items: vec![ListItem { depth: 0, runs: vec![Run::plain("步骤")] }] },
      ExportBlock::Quote {
        blocks: vec![
          ExportBlock::Paragraph { runs: vec![Run::plain("外层引用")] },
          ExportBlock::Quote {
            blocks: vec![ExportBlock::Paragraph { runs: vec![Run::plain("内层引用")] }],
          },
        ],
      },
      ExportBlock::Code { lang: None, text: "fn main() {\n  println!(\"hi\");\n}".into() },
      ExportBlock::Table {
        header: true,
        rows: vec![vec!["列一".into(), "列二".into()], vec!["甲".into(), "乙".into()]],
      },
      ExportBlock::Image { asset: "pic.png".into(), alt: Some("配图".into()) },
    ];
    let bytes = build(&blocks, &dir.to_string_lossy()).expect("生成 docx");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("<w:tbl>"));
    assert!(text.contains(r#"<w:numId w:val="1"/>"#));
    assert!(text.contains(r#"<w:numId w:val="2"/>"#));
    assert_eq!(text.matches("<w:hyperlink ").count(), 1);
    assert!(text.contains(r#"Target="https://example.com/doc" TargetMode="External""#));
    assert!(text.contains("内层引用"));
    assert!(text.contains("<w:sectPr>"));
    assert!(text.contains("word/media/image1.png"));
    // 图片关系与超链接关系都在同一份 rels 里，且 rId 不冲突
    assert!(text.contains(r#"Id="rIdM1""#));
    assert!(text.contains(r#"Id="rIdL100""#));
    if let Ok(sample) = std::env::var("KETTD_DOCX_RICH_SAMPLE") {
      std::fs::write(sample, &bytes).expect("写富样本");
    }
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn export_kb_docx_rejects_missing_item() {
    let dir = temp_dir("noitem");
    let target = format!("{}\\a.docx", dir.to_string_lossy());
    let err = export_document(false, &[ExportBlock::Paragraph { runs: vec![Run::plain("x")] }], &dir.to_string_lossy(), &target)
      .unwrap_err();
    assert_eq!(err, "这篇文档已经不存在了");
    assert!(!std::path::Path::new(&target).exists(), "失败不应产生文件");
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn export_kb_docx_rejects_empty_blocks() {
    let dir = temp_dir("empty");
    let target = format!("{}\\a.docx", dir.to_string_lossy());
    let err = export_document(true, &[], &dir.to_string_lossy(), &target).unwrap_err();
    assert_eq!(err, "正文为空，没有可导出的内容");
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn export_kb_docx_rejects_relative_and_missing_parent() {
    let dir = temp_dir("paths");
    let blocks = [ExportBlock::Paragraph { runs: vec![Run::plain("x")] }];
    assert_eq!(
      export_document(true, &blocks, &dir.to_string_lossy(), "相对路径.docx").unwrap_err(),
      "导出位置必须是完整路径"
    );
    let missing = format!("{}\\不存在\\a.docx", dir.to_string_lossy());
    assert_eq!(
      export_document(true, &blocks, &dir.to_string_lossy(), &missing).unwrap_err(),
      "目标文件夹不存在"
    );
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn export_kb_docx_writes_file_and_returns_path() {
    let dir = temp_dir("writedoc");
    let blocks = [ExportBlock::Paragraph { runs: vec![Run::plain("正文")] }];
    let target = format!("{}\\导出结果", dir.to_string_lossy());
    let returned = export_document(true, &blocks, &dir.to_string_lossy(), &target).expect("导出成功");
    assert!(returned.ends_with("导出结果.docx"), "无扩展名时补 .docx：{returned}");
    let written = std::fs::read(&returned).expect("文件应存在");
    assert_eq!(&written[0..4], b"PK\x03\x04");
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn build_keeps_going_when_image_missing() {
    let dir = temp_dir("missing");
    let blocks = vec![
      ExportBlock::Paragraph { runs: vec![Run::plain("前")] },
      ExportBlock::Image { asset: "nope.png".into(), alt: Some("缺图".into()) },
      ExportBlock::Paragraph { runs: vec![Run::plain("后")] },
    ];
    let bytes = build(&blocks, &dir.to_string_lossy()).expect("仍应导出");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("前"));
    assert!(text.contains("后"));
    assert!(text.contains("[图片缺失：缺图]"));
    assert!(!text.contains("word/media/image1.png"));
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn resolve_target_path_appends_extension_and_rejects_relative() {
    let dir = temp_dir("resolve");
    let base = dir.to_string_lossy().to_string();
    let resolved = resolve_target_path(&format!("{base}\\报告")).expect("补扩展名");
    assert_eq!(resolved.extension().unwrap(), "docx");
    let kept = resolve_target_path(&format!("{base}\\报告.DOCX")).expect("大小写不敏感");
    assert!(kept.to_string_lossy().ends_with("报告.DOCX"));
    assert_eq!(
      resolve_target_path("报告.docx").unwrap_err(),
      "导出位置必须是完整路径"
    );
    assert_eq!(
      resolve_target_path(&format!("{base}\\没有这个目录\\a.docx")).unwrap_err(),
      "目标文件夹不存在"
    );
    assert_eq!(resolve_target_path("  ").unwrap_err(), "没有选择导出位置");
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn write_bytes_replaces_existing_and_leaves_no_temp() {
    let dir = temp_dir("write");
    let target = dir.join("out.docx");
    write_bytes(&target, b"first").expect("首次写");
    write_bytes(&target, b"second").expect("覆盖写");
    assert_eq!(std::fs::read(&target).unwrap(), b"second");
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
      .unwrap()
      .filter_map(Result::ok)
      .filter(|entry| {
        let name = entry.file_name().to_string_lossy().to_string();
        name.ends_with(".tmp") || name.ends_with(".bak")
      })
      .collect();
    assert!(leftovers.is_empty(), "不应留下临时文件");
    let _ = std::fs::remove_dir_all(&dir);
  }
}