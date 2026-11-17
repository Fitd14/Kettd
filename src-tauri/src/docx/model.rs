//! 导出块模型（kb-export-word D2）。
//!
//! 前端 `src-react/src/lib/docx-blocks.ts` 解析 Markdown 后按这份契约序列化，
//! Rust 侧只负责把它写成 OOXML。字段与前端 JSON 一一对应（`kind` 判别、camelCase）。
//!
//! 边界：模型里不放路径之外的 IO 语义；图片只留 `assets/` 相对路径，读字节由 media 模块负责。

use serde::Deserialize;

/// 一段行内文字及其样式。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct Run {
  pub text: String,
  #[serde(default)]
  pub bold: bool,
  #[serde(default)]
  pub italic: bool,
  #[serde(default)]
  pub code: bool,
  #[serde(default)]
  pub link: Option<String>,
}

/// 列表项：`depth` 从 0 起，Word 侧映射成 `w:ilvl`。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ListItem {
  pub depth: u8,
  pub runs: Vec<Run>,
}

/// 块模型：一个块对应 Word 里的一段（或一个表格 / 一张图）。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExportBlock {
  Heading {
    level: u8,
    runs: Vec<Run>,
  },
  Paragraph {
    runs: Vec<Run>,
  },
  Bullets {
    items: Vec<ListItem>,
  },
  Numbers {
    items: Vec<ListItem>,
  },
  Quote {
    blocks: Vec<ExportBlock>,
  },
  Code {
    #[serde(default)]
    lang: Option<String>,
    text: String,
  },
  Table {
    #[serde(default)]
    header: bool,
    rows: Vec<Vec<String>>,
  },
  Image {
    asset: String,
    #[serde(default)]
    alt: Option<String>,
  },
}

impl Run {
  /// 纯文本 run（表格单元格、占位文案用）。
  pub fn plain(text: impl Into<String>) -> Self {
    Self {
      text: text.into(),
      ..Self::default()
    }
  }

  /// 该 run 是否没有任何可见文字——空 run 不产出 `w:r`。
  pub fn is_blank(&self) -> bool {
    self.text.is_empty()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(json: &str) -> ExportBlock {
    serde_json::from_str(json).expect("应能反序列化")
  }

  #[test]
  fn parses_heading_and_paragraph() {
    let heading = parse(r#"{"kind":"heading","level":2,"runs":[{"text":"标题"}]}"#);
    match heading {
      ExportBlock::Heading { level, runs } => {
        assert_eq!(level, 2);
        assert_eq!(runs[0].text, "标题");
        assert!(!runs[0].bold);
      }
      other => panic!("应为 heading，实际 {other:?}"),
    }

    let paragraph = parse(r#"{"kind":"paragraph","runs":[{"text":"粗","bold":true}]}"#);
    match paragraph {
      ExportBlock::Paragraph { runs } => assert!(runs[0].bold),
      other => panic!("应为 paragraph，实际 {other:?}"),
    }
  }

  #[test]
  fn parses_lists_with_depth() {
    let block = parse(
      r#"{"kind":"bullets","items":[{"depth":0,"runs":[{"text":"a"}]},{"depth":2,"runs":[{"text":"b"}]}]}"#,
    );
    match block {
      ExportBlock::Bullets { items } => {
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].depth, 2);
      }
      other => panic!("应为 bullets，实际 {other:?}"),
    }
  }

  #[test]
  fn parses_optional_fields_with_defaults() {
    let code = parse(r#"{"kind":"code","text":"let x = 1;"}"#);
    match code {
      ExportBlock::Code { lang, text } => {
        assert!(lang.is_none());
        assert_eq!(text, "let x = 1;");
      }
      other => panic!("应为 code，实际 {other:?}"),
    }

    let table = parse(r#"{"kind":"table","rows":[["a","b"]]}"#);
    match table {
      ExportBlock::Table { header, rows } => {
        assert!(!header);
        assert_eq!(rows[0], vec!["a".to_string(), "b".to_string()]);
      }
      other => panic!("应为 table，实际 {other:?}"),
    }

    let image = parse(r#"{"kind":"image","asset":"assets/x.png"}"#);
    match image {
      ExportBlock::Image { asset, alt } => {
        assert_eq!(asset, "assets/x.png");
        assert!(alt.is_none());
      }
      other => panic!("应为 image，实际 {other:?}"),
    }
  }

  #[test]
  fn parses_nested_quote() {
    let block = parse(
      r#"{"kind":"quote","blocks":[{"kind":"paragraph","runs":[{"text":"引用"}]},{"kind":"quote","blocks":[]}]}"#,
    );
    match block {
      ExportBlock::Quote { blocks } => {
        assert_eq!(blocks.len(), 2);
        assert!(matches!(blocks[1], ExportBlock::Quote { .. }));
      }
      other => panic!("应为 quote，实际 {other:?}"),
    }
  }

  #[test]
  fn rejects_unknown_kind() {
    let err = serde_json::from_str::<ExportBlock>(r#"{"kind":"video","src":"x"}"#)
      .expect_err("未知 kind 必须报错");
    assert!(err.to_string().contains("unknown variant"), "错误信息应指出未知变体：{err}");
  }
}