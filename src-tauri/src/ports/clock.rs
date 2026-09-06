//! 时间端口（ADR-0002）。此前 `Local::now()` 直调散在 4 个文件 9 处，
//! DND 跨午夜 / 每周重复 / 错过 24h 降级这些时间边界规则一行都测不了。

use chrono::{DateTime, Local};

/// 时间的唯一来源。生产注入 [`SystemClock`]，测试注入 FixedClock。
pub trait Clock: Send {
  fn now(&self) -> DateTime<Local>;
}

/// 真实时钟。只允许出现在组合根（main.rs）与 infra 实现内部。
pub struct SystemClock;

impl Clock for SystemClock {
  fn now(&self) -> DateTime<Local> {
    Local::now()
  }
}

#[cfg(test)]
pub mod test_double {
  use super::Clock;
  use chrono::{DateTime, Duration, Local};
  use std::sync::Mutex;

  /// 固定时钟：`now()` 恒等于被设定的时刻，可在测试中推进/回拨。
  pub struct FixedClock(pub Mutex<DateTime<Local>>);

  impl FixedClock {
    pub fn at(ymd_hm: DateTime<Local>) -> Self {
      FixedClock(Mutex::new(ymd_hm))
    }

    /// 推进（负值即回拨），返回推进后的时刻
    pub fn advance(&self, by: Duration) -> DateTime<Local> {
      let mut guard = self.0.lock().expect("FixedClock 中毒");
      *guard = *guard + by;
      *guard
    }

    pub fn get(&self) -> DateTime<Local> {
      *self.0.lock().expect("FixedClock 中毒")
    }
  }

  impl Clock for FixedClock {
    fn now(&self) -> DateTime<Local> {
      self.get()
    }
  }
}
