/**
 * 图片节点扩展（缺陷 #15 / tasks.md 6.3：粘贴截图 → 正文图片节点）。
 *
 * 只负责 schema 层：一个行内原子节点，`src` 里存 data URL（如 `data:image/webp;base64,...`）。
 * 依赖禁令下没有引入 @tiptap/extension-image，属性解析/渲染按官方 Image 扩展的口径手写。
 *
 * 序列化：tiptap-markdown 内置了 name 为 "image" 的默认 Markdown spec
 * （defaultMarkdownSerializer.nodes.image，读 attrs 的 src/alt/title），按扩展 name 匹配；
 * 因此这里不需要 addStorage，注册本节点后 `![alt](src)` 的读（markdown-it → <img> → parseHTML）
 * 与写（serialize → Markdown）都自动成立。
 */
import { Node, mergeAttributes } from '@tiptap/core'

export const ImageExtension = Node.create({
  name: 'image',

  /** 行内节点：可出现在段落/标题的文本流中，与 Markdown 图片的行内语义一致 */
  inline: true,
  /** 原子节点：整体选中/删除，光标不能进入内部，避免与输入法组合态冲突 */
  atom: true,
  /** 明确归入 inline 组，让 content 表达式能引用它 */
  group: 'inline',
  /** 选中后可拖动换位置 */
  draggable: true,
  /** 点击可整体选中（配合 Delete 删除） */
  selectable: true,

  addAttributes() {
    return {
      // src 语义上必填；无 src 的 <img> 在 parseHTML 阶段被丢弃
      src: {
        default: null,
        parseHTML: (element) => element.getAttribute('src'),
      },
      alt: {
        default: '',
        parseHTML: (element) => element.getAttribute('alt') ?? '',
      },
      // title 可缺省：default 为 null 时 prosemirror-model 不会输出该 HTML 属性
      title: {
        default: null,
        parseHTML: (element) => element.getAttribute('title'),
      },
    }
  },

  parseHTML() {
    return [
      {
        tag: 'img[src]',
        // 空 src 会渲染成破图：返回 false 让 ProseMirror 跳过该 <img>
        getAttrs: (element) => {
          if (typeof element === 'string') return false
          const src = element.getAttribute('src')
          return src && src.trim() ? null : false
        },
      },
    ]
  },

  renderHTML({ HTMLAttributes }) {
    // addAttributes 的默认渲染会带上 src/alt/title；值为 null 的属性由 prosemirror-model 跳过
    return ['img', mergeAttributes(HTMLAttributes)]
  },
})