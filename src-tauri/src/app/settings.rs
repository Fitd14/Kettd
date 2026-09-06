//! 设置规则：热键换绑的**两槽位事务**（ADR-0003 一期，原 commands.rs set_settings 内嵌）。
//!
//! 事务语义：主题/形态/两个热键是一次设置动作 —— 任一热键换绑失败，
//! 已写入的字段全部回滚；若主界面键失败而捕获键已换上新值，还要把捕获键
//! 在 OS 侧还原（用户不能因为换绑失败而失去热键）。

use crate::models::{Settings, SettingsPayload};
use crate::ports::hotkeys::{HotkeyPort, HotkeySlot};
use crate::store::Store;

/// 应用设置补丁（含热键换绑事务）。成功后由调用方负责 save + 托盘/事件同步。
pub fn apply_with_hotkeys(
  store: &mut Store,
  patch: &SettingsPayload,
  hotkeys: &mut dyn HotkeyPort,
) -> Result<(), String> {
  let snapshot = store.data.settings.clone();
  store.apply_settings_patch(patch)?;

  let next_capture = store.data.settings.capture_hotkey.clone();
  if next_capture != snapshot.capture_hotkey {
    if let Err(error) = hotkeys.bind(HotkeySlot::Capture, Some(&next_capture)) {
      restore(store, &snapshot);
      return Err(error);
    }
  }

  let next_main = store.data.settings.main_hotkey.clone();
  if next_main != snapshot.main_hotkey {
    if let Err(error) = hotkeys.bind(HotkeySlot::Main, next_main.as_deref()) {
      restore(store, &snapshot);
      // 捕获键已经换上新值了：两槽位是一个事务，一起还原
      let _ = hotkeys.bind(HotkeySlot::Capture, Some(&snapshot.capture_hotkey));
      return Err(error);
    }
  }
  Ok(())
}

fn restore(store: &mut Store, snapshot: &Settings) {
  store.data.settings.theme = snapshot.theme.clone();
  store.data.settings.sticky_pinned = snapshot.sticky_pinned;
  store.data.settings.sticky_paper = snapshot.sticky_paper.clone();
  store.data.settings.capture_hotkey = snapshot.capture_hotkey.clone();
  store.data.settings.main_hotkey = snapshot.main_hotkey.clone();
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
    store.data.settings.sticky_pinned = true;
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
      sticky_pinned: Some(false),
      capture_hotkey: Some("Ctrl+Shift+C".to_string()),
      ..Default::default()
    };
    let error = apply_with_hotkeys(&mut store, &patch, &mut hotkeys).unwrap_err();
    assert!(error.contains("占用"), "失败要给可展示的中文短句：{error}");
    let settings = &store.data.settings;
    assert_eq!(settings.theme, "纸白", "主题回滚");
    assert!(settings.sticky_pinned, "置顶回滚");
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
}
