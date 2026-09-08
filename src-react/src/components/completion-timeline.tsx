import { useMemo, useRef, useState } from 'react'
import type { Task } from '@/lib/api'
import { getCategoryColor } from '@/lib/api'

interface Props {
  /** 已按 doneAt 倒序（最近在前）的完成项 */
  items: Task[]
  onSelect?: (id: string) => void
  /** 每批渲染条数；滚到右端或点「更早」追加一批（timeline-spec §5 分页 loading 语义） */
  pageSize?: number
}

function hhmm(doneAt: string): string {
  return String(doneAt).slice(11, 16)
}
function mmdd(doneAt: string): string {
  return String(doneAt).slice(5, 10).replace('-', '-')
}

/**
 * CompletionTimeline（timeline-component-spec 定稿）：默认内嵌、横向可滚动、
 * 按 doneAt 倒序的完成时间轴。轨道线 + 分类色圆点 + MM-DD HH:mm 时间标签
 * （<time> 语义），节点可聚焦、Enter 打开详情；右端「更早」分页。
 * shadcn token 自研，不引入 antd（§1 决策）。
 */
export function CompletionTimeline({ items, onSelect, pageSize = 30 }: Props) {
  const [visible, setVisible] = useState(pageSize)
  const scrollerRef = useRef<HTMLDivElement>(null)

  const shown = useMemo(() => items.slice(0, visible), [items, visible])
  const hasMore = items.length > visible

  const loadMore = () => setVisible((v) => Math.min(v + pageSize, items.length))

  const onScroll = () => {
    const el = scrollerRef.current
    if (!el || !hasMore) return
    if (el.scrollLeft + el.clientWidth >= el.scrollWidth - 40) loadMore()
  }

  if (items.length === 0) return null

  return (
    <div
      ref={scrollerRef}
      className="ctl"
      role="list"
      aria-label="完成时间轴"
      onScroll={onScroll}
    >
      {shown.map((t) => (
        <div className="ctl-item" role="listitem" key={t.id}>
          <time className="ctl-time mono" dateTime={String(t.doneAt)}>
            {mmdd(String(t.doneAt))} {hhmm(String(t.doneAt))}
          </time>
          <button
            className={`ctl-dot ${getCategoryColor(t.category)}`}
            title={`${String(t.doneAt).slice(0, 16).replace('T', ' ')} · ${t.title}`}
            aria-label={`${String(t.doneAt).slice(0, 16).replace('T', ' ')} 完成：${t.title}`}
            onClick={() => onSelect?.(t.id)}
          />
          <span className="ctl-title truncate sm">{t.title}</span>
          <span className={`cat-chip ${getCategoryColor(t.category)}`}>{t.category}</span>
        </div>
      ))}
      {hasMore && (
        <div className="ctl-item ctl-more" role="listitem">
          <span className="ctl-time text-muted">…</span>
          <button className="ctl-dot ghost" aria-label="加载更早" onClick={loadMore} />
          <button className="btn ghost xs" onClick={loadMore}>更早 →</button>
        </div>
      )}
    </div>
  )
}
