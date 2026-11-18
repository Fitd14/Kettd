/**
 * PDF 导出的 HTML 组装（kb-export-pdf）。
 *
 * 打印件是「文档」，不是应用界面：这里只提供一套中性排版基线（白纸、黑字、标题层级、
 * 列表、引用、代码、表格、图片），**不读取**应用外观主题的纸色、墨色、圆角与斑马纹——
 * 主题是屏幕上的阅读皮肤，落到纸面反而碍事。
 *
 * 图片仍写相对引用 `assets/<sha>.<ext>`，由 Rust 侧读字节换成 data URI
 * （`src-tauri/src/pdf/inline.rs`），因为 `NavigateToString` 的文档没有资源根。
 */

export interface PdfHtmlInput {
  title: string
  bodyHtml: string
}

/** 正文为空时抛这个错误，调用方直接展示 message。 */
export class PdfHtmlError extends Error {}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

/**
 * 中性排版基线：只表达结构（层级、缩进、等宽、表格线），不表达品牌与主题。
 * 字号用 pt，避免随屏幕缩放变化；颜色用低饱和灰阶，黑白打印也清楚。
 */
const BASE_STYLE = `
  @page { size: A4; margin: 20mm 18mm; }
  * { box-sizing: border-box; }
  html, body {
    margin: 0; padding: 0;
    background: #FFFFFF;
    color: #1A1A1A;
    -webkit-print-color-adjust: exact;
    print-color-adjust: exact;
  }
  body {
    font-family: "Microsoft YaHei", "PingFang SC", "Noto Sans SC", "Source Han Sans SC", sans-serif;
    font-size: 11.5pt;
    line-height: 1.75;
    word-break: break-word;
  }
  h1, h2, h3, h4, h5, h6 {
    color: #000000;
    line-height: 1.35;
    margin: 1.2em 0 .5em;
    page-break-after: avoid;
  }
  h1 { font-size: 1.75em; font-weight: 700; }
  h2 { font-size: 1.4em; font-weight: 700; }
  h3 { font-size: 1.2em; font-weight: 700; }
  h4 { font-size: 1.08em; font-weight: 700; }
  h5 { font-size: 1em; font-weight: 700; }
  h6 { font-size: .95em; font-weight: 700; color: #444444; }
  p { margin: 0 0 .8em; }
  strong { font-weight: 700; }
  em { font-style: italic; }
  a { color: inherit; text-decoration: underline; }
  ul, ol { margin: 0 0 .8em 1.6em; padding: 0; }
  li { margin: .2em 0; }
  blockquote {
    margin: .9em 0;
    padding: .2em 0 .2em .9em;
    border-left: 3px solid #CCCCCC;
    color: #333333;
    page-break-inside: avoid;
  }
  pre {
    margin: .9em 0;
    padding: .7em .8em;
    background: #F5F5F5;
    border: 1px solid #E2E2E2;
    white-space: pre-wrap;
    page-break-inside: avoid;
  }
  pre code {
    font-family: Consolas, "Cascadia Code", "Courier New", monospace;
    font-size: 9.5pt;
    background: none;
    border: 0;
    padding: 0;
  }
  code {
    font-family: Consolas, "Cascadia Code", "Courier New", monospace;
    background: #F2F2F2;
    padding: .1em .3em;
    font-size: .92em;
  }
  table {
    width: 100%;
    margin: .9em 0;
    border-collapse: collapse;
    page-break-inside: avoid;
  }
  th, td {
    border: 1px solid #BBBBBB;
    padding: .4em .6em;
    text-align: left;
    vertical-align: top;
  }
  th { background: #F2F2F2; font-weight: 700; }
  img {
    max-width: 100%;
    height: auto;
    page-break-inside: avoid;
  }
  hr { border: 0; border-top: 1px solid #CCCCCC; margin: 1.2em 0; }
`

export function buildPdfHtml({ title, bodyHtml }: PdfHtmlInput): string {
  if (!bodyHtml || bodyHtml.trim() === '') {
    throw new PdfHtmlError('正文为空，没有可导出的内容')
  }
  return `<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<title>${escapeHtml(title)}</title>
<style>${BASE_STYLE}</style>
</head>
<body>${bodyHtml}</body>
</html>`
}