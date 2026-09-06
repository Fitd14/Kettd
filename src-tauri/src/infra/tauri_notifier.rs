//! Tauri 系统通知实现。点击后的深链由前端窗口处理（不带 payload），与原 runtime 行为一致。

use crate::ports::notifier::Notifier;
use tauri::AppHandle;

pub struct TauriNotifier {
  app: AppHandle,
}

impl TauriNotifier {
  pub fn new(app: AppHandle) -> Self {
    TauriNotifier { app }
  }
}

impl Notifier for TauriNotifier {
  fn notify(&self, title: &str, body: &str) -> bool {
    crate::runtime::notify_title(&self.app, title, body)
  }
}
