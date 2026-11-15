//! 知识库索引/正文拆分（kb-storage-backup D1/D7）的**纯函数**：索引探测、
//! `KbItem ⇄ KbItemMeta` 转换、旧格式条目规范化。
//!
//! 边界：
//! - 本模块不碰 IO、不持 `Store`：正文读写/轮转/迁移编排在 `store.rs`（需要 backend）。
//! - 旧格式探测与「恢复旧备份」共用同一套解析（design D7：恢复复用同一迁移，避免两条转换路径分叉）。
//! - 正则/依赖零新增：只用 `serde_json::Value` 与仓内 `digest::sha256_hex`。

use crate::digest::sha256_hex;
use crate::models::{KbItem, KbItemMeta};
use crate::store::brief;
use serde_json::Value;

/// `notes.json` 是否为旧格式（任一元素内联 `bodyMd` / `body_md`）——design D7 的探测口径。
/// 顶层不是数组（损坏）不算旧格式，交给调用方走 KB_CORRUPT_MESSAGE。
pub fn is_legacy_index(value: &Value) -> bool {
  match value.as_array() {
    Some(entries) => entries.iter().any(|entry| {
      entry.get("bodyMd").is_some() || entry.get("body_md").is_some()
    }),
    None => false,
  }
}

/// 正文内容指纹（D1）：`sha256(规范化正文)` 的 64 位小写十六进制
pub fn content_hash(body: &str) -> String {
  sha256_hex(body.as_bytes())
}

/// 新格式索引：`Vec<KbItemMeta>`（缺省可选字段由 serde default 兜底）
pub fn parse_meta_index(value: Value) -> Result<Vec<KbItemMeta>, String> {
  serde_json::from_value::<Vec<KbItemMeta>>(value).map_err(|error| error.to_string())
}

/// 旧格式 `notes.json` / 旧备份：逐条规范化为内存 `KbItem`（不写盘）。
///
/// 规范化口径（与 `kb.rs` 同源，但不改 `kb.rs`）：
/// - 正文只裁首尾换行（保留行尾空格与缩进，Markdown 空列表项靠它）；
/// - 标题 trim；标签去空白/去空项/去重；
/// - folderId / order / pinned / 时间戳缺省兜底（`KbItem::default` 同款语义）；
/// - 按 id 去重（首次出现为准），保证重复执行迁移不产生重复条目；
/// - contentHash 由正文现算，旧索引里的 `contentHash`（若有）不采信。
pub fn parse_legacy_items(value: &Value) -> Result<Vec<KbItem>, String> {
  let entries = value
    .as_array()
    .ok_or_else(|| "旧知识库不是数组".to_string())?;
  let mut seen: Vec<String> = Vec::with_capacity(entries.len());
  let mut items: Vec<KbItem> = Vec::with_capacity(entries.len());
  for entry in entries {
    if !entry.is_object() {
      return Err("旧知识库条目格式不对".to_string());
    }
    let id = text_field(entry, &["id"]);
    if id.is_empty() {
      return Err("旧知识库条目缺少编号".to_string());
    }
    if seen.iter().any(|existing| existing == &id) {
      continue;
    }
    seen.push(id.clone());
    let body = normalize_body(&string_field(entry, &["bodyMd", "body_md"]));
    let item = KbItem {
      id,
      title: text_field(entry, &["title"]),
      body_md: body.clone(),
      tags: normalized_tags(entry.get("tags")),
      created_at: text_field(entry, &["createdAt", "created_at"]),
      updated_at: text_field(entry, &["updatedAt", "updated_at"]),
      folder_id: optional_text(entry, &["folderId", "folder_id"]),
      order: entry.get("order").and_then(Value::as_i64),
      pinned: entry
        .get("pinned")
        .and_then(Value::as_bool)
        .unwrap_or(false),
      content_hash: Some(content_hash(&body)),
    };
    items.push(item);
  }
  Ok(items)
}

/// 内存 `KbItem` → `notes.json` 索引元素（正文不进索引）。
/// 内存里的 `content_hash` 是唯一真相；个别构造路径（测试/旧恢复）没填时按正文现算兜底。
pub fn meta_of(item: &KbItem) -> KbItemMeta {
  KbItemMeta {
    id: item.id.clone(),
    title: item.title.clone(),
    tags: item.tags.clone(),
    folder_id: item.folder_id.clone(),
    order: item.order,
    pinned: item.pinned,
    created_at: item.created_at.clone(),
    updated_at: item.updated_at.clone(),
    content_hash: item
      .content_hash
      .clone()
      .unwrap_or_else(|| content_hash(&item.body_md)),
  }
}

/// 索引元素 + 正文文件内容 → 内存 `KbItem`。
/// hash 一律按**实际正文**现算（design D1：载入不一致以正文为准，不判损坏）。
pub fn item_with_body(meta: KbItemMeta, body: String) -> KbItem {
  let hash = content_hash(&body);
  KbItem {
    id: meta.id,
    title: meta.title,
    body_md: body,
    tags: meta.tags,
    created_at: meta.created_at,
    updated_at: meta.updated_at,
    folder_id: meta.folder_id,
    order: meta.order,
    pinned: meta.pinned,
    content_hash: Some(hash),
  }
}

/// `notes.json` 新索引 JSON（meta 数组；不含正文/图片，folderId/order 保留 null）
pub fn index_json(items: &[KbItem]) -> Result<String, String> {
  let metas: Vec<KbItemMeta> = items.iter().map(meta_of).collect();
  serde_json::to_string_pretty(&metas)
    .map_err(|error| format!("知识条目索引序列化失败：{}", brief(&error.to_string())))
}

