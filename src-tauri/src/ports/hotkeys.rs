//! 全局热键端口（ADR-0002）。应用共三个热键槽位（快速记录 / 打开主界面 / 新建便签），
//! 「换绑失败回滚旧键」的事务语义由实现负责（对应命令层原内嵌规则，ADR-0003 下沉目标）。

/// 热键槽位：新增槽位 = 新增枚举值 + 设置项
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeySlot {
  /// 呼出快速记录条（默认 Alt+Shift+A）
  Capture,
  /// 打开主界面
  Main,
  /// 新建一张便签（默认 Alt+Shift+S，sticky-separation）
  Sticky,
  /// 呼出/隐藏待办悬浮窗（默认 Alt+Shift+T，todo-float）
  TodoFloat,
}

pub trait HotkeyPort: Send {
  /// 换绑槽位；`None`/空串 = 解绑。失败返回可直接展示的中文短句，
  /// 且实现必须保证槽位仍绑在旧键上（用户不能因为换绑失败而失去热键）。
  fn bind(&mut self, slot: HotkeySlot, combo: Option<&str>) -> Result<(), String>;

  /// 槽位当前生效的组合键（设置页「当前绑定」的唯一事实来源）
  fn bound(&self, slot: HotkeySlot) -> Option<String>;
}

#[cfg(test)]
pub mod test_double {
  use super::{HotkeyPort, HotkeySlot};
  use std::sync::Mutex;

  /// 桩热键：bind 恒成功（或按 fail_when 指定组合失败），记录调用。
  pub struct StubHotkeys {
    pub slots: Mutex<[Option<String>; 4]>,
    /// 换绑到该组合时失败（模拟被占用）
    pub fail_when: Option<String>,
    pub calls: Mutex<Vec<(HotkeySlot, Option<String>)>>,
  }

  impl StubHotkeys {
    pub fn new() -> Self {
      Self {
        slots: Mutex::new([None, None, None, None]),
        fail_when: None,
        calls: Mutex::new(Vec::new()),
      }
    }

    fn index(slot: HotkeySlot) -> usize {
      match slot {
        HotkeySlot::Capture => 0,
        HotkeySlot::Main => 1,
        HotkeySlot::Sticky => 2,
        HotkeySlot::TodoFloat => 3,
      }
    }
  }

  impl Default for StubHotkeys {
    fn default() -> Self {
      Self::new()
    }
  }

  impl HotkeyPort for StubHotkeys {
    fn bind(&mut self, slot: HotkeySlot, combo: Option<&str>) -> Result<(), String> {
      self
        .calls
        .lock()
        .expect("StubHotkeys 中毒")
        .push((slot, combo.map(str::to_string)));
      let target = combo.map(str::trim).filter(|value| !value.is_empty());
      if let (Some(fail), Some(value)) = (&self.fail_when, target) {
        if fail == value {
          return Err("快捷键被占用，请用备用入口".to_string());
        }
      }
      let mut slots = self.slots.lock().expect("StubHotkeys 中毒");
      slots[Self::index(slot)] = target.map(str::to_string);
      Ok(())
    }

    fn bound(&self, slot: HotkeySlot) -> Option<String> {
      self.slots.lock().expect("StubHotkeys 中毒")[Self::index(slot)].clone()
    }
  }
}
