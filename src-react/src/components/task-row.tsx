import { useState } from 'react'
import type { Task } from '@/lib/api'
import { formatDue, getCategoryColor } from '@/lib/api'
import { cn } from '@/lib/utils'

export type RowActions = 'today' | 'inbox' | 'none'

export interface TaskRowProps {
  task: Task
  /** 注入今天（YYYY-MM-DD）；缺省组件内取本地 */
  today?: string
  /** hover 动作集：today 页=排期/⋯；inbox 页=→今天/排期/⋯（spec §A.2） */
  actions?: RowActions
  selected?: boolean
  /** 列表内序号：启用 j/k 焦点导航（容器级 hook 依赖 data-row-index） */
  rowIndex?: number
  /** 启用整行拖拽排序（便签规格 §12.2，数据键 text/kettd-order） */
  dragEnabled?: boolean
  onToggle?: (id: string) => void
  onPlanToday?: (id: string) => void
  onSchedule?: (id: string, dueAt: string | null) => void
  onDelete?: (id: string) => void
  onOpen?: (id: string) => void
  onSelect?: (id: string, checked: boolean) => void
}

/**
 * 统一列表行组件（today-list-ui-spec §A）：今天/收件箱/便签共用一套「行语言」。
 * - 默认态只保留读元素：勾选框 · 标题 · badge · 分类 chip（动作不占位）
 * - hover/focus 行尾浮出紧凑动作；触屏/无 hover 点行展开详情承载（onOpen）
 * - 顺延/滞留 = 琥珀竖条 + 行内 badge（--pri-med 系，非红——守「安静文具」调性）
 * - 键盘：Space 勾选 · T 加入今天 · E 排期 · X 删除 · Enter 打开详情 · j/k 行间移动
 */
export function TaskRow({
  task,
  today,
  actions = 'none',
  selected,
  rowIndex,
  dragEnabled,
  onToggle,
  onPlanToday,
  onSchedule,
  onDelete,
  onOpen,
  onSelect,
}: TaskRowProps) {
  const [scheduling, setScheduling] = useState(false)
  const [scheduleValue, setScheduleValue] = useState('')
  const [menuOpen, setMenuOpen] = useState(false)

  const carried = (Number(task.carriedFrom) || 0) > 0
  const done = !!task.done

  const commitSchedule = () => {
    if (!scheduleValue) {
      setScheduling(false)
      return
    }
    onSchedule?.(task.id, scheduleValue)
    setScheduling(false)
    setScheduleValue('')
  }

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (scheduling) return
    if (e.altKey) return // Alt+↑/↓ 交给容器排序
    if (e.key === ' ') { e.preventDefault(); onToggle?.(task.id) }
    else if (e.key === 'Enter') { e.preventDefault(); onOpen?.(task.id) }
    else if (e.key.toLowerCase() === 't' && onPlanToday) { e.preventDefault(); onPlanToday(task.id) }
    else if (e.key.toLowerCase() === 'e' && onSchedule) { e.preventDefault(); setScheduling(true) }
    else if (e.key.toLowerCase() === 'x' && onDelete) { e.preventDefault(); onDelete(task.id) }
    else if (e.key === 'Escape') { setMenuOpen(false) }
  }

  return (
    <div
      className={cn(
        'trow group',
        done && 'trow-done',
        carried && 'trow-aged',
        selected && 'trow-selected',
      )}
      tabIndex={0}
      role="listitem"
      aria-selected={selected}
      onKeyDown={onKeyDown}
      data-id={task.id}
      data-row-index={rowIndex}
      draggable={!!dragEnabled}
      onDragStart={(e) => {
        if (!dragEnabled || rowIndex === undefined) return
        e.dataTransfer.effectAllowed = 'move'
        e.dataTransfer.setData('text/kettd-order', String(rowIndex))
      }}
    >
      {onSelect ? (
        <input
          type="checkbox"
          className="tcheck"
          aria-label={`选择 ${task.title}`}
          checked={!!selected}
          onClick={(e) => e.stopPropagation()}
          onChange={(e) => onSelect(task.id, e.target.checked)}
        />
      ) : (
        <input
          type="checkbox"
          className="tcheck"
          aria-label={`完成 ${task.title}`}
          checked={done}
          onClick={(e) => e.stopPropagation()}
          onChange={() => onToggle?.(task.id)}
        />
      )}

      <span className={cn('grow truncate sm', done && 'line-through opacity-60')}>
        {task.title}
        {carried && !done && (
          <span className="badge badge-aged">顺延 {task.carriedFrom} 天</span>
        )}
      </span>

      <span className="row-flex items-center gap-1.5 whitespace-nowrap">
        {task.dueAt && (
          <code className={cn('tmeta mono', formatDue(task.dueAt, today).cls)}>
            {formatDue(task.dueAt, today).text}
          </code>
        )}
        <span className={cn('cat-chip', getCategoryColor(task.category))}>{task.category}</span>

        {/* hover 动作：默认不占位（opacity-0），focus-within 同样浮出（键盘等价） */}
        <span className="row-actions" onClick={(e) => e.stopPropagation()}>
          {actions === 'inbox' && onPlanToday && (
            <button className="btn ghost xs" title="加入今天（T）" onClick={() => onPlanToday(task.id)}>→今天</button>
          )}
          {actions !== 'none' && onSchedule && (
            <button className="btn ghost xs" title="排期（E）" onClick={() => setScheduling((v) => !v)}>排期…</button>
          )}
          {onDelete && (
            <button
              className="btn ghost xs"
              title="详情 / 删除"
              aria-haspopup="menu"
              aria-expanded={menuOpen}
              onClick={() => setMenuOpen((v) => !v)}
            >
              ⋯
            </button>
          )}
        </span>
      </span>

      {menuOpen && (
        <span className="menu-pop" role="menu" aria-label="更多动作">
          <button
            className="btn ghost xs"
            role="menuitem"
            onClick={() => { setMenuOpen(false); onOpen?.(task.id) }}
          >
            详情（Enter）
          </button>
          <button
            className="btn ghost xs"
            role="menuitem"
            onClick={() => { setMenuOpen(false); onDelete?.(task.id) }}
          >
            删除（X）
          </button>
        </span>
      )}

      {scheduling && (
        <span className="schedule-pop">
          <input
            type="date"
            className="input xs"
            value={scheduleValue}
            autoFocus
            onChange={(e) => setScheduleValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') commitSchedule()
              if (e.key === 'Escape') setScheduling(false)
            }}
          />
          <button className="btn xs outline" onClick={commitSchedule}>定</button>
        </span>
      )}
    </div>
  )
}
