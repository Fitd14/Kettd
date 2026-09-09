//! Tauri 全局热键实现。注册/回滚的实际行为在 runtime（窗口/托盘耦合），
//! 这里把「三个槽位」翻译成对应的 runtime 调用，使 app::settings 的事务可测试。

use crate::ports::hotkeys::{HotkeyPort, HotkeySlot};
use tauri::AppHandle;

pub struct TauriHotkeys {
  app: AppHandle,
}

impl TauriHotkeys {
  pub fn new(app: &AppHandle) -> Self {
    TauriHotkeys { app: app.clone() }
  }
}

impl HotkeyPort for TauriHotkeys {
  fn bind(&mut self, slot: HotkeySlot, combo: Option<&str>) -> Result<(), String> {
    match slot {
      HotkeySlot::Capture => crate::runtime::register_capture_hotkey(
        &self.app,
        combo.ok_or_else(|| "快捷键不能为空".to_string())?,
      ),
      HotkeySlot::Main => crate::runtime::register_main_hotkey(&self.app, &combo.map(str::to_string)),
      HotkeySlot::Sticky => crate::runtime::register_sticky_hotkey(&self.app, &combo.map(str::to_string)),
      HotkeySlot::TodoFloat => crate::runtime::register_todo_hotkey(&self.app, &combo.map(str::to_string)),
    }
  }

  fn bound(&self, slot: HotkeySlot) -> Option<String> {
    crate::runtime::bound_combo(&self.app, slot)
  }
}
