//! 提醒调度（A3 批复：托盘常驻 = 生产机制唯一确定项）
//!
//! - 1 秒 tick，全程 `chrono::Local`（不做任何 UTC 换算）
//! - 到点 → 系统通知「待办提醒」；触发与落库都 emit `store-changed`
//! - 每小时最多 `remindCapPerHour` 条（内存滑窗，超额排队等窗口放开）
//! - 免打扰时段（默认 23:00–07:30，可关）内静默入账，结束后合并补发一条
//! - 错过超过 24h 不再发通知，只记 missed 并清账
//! - 应用未运行期间错过的，下次启动补一条「错过 N 条提醒」汇总通知
//! - snooze 通过 `snoozedUntil` 重触发
//! - 兼任：嵌入桌面形态的悬浮面板失焦后自动收起

use crate::models::{
  clocks_of, fmt_dt, now_text, parse_clock, parse_wall, repeat_allows, to_local, today, Reminder, Settings,
  StoreEvent, MISSED_GRACE_HOURS,
};
use crate::runtime;
use crate::store::{reminder_key, task_key, Store};
use chrono::{DateTime, Duration, Local, NaiveDateTime, Timelike};
use std::collections::VecDeque;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// 一条排队中的到点事件
#[derive(Clone)]
struct Due {
  key: String,
  id: String,
  title: String,
  at: DateTime<Local>,
  snoozed: bool,
  /// 任务级一次性提醒：触发后要清掉 remindAt
  one_shot_task: bool,
  /// 单次提醒（time 带日期）：触发后置 completed
  one_shot_reminder: bool,
}

pub struct NightState {
  warmed_up: bool,
  queue: Vec<Due>,
  sent: VecDeque<DateTime<Local>>,
  quiet: usize,
  last_dnd: bool,
}

impl NightState {
  fn new() -> Self {
    Self {
      warmed_up: false,
      queue: Vec::new(),
      sent: VecDeque::new(),
      quiet: 0,
      last_dnd: false,
    }
  }
}

/// 启动调度线程（托盘常驻，1s tick）
pub fn start(app: AppHandle) {
  std::thread::spawn(move || {
    let mut state = NightState::new();
    loop {
      std::thread::sleep(std::time::Duration::from_millis(1000));
      let slot = match app.try_state::<Mutex<Store>>() {
        None => continue,
        Some(handle) => handle,
      };
      let mut store = match slot.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
      };
      if !store.is_corrupt() {
        tick(&app, &mut store, &mut state);
      }
      drop(store);
      blur_float_on_desktop_form(&app);
    }
  });
}

