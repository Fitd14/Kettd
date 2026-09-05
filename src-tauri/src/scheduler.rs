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
use crate::telemetry;
use chrono::{DateTime, Duration, Local, NaiveDateTime};
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
///
/// 锁边界（ADR-0004）：一次 tick 分四段，**OS 通知与 emit 一律不在持锁期间发生**：
/// ① 持短锁 plan（只读 + 决策，不写库不通知）→ 放锁
/// ② 锁外发系统通知、记遥测（拿到逐条送达结果）
/// ③ 再持短锁 apply（幂等重校验后落库 + save）→ 放锁
/// ④ 锁外 emit
/// 磁盘写有意留在 ③ 的锁内：读-改-写必须原子，移出锁会引入并发写风险，
/// 且它远快于 OS 通知 IPC —— 这不是"没做完"。
pub fn start(app: AppHandle) {
  std::thread::spawn(move || {
    let mut state = NightState::new();
    loop {
      std::thread::sleep(std::time::Duration::from_millis(1000));
      let slot = match app.try_state::<Mutex<Store>>() {
        None => continue,
        Some(handle) => handle,
      };
      // ① 短锁决策
      let plan = {
        let mut store = match slot.lock() {
          Ok(guard) => guard,
          Err(poisoned) => poisoned.into_inner(),
        };
        match plan_tick(&mut store, &mut state) {
          Some(plan) => plan,
          None => continue,
        }
      }; // ← 锁在此释放
      // ② 锁外通知 + 遥测
      let outcome = deliver(&app, plan);
      // ③ 短锁落库
      let events = {
        let mut store = match slot.lock() {
          Ok(guard) => guard,
          Err(poisoned) => poisoned.into_inner(),
        };
        apply(&mut store, &mut state, outcome)
      };
      // ④ 锁外广播
      broadcast(&app, events);
      blur_float_on_desktop_form(&app);
    }
  });
}

/// ① 阶段的产物：锁外要发的通知 + 锁内要落的库
struct Plan {
  /// 逐条发通知的到点事件（发完在 ③ 落库）
  fires: Vec<Due>,
  /// 汇总类通知（启动补发、免打扰合并）
  batches: Vec<String>,
  /// ③ 阶段要落库的动作：(事件, quiet)
  settles: Vec<(Due, bool)>,
  /// 本 tick 是否有变化（决定是否 save / emit）
  dirty: bool,
}

/// ② 阶段的产物
struct Outcome {
  settles: Vec<(Due, bool)>,
  /// 逐条通知成功送达的条数 → 计入每小时滑窗
  shown: usize,
  dirty: bool,
}

/// ① 持短锁决策：只读 Store、只改线程内的 NightState，**不写库、不发通知**
fn plan_tick(store: &mut Store, state: &mut NightState) -> Option<Plan> {
  if store.is_corrupt() {
    return None; // corrupt 状态下不发通知也不写库（等用户恢复）
  }
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
  let mut plan = Plan {
    fires: Vec::new(),
    batches: Vec::new(),
    settles: Vec::new(),
    dirty: false,
  };

  // 1. 超过 24h 的：不再打扰，只记 missed 清账
  let stale: Vec<Due> = state
    .queue
    .iter()
    .filter(|due| at - due.at > Duration::hours(MISSED_GRACE_HOURS))
    .cloned()
    .collect();
  for due in stale.iter() {
    state.queue.retain(|item| item.key != due.key);
    telemetry::record_str(
      "reminder_missed",
      &[("id", due.id.as_str()), ("reason", "expired")],
    );
    plan.settles.push((due.clone(), true));
    plan.dirty = true;
  }

  // 2. 启动首 tick：错过的合并成一条汇总通知
  if !state.warmed_up {
    state.warmed_up = true;
    let items: Vec<Due> = state.queue.iter().filter(|due| due.at <= at).cloned().collect();
    let count = items.len();
    for due in items.iter() {
      state.queue.retain(|item| item.key != due.key);
      telemetry::record_str(
        "reminder_missed",
        &[("id", due.id.as_str()), ("reason", "app_not_running")],
      );
      plan.settles.push((due.clone(), true));
    }
    if count > 0 {
      plan.batches.push(format!("错过 {} 条提醒", count));
      state.sent.push_back(at);
      plan.dirty = true;
    }
  } else {
    // 3. 常规触发。上限语义：本轮最多放行 `cap - 已发条数` 条，宁少不超
    //    （原实现是"发成功才计数"，同轮内可能多放行；改为预扣后，
    //      通知失败的那条要等下个 tick 才补，绝不会突破每小时上限）
    let mut budget = cap.saturating_sub(state.sent.len());
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
        state.queue.remove(index);
        telemetry::record_str(
          "reminder_missed",
          &[("id", due.id.as_str()), ("reason", "dnd")],
        );
        state.quiet += 1;
        plan.settles.push((due, true));
        plan.dirty = true;
        continue;
      }
      if budget == 0 {
        index += 1;
        continue; // 本小时已达上限：留在队列里，下个 tick 再试
      }
      budget -= 1;
      state.queue.remove(index);
      plan.fires.push(due);
      plan.dirty = true;
    }
  }

  // 4. 免打扰结束：把期间静默入账的合并成一条
  if state.last_dnd && !dnd && state.quiet > 0 {
    let count = state.quiet;
    state.quiet = 0;
    plan.batches.push(format!("免打扰期间有 {} 条提醒", count));
  }
  state.last_dnd = dnd;
  Some(plan)
}

/// ② 锁外执行：发系统通知 + 记逐条遥测（这两样都不碰 Store）
fn deliver(app: &AppHandle, plan: Plan) -> Outcome {
  for body in plan.batches.iter() {
    runtime::notify(app, body);
  }
  let mut shown = 0usize;
  for due in plan.fires.iter() {
    // 通知失败也照样落库记 fired —— 保持既有语义：不重复打扰，但也不静默漏记
    if runtime::notify(app, &body_of(due)) {
      shown += 1;
      telemetry::record_str("reminder_shown", &[("id", due.id.as_str())]);
    } else {
      telemetry::record_str(
        "reminder_missed",
        &[("id", due.id.as_str()), ("reason", "notify_fail")],
      );
    }
  }
  let mut settles = plan.settles;
  for due in plan.fires.into_iter() {
    settles.push((due, false));
  }
  Outcome {
    settles,
    shown,
    dirty: plan.dirty,
  }
}

/// ③ 持短锁落库：幂等重校验 → settle → save（磁盘写有意留在锁内）
fn apply(store: &mut Store, state: &mut NightState, outcome: Outcome) -> Vec<StoreEvent> {
  if !outcome.dirty {
    return Vec::new();
  }
  let now = Local::now();
  let mut events: Vec<StoreEvent> = Vec::new();
  for (due, quiet) in outcome.settles.iter() {
    // 幂等重校验：①–③ 之间这条若已被别处记过，就不再重复落库与广播
    if store.has_fired(&due.key) {
      continue;
    }
    settle(store, due, *quiet, &mut events);
  }
  for _ in 0..outcome.shown {
    state.sent.push_back(now);
  }
  if events.is_empty() {
    return Vec::new();
  }
  if let Err(error) = store.save() {
    store.health = crate::store::HEALTH_WRITE_FAILED.to_string();
    store.last_error = Some(error);
  }
  events
}

/// ④ 锁外广播（放在 save 之后：避免前端重拉到时读到未落盘的旧数据）
fn broadcast(app: &AppHandle, events: Vec<StoreEvent>) {
  for event in events.iter() {
    let _ = app.emit_all("store-changed", event.clone());
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

