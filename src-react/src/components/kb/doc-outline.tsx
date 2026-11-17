import { Button } from '@/components/ui/button'
import type { OutlineItem } from '@/lib/doc-outline'

interface Props {
  items: OutlineItem[]
  /** 当前阅读位置命中的标题下标，-1 表示还在标题之前 */
  active: number
  onPick: (index: number) => void
  /** rail = 页边栏（宽窗并排）；drawer = 窄窗右侧抽屉（自带关闭按钮） */
  variant?: 'rail' | 'drawer'
  onClose?: () => void
  /** 轨道常驻、大纲收起：保留占位但不显形（kb-workspace-layout D3） */
  hidden?: boolean
}

/**
 * 页边大纲（文档右侧，像手稿页边写的批注）：
 * 只读、不参与正文排版；层级靠左缩进与连线表达，没有图标、没有编号。
 * 宽窗是页边栏，窄窗换成右侧抽屉——同一份数据、同一个组件（kb-doc-shell D4）。
 */
export function DocOutline({ items, active, onPick, variant = 'rail', onClose, hidden = false }: Props) {
  return (
    <aside
      className={'kb-outline' + (variant === 'drawer' ? ' kb-outline-drawer' : '') + (hidden ? ' is-hidden' : '')}
      aria-label="文档大纲"
      aria-hidden={hidden || undefined}
    >
      {/* 内层自己 sticky + 自己滚：长文档读到底时大纲不跟着跑（m01554 第 8 条） */}
      <div className="kb-outline-inner">
        <div className="kb-outline-head">
          <span>大纲</span>
          <span className="kb-outline-head-right">
            {onClose && (
              <Button size="icon-xs" onClick={onClose} aria-label="关闭大纲" title="关闭（Esc）">
                <span aria-hidden="true">×</span>
              </Button>
            )}
          </span>
        </div>
      {items.length === 0 ? (
          <p className="kb-outline-empty">用标题分段后，这里会出现目录。</p>
        ) : (
        <ol className="kb-outline-list">
          {items.map((item, i) => (
            <li key={`${i}-${item.text}`}>
              <button
                type="button"
                className={'kb-outline-link' + (i === active ? ' on' : '')}
                data-level={item.level}
                onClick={() => onPick(i)}
                title={item.text || '未命名标题'}
              >
                {item.text || '未命名标题'}
              </button>
            </li>
          ))}
        </ol>
      )}
      </div>
    </aside>
  )
}