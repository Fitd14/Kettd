//! 提醒规则：启停互逆、snooze 边界、删除时 fired 键回收（ADR-0003 一期）。
//! 此前这些规则散在 commands.rs（444-452 / 465-466 / 427 行一带），零测试覆盖。

use crate::models::{fmt_dt, Reminder, ReminderPayload};
use crate::store::{reminder_key, Store};
use chrono::{DateTime, Duration, Local};

/// snooze 边界（原 commands.rs:466 的 1..=720）
pub const SNOOZE_MIN_MINUTES: u32 = 1;
pub const SNOOZE_MAX_MINUTES: u32 = 720;

fn not_found() -> String {
  "找不到这条提醒，可能已经被删除".to_string()
}

/// 更新补丁：重新启用的提醒自动置为未完成（enabled/completed 互逆的一半）
pub fn apply_patch(
  store: &mut Store,
  id: &str,
  patch: &ReminderPayload,
) -> Result<Reminder, String> {
  let index = store.find_reminder_index(id).ok_or_else(not_found)?;
  let mut next = store.data.reminders[index].clone();
  Store::apply_reminder_patch(&mut next, patch)?;
  if Some(true) == patch.enabled {
    next.completed = false;
  }
  store.data.reminders[index] = next.clone();
  Ok(next)
}

/// 启停切换：completed 与 enabled 恒相反；重新启用清掉挂起的稍后提醒
pub fn toggle(store: &mut Store, id: &str) -> Result<Reminder, String> {
  let index = store.find_reminder_index(id).ok_or_else(not_found)?;
  let completed = store.data.reminders[index].completed;
  {
    let item = &mut store.data.reminders[index];
    item.completed = !completed;
    item.enabled = completed;
    if item.enabled {
      item.snoozed_until = None;
    }
  }
  Ok(store.data.reminders[index].clone())
}

/// 稍后提醒：1..=720 分钟；到点 = 注入时刻 + minutes
pub fn snooze(
  store: &mut Store,
  id: &str,
  minutes: u32,
  now: DateTime<Local>,
) -> Result<Reminder, String> {
  if !(SNOOZE_MIN_MINUTES..=SNOOZE_MAX_MINUTES).contains(&minutes) {
    return Err("稍后提醒只能设 1 到 720 分钟".to_string());
  }
  let index = store.find_reminder_index(id).ok_or_else(not_found)?;
  let target = fmt_dt(now + Duration::minutes(minutes as i64));
  {
    let item = &mut store.data.reminders[index];
    item.snoozed_until = Some(target);
    item.completed = false;
    item.enabled = true;
  }
  Ok(store.data.reminders[index].clone())
}

/// 删除提醒并回收它自己的 fired 键（键前缀由生成器造，杜绝手写格式漂移）
pub fn delete(store: &mut Store, id: &str) -> Result<(), String> {
  let index = store.find_reminder_index(id).ok_or_else(not_found)?;
  store.data.reminders.remove(index);
  let prefix = reminder_key(id, "");
  store.runtime.fired.retain(|key| !key.starts_with(&prefix));
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::models::{AppData, Repeat};
  use crate::ports::clock::test_double::FixedClock;
  use crate::ports::store_backend::test_double::InMemoryBackend;
  use chrono::TimeZone;

  fn store_with(reminders: Vec<Reminder>) -> Store {
    let mut store = Store::with_backend(
      Box::new(InMemoryBackend::new()),
      Box::new(FixedClock::at(Local.with_ymd_and_hms(2026, 9, 6, 9, 0, 0).unwrap())),
    );
    store.data = AppData {
      reminders,
      ..Default::default()
    };
    store
  }

  fn reminder(id: &str, time: &str) -> Reminder {
    Reminder {
      id: id.to_string(),
      title: format!("提醒 {}", id),
      time: time.to_string(),
      category: "生活".to_string(),
      enabled: true,
      completed: false,
      repeat: Repeat::default(),
      last_fired: None,
      snoozed_until: None,
      legacy: false,
    }
  }

  #[test]
  fn toggle_keeps_enabled_and_completed_inverse() {
    let mut store = store_with(vec![reminder("r1", "09:00")]);
    let after = toggle(&mut store, "r1").unwrap();
    assert!(!after.enabled && after.completed, "停用 = completed 置位");

    let back = toggle(&mut store, "r1").unwrap();
    assert!(back.enabled && !back.completed, "重新启用 = 互逆复位");
  }

  #[test]
  fn toggle_on_clears_pending_snooze() {
    let mut store = store_with(vec![reminder("r1", "09:00")]);
    snooze(&mut store, "r1", 10, Local.with_ymd_and_hms(2026, 9, 6, 9, 0, 0).unwrap()).unwrap();
    // snooze 会把停用的提醒拉回启用；先停用再启用验证清 snooze
    toggle(&mut store, "r1").unwrap();
    let back = toggle(&mut store, "r1").unwrap();
    assert!(back.snoozed_until.is_none(), "重新启用必须清掉挂起的稍后提醒");
  }

  #[test]
  fn snooze_rejects_out_of_range_minutes() {
    let mut store = store_with(vec![reminder("r1", "09:00")]);
    let now = Local.with_ymd_and_hms(2026, 9, 6, 9, 0, 0).unwrap();
    assert!(snooze(&mut store, "r1", 0, now).is_err());
    assert!(snooze(&mut store, "r1", 721, now).is_err());
    let updated = snooze(&mut store, "r1", 720, now).unwrap();
    assert_eq!(updated.snoozed_until.as_deref(), Some("2026-09-06T21:00"));
    assert!(updated.enabled && !updated.completed, "snooze 拉回启用侧");
  }

  #[test]
  fn delete_recycles_only_its_own_fired_keys() {
    let mut store = store_with(vec![reminder("r1", "09:00")]);
    store.runtime.fired = vec![
      reminder_key("r1", "2026-09-05T09:00"),
      reminder_key("r2", "2026-09-05T09:00"),
      "t|t9|2026-09-05T09:00".to_string(),
    ];
    delete(&mut store, "r1").unwrap();
    assert_eq!(
      store.runtime.fired,
      vec![
        reminder_key("r2", "2026-09-05T09:00"),
        "t|t9|2026-09-05T09:00".to_string(),
      ],
      "只回收被删提醒自己的键，任务与他人的键一个不能少"
    );
    assert!(delete(&mut store, "r1").is_err(), "重复删除要报错");
  }
}
