//! 系统通知端口（ADR-0002）。调度器的 ② 段经此发通知，
//! 测试用 RecordingNotifier 断言「发了什么、发了几条」，不建窗、不弹真通知。

pub trait Notifier: Send {
  /// `false` = 通道不可用。调度语义：失败也照常落库记 fired
  /// （不重复打扰，也不静默漏记），调用方不重试。
  fn notify(&self, title: &str, body: &str) -> bool;
}

#[cfg(test)]
pub mod test_double {
  use super::Notifier;
  use std::sync::Mutex;

  /// 记录型通知器：永远"送达成功"，把调用记录下来供断言。
  pub struct RecordingNotifier {
    pub calls: Mutex<Vec<(String, String)>>,
    pub fail: bool,
  }

  impl RecordingNotifier {
    pub fn new() -> Self {
      Self {
        calls: Mutex::new(Vec::new()),
        fail: false,
      }
    }

    pub fn bodies(&self) -> Vec<String> {
      self
        .calls
        .lock()
        .expect("RecordingNotifier 中毒")
        .iter()
        .map(|(_, body)| body.clone())
        .collect()
    }
  }

  impl Default for RecordingNotifier {
    fn default() -> Self {
      Self::new()
    }
  }

  impl Notifier for RecordingNotifier {
    fn notify(&self, title: &str, body: &str) -> bool {
      self
        .calls
        .lock()
        .expect("RecordingNotifier 中毒")
        .push((title.to_string(), body.to_string()));
      !self.fail
    }
  }
}
