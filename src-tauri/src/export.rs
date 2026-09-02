//! 周汇总导出（PRD 6.5 的导出面）：md = 标题三桶 + 按日明细；csv = 全字段
//!
//! - 文件名含 ISO 周（例：`周汇总-2026-W36.md`）
//! - 目标不可写 → Err("这个位置写不了，换个位置")
//! - 二次导出不覆盖首版：自动补 -2 / -3 后缀
//! - 时间全部本地语义字符串，按日期前缀比较，不做时区换算

use crate::models::{iso_week_label, fmt_day, today, week_range, week_start, Settings, Task};
use crate::store::{data_dir, safe_file_stem};
use chrono::{Duration, NaiveDate};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const WEEKDAY_LABELS: [&str; 7] = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];

/// 解析 week 入参：current / this / 空 → 本周，last → 上周，否则 YYYY-Www
pub fn resolve_week(raw: &str) -> Result<(NaiveDate, NaiveDate), String> {
  let text = raw.trim().to_lowercase();
  let monday = if text.is_empty() || text == "current" || text == "this" || text == "本周" {
    let now = today();
    week_start(&iso_week_label(&now)).ok_or_else(|| "本周编号算不出来，请选具体周次".to_string())?
  } else if text == "last" || text == "previous" || text == "上周" {
    let now = today() - Duration::weeks(1);
    week_start(&iso_week_label(&now)).ok_or_else(|| "上周编号算不出来，请选具体周次".to_string())?
  } else {
    week_start(&text).ok_or_else(|| "周次格式不对，请用 2026-W36 这种写法".to_string())?
  };
  match week_range(&monday) {
    Some(range) => Ok(range),
    None => Err("周次范围算不出来，请选具体周次".to_string()),
  }
}

fn day_text(date: &NaiveDate) -> String {
  fmt_day(date)
}

fn in_range(text: &str, from: &str, to: &str) -> bool {
  text.len() >= 10 && &text[..10] >= from && &text[..10] <= to
}

fn starts_with(text: &Option<String>, prefix: &str) -> bool {
  match text {
    Some(value) => value.starts_with(prefix),
    None => false,
  }
}

/// 三桶：doneByDay / carrying / stale；只统计未软删除的任务
struct Buckets {
  week_done: Vec<Task>,
  undone: Vec<Task>,
  overdue: Vec<Task>,
  carry: Vec<Task>,
  candidates: Vec<Task>,
  created: Vec<Task>,
}

fn bucket(tasks: &[Task], monday: &NaiveDate, sunday: &NaiveDate) -> Buckets {
  let from = day_text(monday);
  let to = day_text(sunday);
  let alive: Vec<Task> = tasks.iter().filter(|task| !task.in_trash()).cloned().collect();
  let week_done: Vec<Task> = alive
    .iter()
    .filter(|task| {
      task.done
        && match &task.done_at {
          Some(text) => in_range(text, &from, &to),
          None => false,
        }
    })
    .cloned()
    .collect();
  let created: Vec<Task> = alive
    .iter()
    .filter(|task| in_range(&task.created_at, &from, &to))
    .cloned()
    .collect();
  let undone: Vec<Task> = alive.iter().filter(|task| !task.done).cloned().collect();
  let overdue: Vec<Task> = undone
    .iter()
    .filter(|task| match &task.due_at {
      Some(text) => text.len() >= 10 && text[..10] < from,
      None => false,
    })
    .cloned()
    .collect();
  let carry: Vec<Task> = undone
    .iter()
    .filter(|task| {
      task.carried_from >= 1
        || starts_with(&task.planned_date, &from)
        || (task.due_at.is_some() && task.due_at_is_week(&from, &to))
    })
    .cloned()
    .collect();
  let mut candidates: Vec<Task> = undone
    .iter()
    .filter(|task| task.carried_from >= 3)
    .cloned()
    .collect();
  candidates.sort_by(|left, right| {
    let weight = |task: &Task| match task.priority.as_str() {
      "high" => 0u8,
      "med" => 1u8,
      _ => 2u8,
    };
    right
      .carried_from
      .cmp(&left.carried_from)
      .then(weight(left).cmp(&weight(right)))
  });
  candidates.truncate(5);
  Buckets {
    week_done,
    undone,
    overdue,
    carry,
    candidates,
    created,
  }
}

impl Task {
  fn due_at_is_week(&self, from: &str, to: &str) -> bool {
    match &self.due_at {
      Some(text) => in_range(text, from, to),
      None => false,
    }
  }
}

