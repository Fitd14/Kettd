#![allow(dead_code)]

//! 备份 / 迁移 / 资源相关用户可见中文文案的单一来源（kb-storage-backup 1.3）。
//!
//! 约束（AGENTS §2.3）：
//! - 一律中文短句，**不含文件路径与堆栈**，不以 `Error` / `错误:` 等技术前缀开头；
//! - 后续批次（导出、导入、迁移、图片抽取）直接引用这些常量，不再散落字符串；
//! - 本模块只放**用户可见文案**，内部日志/测试消息不在此列。

/// 备份包整体格式不认识（不是本应用导出的 JSON / 缺关键字段）
pub const BACKUP_FORMAT_INVALID: &str = "备份文件格式不正确，请选择本应用导出的备份文件";

/// `formatVersion` 高于当前应用支持的版本
pub const BACKUP_VERSION_UNSUPPORTED: &str = "备份文件版本不受支持，请升级应用后重试";

/// 导入 payload 超过体积上限（当前 256MB）
pub const BACKUP_FILE_TOO_LARGE: &str = "备份文件过大，无法导入";

/// 备份包结构残缺（缺 data / kb / assets 等必需段）
pub const BACKUP_CONTENT_INCOMPLETE: &str = "备份文件内容不完整，无法导入";

/// assets 条目重算 sha256 与文件名不符（防篡改 / 防损坏）
pub const ASSET_CHECK_FAILED: &str = "图片资源校验失败，文件可能已损坏";

/// 旧版 `notes.json` 一次性迁移失败（design D7 的专属文案）
pub const KB_UPGRADE_FAILED: &str = "知识库升级失败，原数据已保留，请重启后重试";

/// 手动导出备份失败
pub const EXPORT_FAILED: &str = "导出失败，请稍后重试";

/// 手动导入备份失败
pub const IMPORT_FAILED: &str = "导入失败，请稍后重试";

/// 备份包条目 / 资源数量超过安全上限（防内存放大攻击）
pub const BACKUP_TOO_MANY_ENTRIES: &str = "备份内容超出数量上限，无法导入";

/// 备份包中的图片资源不合法（base64 / 扩展名 / 体积 / 声明与内容不符）
pub const BACKUP_ASSET_INVALID: &str = "备份中的图片资源不合法，无法导入";

/// 全部用户可见文案（测试与文档校验遍历用）
pub const ALL_USER_MESSAGES: &[&str] = &[
  BACKUP_FORMAT_INVALID,
  BACKUP_VERSION_UNSUPPORTED,
  BACKUP_FILE_TOO_LARGE,
  BACKUP_CONTENT_INCOMPLETE,
  ASSET_CHECK_FAILED,
  KB_UPGRADE_FAILED,
  EXPORT_FAILED,
  IMPORT_FAILED,
  BACKUP_TOO_MANY_ENTRIES,
  BACKUP_ASSET_INVALID,
];

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn messages_are_non_empty_trimmed_short_chinese() {
    for message in ALL_USER_MESSAGES {
      assert!(!message.trim().is_empty(), "文案不能为空");
      assert_eq!(message.trim(), *message, "不允许首尾空白: {message}");
      assert!(
        message.chars().count() <= 40,
        "用户文案应为短句（≤40 字符）: {message}"
      );
      assert!(
        message
          .chars()
          .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
        "必须含中文: {message}"
      );
      assert!(
        !message.ends_with('。') && !message.ends_with('，'),
        "提示文案不带句末标点（与现有 KB_CORRUPT_MESSAGE 风格一致）: {message}"
      );
    }
  }

  #[test]
  fn messages_contain_no_path_or_stack_fragments() {
    const FORBIDDEN: [&str; 8] = [
      "/", "\\", "C:", "src", ".rs", "panic", "unwrap", "thread '",
    ];
    for message in ALL_USER_MESSAGES {
      for needle in FORBIDDEN {
        assert!(
          !message.contains(needle),
          "文案不得含路径/堆栈片段 {needle:?}: {message}"
        );
      }
    }
  }

  #[test]
  fn messages_have_no_technical_prefix() {
    for message in ALL_USER_MESSAGES {
      let lower = message.to_ascii_lowercase();
      for prefix in ["error", "err:", "warning", "panic", "错误:", "错误：", "异常"] {
        assert!(
          !lower.starts_with(prefix),
          "不得以技术前缀 {prefix:?} 开头: {message}"
        );
      }
      for marker in [':', '：', '-', '—', '!'] {
        assert!(
          !message.starts_with(marker),
          "不得以标点 {marker:?} 开头: {message}"
        );
      }
    }
  }

  #[test]
  fn messages_are_unique() {
    for (index, message) in ALL_USER_MESSAGES.iter().enumerate() {
      for other in ALL_USER_MESSAGES.iter().skip(index + 1) {
        assert_ne!(message, other, "文案重复: {message}");
      }
    }
  }
}