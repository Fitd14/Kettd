/**
 * `[@文本](url)` 链接语法（kb-writing-experience D7/D8）。
 *
 * 为什么是 `[@…]` 而不是 `[…]`：正文里 `[1]`、`[注]` 这类非链接方括号太常见，
 * 直接吃 `[文本](url)` 会在用户写数学式或引用时误转换；多一个 `@` 只多一次按键，
 * 却把误判面压到接近零。
 *
 * 三条路径共用同一套匹配与安全校验：
 * 1. 键入：敲完 `)` 由 InputRule 当场转换（光标处不再留 `@`）；
 * 2. 粘贴：剪贴板整段就是一条 `[@文本](url)` 时直接吃掉，不再触发上面的规则；
 * 3. 落盘前：`normalizeLinkSyntax` 扫全文，把历史文本里的字面量补成链接，
 *    保证 Markdown 里不会留下 `[@`（代码块与行内代码内一律跳过）。
 *
 * 反方向永远成立：序列化出去的是标准 `[文本](url)`，因为 `@` 只存在于输入形态，
 * 转换后文本节点里只剩「文本」。
 */
import { Extension, InputRule, type Editor } from '@tiptap/core'
import { Plugin } from '@tiptap/pm/state'
import { TextSelection } from '@tiptap/pm/state'
import { isSafeLink } from '@/lib/link-url'

/** 单条语法的正文字符串（用于拼三处正则，避免三份口径各写一遍） */
const PATTERN = String.raw`\[@([^\]\n]{1,200})\]\((https?:\/\/[^\s()<>]+|mailto:[^\s()<>]+)\)`

const INLINE_PATTERN = new RegExp(`${PATTERN}$`)
const FULL_PATTERN = new RegExp(`^${PATTERN}$`)

/** 模板里的默认链接文字（无选区时用它当占位） */
const TEMPLATE_LABEL = '文本'

/**
 * 插入链接模板。
 * - 无选区：插入 `[@文本](https://)` 并选中「文本」，直接键入即替换；
 * - 有选区：把选中的文字当作链接文字（剥掉会破坏语法的 `]` 与换行），
 *   并选中 `https://` 占位，键入地址即可。
 */
export function insertLinkTemplate(editor: Editor): void {
  const { from, to, empty } = editor.state.selection
  const picked = empty
    ? ''
    : editor.state.doc
        .textBetween(from, to, ' ')
        .replace(/[\]\n]/g, ' ')
        .slice(0, 200)
        .trim()
  const label = picked || TEMPLATE_LABEL
  const text = `[@${label}](https://)`

  const tr = editor.state.tr
  if (!empty) tr.delete(from, to)
  tr.insertText(text, from)
  // 选区坐标：`[`/@/label/`]`/`(` 之后是 `https://`，共 8 个字符
  const urlStart = from + 4 + label.length
  const selection = empty
    ? TextSelection.create(tr.doc, from + 2, from + 4)
    : TextSelection.create(tr.doc, urlStart, urlStart + 8)
  tr.setSelection(selection)
  editor.view.dispatch(tr)
  editor.view.focus()
}

/**
 * 把文档里**尚未变成链接**的 `[@文本](url)` 字面量转成链接（落盘前调用）。
 * 返回 true 表示做了改动（调用方据此决定要不要重新序列化）。
 * 跳过：代码块内部、行内代码、已经是链接的文本。
 */
export function normalizeLinkSyntax(editor: Editor): boolean {
  const linkType = editor.state.schema.marks.link
  if (!linkType) return false

  const edits: { from: number; to: number; text: string; href: string }[] = []
  editor.state.doc.descendants((node, pos) => {
    if (node.type.name === 'codeBlock') return false // 代码块整棵子树跳过
    if (!node.isText || !node.text) return undefined
    if (node.marks.some((m) => m.type.name === 'link' || m.type.name === 'code')) return undefined
    const re = new RegExp(PATTERN, 'g')
    let hit: RegExpExecArray | null
    while ((hit = re.exec(node.text))) {
      if (!isSafeLink(hit[2])) continue
      edits.push({ from: pos + hit.index, to: pos + hit.index + hit[0].length, text: hit[1], href: hit[2] })
    }
    return undefined
  })
  if (!edits.length) return false

  const tr = editor.state.tr
  // 从后往前替换：后面先改不会让前面的位置失效
  for (let i = edits.length - 1; i >= 0; i -= 1) {
    const edit = edits[i]
    tr.replaceWith(edit.from, edit.to, editor.state.schema.text(edit.text, [linkType.create({ href: edit.href })]))
  }
  editor.view.dispatch(tr)
  return true
}

export const LinkSyntax = Extension.create({
  name: 'linkSyntax',

  addInputRules() {
    const linkType = this.editor.schema.marks.link
    if (!linkType) return []

    return [
      new InputRule({
        find: INLINE_PATTERN,
        handler: ({ chain, range, match }) => {
          const text = (match[1] ?? '').trim()
          const href = (match[2] ?? '').trim()
          // 协议不合法就保持字面量：宁可让用户看见原文，也不生成一个点不动的链接
          if (!text || !isSafeLink(href)) return
          chain()
            .deleteRange(range)
            .insertContent([{ type: 'text', text, marks: [{ type: 'link', attrs: { href } }] }])
            .run()
        },
      }),
    ]
  },

  addProseMirrorPlugins() {
    return [
      new Plugin({
        props: {
          /**
           * 粘贴整条语法时直接转成链接（保留 InputRule 的语义，但粘贴不触发 input rule）。
           * 只认「剪贴板里就只有这一条」的情况；混在长文里的留给 normalizeLinkSyntax。
           */
          handlePaste: (view, event) => {
            const raw = event.clipboardData?.getData('text/plain') ?? ''
            const hit = raw.trim().match(FULL_PATTERN)
            if (!hit || !isSafeLink(hit[2])) return false
            const linkType = view.state.schema.marks.link
            if (!linkType) return false
            const node = view.state.schema.text(hit[1], [linkType.create({ href: hit[2] })])
            view.dispatch(view.state.tr.replaceSelectionWith(node))
            return true
          },
        },
      }),
    ]
  },
})