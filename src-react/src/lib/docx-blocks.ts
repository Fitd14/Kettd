/**
 * Markdown → 导出块模型（design D2）。
 *
 * 一份契约两处消费：Rust 端据此写 OOXML（本期 Word），将来的 PDF 也用同一份。
 *
 * 为什么这里自己建 MarkdownIt 而不是 import `rendering/markdown.ts`：
 * 那个模块挂了 HTML renderer 覆盖（highlight / convertFileSrc），在纯 Node 测试环境里
 * 会连带加载浏览器侧依赖；而块模型只吃 token 流，renderer 覆盖对它没有任何影响。
 * 配置必须与 `rendering/markdown.ts` 完全一致，`src-react/test/docx-blocks.test.mjs`
 * 里有一条漂移守卫，逐字比对两边的 html/linkify/breaks。
 */
import MarkdownIt from 'markdown-it'

export const DOCX_MARKDOWN_OPTIONS = { html: false, linkify: true, breaks: true }

const md = new MarkdownIt(DOCX_MARKDOWN_OPTIONS)

export interface InlineRun {
  text: string
  bold?: boolean
  italic?: boolean
  code?: boolean
  link?: string
}

export type ExportBlock =
  | { kind: 'heading'; level: 1 | 2 | 3; runs: InlineRun[] }
  | { kind: 'paragraph'; runs: InlineRun[] }
  | { kind: 'bullets'; items: ExportListItem[] }
  | { kind: 'numbers'; items: ExportListItem[] }
  | { kind: 'quote'; blocks: ExportBlock[] }
  | { kind: 'code'; lang?: string; text: string }
  | { kind: 'table'; header: boolean; rows: string[][] }
  | { kind: 'image'; asset: string; alt?: string }

export interface ExportListItem {
  depth: number
  runs: InlineRun[]
}

interface ListContext {
  ordered: boolean
  items: ExportListItem[]
}

/** 给块数组压入一个块；相邻同款纯文本段落不合并——Word 分段更稳妥 */
function pushBlock(target: ExportBlock[], block: ExportBlock): void {
  target.push(block)
}

/** 连续同样式 run 合并，避免一个词拆成三段 run */
function pushRun(runs: InlineRun[], run: InlineRun): void {
  if (!run.text) return
  const last = runs[runs.length - 1]
  if (
    last
    && last.bold === run.bold
    && last.italic === run.italic
    && last.code === run.code
    && last.link === run.link
  ) {
    last.text += run.text
    return
  }
  runs.push(run)
}

function plainText(runs: InlineRun[]): string {
  return runs.map((r) => r.text).join('')
}

interface InlineState {
  bold: number
  italic: number
  link: string | undefined
}

/** 行内 token 流 → run 数组；图片单独由调用方处理 */
function inlineRuns(children: readonly unknown[], state?: InlineState): InlineRun[] {
  const runs: InlineRun[] = []
  const stack: InlineState = {
    bold: state?.bold ?? 0,
    italic: state?.italic ?? 0,
    link: state?.link,
  }
  for (const raw of children) {
    const token = raw as {
      type: string
      content?: string
      markup?: string
      attrGet?: (name: string) => string | null
    }
    switch (token.type) {
      case 'text':
        pushRun(runs, {
          text: token.content ?? '',
          bold: stack.bold > 0 || undefined,
          italic: stack.italic > 0 || undefined,
          link: stack.link,
        })
        break
      case 'strong_open':
        stack.bold += 1
        break
      case 'strong_close':
        stack.bold = Math.max(0, stack.bold - 1)
        break
      case 'em_open':
        stack.italic += 1
        break
      case 'em_close':
        stack.italic = Math.max(0, stack.italic - 1)
        break
      case 'code_inline':
        pushRun(runs, {
          text: token.content ?? '',
          code: true,
          bold: stack.bold > 0 || undefined,
          italic: stack.italic > 0 || undefined,
        })
        break
      case 'link_open': {
        const href = token.attrGet?.('href') ?? undefined
        stack.link = href || undefined
        break
      }
      case 'link_close':
        stack.link = undefined
        break
      case 'softbreak':
      case 'hardbreak':
        pushRun(runs, { text: '\n', link: stack.link })
        break
      case 'image': {
        const alt = (token.content ?? '').trim()
        pushRun(runs, { text: alt ? `[图片：${alt}]` : '[图片]', link: stack.link })
        break
      }
      default:
        // s_open/s_close（删除线）、html_inline 等：保留文字，不表达样式
        if (typeof token.content === 'string' && token.content) {
          pushRun(runs, { text: token.content, link: stack.link })
        }
    }
  }
  return runs
}

function imageTokens(children: readonly unknown[]): Array<{ asset: string; alt?: string }> {
  const out: Array<{ asset: string; alt?: string }> = []
  for (const raw of children) {
    const token = raw as {
      type: string
      content?: string
      attrGet?: (name: string) => string | null
    }
    if (token.type !== 'image') continue
    const src = token.attrGet?.('src') ?? ''
    if (!src) continue
    const alt = (token.content ?? '').trim()
    out.push({ asset: src, alt: alt || undefined })
  }
  return out
}

/**
 * Markdown → 块模型。
 * 约定：h1–h3 映射标题 1–3；h4–h6 降级为加粗段落（Word 标题样式只到 3 级，再深看不出层级）。
 */