fn tick(app: &AppHandle, store: &mut Store, state: &mut NightState) {
  let at = Local::now();
  let dnd = in_dnd(&store.data.settings, &at);
  let mut candidates: Vec<Due> = Vec::new();
  collect(store, &at, &mut candidates);
  for due in candidates.into_iter() {
    if store.has_fired(&due.key) || state.queue.iter().any(|item| item.key == due.key) {
      continue;
    }
    state.queue.push(due);
  }
  state
    .sent
    .retain(|when| *when > at - Duration::hours(1));
  let cap = store.data.settings.remind_cap_per_hour.max(1) as usize;
  let mut events: Vec<StoreEvent> = Vec::new();
  let mut changed = false;

  // 1. 超过 24h 的：不再打扰，只记 missed 清账
  let stale: Vec<Due> = state
    .queue
    .iter()
    .filter(|due| at - due.at > Duration::hours(MISSED_GRACE_HOURS))
    .cloned()
    .collect();
  for due in stale.iter() {
    settle(store, due, true, &mut events);
    state.queue.retain(|item| item.key != due.key);
    changed = true;
  }

  // 2. 启动首 tick：错过的合并成一条汇总通知
  if !state.warmed_up {
    state.warmed_up = true;
    if !state.queue.is_empty() {
      let items: Vec<Due> = state.queue.iter().filter(|due| due.at <= at).cloned().collect();
      let count = items.len();
      for due in items.iter() {
        settle(store, due, true, &mut events);
        state.queue.retain(|item| item.key != due.key);
      }
      if count > 0 {
        changed = true;
        runtime::notify(app, &format!("错过 {} 条提醒", count));
        state.sent.push_back(at);
      }
    }
  } else {
    // 3. 常规触发
    let mut index = 0;
    while index < state.queue.len() {
      let due = match state.queue.get(index) {
        Some(value) => value.clone(),
        None => break,
      };
      if due.at > at {
        index += 1;
        continue;
      }
      if dnd {
        // 免打扰：静默入账，等时段结束合并补发
        settle_quiet(store, &due, &mut events);
        state.quiet += 1;
        state.queue.remove(index);
        changed = true;
        continue;
      }
      if state.sent.len() >= cap {
        // 本小时已达上限：留在队列里，下个 tick 再试
        index += 1;
        continue;
      }
      if runtime::notify(app, &body_of(&due)) {
        state.sent.push_back(at);
      }
      settle(store, &due, false, &mut events);
      state.queue.remove(index);
      changed = true;
    }
  }

  // 4. 免打扰结束：把期间静默入账的合并成一条
  if state.last_dnd && !dnd && state.quiet > 0 {
    let count = state.quiet;
    state.quiet = 0;
    runtime::notify(app, &format!("免打扰期间有 {} 条提醒", count));
  }
  state.last_dnd = dnd;
  if changed {
    commit(store, app, events);
  }
}

/// 落库：记 fired；`quiet`=以 missed 语义告知前端
fn settle(store: &mut Store, due: &Due, quiet: bool, events: &mut Vec<StoreEvent>) {
  store.remember_fired(&due.key);
  if due.one_shot_task {
    if let Some(index) = store.find_index(&due.id) {
      store.data.tasks[index].remind_at = None;
      store.data.tasks[index].updated_at = now_text();
    }
  } else if due.one_shot_reminder {
    if let Some(index) = store.find_reminder_index(&due.id) {
      store.data.reminders[index].completed = true;
      store.data.reminders[index].enabled = false;
      store.data.reminders[index].last_fired = Some(fmt_dt(due.at));
      store.data.reminders[index].snoozed_until = None;
    }
  } else if let Some(index) = store.find_reminder_index(&due.id) {
    store.data.reminders[index].last_fired = Some(fmt_dt(due.at));
    store.data.reminders[index].snoozed_until = None;
  }
  if quiet {
    events.push(StoreEvent::missed(&due.id));
  } else {
    events.push(StoreEvent::fired(&due.id));
  }
}

fn settle_quiet(store: &mut Store, due: &Due, events: &mut Vec<StoreEvent>) {
  settle(store, due, true, events);
}

fn commit(store: &mut Store, app: &AppHandle, events: Vec<StoreEvent>) {
  for event in events.iter() {
    let _ = app.emit_all("store-changed", event.clone());
  }
  if let Err(error) = store.save() {
    store.health = crate::store::HEALTH_WRITE_FAILED.to_string();
    store.last_error = Some(error);
  }
}

fn body_of(due: &Due) -> String {
  let mut text = format!("{} · {}", due.title, fmt_dt(due.at));
  if due.snoozed {
    text.push_str("（稍后提醒）");
  }
  text
}

// ---------------------------------------------------------------- 收集到点事件

