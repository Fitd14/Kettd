//! 知识库最小地基（frame H1b · ADR-0007 后续知识网的"库"侧）。
//!
//! 边界（反功能广度红线）：
//! - flomo 式片段（title + body_md + tags），**不做**长文档编辑器/双链/图谱；
//! - 检索 = 内存线性扫（条目 >5k 或 P95 >200ms 才考虑依赖，架构 §9）；
//! - 任务经 `Task.kb_refs` **单向**引用本库：KB 侧永不持有 Task（ACL，架构 §3），
//!   命门假设（用户真会往里记）失败时本模块可整体退役而不伤 Task 聚合。
//! - 独立 notes.json：与用户数据物理隔离（frame D1），原子写、v1 不轮转（H2 成熟化）。

use crate::models::KbItem;

pub const NOTES_FILE_MAX_ITEMS: usize = 5_000;

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
    body_md: body_md.trim().to_string(),
    tags: normalized_tags(tags),
    created_at: stamp.to_string(),
    updated_at: stamp.to_string(),
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
    item.body_md = value.trim().to_string();
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

/// 内存扫全文检索：title 命中权重 ×2、tag 命中 ×1.5、body ×1；大小写不敏感。
/// 空查询 = 全量（按更新时间倒序）。返回按相关度排序的 id 列表。
pub fn search(items: &[KbItem], query: &str) -> Vec<String> {
  let q = query.trim().to_lowercase();
  let mut scored: Vec<(u32, &KbItem)> = items
    .iter()
    .map(|item| {
      if q.is_empty() {
        return (0, item);
      }
      let mut score = 0;
      if item.title.to_lowercase().contains(&q) {
        score += 2;
      }
      if item.tags.iter().any(|tag| tag.to_lowercase().contains(&q)) {
        score += 1;
      }
      if item.body_md.to_lowercase().contains(&q) {
        score += 1;
      }
      (score, item)
    })
    .filter(|(score, _)| *score > 0 || q.is_empty())
    .collect();
  // 相关度降序，同分按更新时间倒序（本地语义字符串可直接比较）
  scored.sort_by(|a, b| {
    b.0.cmp(&a.0)
      .then_with(|| b.1.updated_at.cmp(&a.1.updated_at))
  });
  scored.into_iter().map(|(_, item)| item.id.clone()).collect()
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
    }
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
  fn search_ranks_title_over_body_and_is_case_insensitive() {
    let items = vec![
      item("k1", "周报模板", "随便写写 rust", &[], "2026-09-01T09:00"),
      item("k2", "Rust 入门", "安装 rustup", &["rust"], "2026-09-02T09:00"),
      item("k3", "购物清单", "牛奶 鸡蛋", &[], "2026-09-03T09:00"),
    ];
    let hits = search(&items, "RUST");
    assert_eq!(hits.first().unwrap(), "k2", "title+tag 双命中排最前");
    assert!(hits.contains(&"k1".to_string()), "body 命中也入选");
    assert!(!hits.contains(&"k3".to_string()), "无关条目不入选");
  }

  #[test]
  fn search_empty_query_returns_all_recency_first() {
    let items = vec![
      item("k1", "a", "", &[], "2026-09-01T09:00"),
      item("k2", "b", "", &[], "2026-09-06T09:00"),
    ];
    assert_eq!(search(&items, "  "), vec!["k2".to_string(), "k1".to_string()]);
  }
}
