//! 知识库最小地基（frame H1b · ADR-0007 后续知识网的"库"侧）。
//!
//! 边界（反功能广度红线）：
//! - flomo 式片段（title + body_md + tags），**不做**长文档编辑器/双链/图谱；
//! - 检索 = 内存线性扫（条目 >5k 或 P95 >200ms 才考虑依赖，架构 §9）；
//! - 任务经 `Task.kb_refs` **单向**引用本库：KB 侧永不持有 Task（ACL，架构 §3），
//!   命门假设（用户真会往里记）失败时本模块可整体退役而不伤 Task 聚合。
//! - 独立 notes.json：与用户数据物理隔离（frame D1），原子写、v1 不轮转（H2 成熟化）。

use std::collections::HashSet;

use crate::digest::{parse_data_url, sha256_hex};
use crate::models::{KbFolder, KbItem};

pub const NOTES_FILE_MAX_ITEMS: usize = 5_000;

/// 从正文里抽出的图片资源。
///
/// 落盘文件名固定为 `assets/<sha256>.<ext>`（调用方经 `StoreBackend::write_asset` 写入），
/// 正文里只保留该相对路径，**不写绝对路径**：数据目录可迁移，绝对路径一挪就坏。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedAsset {
  pub sha256: String,
  pub ext: String,
  pub bytes: Vec<u8>,
}

/// 抽图结果：改写后正文 + 待落盘资源（按 sha 去重）+ 保留内联的图片数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageExtractReport {
  pub body: String,
  pub assets: Vec<ExtractedAsset>,
  /// 识别为图片数据但无法外置的张数（非白名单 MIME / 非法 base64 / 格式异常）。
  /// 这些图片原样保留在正文里：宁可内联，绝不丢图。
  pub skipped: usize,
}

/// 白名单 MIME → 扩展名（jpg 一系归一为 jpg；svg/bmp 等一律不外置）。
fn ext_for_mime(mime: &str) -> Option<&'static str> {
  match mime {
    "image/png" => Some("png"),
    "image/jpeg" | "image/jpg" => Some("jpg"),
    "image/webp" => Some("webp"),
    "image/gif" => Some("gif"),
    _ => None,
  }
}

/// 抽图：把正文中的 Markdown 图片 `![alt](data:image/<mime>;base64,...)` 改写为
/// `![alt](assets/<sha256>.<ext>)`，同内容（sha256 相同）只产出一次。
///
/// - 保留 alt 与可选 title（`![alt](url "title")` 的 title 原样带回）；
/// - 非白名单 MIME / 非法 base64 / 解析失败：该处文本原样保留，`skipped += 1`；
/// - 普通链接图片（http/https）、纯文本、代码块内容等一律不动；
/// - 空正文返回空正文（幂等：对已改写正文再跑一遍结果逐字节不变）。
pub fn extract_images_report(body: &str) -> ImageExtractReport {
  let bytes = body.as_bytes();
  let mut out = String::with_capacity(body.len());
  let mut assets: Vec<ExtractedAsset> = Vec::new();
  let mut skipped = 0usize;
  let mut last = 0usize;
  let mut cursor = 0usize;

  while let Some(rel) = body[cursor..].find("![") {
    let start = cursor + rel;
    let alt_start = start + 2;
    let mut scan = alt_start;
    let mut alt_end = None;
    while scan < body.len() {
      match bytes[scan] {
        b'\\' => scan += 2,
        b']' => {
          alt_end = Some(scan);
          break;
        }
        _ => scan += 1,
      }
    }
    let Some(alt_end) = alt_end else { break };
    if bytes.get(alt_end + 1) != Some(&b'(') {
      cursor = start + 2;
      continue;
    }
    let inner_start = alt_end + 2;
    let Some(close_rel) = body[inner_start..].find(')') else {
      break;
    };
    let close = inner_start + close_rel;
    let inner = &body[inner_start..close];
    let url_end = inner
      .find(|c: char| c.is_ascii_whitespace())
      .unwrap_or(inner.len());
    let url = &inner[..url_end];
    let is_data_url = url.len() >= 5 && url[..5].eq_ignore_ascii_case("data:");
    if !is_data_url {
      // 普通链接图片不归抽图管，继续往后扫（可能还有真正的内联图）。
      cursor = start + 2;
      continue;
    }

    match parse_data_url(url).ok().and_then(|data| {
      ext_for_mime(&data.mime).map(|ext| {
        let sha = sha256_hex(&data.bytes);
        (sha, ext.to_string(), data.bytes)
      })
    }) {
      Some((sha, ext, bytes)) => {
        if !assets.iter().any(|a| a.sha256 == sha) {
          assets.push(ExtractedAsset {
            sha256: sha.clone(),
            ext: ext.clone(),
            bytes,
          });
        }
        out.push_str(&body[last..start]);
        out.push_str("![");
        out.push_str(&body[alt_start..alt_end]);
        out.push_str("](assets/");
        out.push_str(&sha);
        out.push('.');
        out.push_str(&ext);
        out.push_str(&inner[url_end..]);
        out.push(')');
        last = close + 1;
        cursor = close + 1;
      }
      None => {
        // 照片格式不认识/数据坏了：保留内联，计一笔跳过，继续扫后面的图。
        skipped += 1;
        cursor = close + 1;
      }
    }
  }

  let body = if last == 0 {
    body.to_string()
  } else {
    out.push_str(&body[last..]);
    out
  };
  ImageExtractReport {
    body,
    assets,
    skipped,
  }
}