fn collect(store: &Store, at: &DateTime<Local>, out: &mut Vec<Due>) {
  for reminder in store.data.reminders.iter() {
    if reminder.time.trim().is_empty() {
      continue;
    }
    if let Some(due) = snooze_due(reminder) {
      if due.at <= *at {
        out.push(due);
      }
      continue;
    }
    if !reminder.enabled || reminder.completed {
      continue;
    }
    let clocks = clocks_of(&reminder.time);
    if !clocks.is_empty() {
      // 循环提醒：今天已过的每个时刻各算一次
      for clock in clocks.iter() {
        let wall = today().and_time(*clock);
        if !repeat_allows(reminder, &wall) {
          continue;
        }
        let local = match to_local(&wall) {
          Some(value) => value,
          None => continue,
        };
        if local > *at {
          continue;
        }
        out.push(Due {
          key: reminder_key(&reminder.id, &fmt_naive(&wall)),
          id: reminder.id.clone(),
          title: reminder.title.clone(),
          at: local,
          snoozed: false,
          one_shot_task: false,
          one_shot_reminder: false,
        });
      }
      continue;
    }
    if let Some(due) = one_shot_reminder(reminder, at) {
      out.push(due);
    }
  }
  for task in store.data.tasks.iter() {
    if task.in_trash() || task.done {
      continue;
    }
    let raw = match &task.remind_at {
      Some(text) if !text.trim().is_empty() => text.clone(),
      _ => continue,
    };
    let wall = match parse_wall(&raw) {
      Some(value) => value,
      None => continue,
    };
    let local = match to_local(&wall) {
      Some(value) => value,
      None => continue,
    };
    if local > *at {
      continue;
    }
    // 只写 HH:MM 的 remindAt 视作每日循环（v1 兼容），不清 remindAt
    let daily = raw.chars().count() == 5;
    let key = if daily {
      task_key(&task.id, &fmt_naive(&wall))
    } else {
      task_key(&task.id, &raw)
    };
    out.push(Due {
      key,
      id: task.id.clone(),
      title: task.title.clone(),
      at: local,
      snoozed: false,
      one_shot_task: !daily,
      one_shot_reminder: false,
    });
  }
}

fn fmt_naive(wall: &NaiveDateTime) -> String {
  wall.format("%Y-%m-%dT%H:%M").to_string()
}

fn one_shot_reminder(reminder: &Reminder, at: &DateTime<Local>) -> Option<Due> {
  let wall = parse_wall(&reminder.time)?;
  let local = to_local(&wall)?;
  if local > *at {
    return None;
  }
  Some(Due {
    key: reminder_key(&reminder.id, &fmt_naive(&wall)),
    id: reminder.id.clone(),
    title: reminder.title.clone(),
    at: local,
    snoozed: false,
    one_shot_task: false,
    one_shot_reminder: true,
  })
}

fn snooze_due(reminder: &Reminder) -> Option<Due> {
  let text = reminder.snoozed_until.as_deref()?;
  let wall = parse_wall(text)?;
  let local = to_local(&wall)?;
  Some(Due {
    key: reminder_key(&reminder.id, &format!("snooze|{}", text)),
    id: reminder.id.clone(),
    title: reminder.title.clone(),
    at: local,
    snoozed: true,
    one_shot_task: false,
    one_shot_reminder: false,
  })
}

// ---------------------------------------------------------------- 免打扰

/// 免打扰时段（支持跨零点）
pub fn in_dnd(settings: &Settings, at: &DateTime<Local>) -> bool {
  if !settings.dnd.enabled {
    return false;
  }
  let from = match parse_clock(&settings.dnd.from) {
    Some(value) => value,
    None => return false,
  };
  let to = match parse_clock(&settings.dnd.to) {
    Some(value) => value,
    None => return false,
  };
  let current = at.time();
  if from <= to {
    current >= from && current < to
  } else {
    current >= from || current < to
  }
}

// ---------------------------------------------------------------- 桌面形态失焦收起

fn blur_float_on_desktop_form(app: &AppHandle) {
  if !runtime::float_should_hide_on_blur(app) {
    return;
  }
  if !runtime::float_is_visible(app) || runtime::float_is_focused(app) {
    return;
  }
  if runtime::capture_is_visible(app) {
    return;
  }
  let _ = runtime::hide_window(app, runtime::FLOAT_LABEL);
}

