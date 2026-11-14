/**
 * 条目内查找（缺陷 #6：Ctrl+F）—— ProseMirror 插件层。
 *
 * 设计：
 * - 匹配结果与当前项存放在 Plugin state 里（term / matches / current），
 *   文档编辑后在 state.apply 中按 term 重算，位置天然跟着文档走，
 *   不会出现「React 侧缓存旧 position → Decoration 越界」的问题。
 * - 高亮由 props.decorations 从 plugin state 派生：所有命中 `.kb-find-hit`，
 *   当前项额外 `.kb-find-hit-current`；只加 Decoration，不修改文档内容。
 * - React 侧只负责查找栏 UI 与计数：dispatch meta（set-term / goto / clear）后
 *   同步读取 plugin state，不存在「每事务整组件重渲染」的性能回归。
 */
import { Extension } from '@tiptap/core'
import type { Editor } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import type { EditorState, Transaction } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'
import type { Node as PMNode } from '@tiptap/pm/model'

/** 一处匹配在文档中的位置（ProseMirror position，UTF-16 偏移） */
export interface FindMatch {
  from: number
  to: number
}

/** 查找插件 state：关键词、全部命中、当前命中下标（0 基） */
export interface FindState {
  term: string
  matches: FindMatch[]
  current: number
}

type FindMeta =
  | { type: 'set-term'; term: string }
  | { type: 'goto'; index: number }
  | { type: 'clear' }

export const FIND_PLUGIN_KEY = new PluginKey<FindState>('kbFind')

const EMPTY_FIND_STATE: FindState = { term: '', matches: [], current: 0 }

/** 行内非文本节点（hardBreak / image 等）的匹配隔断符：匹配不得跨过它 */
const SENTINEL = '\u0000'

/* ─── 匹配算法 ───────────────────────────────────────────────────────── */