/// 兼容窄接口：只要（改写后正文，资源列表），跳过数交给 [`extract_images_report`]。
pub fn extract_images(body: &str) -> (String, Vec<ExtractedAsset>) {
  let report = extract_images_report(body);
  (report.body, report.assets)
}

/// 正文归一：只裁掉首尾空行（换行符），**保留行尾空格与缩进**。
///
/// Markdown 的语义依赖它们：空列表项序列化为 `"- "` / `"1. "`（行尾一个空格），
/// 用 `trim()` 会变成 `"-"` / `"1."`，重开条目时被解析成普通段落，列表凭空消失。
fn normalize_body_md(value: &str) -> String {
  value
    .trim_matches(|c| c == '\n' || c == '\r')
    .to_string()
}

/// 新建条目：标题非空；时间戳由调用方注入（app 层纪律同 ADR-0003）
pub fn create(
  items: &mut Vec<KbItem>,
  id: String,
  title: &str,
  body_md: &str,
  tags: Vec<String>,
  stamp: &str,
) -> Result<KbItem, String> {
  let title = title.trim();
  if title.is_empty() {
    return Err("标题不能为空".to_string());
  }
  if items.len() >= NOTES_FILE_MAX_ITEMS {
    return Err("知识条目已达上限，请先清理不需要的条目".to_string());
  }
  let item = KbItem {
    id,
    title: title.to_string(),
    body_md: normalize_body_md(body_md),
    tags: normalized_tags(tags),
    created_at: stamp.to_string(),
    updated_at: stamp.to_string(),
    folder_id: None,
    order: None,
    pinned: false,
    // D1：正文指纹由 Store 写入正文文件时填充（kb.rs 只造内存条目，保持无 IO）
    content_hash: None,
  };
  items.push(item.clone());
  Ok(item)
}

/// 更新条目；不存在即报错
pub fn update(
  items: &mut Vec<KbItem>,
  id: &str,
  title: Option<&str>,
  body_md: Option<&str>,
  tags: Option<Vec<String>>,
  stamp: &str,
) -> Result<KbItem, String> {
  let index = items
    .iter()
    .position(|item| item.id == id)
    .ok_or_else(|| "找不到这条资料，可能已经被删除".to_string())?;
  let item = &mut items[index];
  if let Some(value) = title {
    let trimmed = value.trim();
    if trimmed.is_empty() {
      return Err("标题不能为空".to_string());
    }
    item.title = trimmed.to_string();
  }
  if let Some(value) = body_md {
    item.body_md = normalize_body_md(value);
  }
  if let Some(value) = tags {
    item.tags = normalized_tags(value);
  }
  item.updated_at = stamp.to_string();
  Ok(items[index].clone())
}

/// 删除条目；返回 true 表示有任务曾引用它（调用方可据此提示"引用已悬空"）
pub fn delete(items: &mut Vec<KbItem>, id: &str) -> Result<bool, String> {
  let before = items.len();
  items.retain(|item| item.id != id);
  if items.len() == before {
    return Err("找不到这条资料，可能已经被删除".to_string());
  }
  Ok(true)
}

/// 文件夹父链上溯深度上限：防御损坏数据里的环 / 超深链把搜索拖死
const FOLDER_CHAIN_MAX_DEPTH: usize = 64;

