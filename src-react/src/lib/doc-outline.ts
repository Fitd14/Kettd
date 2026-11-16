import type { Node as PMNode } from '@tiptap/pm/model'

/** 大纲条目：只保留层级与文字，位置由渲染顺序决定（标题 DOM 顺序与文档顺序一致） */
export interface OutlineItem {
  /** 1–3（H4 以下不进大纲，避免页边栏退化成目录树） */
  level: number
  text: string
}

/**
 * 从 ProseMirror 文档里抽标题，供右侧大纲使用。
 * 空标题保留（显示为「未命名标题」），因为空标题也是用户可定位的分节。
 */
export function collectOutline(doc: PMNode, maxLevel = 3): OutlineItem[] {
  const items: OutlineItem[] = []
  doc.descendants((node) => {
    if (node.type.name !== 'heading') return true
    const level = Number(node.attrs.level) || 1
    if (level <= maxLevel) {
      items.push({ level, text: node.textContent.trim() })
    }
    // 标题内部不会再嵌套标题，跳过子树省一遍遍历
    return false
  })
  return items
}