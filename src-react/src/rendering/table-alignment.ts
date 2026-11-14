/**
 * 表格 Markdown 序列化（对齐保留 + 单元格竖线转义）
 *
 * 背景（OpenSpec fix-known-defects-iteration #10 / tasks.md 6.2）：
 * - `@tiptap/extension-table` 3.31.3 的 TableCell/TableHeader 上游**已内置 `align` attr**
 *   （markdown-it 渲染出的 `<th>/<td style="text-align:...">` 会被解析成 left/center/right/null），
 *   但 tiptap-markdown 0.9.0 自带的 table serializer 分隔行恒为 `---`，且单元格里出现字面 `|`
 *   时不做转义（prosemirror-markdown 的 `esc()` 不转义 `|`），导致二次保存时：
 *   ① 对齐信息全部丢失；② 裸 `|` 被当成列分隔符，内容被截断（非幂等）。
 * - 本模块用一个自定义 Table 扩展覆盖 `storage.markdown.serialize`（tiptap-markdown 的
 *   `getMarkdownSpec()` 会用 `extension.storage.markdown` 覆盖内置同名序列化器，parse 缺省沿用内置）。
 *
 * 设计取舍：
 * - 对齐来源：每列取「该列第一个非 null 的 align」，分隔行输出 `:---` / `:---:` / `---:`；
 *   body 单元格即使与 header 不同也按列归一（二次保存稳定）。
 * - 列数：取所有行 childCount 的最大值，避免某行列数更多时内容被 markdown-it 截断（宁可多一列空单元格）。
 * - 表头：GFM 表格必须有表头。首行全是 tableHeader 时正常输出；否则先写一行全空的合成表头再写分隔行
 *   （无表头表格的归一化，内容不丢；本编辑器的 Markdown 解析路径下不会产生无表头表格，见 parse）。
 * - 单元格内容：GFM 单元格没有块语法，多块内容（段落/代码块等）用单个空格扁平化；
 *   渲染完成后对本次写入片段做「竖线补齐转义」（按前导反斜杠奇偶补齐，代码 span 内容也会覆盖）。
 *   注意：tiptap-markdown 的 `MarkdownSerializerState.render()` 会用 `inlines` 栈里记录的
 *   mark 起止下标做延迟 trim（把 `** x **` 里的空格挪到 mark 外）；直接改写 `state.out` 后这些
 *   下标会错位，导致粗体单元格里的 `|` 转义后整段被 trim 破坏（如 `**a | b**` → `**a \|** *`）。
 *   因此转义后必须同步平移待 trim 的 mark 下标，见 `shiftPendingInlineOffsets`。
 * - 单元格内的硬换行：GFM 单元格无法表达换行（反斜杠硬换行会把表格行断开），这里把
 *   hardBreak 归一化为一个空格（默认行为会写 `[hardBreak]` 占位文本，二次保存还会被转义成
 *   `\[hardBreak\]`，非幂等且污染内容）；仅当 hardBreak 后还有非 hardBreak 兄弟节点时才写空格，
 *   与默认 hard-break 序列化器的「末尾不写」规则保持一致。
 * - 性能：只在保存（800ms 防抖）时由 `getMarkdown()` 调用一次，不参与编辑期渲染。
 */

import { Table } from '@tiptap/extension-table'
import type { MarkdownNodeSpec } from 'tiptap-markdown'
import type { Node as ProseMirrorNode } from '@tiptap/pm/model'

/** 表格单元格对齐（与 @tiptap/extension-table 的 align attr 取值一致）。 */
type CellAlignment = 'left' | 'center' | 'right'

/** 从 tiptap-markdown 的 `MarkdownNodeSpec` 反推 `MarkdownSerializerState` 类型（避免直接依赖 prosemirror-markdown）。 */
type TableSerializerState = Parameters<MarkdownNodeSpec['serialize']>[0]

/** prosemirror-markdown 节点序列化器签名（state, node, parent, index）。 */
type MarkdownNodeSerializer = (
  state: TableSerializerState,
  node: ProseMirrorNode,
  parent: ProseMirrorNode,
  index: number,
) => void

/** tiptap-markdown 的 `MarkdownSerializerState` 运行时带、但公开 d.ts 未声明的内部字段。 */
type MutableTableState = {
  /** 当前输出缓冲（prosemirror-markdown 运行时字段）。 */
  out: string
  /** tiptap-markdown 扩展字段：是否处于表格内（影响 hardBreak 等节点的序列化分支）。 */
  inTable: boolean
  /** 节点序列化器映射（prosemirror-markdown 运行时字段）。 */
  nodes?: Record<string, MarkdownNodeSerializer>
  /** tiptap-markdown 扩展字段：待延迟 trim 的行内 mark 位置栈。 */
  inlines?: InlineTrimEntry[]
}