/// 标题打分：完全相等 100 / 前缀 50 / 包含 20（q 已 trim + 小写）
fn title_score(title: &str, q: &str) -> u32 {
  let title = title.trim().to_lowercase();
  if title == q {
    100
  } else if title.starts_with(q) {
    50
  } else if title.contains(q) {
    20
  } else {
    0
  }
}

/// 文件夹名打分：完全相等 8 / 前缀 4 / 包含 2
fn folder_score(name: &str, q: &str) -> u32 {
  let name = name.trim().to_lowercase();
  if name == q {
    8
  } else if name.starts_with(q) {
    4
  } else if name.contains(q) {
    2
  } else {
    0
  }
}

/// 沿 folderId 上溯取所有祖先文件夹名的最高分（不累加）；visited 防环 + 深度上限
fn folder_chain_score(folder_id: Option<&str>, folders: &[KbFolder], q: &str) -> u32 {
  let mut best = 0;
  let mut visited: Vec<&str> = Vec::new();
  let mut current = folder_id;
  let mut depth = 0;
  while let (Some(id), true) = (current, depth < FOLDER_CHAIN_MAX_DEPTH) {
    if visited.contains(&id) {
      break;
    }
    visited.push(id);
    match folders.iter().find(|folder| folder.id == id) {
      Some(folder) => {
        best = best.max(folder_score(&folder.name, q));
        current = folder.parent_id.as_deref();
      }
      None => break,
    }
    depth += 1;
  }
  best
}

/// 内存检索（kb-storage-backup S4 收窄）：只匹配标题 + 所有祖先文件夹名，
/// 不读正文、不匹配 tags（标签筛选由前端独立过滤，互不影响）。
/// 打分：标题完全相等 100 / 前缀 50 / 包含 20；文件夹名 8 / 4 / 2（祖先取最高，不累加）。
/// 排序：分数降序 → updatedAt 降序 → 标题升序；同一 id 只出现一次。
/// 空查询 = 全量（按 updatedAt 降序）。返回按相关度排序的 id 列表。
pub fn search(items: &[KbItem], folders: &[KbFolder], query: &str) -> Vec<String> {
  let q = query.trim().to_lowercase();
  let mut scored: Vec<(u32, &KbItem)> = items
    .iter()
    .map(|item| {
      let score = if q.is_empty() {
        0
      } else {
        title_score(&item.title, &q)
          + folder_chain_score(item.folder_id.as_deref(), folders, &q)
      };
      (score, item)
    })
    .filter(|(score, _)| q.is_empty() || *score > 0)
    .collect();
  // 分数降序 → 更新时间倒序 → 标题升序（本地语义字符串可直接比较）
  scored.sort_by(|a, b| {
    b.0.cmp(&a.0)
      .then_with(|| b.1.updated_at.cmp(&a.1.updated_at))
      .then_with(|| a.1.title.cmp(&b.1.title))
  });
  // 同一 id 只保留最高分（排序后第一次出现即最高）
  let mut seen: HashSet<&str> = HashSet::new();
  scored
    .into_iter()
    .filter(|(_, item)| seen.insert(item.id.as_str()))
    .map(|(_, item)| item.id.clone())
    .collect()
}