function escapeRegExp(input: string): string {
  return input.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

interface TextSegment {
  /** 该 text 节点在「块内拼接文本」中的起始下标 */
  hayStart: number
  /** 该 text 节点在文档中的起始位置（ProseMirror position） */
  docFrom: number
  text: string
}

/** 把块内拼接文本的下标映射回文档位置；落在哨兵/段外时返回 -1 */
function docPosAt(segments: TextSegment[], hayIndex: number): number {
  for (const seg of segments) {
    if (hayIndex >= seg.hayStart && hayIndex < seg.hayStart + seg.text.length) {
      return seg.docFrom + (hayIndex - seg.hayStart)
    }
  }
  return -1
}

/**
 * 在文档里查找 term（大小写不敏感）。
 * 以 textblock（段落/标题/代码块/单元格内段落…）为单位拼接其 inline 文本，
 * 因此匹配可以跨同一块内被 mark 拆开的多个 text 节点，但不跨块；
 * 块内遇到 hardBreak 等非文本 inline 节点时用哨兵隔断。
 */
export function findMatches(doc: PMNode, term: string): FindMatch[] {
  if (!term) return []
  // 用正则的 i 标志做大小写不敏感匹配（保持下标与原文一致；不用 toLowerCase 以免个别
  // Unicode 字符转换后长度变化导致位置错位）。term 已转义，不存在正则注入。
  const pattern = new RegExp(escapeRegExp(term), 'giu')
  const found: FindMatch[] = []

  doc.descendants((node, pos) => {
    if (!node.isTextblock) return true

    const segments: TextSegment[] = []
    let haystack = ''
    node.forEach((child, offset) => {
      const docFrom = pos + 1 + offset
      if (child.isText && child.text) {
        segments.push({ hayStart: haystack.length, docFrom, text: child.text })
        haystack += child.text
      } else {
        haystack += SENTINEL
      }
    })
    if (!haystack) return false

    pattern.lastIndex = 0
    let hit: RegExpExecArray | null = pattern.exec(haystack)
    while (hit) {
      const start = hit.index
      const end = start + hit[0].length
      if (hit[0].length > 0 && !haystack.slice(start, end).includes(SENTINEL)) {
        const from = docPosAt(segments, start)
        const to = docPosAt(segments, end - 1)
        if (from >= 0 && to >= from) found.push({ from, to: to + 1 })
      }
      hit = pattern.exec(haystack)
    }
    // 文本块内部由上面的 inline 拼接覆盖，无需继续下钻
    return false
  })

  return found
}

/* ─── 插件 state / decorations ───────────────────────────────────────── */

function computeFindState(doc: PMNode, term: string, current: number): FindState {
  const matches = findMatches(doc, term)
  if (!matches.length) return { term, matches, current: 0 }
  return { term, matches, current: ((current % matches.length) + matches.length) % matches.length }
}

function applyFindTransaction(tr: Transaction, prev: FindState): FindState {
  const meta = tr.getMeta(FIND_PLUGIN_KEY) as FindMeta | undefined
  if (meta) {
    if (meta.type === 'clear') return prev.term ? EMPTY_FIND_STATE : prev
    if (meta.type === 'set-term') {
      if (meta.term === prev.term) return prev
      return computeFindState(tr.doc, meta.term, 0)
    }
    // goto：循环回绕在 React 侧先算好，这里只做防御性归一
    if (meta.type === 'goto') {
      if (!prev.matches.length) return prev
      return { ...prev, current: ((meta.index % prev.matches.length) + prev.matches.length) % prev.matches.length }
    }
  }
  // 文档被编辑：按当前 term 重算，位置随新文档更新（查找栏关闭时 term 为空，零开销）
  if (tr.docChanged && prev.term) {
    return computeFindState(tr.doc, prev.term, prev.current)
  }
  return prev
}

function buildFindDecorations(state: EditorState): DecorationSet | null {
  const find = FIND_PLUGIN_KEY.getState(state)
  if (!find || find.matches.length === 0) return null
  const decorations = find.matches.map((match, index) =>
    Decoration.inline(match.from, match.to, {
      class: index === find.current ? 'kb-find-hit kb-find-hit-current' : 'kb-find-hit',
    }),
  )
  return DecorationSet.create(state.doc, decorations)
}

/** 创建查找插件（导出供 jsdom 功能测试直接构造 EditorState，无需完整 Editor） */
export function createFindPlugin(): Plugin<FindState> {
  return new Plugin<FindState>({
    key: FIND_PLUGIN_KEY,
    state: {
      init: () => EMPTY_FIND_STATE,
      apply: applyFindTransaction,
    },
    props: {
      decorations: (state) => buildFindDecorations(state),
    },
  })
}

/** TipTap 扩展：只挂查找插件（无 options、无 React 依赖，模块级单例） */
export const FindExtension = Extension.create({
  name: 'kbFind',
  addProseMirrorPlugins() {
    return [createFindPlugin()]
  },
})

/* ─── React 侧使用的命令 / 读取 / 滚动辅助 ───────────────────────────── */

/**
 * 向插件派发一次 meta 事务。编辑器销毁 / 视图尚未挂载的竞态下静默忽略：
 * 此时插件 state 会随 editor 一起消失，不需要额外清理。
 */
function dispatchFind(editor: Editor, meta: FindMeta): void {
  if (editor.isDestroyed) return
  try {
    editor.view.dispatch(editor.state.tr.setMeta(FIND_PLUGIN_KEY, meta))
  } catch {
    // 卸载竞态：忽略
  }
}

export function setFindTerm(editor: Editor, term: string): void {
  dispatchFind(editor, { type: 'set-term', term })
}

export function goToFindMatch(editor: Editor, index: number): void {
  dispatchFind(editor, { type: 'goto', index })
}

export function clearFind(editor: Editor): void {
  dispatchFind(editor, { type: 'clear' })
}

/** 读取插件 state（编辑器未初始化 / 无插件时返回空状态，调用方无需判空） */
export function getFindState(editor: Editor): FindState {
  if (editor.isDestroyed) return EMPTY_FIND_STATE
  return FIND_PLUGIN_KEY.getState(editor.state) ?? EMPTY_FIND_STATE
}

/** 下一帧把当前命中滚到编辑器可视区中部（等 Decoration 渲染成 DOM 后再滚） */
export function scrollFindCurrentIntoView(editor: Editor): void {
  if (editor.isDestroyed) return
  requestAnimationFrame(() => {
    if (editor.isDestroyed) return
    const current = editor.view.dom.querySelector<HTMLElement>('.kb-find-hit-current')
    current?.scrollIntoView({ block: 'center', inline: 'nearest' })
  })
}