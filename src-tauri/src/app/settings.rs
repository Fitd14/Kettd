//! 设置规则：热键换绑的**三槽位事务**（ADR-0003，原两槽位扩展 sticky-separation）。
//!
//! 事务语义：主题/热键是一次设置动作 —— 任一热键换绑失败，
//! 已写入的字段全部回滚；且先前已换上新值的槽位要在 OS 侧还原
//! （用户不能因为换绑失败而失去热键）。

use crate::models::{Settings, SettingsPayload};
use crate::ports::hotkeys::{HotkeyPort, HotkeySlot};
use crate::store::Store;

/// 应用设置补丁（含热键换绑事务）。成功后由调用方负责 save + 托盘/事件同步。
/// 四槽位顺序换绑；任一失败 → 设置字段整批回滚 + 已换新值的槽位在 OS 侧还原旧键
/// （没动过的槽位本就绑在旧键上，不碰）。
pub fn apply_with_hotkeys(
  store: &mut Store,
  patch: &SettingsPayload,
  hotkeys: &mut dyn HotkeyPort,
) -> Result<(), String> {
  let snapshot = store.data.settings.clone();
  store.apply_settings_patch(patch)?;

  let slots: [(HotkeySlot, Option<String>, Option<String>); 4] = [
    (
      HotkeySlot::Capture,
      Some(store.data.settings.capture_hotkey.clone()),
      Some(snapshot.capture_hotkey.clone()),
    ),
    (
      HotkeySlot::Main,
      store.data.settings.main_hotkey.clone(),
      snapshot.main_hotkey.clone(),
    ),
    (
      HotkeySlot::Sticky,
      store.data.settings.sticky_hotkey.clone(),
      snapshot.sticky_hotkey.clone(),
    ),
    (
      HotkeySlot::TodoFloat,
      store.data.settings.todo_hotkey.clone(),
      snapshot.todo_hotkey.clone(),
    ),
  ];
  let mut applied: Vec<(HotkeySlot, Option<String>)> = Vec::new();
  for (slot, next, previous) in slots {
    if next == previous {
      continue;
    }
    if let Err(error) = hotkeys.bind(slot, next.as_deref()) {
      restore(store, &snapshot);
      for (s, old) in applied {
        let _ = hotkeys.bind(s, old.as_deref());
      }
      return Err(error);
    }
    applied.push((slot, previous));
  }
  Ok(())
}

