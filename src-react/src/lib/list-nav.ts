import { useCallback, type RefObject } from 'react'

export interface RowNavOptions {
  /** 列表容器：j/k 与 Alt+↑/↓ 在容器内带 [data-row-index] 的行间生效 */
  containerRef: RefObject<HTMLElement | null>
  /** 当前可见行序；传入 onReorder 时启用 Alt+↑/↓ 与拖拽排序（便签规格 §12.2） */
  ids?: string[]
  onReorder?: (next: string[]) => void
}

function isEditableTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null
  if (!el) return false
  const tag = el.tagName
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || el.isContentEditable
}

function rowIndexOf(target: EventTarget | null): number {
  const row = (target as HTMLElement | null)?.closest?.('[data-row-index]') as HTMLElement | null
  return row ? Number(row.dataset.rowIndex) : NaN
}

/**
 * 列表键盘导航 + 手动排序（today-list-ui-spec §A.3 / sticky §12.2）：
 * - j / k：焦点移到下一行 / 上一行（vim 语义，j=下）
 * - Alt+↑/↓：与相邻行交换顺序并回调 onReorder（整表顺序语义）
 * - 拖拽：容器 onDrop 按 dataTransfer 'text/kettd-order' 的来源行插入落点行位置
 * TaskRow 侧用 rowIndex/dragEnabled 把 data-row-index 与 draggable 落到行元素上。
 */
export function useRowNav({ containerRef, ids, onReorder }: RowNavOptions) {
  const moveFocus = useCallback(
    (to: number) => {
      const rows = containerRef.current?.querySelectorAll<HTMLElement>('[data-row-index]')
      const target = rows?.[to]
      if (target) target.focus()
    },
    [containerRef],
  )

  const reorderTo = useCallback(
    (from: number, to: number) => {
      if (!ids || !onReorder) return
      if (Number.isNaN(from) || Number.isNaN(to)) return
      if (to < 0 || to >= ids.length || from === to) return
      const next = [...ids]
      const [moved] = next.splice(from, 1)
      next.splice(to, 0, moved)
      onReorder(next)
      requestAnimationFrame(() => moveFocus(to))
    },
    [ids, onReorder, moveFocus],
  )

  const containerProps = {
    onKeyDown: useCallback(
      (e: React.KeyboardEvent) => {
        if (isEditableTarget(e.target)) return
        const index = rowIndexOf(e.target)
        if (Number.isNaN(index)) return
        if (onReorder && e.altKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
          e.preventDefault()
          reorderTo(index, e.key === 'ArrowUp' ? index - 1 : index + 1)
          return
        }
        if (!e.altKey && !e.ctrlKey && !e.metaKey && (e.key === 'j' || e.key === 'k')) {
          e.preventDefault()
          moveFocus(e.key === 'j' ? index + 1 : index - 1)
        }
      },
      [onReorder, reorderTo, moveFocus],
    ),
    ...(onReorder
      ? {
          onDragOver: (e: React.DragEvent) => {
            if (e.dataTransfer.types.includes('text/kettd-order')) {
              e.preventDefault()
              e.dataTransfer.dropEffect = 'move'
            }
          },
          onDrop: (e: React.DragEvent) => {
            if (!e.dataTransfer.types.includes('text/kettd-order')) return
            e.preventDefault()
            const from = Number(e.dataTransfer.getData('text/kettd-order'))
            reorderTo(from, rowIndexOf(e.target))
          },
        }
      : {}),
  }

  return { containerProps }
}