export function toDocxBlocks(markdown: string): ExportBlock[] {
  if (!markdown || !markdown.trim()) return []
  const tokens = md.parse(markdown, {}) as unknown as Array<{
    type: string
    tag?: string
    content: string
    info?: string
    level: number
    children?: unknown[]
    attrs?: unknown
  }>

  const blocks: ExportBlock[] = []
  const quoteStack: ExportBlock[][] = []
  const listStack: ListContext[] = []
  let listDepth = 0
  let currentItem: ExportListItem | null = null
  let table: { header: boolean; rows: string[][] } | null = null
  let tableRow: string[] | null = null
  let tableHeaderNext = false

  const target = (): ExportBlock[] => quoteStack[quoteStack.length - 1] ?? blocks

  for (let i = 0; i < tokens.length; i += 1) {
    const token = tokens[i]

    switch (token.type) {
      case 'heading_open': {
        const level = Number.parseInt((token.tag ?? 'h1').slice(1), 10)
        const inline = tokens[i + 1]
        const runs = inlineRuns(inline?.children ?? [])
        if (level <= 3) {
          pushBlock(target(), { kind: 'heading', level: level as 1 | 2 | 3, runs })
        } else {
          const bold = runs.map((r) => ({ ...r, bold: true }))
          pushBlock(target(), { kind: 'paragraph', runs: bold })
        }
        break
      }
      case 'paragraph_open': {
        const inline = tokens[i + 1]
        const children = inline?.children ?? []
        const images = imageTokens(children)
        const nonImage = children.filter((child) => {
          const t = (child as { type: string }).type
          return t !== 'image' && t !== 'softbreak'
        })
        if (images.length > 0 && nonImage.length === 0) {
          for (const image of images) pushBlock(target(), { kind: 'image', ...image })
        } else {
          const runs = inlineRuns(children)
          if (currentItem) {
            if (currentItem.runs.length > 0) pushRun(currentItem.runs, { text: '\n' })
            for (const run of runs) pushRun(currentItem.runs, run)
          } else if (runs.length > 0) {
            pushBlock(target(), { kind: 'paragraph', runs })
          }
        }
        break
      }
      case 'bullet_list_open':
      case 'ordered_list_open': {
        // 只用一份「最外层」上下文：嵌套列表只抬深度，不另开上下文，
        // 否则内层关闭时它的 items 会被一起丢掉（嵌套项整段消失）。
        if (listStack.length === 0) {
          listStack.push({ ordered: token.type === 'ordered_list_open', items: [] })
        }
        listDepth += 1
        break
      }
      case 'bullet_list_close':
      case 'ordered_list_close': {
        listDepth = Math.max(0, listDepth - 1)
        currentItem = null
        if (listDepth === 0) {
          const ctx = listStack.pop()
          if (ctx && ctx.items.length > 0) {
            pushBlock(target(), { kind: ctx.ordered ? 'numbers' : 'bullets', items: ctx.items })
          }
        }
        break
      }
      case 'list_item_open': {
        const ctx = listStack[0]
        if (ctx) {
          currentItem = { depth: Math.max(0, listDepth - 1), runs: [] }
          ctx.items.push(currentItem)
        }
        break
      }
      case 'list_item_close':
        currentItem = null
        break
      case 'blockquote_open': {
        // 用 token.level 判断真实嵌套深度：markdown-it 会把 `> >` 与 `> > >`
        // 解析成同一父引用下的兄弟，靠栈长度判断会错位。三层起拍平进当前引用
        // （Word 缩进再深就看不出层级）。
        if (token.level >= 2) {
          quoteStack.push(quoteStack[quoteStack.length - 1])
        } else {
          const quote: ExportBlock[] = []
          pushBlock(target(), { kind: 'quote', blocks: quote })
          quoteStack.push(quote)
        }
        break
      }
      case 'blockquote_close':
        quoteStack.pop()
        break
      case 'fence':
      case 'code_block': {
        const lang = (token.info ?? '').trim().split(/\s+/)[0] || undefined
        pushBlock(target(), { kind: 'code', lang, text: token.content.replace(/\n$/, '') })
        break
      }
      case 'table_open':
        table = { header: true, rows: [] }
        tableHeaderNext = true
        break
      case 'thead_open':
        tableHeaderNext = true
        break
      case 'tr_open':
        tableRow = []
        break
      case 'th_open':
        tableHeaderNext = tableHeaderNext || table?.rows.length === 0
        break
      case 'td_open':
        break
      case 'inline': {
        if (tableRow) {
          const runs = inlineRuns(token.children ?? [])
          tableRow.push(plainText(runs))
        }
        break
      }
      case 'tr_close':
        if (tableRow && table) {
          table.rows.push(tableRow)
          tableRow = null
        }
        break
      case 'table_close':
        if (table) {
          pushBlock(target(), { kind: 'table', header: table.header, rows: table.rows })
        }
        table = null
        tableHeaderNext = false
        break
      default:
        break
    }
  }

  return blocks
}

// eslint-disable-next-line no-control-regex -- Windows 文件名不允许控制字符，这里必须显式匹配
const FORBIDDEN_NAME = /[<>:"/\\|?*\u0000-\u001f]/g

/** 导出的默认文件名：去掉 Windows 不认的字符、折叠空白、截断 80 字、空则占位 */
export function sanitizeDocxName(title: string): string {
  const cleaned = String(title ?? '')
    .replace(FORBIDDEN_NAME, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/[. ]+$/g, '')
  if (!cleaned) return '未命名文档'
  return cleaned.slice(0, 80)
}