/** tiptap-markdown `inlines` 栈的元素（start/end 为 state.out 下标）。 */
type InlineTrimEntry = {
  start?: number
  end?: number
}

/** 三种对齐对应的 GFM 分隔行标记。 */
const ALIGN_MARKERS: Record<CellAlignment, string> = {
  left: ':---',
  center: ':---:',
  right: '---:',
}

function asMutableState(state: TableSerializerState): TableSerializerState & MutableTableState {
  return state as TableSerializerState & MutableTableState
}

function getCellAlignment(cell: ProseMirrorNode): CellAlignment | null {
  const align = cell.attrs.align
  return align === 'left' || align === 'center' || align === 'right' ? align : null
}

/** 首行是否全部为 tableHeader（GFM 表头）。 */
function isHeaderRow(row: ProseMirrorNode): boolean {
  if (row.childCount === 0) return false
  let header = true
  row.forEach((cell) => {
    if (cell.type.name !== 'tableHeader') header = false
  })
  return header
}

/** 每列对齐：取该列第一个非 null 的 align（表头优先于正文，因为遍历顺序从首行开始）。 */
function collectColumnAlignments(rows: ProseMirrorNode[], columnCount: number): (CellAlignment | null)[] {
  const alignments: (CellAlignment | null)[] = new Array<CellAlignment | null>(columnCount).fill(null)
  for (const row of rows) {
    row.forEach((cell, _offset, column) => {
      if (column >= columnCount || alignments[column] !== null) return
      const alignment = getCellAlignment(cell)
      if (alignment) alignments[column] = alignment
    })
  }
  return alignments
}

type PipeEscapeResult = {
  /** 补齐转义后的文本。 */
  text: string
  /** 在原文中插入 `\` 的位置（原文下标，升序）。 */
  insertions: number[]
}

/**
 * 给即将写入表格单元格的文本片段补齐竖线转义。
 *
 * 规则：遇到 `|` 时看前面连续反斜杠数量——偶数说明尚未被转义（prosemirror-markdown 的 esc()
 * 不处理 `|`），补一个 `\`；奇数说明已是 `\|`，保持原样。markdown-it 的 escapedSplit 只认
 * 「`|` 前一个字符是否为 `\`」，因此普通文本与 code span（code mark escape=false，绕过 esc）
 * 都能正确 round-trip。插入位置用于同步平移待 trim 的 mark 下标。
 */
function escapeTableCellPipes(text: string): PipeEscapeResult {
  let escaped = ''
  const insertions: number[] = []
  let precedingBackslashes = 0
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index]
    if (char === '\\') {
      precedingBackslashes += 1
      escaped += char
      continue
    }
    if (char === '|') {
      // 偶数个前导反斜杠 = 该竖线尚未被转义，补一个；奇数个则已是 \| 形式
      if (precedingBackslashes % 2 === 0) {
        escaped += '\\'
        insertions.push(index)
      }
      escaped += '|'
      precedingBackslashes = 0
      continue
    }
    precedingBackslashes = 0
    escaped += char
  }
  return { text: escaped, insertions }
}

/** 把转义前片段内的偏移映射为转义后片段内的偏移（按插入点累计）。 */
function mapEscapedOffset(offset: number, insertions: number[]): number {
  let shift = 0
  for (const insertion of insertions) {
    if (insertion >= offset) break
    shift += 1
  }
  return offset + shift
}

/**
 * 修正 tiptap-markdown `inlines` 栈中待 trim 的 mark 下标。
 *
 * `MarkdownSerializerState.render()` 会把 mark 的打开/关闭位置记在 `inlines` 栈里，等下一次
 * render() 时才真正 trim；如果在记录之后改写了 `state.out`（本模块补 `|` 转义），下标就会错位。
 * 这里把落在本次单元格片段内的 start/end 按插入点平移，保持 trim 行为正确。
 */
function shiftPendingInlineOffsets(state: TableSerializerState, chunkStart: number, insertions: number[]): void {
  if (insertions.length === 0) return
  const entries = asMutableState(state).inlines
  if (!entries) return
  for (const entry of entries) {
    if (typeof entry.start === 'number' && entry.start >= chunkStart) {
      entry.start = chunkStart + mapEscapedOffset(entry.start - chunkStart, insertions)
    }
    if (typeof entry.end === 'number' && entry.end >= chunkStart) {
      entry.end = chunkStart + mapEscapedOffset(entry.end - chunkStart, insertions)
    }
  }
}

/**
 * 单元格内的 hardBreak 序列化：写一个空格；如果是行内最后一个节点则什么都不写
 * （与 tiptap-markdown 默认 hard-break 序列化器一致，避免单元格末尾多出空格）。
 */