/// tag 归一：去空白、去空项、去重
fn normalized_tags(tags: Vec<String>) -> Vec<String> {
  let mut out: Vec<String> = Vec::new();
  for tag in tags {
    let trimmed = tag.trim().to_string();
    if !trimmed.is_empty() && !out.iter().any(|existing| *existing == trimmed) {
      out.push(trimmed);
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  fn item(id: &str, title: &str, body: &str, tags: &[&str], updated: &str) -> KbItem {
    KbItem {
      id: id.to_string(),
      title: title.to_string(),
      body_md: body.to_string(),
      tags: tags.iter().map(|t| t.to_string()).collect(),
      created_at: updated.to_string(),
      updated_at: updated.to_string(),
      folder_id: None,
      order: None,
      pinned: false,
      content_hash: None,
    }
  }

  fn folder(id: &str, name: &str, parent: Option<&str>) -> KbFolder {
    KbFolder {
      id: id.to_string(),
      name: name.to_string(),
      parent_id: parent.map(str::to_string),
      order: 0,
      pinned: false,
    }
  }

  fn with_folder(mut item: KbItem, folder_id: &str) -> KbItem {
    item.folder_id = Some(folder_id.to_string());
    item
  }

  #[test]
  fn create_rejects_blank_title_and_normalizes_tags() {
    let mut items = Vec::new();
    assert!(create(&mut items, "k1".into(), "   ", "body", vec![], "t").is_err());
    let made = create(&mut items, "k1".into(), " Rust 教程 ", "  body  ", vec!["rust".to_string(), " rust ".to_string(), String::new(), "笔记".to_string()], "t1").unwrap();
    assert_eq!(made.title, "Rust 教程");
    assert_eq!(made.tags, vec!["rust", "笔记"], "去空白/去重/去空项");
  }

  #[test]
  fn body_md_keeps_trailing_space_for_empty_list_items() {
    let mut items = Vec::new();
    let made = create(&mut items, "k1".into(), "清单", "- ", vec![], "t1").unwrap();
    assert_eq!(made.body_md, "- ", "空列表项的行尾空格必须保留（否则重开被解析成段落）");
    let updated = update(&mut items, "k1", None, Some("1. \n"), None, "t2").unwrap();
    assert_eq!(updated.body_md, "1. ", "只裁首尾换行，行尾空格保留");
    let blank = update(&mut items, "k1", None, Some("\n\n"), None, "t3").unwrap();
    assert_eq!(blank.body_md, "", "纯空行归一为空串");
  }

  #[test]
  fn update_touches_only_given_fields_and_stamps() {
    let mut items = vec![item("k1", "旧题", "旧文", &["a"], "2026-09-01T09:00")];
    let updated = update(&mut items, "k1", Some("新题"), None, None, "2026-09-06T10:00").unwrap();
    assert_eq!(updated.title, "新题");
    assert_eq!(updated.body_md, "旧文", "未给的字段不动");
    assert_eq!(updated.updated_at, "2026-09-06T10:00");
    assert!(update(&mut items, "missing", Some("x"), None, None, "t").is_err());
  }

  #[test]
  fn delete_reports_missing_and_removes() {
    let mut items = vec![item("k1", "t", "b", &[], "t")];
    assert!(delete(&mut items, "nope").is_err());
    assert!(delete(&mut items, "k1").unwrap());
    assert!(items.is_empty());
  }

  #[test]
  fn search_matches_title_only_not_body_or_tags() {
    let items = vec![
      item("body", "周报", "正文里写着 rust", &[], "2026-09-01T09:00"),
      item("tag", "杂记", "", &["rust"], "2026-09-02T09:00"),
      item("title", "Rust 入门", "", &[], "2026-09-03T09:00"),
    ];
    let hits = search(&items, &[], "rust");
    assert_eq!(hits, vec!["title".to_string()], "正文 / 标签命中不再入选（标签筛选由前端独立过滤）");
  }

  #[test]
  fn search_matches_ancestor_folder_names_case_insensitively() {
    let folders = vec![
      folder("root", "工作", None),
      folder("mid", "项目", Some("root")),
      folder("leaf", "周报", Some("mid")),
    ];
    let items = vec![
      with_folder(item("n", "笔记", "", &[], "2026-09-01T09:00"), "leaf"),
      item("other", "无关", "", &[], "2026-09-02T09:00"),
    ];
    assert_eq!(search(&items, &folders, "工作"), vec!["n".to_string()], "祖先文件夹名命中");
    assert_eq!(
      search(&items, &folders, " 项 "),
      vec!["n".to_string()],
      "查询先 trim，再按前缀匹配「项目」"
    );
  }

  #[test]
  fn search_title_hits_outrank_folder_hits() {
    let folders = vec![folder("f", "alpha", None)];
    let items = vec![
      with_folder(item("folderhit", "杂记", "", &[], "2026-09-01T09:00"), "f"),
      item("titlehit", "Alpha 计划", "", &[], "2026-09-02T09:00"),
    ];
    assert_eq!(
      search(&items, &folders, "alpha"),
      vec!["titlehit".to_string(), "folderhit".to_string()],
      "标题前缀 50 > 文件夹名完全相等 8"
    );
  }

  #[test]
  fn search_title_exact_then_prefix_then_contains() {
    let items = vec![
      item("contains", "旧 Rust 笔记", "", &[], "2026-09-01T09:00"),
      item("prefix", "Rust 入门", "", &[], "2026-09-02T09:00"),
      item("exact", "Rust", "", &[], "2026-09-03T09:00"),
    ];
    assert_eq!(
      search(&items, &[], "rust"),
      vec!["exact".to_string(), "prefix".to_string(), "contains".to_string()],
      "完全相等 100 > 前缀 50 > 包含 20"
    );
  }

  #[test]
  fn search_folder_scores_exact_prefix_contains_in_order() {
    let folders = vec![
      folder("exact", "Rust", None),
      folder("prefix", "Rust 资料", None),
      folder("contains", "旧 Rust", None),
    ];
    let items = vec![
      with_folder(item("c", "甲", "", &[], "2026-09-01T09:00"), "contains"),
      with_folder(item("p", "乙", "", &[], "2026-09-01T09:00"), "prefix"),
      with_folder(item("e", "丙", "", &[], "2026-09-01T09:00"), "exact"),
    ];
    assert_eq!(
      search(&items, &folders, "rust"),
      vec!["e".to_string(), "p".to_string(), "c".to_string()],
      "文件夹名完全相等 8 > 前缀 4 > 包含 2"
    );
  }

  #[test]
  fn search_dedupes_same_id_keeping_highest_score() {
    let folders = vec![folder("f", "资料", None)];
    let items = vec![
      with_folder(item("dup", "杂记", "", &[], "2026-09-01T09:00"), "f"),
      item("dup", "资料大全", "", &[], "2026-09-02T09:00"),
    ];
    assert_eq!(
      search(&items, &folders, "资料"),
      vec!["dup".to_string()],
      "同一 id 只保留最高分、只出现一次"
    );
  }

  #[test]
  fn search_trims_query_and_ignores_case() {
    let items = vec![item("k1", "Rust 教程", "", &[], "2026-09-01T09:00")];
    assert_eq!(search(&items, &[], "  rUsT  "), vec!["k1".to_string()]);
    assert!(search(&items, &[], "python").is_empty(), "无命中返回空");
  }

  #[test]
  fn search_empty_query_returns_all_recency_first() {
    let items = vec![
      item("k1", "a", "", &[], "2026-09-01T09:00"),
      item("k2", "b", "", &[], "2026-09-06T09:00"),
    ];
    assert_eq!(search(&items, &[], "  "), vec!["k2".to_string(), "k1".to_string()]);
  }

  #[test]
  fn search_survives_folder_cycle_and_deep_chain() {
    // 环：a → b → a，必须终止
    let cycle = vec![folder("a", "甲组", Some("b")), folder("b", "乙组", Some("a"))];
    let cycle_items = vec![with_folder(item("c", "笔记", "", &[], "2026-09-01T09:00"), "a")];
    assert_eq!(search(&cycle_items, &cycle, "乙组"), vec!["c".to_string()]);

    // 超深父链：100 层父链，深度 0 命中；超过 64 层上限的祖先不再上溯且不崩
    let mut deep: Vec<KbFolder> = vec![folder("f0", "顶层", None)];
    for i in 1..100 {
      let parent = format!("f{}", i - 1);
      deep.push(folder(&format!("f{i}"), &format!("层{i}"), Some(&parent)));
    }
    let deep_items = vec![with_folder(item("d", "笔记", "", &[], "2026-09-01T09:00"), "f99")];
    assert_eq!(search(&deep_items, &deep, "层99"), vec!["d".to_string()]);
    assert!(
      search(&deep_items, &deep, "顶层").is_empty(),
      "超过深度上限的祖先不再上溯"
    );
  }

  fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{};base64,{}", mime, crate::digest::base64_encode(bytes))
  }

  /// 单图：正文改写为 assets/<sha256>.<ext> 相对路径，资源字节原样带出。等。
  #[test]
  fn extract_single_image_rewrites_to_relative_asset_path() {
    let payload = vec![0x89u8, b'P', b'N', b'G', 1, 2, 3];
    let body = format!("前\n![截图]({})\n后", data_url("image/png", &payload));
    let report = extract_images_report(&body);
    assert_eq!(report.assets.len(), 1);
    let asset = &report.assets[0];
    assert_eq!(asset.ext, "png");
    assert_eq!(asset.sha256, crate::digest::sha256_hex(&payload));
    assert_eq!(asset.bytes, payload);
    assert_eq!(
      report.body,
      format!("前\n![截图](assets/{}.png)\n后", asset.sha256),
      "正文只留相对引用、保留 alt"
    );
    assert_eq!(report.skipped, 0);
    let again = extract_images_report(&report.body);
    assert_eq!(again.body, report.body, "已改写正文再抽一遍逐字节不变");
    assert!(again.assets.is_empty());
    assert_eq!(again.skipped, 0);
  }

  /// 多图：各自成资源，顺序与出现顺序一致，MIME 映射扩展名。
  #[test]
  fn extract_multiple_images_maps_mimes_in_order() {
    let jpeg = data_url("image/jpeg", b"jpeg-bytes");
    let webp = data_url("image/webp", b"webp-bytes");
    let jpg = data_url("image/jpg", b"jpg-bytes");
    let body = format!("![一]({jpeg}) 中间 ![二]({webp}) ![三]({jpg})");
    let report = extract_images_report(&body);
    assert_eq!(report.assets.len(), 3);
    assert_eq!(report.assets[0].ext, "jpg", "image/jpeg 归一为 jpg");
    assert_eq!(report.assets[1].ext, "webp");
    assert_eq!(report.assets[2].ext, "jpg", "非标准 image/jpg 也归一到 jpg");
    assert_eq!(report.skipped, 0);
    assert_eq!(
      report.body,
      format!(
        "![一](assets/{}.jpg) 中间 ![二](assets/{}.webp) ![三](assets/{}.jpg)",
        report.assets[0].sha256, report.assets[1].sha256, report.assets[2].sha256
      )
    );
  }

  /// 重复图去重：同字节图片即使 alt 不同也只产出一个资源，但两处都改写。
  #[test]
  fn extract_dedups_repeated_image_by_sha() {
    let url = data_url("image/png", b"same-image-bytes");
    let body = format!("![图一]({url})\n![图二]({url})");
    let report = extract_images_report(&body);
    assert_eq!(report.assets.len(), 1, "同内容只写一份 assets");
    let path = format!("assets/{}.png", report.assets[0].sha256);
    assert_eq!(report.body.matches(&path).count(), 2, "两处引用都改写");
    assert!(report.body.contains("![图一]("), "alt 各自保留");
    assert!(report.body.contains("![图二]("));
  }

  /// 非白名单 MIME（svg）：不外置、原样保留、计一笔跳过。
  #[test]
  fn extract_keeps_non_whitelisted_mime_inline() {
    let url = data_url("image/svg+xml", b"<svg></svg>");
    let body = format!("![矢量图]({url})");
    let report = extract_images_report(&body);
    assert!(report.assets.is_empty(), "svg 不在白名单");
    assert_eq!(report.body, body, "原内联文本保留");
    assert_eq!(report.skipped, 1);
  }

  /// 非法 base64：不丢数据（原样保留），计一笔跳过。
  #[test]
  fn extract_keeps_invalid_base64_inline_and_counts_skip() {
    let body = "![坏图](data:image/png;base64,@@not-base64@@)";
    let report = extract_images_report(body);
    assert!(report.assets.is_empty());
    assert_eq!(report.body, body);
    assert_eq!(report.skipped, 1);
  }

  /// 空正文 / 普通文本 / 远程链接图片：一律不动，跳过数不虚增。
  #[test]
  fn extract_leaves_plain_text_and_remote_images_untouched() {
    assert_eq!(
      extract_images_report(""),
      ImageExtractReport {
        body: String::new(),
        assets: Vec::new(),
        skipped: 0
      }
    );
    let plain = "普通笔记，没有图片\n- [ ] 待办";
    assert_eq!(extract_images_report(plain).body, plain);
    let remote = "![截图](https://example.com/a.png)";
    let report = extract_images_report(remote);
    assert_eq!(report.body, remote);
    assert_eq!(report.skipped, 0, "不是 data: 图片不算格式异常");
  }

  /// alt 里的转义字符与 URL 后的 title 都原样保留（只替换 URL 本体）。
  #[test]
  fn extract_preserves_escaped_alt_and_title() {
    let url = data_url("image/gif", b"gif-bytes");
    let body = format!("![a\\]b]({url} \"截图\")");
    let report = extract_images_report(&body);
    let path = format!("assets/{}.gif", report.assets[0].sha256);
    assert_eq!(report.body, format!("![a\\]b]({path} \"截图\")"));
  }
}
