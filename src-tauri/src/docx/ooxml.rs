//! OOXML 生成（kb-export-word D4 / task 3.3）。
//!
//! 只写 `.docx` 必需的那几个部件：`[Content_Types].xml`、两处 rels、`document.xml`、
//! `styles.xml`、`numbering.xml`，图片另加到 `word/media/`。
//!
//! 边界：
//! - 排版走「直接格式 + 三个标题样式」：标题进导航窗格，其余靠 `w:rPr`，不引入主题字体；
//! - 引用用「左缩进 + 左边框」表达，嵌套一层加 360 twips；
//! - 表格单线边框、首行加粗加底纹；不做列宽自适应（Word 会按内容摊开）。

use crate::docx::media::MediaImage;

/// Word 正文里引用的媒体条目（rId 在写 `document.xml.rels` 时才拼接）
#[derive(Debug, Clone)]
pub struct MediaRef {
  pub file_name: String,
  pub ext: String,
}

/// 文档写出器：累积正文 XML 与关系表需要的 id
pub struct Writer {
  body: String,
  /// 超链接目标，按出现顺序分配 rId（从 10 起，前 10 个留给样式/编号/媒体以外的固定项）
  hyperlink_targets: Vec<String>,
  media: Vec<MediaRef>,
  drawing_id: u64,
}

impl Default for Writer {
  fn default() -> Self {
    Self::new()
  }
}

impl Writer {
  pub fn new() -> Self {
    Self {
      body: String::new(),
      hyperlink_targets: Vec::new(),
      media: Vec::new(),
      drawing_id: 0,
    }
  }

  pub fn media(&self) -> &[MediaRef] {
    &self.media
  }

  pub fn hyperlink_targets(&self) -> &[String] {
    &self.hyperlink_targets
  }

