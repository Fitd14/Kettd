//! 待办规则：创建/更新时的拖留天数、创建入口（ADR-0003 一期）。
//! 软删与撤销语义已是 Store 的方法（soft_delete_task / undo_last），此处不重复。

use crate::models::{carry_days_from, new_id, Source, Task, TaskPayload};
use chrono::NaiveDate;
use crate::store::Store;

/// 新任务的拖留天数：已完成一律 0；「今天」注入（可固定日期测试）
pub fn carried_on_create(done: bool, due_at: &Option<String>, today: NaiveDate) -> u32 {
  if done {
    0
  } else {
    carry_days_from(due_at, today)
  }
}

/// 更新时的拖留天数：历史拖留只增不减；完成/回收站保持原值
pub fn carried_on_update(
  previous: u32,
  done: bool,
  in_trash: bool,
  due_at: &Option<String>,
  today: NaiveDate,
) -> u32 {
  if done || in_trash {
    previous
  } else {
    previous.max(carry_days_from(due_at, today))
  }
}

/// 创建待办：标题非空 + 默认值 + 拖留初值。时间戳与「今天」由调用方注入。
pub fn add(
  store: &mut Store,
  args: TaskPayload,
  stamp: &str,
  today: NaiveDate,
) -> Result<Task, String> {
  if args.title.as_deref().unwrap_or("").trim().is_empty() {
    return Err("标题不能为空".to_string());
  }
  let mut task = Task {
    id: new_id("t"),
    created_at: stamp.to_string(),
    updated_at: stamp.to_string(),
    source: args.source.unwrap_or(Source::Manual),
    ..Default::default()
  };
  Store::apply_task_patch(&mut task, &args)?;
  task.carried_from = carried_on_create(task.done, &task.due_at, today);
  store.data.tasks.push(task.clone());
  Ok(task)
}

/// 更新待办：补丁应用 + 拖留只增不减
pub fn update(
  store: &mut Store,
  id: &str,
  patch: TaskPayload,
  stamp: &str,
  today: NaiveDate,
) -> Result<Task, String> {
  let index = store
    .find_index(id)
    .ok_or_else(|| "找不到这条待办，可能已经被删除".to_string())?;
  let mut next = store.data.tasks[index].clone();
  Store::apply_task_patch(&mut next, &patch)?;
  next.updated_at = stamp.to_string();
  next.carried_from = carried_on_update(
    next.carried_from,
    next.done,
    next.in_trash(),
    &next.due_at,
    today,
  );
  store.data.tasks[index] = next.clone();
  Ok(next)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::models::AppData;
  use crate::ports::clock::test_double::FixedClock;
  use crate::ports::store_backend::test_double::InMemoryBackend;
  use chrono::{Local, TimeZone};

  fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 6).unwrap()
  }

  fn mem_store() -> Store {
    let mut store = Store::with_backend(
      Box::new(InMemoryBackend::new()),
      Box::new(FixedClock::at(Local.with_ymd_and_hms(2026, 9, 6, 9, 0, 0).unwrap())),
    );
    store.data = AppData::default();
    store
  }

  fn payload(title: &str, due_at: Option<&str>) -> TaskPayload {
    TaskPayload {
      title: Some(title.to_string()),
      due_at: due_at.map(|text| serde_json::Value::String(text.to_string())),
      ..Default::default()
    }
  }

  #[test]
  fn create_rejects_blank_title_without_touching_store() {
    let mut store = mem_store();
    assert!(add(&mut store, payload("   ", None), "stamp", today()).is_err());
    assert!(store.data.tasks.is_empty(), "失败创建不得留半成品");
  }

  #[test]
  fn create_computes_carried_days_from_injected_today() {
    let mut store = mem_store();
    // 截止 09-04，今天 09-06 → 拖了 2 天
    let task = add(&mut store, payload("作业", Some("2026-09-04T18:00")), "s", today()).unwrap();
    assert_eq!(task.carried_from, 2);
    // 未来截止 → 0
    let task = add(&mut store, payload("明天的事", Some("2026-09-07T09:00")), "s", today()).unwrap();
    assert_eq!(task.carried_from, 0);
    // 已完成 → 0（即使截止早过了）
    let mut args = payload("旧事", Some("2026-08-01T09:00"));
    args.done = Some(true);
    let task = add(&mut store, args, "s", today()).unwrap();
    assert_eq!(task.carried_from, 0);
  }

  #[test]
  fn update_never_shrinks_carried_days() {
    let mut store = mem_store();
    let task = add(&mut store, payload("旧账", Some("2026-08-01T09:00")), "s", today()).unwrap();
    assert!(task.carried_from > 30);
    // 把截止改成未来：历史拖留不因改字段被抹掉
    let next = update(
      &mut store,
      &task.id,
      payload("旧账", Some("2026-09-07T09:00")),
      "s2",
      today(),
    )
    .unwrap();
    assert_eq!(
      next.carried_from, task.carried_from,
      "拖留只增不减：改截止不清历史"
    );
  }

  #[test]
  fn update_keeps_carried_when_done_or_trashed() {
    let mut store = mem_store();
    let task = add(&mut store, payload("事项", Some("2026-08-01T09:00")), "s", today()).unwrap();
    let prev = task.carried_from;
    let next = update(
      &mut store,
      &task.id,
      TaskPayload { done: Some(true), ..Default::default() },
      "s2",
      today(),
    )
    .unwrap();
    assert_eq!(next.carried_from, prev, "完成态不重算拖留");
  }
}