/// 正文归一：只裁首尾空行（换行符），保留行尾空格与缩进（与 `kb.rs` 同口径）
fn normalize_body(value: &str) -> String {
  value
    .trim_matches(|ch| ch == '\n' || ch == '\r')
    .to_string()
}

/// 取第一个非空字符串字段并 trim（camelCase 旧字段与 snake_case 兜底）
fn text_field(object: &Value, keys: &[&str]) -> String {
  keys
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_str))
    .map(|text| text.trim().to_string())
    .unwrap_or_default()
}

/// 取可空文本：空串 = 未分类/无值（三态口径与 folder_target 一致）
fn optional_text(object: &Value, keys: &[&str]) -> Option<String> {
  let text = text_field(object, keys);
  if text.is_empty() {
    None
  } else {
    Some(text)
  }
}

/// 取字符串字段原样（正文不 trim：只裁首尾换行由 normalize_body 负责）
fn string_field(object: &Value, keys: &[&str]) -> String {
  keys
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_str))
    .unwrap_or("")
    .to_string()
}

/// tag 归一：去空白、去空项、去重（与 `kb.rs` 同口径）
fn normalized_tags(raw: Option<&Value>) -> Vec<String> {
  let mut out: Vec<String> = Vec::new();
  if let Some(Value::Array(values)) = raw {
    for value in values {
      if let Some(text) = value.as_str() {
        let trimmed = text.trim();
        if !trimmed.is_empty() && !out.iter().any(|existing| existing == trimmed) {
          out.push(trimmed.to_string());
        }
      }
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn legacy_detection_accepts_both_body_spellings() {
    assert!(is_legacy_index(&json!([{ "id": "k1", "bodyMd": "" }])));
    assert!(is_legacy_index(&json!([{ "id": "k1", "body_md": "" }])));
    assert!(
      !is_legacy_index(&json!([{ "id": "k1", "contentHash": "abc" }])),
      "新索引（contentHash、无正文）不是旧格式"
    );
    assert!(!is_legacy_index(&json!({ "not": "an array" })));
    assert!(!is_legacy_index(&json!([])));
  }

  #[test]
  fn legacy_items_are_normalized_like_kb_create() {
    let legacy = json!([
      {
        "id": "k1",
        "title": "  旧标题 ",
        "bodyMd": "\n- \n正文行 \n\n",
        "tags": [" rust ", "", "rust", "笔记"],
        "folderId": "f1",
        "order": 2,
        "pinned": true,
        "createdAt": "2026-09-01T09:00",
        "updatedAt": "2026-09-02T09:00"
      },
      {
        "id": "k1",
        "title": "重复 id 丢弃",
        "body_md": "重复"
      },
      {
        "id": "k2",
        "title": "缺省字段"
      }
    ]);
    let items = parse_legacy_items(&legacy).expect("旧格式必须可规范化");
    assert_eq!(items.len(), 2, "重复 id 只保留首次出现（迁移幂等不产生重复条目）");
    assert_eq!(items[0].title, "旧标题");
    assert_eq!(
      items[0].body_md, "- \n正文行 ",
      "只裁首尾换行、保留行尾空格（Markdown 空列表项）"
    );
    assert_eq!(items[0].tags, vec!["rust", "笔记"]);
    assert_eq!(items[0].folder_id.as_deref(), Some("f1"));
    assert_eq!(items[0].order, Some(2));
    assert!(items[0].pinned);
    assert_eq!(
      items[0].content_hash.as_deref(),
      Some(content_hash("- \n正文行 ").as_str()),
      "contentHash = sha256(规范化正文)"
    );
    assert_eq!(items[1].title, "缺省字段");
    assert_eq!(items[1].folder_id, None);
    assert_eq!(items[1].order, None);
    assert!(!items[1].pinned);
    assert_eq!(items[1].body_md, "");
  }

  #[test]
  fn meta_round_trip_recomputes_hash_from_body() {
    let item = parse_legacy_items(&json!([{ "id": "k1", "title": "t", "bodyMd": "正文" }]))
      .unwrap()
      .remove(0);
    let meta = meta_of(&item);
    assert_eq!(meta.content_hash, content_hash("正文"));
    let restored = item_with_body(meta.clone(), "正文".to_string());
    assert_eq!(restored, item);

    // 索引里的 hash 被篡改：以实际正文为准重算（design D1 可修复不一致）
    let mut tampered = meta;
    tampered.content_hash = "tampered".to_string();
    let restored = item_with_body(tampered, "正文".to_string());
    assert_eq!(restored.content_hash.as_deref(), Some(content_hash("正文").as_str()));

    // 内存 item 缺 content_hash（测试/旧恢复路径）：meta_of 按正文现算兜底
    let mut bare = item;
    bare.content_hash = None;
    assert_eq!(meta_of(&bare).content_hash, content_hash("正文"));
  }

  #[test]
  fn index_json_has_no_body_and_keeps_nulls() {
    let items = parse_legacy_items(&json!([
      { "id": "k1", "title": "t", "bodyMd": "正文", "tags": ["a"] }
    ]))
    .unwrap();
    let raw = index_json(&items).expect("索引必须可序列化");
    let value: Value = serde_json::from_str(&raw).unwrap();
    assert!(value.is_array(), "notes.json 根必须是裸数组");
    assert!(
      value[0].get("bodyMd").is_none() && value[0].get("body_md").is_none(),
      "索引 MUST NOT 内联正文：{value}"
    );
    assert_eq!(value[0]["contentHash"], content_hash("正文"));
    assert!(matches!(value[0].get("folderId"), Some(Value::Null)));
    assert!(matches!(value[0]["order"], Value::Null));
    assert_eq!(value[0]["pinned"], json!(false));
  }
}