fn restore(store: &mut Store, snapshot: &Settings) {
  store.data.settings.theme = snapshot.theme.clone();
  store.data.settings.sticky_paper = snapshot.sticky_paper.clone();
  store.data.settings.capture_hotkey = snapshot.capture_hotkey.clone();
  store.data.settings.main_hotkey = snapshot.main_hotkey.clone();
  store.data.settings.sticky_hotkey = snapshot.sticky_hotkey.clone();
  store.data.settings.todo_hotkey = snapshot.todo_hotkey.clone();
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::models::AppData;
  use crate::ports::clock::test_double::FixedClock;
  use crate::ports::hotkeys::test_double::StubHotkeys;
  use crate::ports::store_backend::test_double::InMemoryBackend;
  use chrono::{Local, TimeZone};

  fn mem_store() -> Store {
    let mut store = Store::with_backend(
      Box::new(InMemoryBackend::new()),
      Box::new(FixedClock::at(Local.with_ymd_and_hms(2026, 9, 6, 9, 0, 0).unwrap())),
    );
    store.data = AppData::default();
    store.data.settings.capture_hotkey = "Alt+Shift+A".to_string();
    store.data.settings.theme = "纸白".to_string();
    store
  }

  fn patch_theme(theme: &str) -> SettingsPayload {
    SettingsPayload {
      theme: Some(theme.to_string()),
      ..Default::default()
    }
  }

  #[test]
  fn theme_only_patch_never_touches_hotkey_slots() {
    let mut store = mem_store();
    let mut hotkeys = StubHotkeys::new();
    apply_with_hotkeys(&mut store, &patch_theme("石墨"), &mut hotkeys).unwrap();
    assert_eq!(store.data.settings.theme, "石墨");
    assert!(
      hotkeys.calls.lock().unwrap().is_empty(),
      "热键没变就不得触碰 OS 绑定"
    );
  }

  #[test]
  fn capture_conflict_rolls_back_every_field() {
    let mut store = mem_store();
    let mut hotkeys = StubHotkeys::new();
    hotkeys.fail_when = Some("Ctrl+Shift+C".to_string());
    let patch = SettingsPayload {
      theme: Some("石墨".to_string()),
      capture_hotkey: Some("Ctrl+Shift+C".to_string()),
      ..Default::default()
    };
    let error = apply_with_hotkeys(&mut store, &patch, &mut hotkeys).unwrap_err();
    assert!(error.contains("占用"), "失败要给可展示的中文短句：{error}");
    let settings = &store.data.settings;
    assert_eq!(settings.theme, "纸白", "主题回滚");
    assert_eq!(settings.capture_hotkey, "Alt+Shift+A", "捕获键回滚");
  }

  #[test]
  fn main_conflict_rolls_back_and_restores_previous_capture_binding() {
    let mut store = mem_store();
    store.data.settings.main_hotkey = Some("Alt+Shift+O".to_string());
    let mut hotkeys = StubHotkeys::new();
    hotkeys.fail_when = Some("Ctrl+Shift+O".to_string());
    let patch = SettingsPayload {
      capture_hotkey: Some("Alt+Shift+Q".to_string()),
      main_hotkey: Some("Ctrl+Shift+O".to_string()),
      ..Default::default()
    };
    assert!(apply_with_hotkeys(&mut store, &patch, &mut hotkeys).is_err());

    let calls = hotkeys.calls.lock().unwrap();
    let seq: Vec<_> = calls
      .iter()
      .map(|(slot, combo)| (*slot, combo.clone()))
      .collect();
    assert_eq!(
      seq,
      vec![
        (HotkeySlot::Capture, Some("Alt+Shift+Q".to_string())),
        (HotkeySlot::Main, Some("Ctrl+Shift+O".to_string())),
        (HotkeySlot::Capture, Some("Alt+Shift+A".to_string())),
      ],
      "捕获键先换新 → 主键失败 → 把捕获键还原回旧值（两槽位一个事务）"
    );
    assert_eq!(store.data.settings.capture_hotkey, "Alt+Shift+A");
    assert_eq!(
      store.data.settings.main_hotkey,
      Some("Alt+Shift+O".to_string()),
      "主键回滚到换绑前的值"
    );
  }

  #[test]
  fn invalid_patch_rejected_before_any_hotkey_call() {
    let mut store = mem_store();
    let mut hotkeys = StubHotkeys::new();
    let patch = SettingsPayload {
      theme: Some("  ".to_string()),
      capture_hotkey: Some("Ctrl+Shift+C".to_string()),
      ..Default::default()
    };
    assert!(apply_with_hotkeys(&mut store, &patch, &mut hotkeys).is_err());
    assert!(hotkeys.calls.lock().unwrap().is_empty(), "校验失败不碰 OS");
    assert_eq!(store.data.settings.capture_hotkey, "Alt+Shift+A");
  }

  #[test]
  fn unbinding_main_hotkey_is_a_valid_operation() {
    let mut store = mem_store();
    store.data.settings.main_hotkey = Some("Alt+Shift+O".to_string());
    let mut hotkeys = StubHotkeys::new();
    let patch = SettingsPayload {
      main_hotkey: Some("".to_string()),
      ..Default::default()
    };
    apply_with_hotkeys(&mut store, &patch, &mut hotkeys).unwrap();
    assert_eq!(store.data.settings.main_hotkey, None);
    assert_eq!(
      hotkeys.bound(HotkeySlot::Main),
      None,
      "空串=解绑，槽位清空"
    );
  }

  /// 便签热键（sticky-separation 第三槽）与其他槽位同一事务语义。
  #[test]
  fn sticky_conflict_rolls_back_all_preceding_slots() {
    let mut store = mem_store();
    store.data.settings.sticky_hotkey = Some("Alt+Shift+S".to_string());
    let mut hotkeys = StubHotkeys::new();
    hotkeys.fail_when = Some("Ctrl+Shift+S".to_string());
    let patch = SettingsPayload {
      capture_hotkey: Some("Alt+Shift+Q".to_string()),
      sticky_hotkey: Some("Ctrl+Shift+S".to_string()),
      ..Default::default()
    };
    assert!(apply_with_hotkeys(&mut store, &patch, &mut hotkeys).is_err());

    let calls = hotkeys.calls.lock().unwrap();
    let seq: Vec<_> = calls
      .iter()
      .map(|(slot, combo)| (*slot, combo.clone()))
      .collect();
    assert_eq!(
      seq,
      vec![
        (HotkeySlot::Capture, Some("Alt+Shift+Q".to_string())),
        (HotkeySlot::Sticky, Some("Ctrl+Shift+S".to_string())),
        (HotkeySlot::Capture, Some("Alt+Shift+A".to_string())),
      ],
      "捕获键先换新 → 便签键失败 → 把捕获键还原（三槽位一个事务）"
    );
    assert_eq!(store.data.settings.capture_hotkey, "Alt+Shift+A");
    assert_eq!(
      store.data.settings.sticky_hotkey,
      Some("Alt+Shift+S".to_string()),
      "便签键回滚到换绑前的值"
    );
  }

  #[test]
  fn unbinding_sticky_hotkey_is_a_valid_operation() {
    let mut store = mem_store();
    store.data.settings.sticky_hotkey = Some("Alt+Shift+S".to_string());
    let mut hotkeys = StubHotkeys::new();
    let patch = SettingsPayload {
      sticky_hotkey: Some("".to_string()),
      ..Default::default()
    };
    apply_with_hotkeys(&mut store, &patch, &mut hotkeys).unwrap();
    assert_eq!(store.data.settings.sticky_hotkey, None);
    assert_eq!(hotkeys.bound(HotkeySlot::Sticky), None, "空串=解绑");
  }

  /// 待办悬浮窗热键（todo-float 第四槽）与其他槽位同一事务语义。
  #[test]
  fn todo_conflict_rolls_back_all_preceding_slots() {
    let mut store = mem_store();
    store.data.settings.todo_hotkey = Some("Alt+Shift+T".to_string());
    let mut hotkeys = StubHotkeys::new();
    hotkeys.fail_when = Some("Ctrl+Shift+T".to_string());
    let patch = SettingsPayload {
      capture_hotkey: Some("Alt+Shift+Q".to_string()),
      sticky_hotkey: Some("Ctrl+Shift+Z".to_string()),
      todo_hotkey: Some("Ctrl+Shift+T".to_string()),
      ..Default::default()
    };
    assert!(apply_with_hotkeys(&mut store, &patch, &mut hotkeys).is_err());

    let calls = hotkeys.calls.lock().unwrap();
    let seq: Vec<_> = calls
      .iter()
      .map(|(slot, combo)| (*slot, combo.clone()))
      .collect();
    assert_eq!(
      seq,
      vec![
        (HotkeySlot::Capture, Some("Alt+Shift+Q".to_string())),
        (HotkeySlot::Sticky, Some("Ctrl+Shift+Z".to_string())),
        (HotkeySlot::TodoFloat, Some("Ctrl+Shift+T".to_string())),
        (HotkeySlot::Capture, Some("Alt+Shift+A".to_string())),
        (HotkeySlot::Sticky, Some("Alt+Shift+S".to_string())),
      ],
      "捕获/便签先换新 → 待办键失败 → 把变更过的槽位按序还原"
    );
    assert_eq!(store.data.settings.capture_hotkey, "Alt+Shift+A");
    assert_eq!(
      store.data.settings.sticky_hotkey,
      Some("Alt+Shift+S".to_string())
    );
    assert_eq!(
      store.data.settings.todo_hotkey,
      Some("Alt+Shift+T".to_string()),
      "待办键回滚到换绑前的值"
    );
  }

  #[test]
  fn unbinding_todo_hotkey_is_a_valid_operation() {
    let mut store = mem_store();
    store.data.settings.todo_hotkey = Some("Alt+Shift+T".to_string());
    let mut hotkeys = StubHotkeys::new();
    let patch = SettingsPayload {
      todo_hotkey: Some("".to_string()),
      ..Default::default()
    };
    apply_with_hotkeys(&mut store, &patch, &mut hotkeys).unwrap();
    assert_eq!(store.data.settings.todo_hotkey, None);
    assert_eq!(hotkeys.bound(HotkeySlot::TodoFloat), None, "空串=解绑");
  }
}