function writeCellHardBreak(
  state: TableSerializerState,
  _node: ProseMirrorNode,
  parent: ProseMirrorNode,
  index: number,
): void {
  for (let next = index + 1; next < parent.childCount; next += 1) {
    if (parent.child(next).type.name !== 'hardBreak') {
      state.write(' ')
      return
    }
  }
}

/** 渲染单元格内容到 state.out，并对本次写入片段补竖线转义、修正延迟 trim 下标。 */
function renderCellContent(state: TableSerializerState, cell: ProseMirrorNode): void {
  const mutable = asMutableState(state)
  const chunkStart = mutable.out.length
  // 临时替换 hardBreak 序列化器（只影响本单元格渲染窗口，finally 还原）
  const originalNodes = mutable.nodes
  if (originalNodes && originalNodes.hardBreak) {
    mutable.nodes = { ...originalNodes, hardBreak: writeCellHardBreak }
  }
  try {
    let firstBlock = true
    cell.forEach((block) => {
      if (!firstBlock) state.text(' ', false)
      firstBlock = false
      if (block.isTextblock) {
        // 复用序列化器的行内渲染（marks/link/code 等与正文一致）
        state.renderInline(block)
      } else {
        // GFM 单元格不支持块级语法：扁平化为纯文本（内容不丢）
        state.text(block.textContent, false)
      }
    })
  } finally {
    if (originalNodes) mutable.nodes = originalNodes
  }

  const rendered = mutable.out.slice(chunkStart)
  if (!rendered) return
  const { text: escaped, insertions } = escapeTableCellPipes(rendered)
  if (insertions.length === 0) return
  mutable.out = mutable.out.slice(0, chunkStart) + escaped
  shiftPendingInlineOffsets(state, chunkStart, insertions)
}

/** 写一行表格（row 为 null 时写全空单元格，用于无表头表格的合成表头）。 */
function writeTableRow(state: TableSerializerState, row: ProseMirrorNode | null, columnCount: number): void {
  state.write('| ')
  for (let column = 0; column < columnCount; column += 1) {
    if (column > 0) state.write(' | ')
    if (!row || column >= row.childCount) continue
    const cell = row.child(column)
    if (cell.childCount > 0) renderCellContent(state, cell)
  }
  state.write(' |')
  state.ensureNewLine()
}

/** 写分隔行（GFM 表格对齐标记载体）。 */
function writeDelimiterRow(
  state: TableSerializerState,
  alignments: (CellAlignment | null)[],
  columnCount: number,
): void {
  state.write('| ')
  for (let column = 0; column < columnCount; column += 1) {
    if (column > 0) state.write(' | ')
    const alignment = alignments[column]
    state.write(alignment ? ALIGN_MARKERS[alignment] : '---')
  }
  state.write(' |')
  state.ensureNewLine()
}

/** tiptap-markdown 的 table 序列化器（签名见 MarkdownNodeSpec.serialize）。 */
function serializeTable(state: TableSerializerState, node: ProseMirrorNode): void {
  const mutable = asMutableState(state)
  const rows: ProseMirrorNode[] = []
  node.forEach((row) => {
    rows.push(row)
  })
  if (rows.length === 0) {
    state.closeBlock(node)
    return
  }
  let columnCount = 0
  for (const row of rows) columnCount = Math.max(columnCount, row.childCount)
  if (columnCount === 0) {
    state.closeBlock(node)
    return
  }

  const alignments = collectColumnAlignments(rows, columnCount)
  const hasHeader = isHeaderRow(rows[0])
  const dataRows = hasHeader ? rows.slice(1) : rows
  // inTable=true 与内置 table serializer 保持一致（本模块对 cell 内 hardBreak 另有覆盖）
  const previousInTable = mutable.inTable
  mutable.inTable = true
  try {
    if (hasHeader) {
      writeTableRow(state, rows[0], columnCount)
    } else {
      // GFM 表格必须有表头：合成一行全空单元格，原内容全部作为数据行保留
      writeTableRow(state, null, columnCount)
    }
    writeDelimiterRow(state, alignments, columnCount)
    for (const row of dataRows) writeTableRow(state, row, columnCount)
  } finally {
    mutable.inTable = previousInTable
  }
  state.closeBlock(node)
}

const tableMarkdownSpec: MarkdownNodeSpec = {
  serialize(state, node) {
    serializeTable(state, node)
  },
}

/**
 * 覆盖 tiptap-markdown 表格序列化器的 Table 扩展。
 * 用法与原生 Table 完全一致：`TableWithAlignment.configure({ resizable: false })`；
 * 不要同时注册原生 `Table`（同名节点会重复注册报错）。
 */
export const TableWithAlignment = Table.extend({
  addStorage() {
    return {
      markdown: tableMarkdownSpec,
    }
  },
})