fn category_line(buckets: &Buckets) -> String {
  let mut counts: Vec<(&'static str, usize)> = vec![("工作", 0), ("学习", 0), ("生活", 0)];
  for task in buckets.week_done.iter() {
    let slot = counts
      .iter_mut()
      .find(|(name, _)| *name == task.category.as_str());
    match slot {
      Some((_, value)) => *value += 1,
      None => {}
    }
  }
  let total = buckets.week_done.len();
  let parts: Vec<String> = counts
    .iter()
    .map(|(name, value)| format!("{} {} 条", name, value))
    .collect();
  format!("共 {} 条：{}", total, parts.join(" · "))
}

fn title_cell(task: &Task) -> String {
  if task.legacy {
    format!("{}（完成时刻未记录）", task.title)
  } else if task.carry_over_note() {
    format!("{}（拖 {} 天）", task.title, task.carried_from)
  } else {
    task.title.clone()
  }
}

impl Task {
  fn carry_over_note(&self) -> bool {
    self.carried_from >= 1 && !self.done
  }
}

fn time_cell(text: &Option<String>) -> String {
  match text {
    None => "—".to_string(),
    Some(value) => {
      let chars: Vec<char> = value.chars().collect();
      if chars.len() > 10 {
        let day: String = chars.iter().take(10).collect();
        let clock: String = chars.iter().skip(11).collect();
        format!("{} {}", day, clock)
      } else {
        value.clone()
      }
    }
  }
}

fn md_bytes(
  buckets: &Buckets,
  label: &str,
  monday: &NaiveDate,
  sunday: &NaiveDate,
) -> Vec<u8> {
  let mut text = String::new();
  text.push_str(&format!("# 本周汇总 · {}\n\n", label));
  text.push_str(&format!(
    "区间：{}（周一）～ {}（周日）\n\n",
    day_text(monday),
    day_text(sunday)
  ));
  text.push_str("## 三桶概览\n\n");
  text.push_str(&format!("- 本周完成：{} 条\n", buckets.week_done.len()));
  text.push_str(&format!("- 顺延未完成：{} 条\n", buckets.carry.len()));
  text.push_str(&format!("- 已逾期：{} 条\n", buckets.overdue.len()));
  text.push_str(&format!("- 新建：{} 条\n\n", buckets.created.len()));
  text.push_str(&format!("分类分布（完成口径）：{}\n\n", category_line(buckets)));
  text.push_str("## 下周候选（拖留 ≥3 天 Top5）\n\n");
  if buckets.candidates.is_empty() {
    text.push_str("- 没有拖留三天以上的存量\n\n");
  } else {
    for task in buckets.candidates.iter() {
      text.push_str(&format!(
        "- [ ] {}（{} · {}）\n",
        task.title, task.category, task.priority
      ));
    }
    text.push_str("\n");
  }
  text.push_str("## 按日明细\n\n");
  let mut day = *monday;
  let mut index = 0;
  while index < 7 {
    let key = day_text(&day);
    let weekday = WEEKDAY_LABELS[index];
    let done_here: Vec<&Task> = buckets
      .week_done
      .iter()
      .filter(|task| starts_with(&task.done_at, &key))
      .collect();
    let created_here: Vec<&Task> = buckets
      .created
      .iter()
      .filter(|task| starts_with(&Some(task.created_at.clone()), &key))
      .collect();
    let due_here: Vec<&Task> = buckets
      .undone
      .iter()
      .filter(|task| match &task.due_at {
        Some(value) => value.starts_with(&key),
        None => false,
      })
      .collect();
    text.push_str(&format!("### {} {}\n\n", key, weekday));
    if done_here.is_empty() && created_here.is_empty() && due_here.is_empty() {
      text.push_str("- 无记录\n\n");
    } else {
      for task in done_here.iter() {
        text.push_str(&format!("- ✅ 完成 {} · 时刻 {}\n", title_cell(task), time_cell(&task.done_at)));
      }
      for task in created_here.iter() {
        text.push_str(&format!("- ✚ 记下 {}\n", task.title));
      }
      for task in due_here.iter() {
        text.push_str(&format!("- ⏳ 到期未完成 {}（{} · 拖 {} 天）\n", task.title, task.priority, task.carried_from));
      }
      text.push_str("\n");
    }
    day += Duration::days(1);
    index += 1;
  }
  text.push_str("## 本周完成明细\n\n");
  if buckets.week_done.is_empty() {
    text.push_str("本周没有完成记录——不算失败，下周接着来。\n");
  } else {
    for task in buckets.week_done.iter() {
      text.push_str(&format!(
        "- {} · {} · {}\n",
        title_cell(task),
        task.category,
        time_cell(&task.done_at)
      ));
    }
  }
  text.push_str("\n> 由「待办列表」本地导出，不含任何网络上报。\n");
  text.into_bytes()
}

fn csv_bytes(tasks: &[Task], label: &str) -> Vec<u8> {
  let mut out: Vec<u8> = Vec::new();
  out.extend_from_slice(&[0xEFu8, 0xBBu8, 0xBFu8]);
  let head = [
    "ISO周", "标题", "分类", "优先级", "截止", "提醒", "计划日", "拖留天数", "状态", "完成时刻",
    "子任务", "进度", "备注数", "旧数据", "来源", "创建时间", "更新时间",
  ];
  write_row(&mut out, &head);
  for task in tasks.iter().filter(|task| !task.in_trash()) {
    let done_sub = task.subtasks.iter().filter(|item| item.done).count();
    let status = if task.done { "已完成" } else { "未完成" };
    let subtasks: Vec<String> = task
      .subtasks
      .iter()
      .map(|item| {
        if item.done {
          format!("√{}", item.title)
        } else {
          item.title.clone()
        }
      })
      .collect();
    let legacy = if task.legacy { "是" } else { "否" };
    let carried = task.carried_from.to_string();
    let progress = format!("{}/{}", done_sub, task.subtasks.len());
    let note_count = task.notes.len().to_string();
    let joined = subtasks.join(" / ");
    write_row(&mut out, &[
      label,
      &task.title,
      &task.category,
      &task.priority,
      task.due_at.as_deref().unwrap_or(""),
      task.remind_at.as_deref().unwrap_or(""),
      task.planned_date.as_deref().unwrap_or(""),
      &carried,
      status,
      task.done_at.as_deref().unwrap_or(""),
      &joined,
      &progress,
      &note_count,
      legacy,
      source_text(task.source),
      &task.created_at,
      &task.updated_at,
    ]);
  }
  out
}

fn source_text(source: crate::models::Source) -> &'static str {
  match source {
    crate::models::Source::Capture => "capture",
    crate::models::Source::Manual => "manual",
    crate::models::Source::Seed => "seed",
  }
}