  pub fn finish(self) -> String {
    format!(
      concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#,
        r#" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#,
        r#" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing""#,
        r#" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#,
        r#" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
        r#"<w:body>{}{}</w:body></w:document>"#
      ),
      self.body,
      // 默认页面设置：A4（11906×16838 twips）+ 2.54cm 页边距；
      // 缺 sectPr 时 Word 会退回 Letter，中文文档通常期望 A4。
      concat!(
        r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/>"#,
        r#"<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="851" w:footer="992" w:gutter="0"/>"#,
        r#"</w:sectPr>"#
      )
    )
  }

  pub fn heading(&mut self, level: u8, runs: &[crate::docx::model::Run]) {
    let style = match level {
      1 => "Heading1",
      2 => "Heading2",
      _ => "Heading3",
    };
    self.push_paragraph(runs, Some(style), None, false);
  }

  pub fn paragraph(&mut self, runs: &[crate::docx::model::Run]) {
    self.push_paragraph(runs, None, None, false);
  }

  /// 列表：`ordered` 选编号定义（1 项目符号 / 2 数字），`depth` 映射 `w:ilvl`
  pub fn list(&mut self, items: &[crate::docx::model::ListItem], ordered: bool) {
    let num_id = if ordered { 2 } else { 1 };
    for item in items {
      let level = item.depth.min(8);
      let mut xml = String::from("<w:p><w:pPr>");
      xml.push_str(&format!(
        r#"<w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="{num_id}"/></w:numPr>"#
      ));
      xml.push_str(r#"<w:spacing w:after="60"/></w:pPr>"#);
      xml.push_str(&self.runs_xml(&item.runs));
      xml.push_str("</w:p>");
      self.body.push_str(&xml);
    }
  }

  /// 引用：左缩进 + 左边框；嵌套多一层再缩进一级
  pub fn quote(&mut self, blocks: &[crate::docx::model::ExportBlock], depth: usize) {
    let indent = 360 * (depth as i32 + 1);
    for block in blocks {
      match block {
        crate::docx::model::ExportBlock::Quote { blocks: inner } => {
          self.quote(inner, depth + 1);
        }
        crate::docx::model::ExportBlock::Paragraph { runs } => {
          self.push_paragraph(runs, None, Some(indent), true);
        }
        other => {
          // 引用里的其它块（标题/列表/代码）先按普通段落兜底，保持引用边框连续
          let text = block_plain_text(other);
          if !text.is_empty() {
            self.push_paragraph(&[crate::docx::model::Run::plain(text)], None, Some(indent), true);
          }
        }
      }
    }
  }

  pub fn code(&mut self, text: &str) {
    let mut lines = text.split('\n').peekable();
    while let Some(line) = lines.next() {
      let mut xml = String::from(
        r#"<w:p><w:pPr><w:shd w:val="clear" w:color="auto" w:fill="F2F2F2"/><w:spacing w:after="0"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas" w:eastAsia="Consolas"/><w:sz w:val="19"/></w:rPr>"#,
      );
      xml.push_str(r#"<w:t xml:space="preserve">"#);
      xml.push_str(&escape_xml(line));
      xml.push_str("</w:t></w:r></w:p>");
      self.body.push_str(&xml);
      // 行间不额外留空：每行一个段落，段后间距 0 已足够
      let _ = lines.peek();
    }
  }

  pub fn table(&mut self, header: bool, rows: &[Vec<String>]) {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 {
      return;
    }
    let mut xml = String::from(concat!(
      r#"<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="0" w:type="auto"/>"#,
      r#"<w:tblBorders>"#,
      r#"<w:top w:val="single" w:sz="4" w:space="0" w:color="D0D0D0"/>"#,
      r#"<w:left w:val="single" w:sz="4" w:space="0" w:color="D0D0D0"/>"#,
      r#"<w:bottom w:val="single" w:sz="4" w:space="0" w:color="D0D0D0"/>"#,
      r#"<w:right w:val="single" w:sz="4" w:space="0" w:color="D0D0D0"/>"#,
      r#"<w:insideH w:val="single" w:sz="4" w:space="0" w:color="D0D0D0"/>"#,
      r#"<w:insideV w:val="single" w:sz="4" w:space="0" w:color="D0D0D0"/>"#,
      r#"</w:tblBorders></w:tblPr><w:tblGrid>"#
    ));
    for _ in 0..columns {
      xml.push_str(r#"<w:gridCol w:w="2400"/>"#);
    }
    xml.push_str("</w:tblGrid>");
    for (index, row) in rows.iter().enumerate() {
      let is_header = header && index == 0;
      xml.push_str("<w:tr>");
      for cell in row {
        xml.push_str("<w:tc><w:tcPr>");
        if is_header {
          xml.push_str(r#"<w:shd w:val="clear" w:color="auto" w:fill="EFEFEF"/>"#);
        }
        xml.push_str(r#"<w:tcMar><w:top w:w="60" w:type="dxa"/><w:bottom w:w="60" w:type="dxa"/><w:left w:w="100" w:type="dxa"/><w:right w:w="100" w:type="dxa"/></w:tcMar></w:tcPr>"#);
        let run = crate::docx::model::Run {
          text: cell.clone(),
          bold: is_header,
          ..crate::docx::model::Run::default()
        };
        xml.push_str("<w:p><w:pPr><w:spacing w:after=\"0\"/></w:pPr>");
        xml.push_str(&self.runs_xml(&[run]));
        xml.push_str("</w:p></w:tc>");
      }
      xml.push_str("</w:tr>");
    }
    xml.push_str("</w:tbl>");
    self.body.push_str(&xml);
    // 表格后补一个空段：避免两张表紧贴成一张
    self.body.push_str(r#"<w:p><w:pPr><w:spacing w:after="0"/></w:pPr></w:p>"#);
  }

  /// 插一张图；同时登记媒体关系（`word/media/<file_name>`）
  pub fn image(&mut self, image: &MediaImage, alt: Option<&str>) {
    self.drawing_id += 1;
    let id = self.drawing_id;
    let file_name = format!("image{id}.{}", image.ext);
    let alt_text = alt.unwrap_or("").to_string();
    let media_ref = MediaRef {
      file_name: file_name.clone(),
      ext: image.ext.clone(),
    };
    let r_id = format!("rIdM{id}");
    self.media.push(media_ref);
    let xml = format!(
      concat!(
        r#"<w:p><w:pPr><w:spacing w:after="120"/></w:pPr><w:r><w:drawing>"#,
        r#"<wp:inline distT="0" distB="0" distL="0" distR="0">"#,
        r#"<wp:extent cx="{cx}" cy="{cy}"/>"#,
        r#"<wp:docPr id="{id}" name="图片 {id}" descr="{alt}"/>"#,
        r#"<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
        r#"<pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="{file}"/><pic:cNvPicPr/></pic:nvPicPr>"#,
        r#"<pic:blipFill><a:blip r:embed="{r_id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>"#,
        r#"<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
        r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic>"#,
        r#"</a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
      ),
      cx = image.width_emu,
      cy = image.height_emu,
      id = id,
      alt = escape_xml(&alt_text),
      file = escape_xml(&file_name),
      r_id = r_id,
    );
    self.body.push_str(&xml);
  }

  fn push_paragraph(
    &mut self,
    runs: &[crate::docx::model::Run],
    style: Option<&str>,
    indent: Option<i32>,
    quote_border: bool,
  ) {
    let mut xml = String::from("<w:p><w:pPr>");
    if let Some(style) = style {
      xml.push_str(&format!(r#"<w:pStyle w:val="{style}"/>"#));
    }
    if let Some(indent) = indent {
      xml.push_str(&format!(
        r#"<w:ind w:left="{indent}" w:right="240"/><w:spacing w:after="80"/>"#
      ));
    }
    if quote_border {
      xml.push_str(concat!(
        r#"<w:pBdr><w:left w:val="single" w:sz="18" w:space="8" w:color="D0D0D0"/></w:pBdr>"#
      ));
    }
    xml.push_str("</w:pPr>");
    xml.push_str(&self.runs_xml(runs));
    xml.push_str("</w:p>");
    self.body.push_str(&xml);
  }

  /// 行内 run → `w:r` 序列；超链接包一层 `w:hyperlink`
  fn runs_xml(&mut self, runs: &[crate::docx::model::Run]) -> String {
    let mut xml = String::new();
    let mut index = 0;
    while index < runs.len() {
      let run = &runs[index];
      if run.is_blank() {
        index += 1;
        continue;
      }
      // 同一条链接上的连续 run 合成一个 hyperlink 容器
      if let Some(target) = run.link.clone().filter(|value| !value.is_empty()) {
        let mut group: Vec<&crate::docx::model::Run> = vec![run];
        let mut next = index + 1;
        while next < runs.len() && runs[next].link.as_deref() == Some(target.as_str()) {
          group.push(&runs[next]);
          next += 1;
        }
        let r_id = self.hyperlink_id(&target);
        xml.push_str(&format!(r#"<w:hyperlink r:id="{r_id}">"#));
        for item in group {
          xml.push_str(&run_xml(item));
        }
        xml.push_str("</w:hyperlink>");
        index = next;
        continue;
      }
      xml.push_str(&run_xml(run));
      index += 1;
    }
    xml
  }

  fn hyperlink_id(&mut self, target: &str) -> String {
    if let Some(position) = self.hyperlink_targets.iter().position(|value| value == target) {
      return format!("rIdL{}", 100 + position);
    }
    self.hyperlink_targets.push(target.to_string());
    format!("rIdL{}", 100 + self.hyperlink_targets.len() - 1)
  }
}

fn run_xml(run: &crate::docx::model::Run) -> String {
  let mut props = String::new();
  if run.bold {
    props.push_str("<w:b/>");
  }
  if run.italic {
    props.push_str("<w:i/>");
  }
  let mut font = String::new();
  if run.code {
    font = r#"<w:rFonts w:ascii="Consolas" w:hAnsi="Consolas" w:eastAsia="Consolas"/>"#.to_string();
  }
  let styling = if props.is_empty() && font.is_empty() {
    String::new()
  } else {
    format!("<w:rPr>{font}{props}</w:rPr>")
  };
  let mut xml = String::from("<w:r>");
  xml.push_str(&styling);
  let mut pieces = run.text.split('\n').peekable();
  while let Some(piece) = pieces.next() {
    if !piece.is_empty() {
      xml.push_str(r#"<w:t xml:space="preserve">"#);
      xml.push_str(&escape_xml(piece));
      xml.push_str("</w:t>");
    }
    if pieces.peek().is_some() {
      xml.push_str("<w:br/>");
    }
  }
  xml.push_str("</w:r>");
  xml
}

/// 引用里出现的非段落块：抽成一行纯文本
fn block_plain_text(block: &crate::docx::model::ExportBlock) -> String {
  use crate::docx::model::ExportBlock;
  match block {
    ExportBlock::Heading { runs, .. } | ExportBlock::Paragraph { runs } => {
      runs.iter().map(|run| run.text.clone()).collect::<Vec<_>>().join("")
    }
    ExportBlock::Bullets { items } | ExportBlock::Numbers { items } => items
      .iter()
      .map(|item| item.runs.iter().map(|run| run.text.clone()).collect::<String>())
      .collect::<Vec<_>>()
      .join(" / "),
    ExportBlock::Code { text, .. } => text.clone(),
    ExportBlock::Table { rows, .. } => rows
      .iter()
      .map(|row| row.join(" | "))
      .collect::<Vec<_>>()
      .join(" / "),
    ExportBlock::Image { alt, .. } => alt.clone().unwrap_or_default(),
    ExportBlock::Quote { blocks } => blocks.iter().map(block_plain_text).collect::<Vec<_>>().join(" "),
  }
}

/// XML 文本转义（属性与正文共用）
pub fn escape_xml(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  for ch in text.chars() {
    match ch {
      '&' => out.push_str("&amp;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      '"' => out.push_str("&quot;"),
      '\'' => out.push_str("&apos;"),
      _ => out.push(ch),
    }
  }
  out
}

/// `[Content_Types].xml`：图片扩展名各要一条 Default
pub fn content_types(image_exts: &[String]) -> String {
  let mut defaults = String::new();
  let mut seen: Vec<&str> = Vec::new();
  for ext in image_exts {
    if seen.contains(&ext.as_str()) {
      continue;
    }
    seen.push(ext.as_str());
    let mime = match ext.as_str() {
      "jpg" | "jpeg" => "image/jpeg",
      "gif" => "image/gif",
      "bmp" => "image/bmp",
      "webp" => "image/webp",
      _ => "image/png",
    };
    defaults.push_str(&format!(
      r#"<Default Extension="{ext}" ContentType="{mime}"/>"#
    ));
  }
  format!(
    concat!(
      r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
      r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
      r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#,
      r#"<Default Extension="xml" ContentType="application/xml"/>"#,
      r#"{defaults}"#,
      r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>"#,
      r#"<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>"#,
      r#"<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>"#,
      r#"</Types>"#
    ),
    defaults = defaults
  )
}

/// 包级关系：document 是唯一入口
pub fn root_rels() -> String {
  concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>"#,
    r#"</Relationships>"#
  )
  .to_string()
}

/// `word/_rels/document.xml.rels`：样式、编号、图片、超链接
pub fn document_rels(media: &[MediaRef], hyperlinks: &[String]) -> String {
  let mut body = String::new();
  body.push_str(r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#);
  body.push_str(r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>"#);
  for (index, media_ref) in media.iter().enumerate() {
    body.push_str(&format!(
      r#"<Relationship Id="rIdM{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/{}"/>"#,
      index + 1,
      media_ref.file_name
    ));
  }
  for (index, target) in hyperlinks.iter().enumerate() {
    body.push_str(&format!(
      r#"<Relationship Id="rIdL{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="{}" TargetMode="External"/>"#,
      100 + index,
      escape_xml(target)
    ));
  }
  format!(
    concat!(
      r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
      r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
      r#"{body}</Relationships>"#
    ),
    body = body
  )
}

/// 三个标题样式 + 正文默认：字号跟 15px 阅读档位折算（11pt），标题 20/16/13pt
pub fn styles_xml() -> String {
  concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    r#"<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Segoe UI" w:hAnsi="Segoe UI" w:eastAsia="Microsoft YaHei"/><w:sz w:val="22"/></w:rPr></w:rPrDefault>"#,
    r#"<w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="320" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>"#,
    r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:before="360" w:after="180"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="40"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:before="280" w:after="140"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:before="220" w:after="120"/><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:sz w:val="26"/></w:rPr></w:style>"#,
    r#"</w:styles>"#
  )
  .to_string()
}

/// 两套编号定义：numId=1 项目符号、numId=2 数字，各三层
pub fn numbering_xml() -> String {
  let mut xml = String::from(concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#
  ));
  // 抽象定义 0：项目符号
  xml.push_str(r#"<w:abstractNum w:abstractNumId="0">"#);
  xml.push_str(r#"<w:multiLevelType w:val="hybridMultilevel"/>"#);
  for level in 0..3 {
    let char = ["•", "o", "▪"][level];
    xml.push_str(&format!(
      concat!(
        r#"<w:lvl w:ilvl="{level}"><w:start w:val="1"/><w:numFmt w:val="bullet"/>"#,
        r#"<w:lvlText w:val="{char}"/><w:lvlJc w:val="left"/>"#,
        r#"<w:pPr><w:ind w:left="{indent}" w:hanging="360"/></w:pPr></w:lvl>"#
      ),
      level = level,
      char = char,
      indent = 720 * (level + 1)
    ));
  }
  xml.push_str("</w:abstractNum>");

  // 抽象定义 1：阿拉伯数字
  xml.push_str(r#"<w:abstractNum w:abstractNumId="1">"#);
  xml.push_str(r#"<w:multiLevelType w:val="hybridMultilevel"/>"#);
  for level in 0..3 {
    xml.push_str(&format!(
      concat!(
        r#"<w:lvl w:ilvl="{level}"><w:start w:val="1"/><w:numFmt w:val="decimal"/>"#,
        r#"<w:lvlText w:val="%{num}."/><w:lvlJc w:val="left"/>"#,
        r#"<w:pPr><w:ind w:left="{indent}" w:hanging="360"/></w:pPr></w:lvl>"#
      ),
      level = level,
      num = level + 1,
      indent = 720 * (level + 1)
    ));
  }
  xml.push_str("</w:abstractNum>");

  xml.push_str(r#"<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#);
  xml.push_str(r#"<w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>"#);
  xml.push_str("</w:numbering>");
  xml
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::docx::model::{ExportBlock, ListItem, Run};

  #[test]
  fn escapes_xml_specials() {
    assert_eq!(
      escape_xml(r#"<a> & "b" 'c'"#),
      "&lt;a&gt; &amp; &quot;b&quot; &apos;c&apos;"
    );
  }

  #[test]
  fn heading_uses_style_and_preserves_spaces() {
    let mut writer = Writer::new();
    writer.heading(1, &[Run::plain("第一章")]);
    writer.heading(4, &[Run::plain("更深")]);
    let xml = writer.finish();
    assert!(xml.contains(r#"<w:pStyle w:val="Heading1"/>"#));
    // h4 起映射到 Heading3
    assert!(xml.contains(r#"<w:pStyle w:val="Heading3"/>"#));
    assert!(xml.contains(r#"<w:t xml:space="preserve">第一章</w:t>"#));
  }

  #[test]
  fn lists_emit_numPr_with_level_and_num_id() {
    let mut writer = Writer::new();
    writer.list(
      &[
        ListItem { depth: 0, runs: vec![Run::plain("a")] },
        ListItem { depth: 2, runs: vec![Run::plain("b")] },
      ],
      true,
    );
    let xml = writer.finish();
    assert!(xml.contains(r#"<w:ilvl w:val="2"/><w:numId w:val="2"/>"#));
    assert!(xml.contains(&format!(r#"<w:ilvl w:val="0"/><w:numId w:val="2"/>"#)));
  }

  #[test]
  fn quote_adds_left_border_and_indent() {
    let mut writer = Writer::new();
    writer.quote(
      &[ExportBlock::Paragraph {
        runs: vec![Run::plain("引用")],
      }],
      0,
    );
    let xml = writer.finish();
    assert!(xml.contains(r#"<w:ind w:left="360""#));
    assert!(xml.contains(r#"<w:pBdr><w:left"#));
  }

  #[test]
  fn code_block_uses_monospace_font_and_shading() {
    let mut writer = Writer::new();
    writer.code("let x = 1;\nlet y = 2;");
    let xml = writer.finish();
    assert!(xml.contains(r#"w:fill="F2F2F2""#));
    assert_eq!(xml.matches(r#"w:ascii="Consolas""#).count(), 2, "每行一个段落");
  }

  #[test]
  fn table_emits_borders_grid_and_header_shading() {
    let mut writer = Writer::new();
    writer.table(
      true,
      &[
        vec!["列一".into(), "列二".into()],
        vec!["值".into(), "值2".into()],
      ],
    );
    let xml = writer.finish();
    assert!(xml.contains("<w:tbl>"));
    assert!(xml.contains(r#"<w:tblGrid><w:gridCol w:w="2400"/><w:gridCol w:w="2400"/></w:tblGrid>"#));
    assert!(xml.contains(r#"w:fill="EFEFEF""#));
    assert_eq!(xml.matches("<w:tr>").count(), 2);
  }

  #[test]
  fn hyperlink_groups_consecutive_runs_and_registers_relationship() {
    let mut writer = Writer::new();
    writer.paragraph(&[
      Run { text: "看这里".into(), link: Some("https://example.com".into()), ..Run::default() },
      Run { text: "也行".into(), link: Some("https://example.com".into()), ..Run::default() },
      Run::plain("结束"),
    ]);
    let targets = writer.hyperlink_targets().to_vec();
    let xml = writer.finish();
    assert_eq!(targets, vec!["https://example.com".to_string()]);
    assert_eq!(xml.matches("<w:hyperlink ").count(), 1);
    assert!(xml.contains(r#"r:id="rIdL100""#));

    let rels = document_rels(&[], &targets);
    assert!(rels.contains(r#"Target="https://example.com" TargetMode="External""#));
  }

  #[test]
  fn image_registers_media_name_and_drawing_extent() {
    let mut writer = Writer::new();
    let image = MediaImage {
      bytes: vec![1, 2, 3],
      ext: "png".into(),
      width_emu: 952_500,
      height_emu: 476_250,
    };
    writer.image(&image, Some("示意图"));
    let media = writer.media().to_vec();
    let xml = writer.finish();
    assert!(xml.contains(r#"<wp:extent cx="952500" cy="476250"/>"#));
    assert!(xml.contains(r#"r:embed="rIdM1""#));
    assert_eq!(media[0].file_name, "image1.png");
    assert!(document_rels(&media, &[]).contains(r#"Target="media/image1.png""#));
  }

  #[test]
  fn content_types_lists_each_image_extension_once() {
    let xml = content_types(&["png".into(), "png".into(), "jpeg".into()]);
    assert_eq!(xml.matches(r#"Extension="png""#).count(), 1);
    assert!(xml.contains(r#"ContentType="image/png""#));
    assert!(xml.contains(r#"ContentType="image/jpeg""#));
  }

  #[test]
  fn numbering_declares_two_nums_and_three_levels() {
    let xml = numbering_xml();
    assert_eq!(xml.matches("<w:abstractNum ").count(), 2);
    assert_eq!(xml.matches("<w:num ").count(), 2);
    assert_eq!(xml.matches("<w:lvl ").count(), 6);
    assert!(xml.contains(r#"<w:lvlText w:val="•"/>"#));
    assert!(xml.contains(r#"<w:lvlText w:val="%1."/>"#));
  }
}