fn write_row(out: &mut Vec<u8>, cells: &[&str]) {
  let mut first = true;
  for cell in cells {
    if !first {
      out.push(b',');
    }
    first = false;
    let escaped = cell.replace('"', "\"\"");
    let needs_quotes = escaped.contains(',') || escaped.contains('"') || escaped.contains('\n');
    if needs_quotes {
      out.push(b'"');
      out.extend_from_slice(escaped.as_bytes());
      out.push(b'"');
    } else {
      out.extend_from_slice(escaped.as_bytes());
    }
  }
  out.push(b'\n');
}

fn target_dir(dir: Option<&str>, settings: &Settings) -> PathBuf {
  match dir {
    Some(text) if !text.trim().is_empty() => PathBuf::from(text.trim()),
    _ => match &settings.export_dir {
      Some(text) if !text.trim().is_empty() => PathBuf::from(text.trim()),
      _ => data_dir().join("exports"),
    },
  }
}

/// 生成并落盘；返回完整路径
pub fn write_weekly(
  settings: &Settings,
  tasks: &[Task],
  week: Option<String>,
  format: Option<String>,
  dir: Option<String>,
) -> Result<String, String> {
  let raw_week = week.unwrap_or_else(|| "current".to_string());
  let (monday, sunday) = resolve_week(&raw_week)?;
  let label = iso_week_label(&monday);
  let kind = format.unwrap_or_else(|| "md".to_string());
  let ext = match kind.trim().to_lowercase().as_str() {
    "md" | "markdown" => "md",
    "csv" => "csv",
    _ => return Err("只支持 md 和 csv 两种格式".to_string()),
  };
  let dir = target_dir(dir.as_deref(), settings);
  if !dir.exists() {
    if fs::create_dir_all(&dir).is_err() {
      return Err("这个位置写不了，换个位置".to_string());
    }
  }
  let stem = safe_file_stem(&format!("周汇总-{}", label));
  let mut name = format!("{}.{}", stem, ext);
  let mut path = dir.join(&name);
  let mut suffix = 2;
  while path.exists() {
    name = format!("{}-{}.{}", stem, suffix, ext);
    path = dir.join(&name);
    suffix += 1;
    if suffix > 99 {
      return Err("同名导出文件太多，先清理一下".to_string());
    }
  }
  let bytes = if ext == "md" {
    md_bytes(&bucket(tasks, &monday, &sunday), &label, &monday, &sunday)
  } else {
    csv_bytes(tasks, &label)
  };
  match fs::File::create(&path) {
    Ok(mut file) => match file.write_all(&bytes) {
      Ok(()) => {}
      Err(_) => return Err("这个位置写不了，换个位置".to_string()),
    },
    Err(_) => return Err("这个位置写不了，换个位置".to_string()),
  }
  Ok(path.display().to_string())
}
