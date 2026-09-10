import type { KbItem } from '@/lib/api'

interface Props {
  item: KbItem
  isActive: boolean
  onSelect: (id: string) => void
  /** AI 语义命中标记（Phase 3 接线） */
  isAiHit?: boolean
}

function relativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime()
  const mins = Math.floor(diff / 60000)
  if (mins < 60) return `${mins}分钟前`
  const hrs = Math.floor(mins / 60)
  if (hrs < 24) return `${hrs}小时前`
  const days = Math.floor(hrs / 24)
  return `${days}天前`
}

export function KbItemRow({ item, isActive, onSelect, isAiHit }: Props) {
  return (
    <button
      type="button"
      className={`kb-item-row${isActive ? ' active' : ''}`}
      onClick={() => onSelect(item.id)}
      role="option"
      aria-selected={isActive}
    >
      <span className="item-title">{item.title}</span>
      <span className="item-meta">
        {item.tags?.slice(0, 2).map((t) => (
          <span key={t} className="kb-tag-chip">{t}</span>
        ))}
        <span>{relativeTime(item.updatedAt)}</span>
        {isAiHit && <span className="ai-badge">AI</span>}
      </span>
    </button>
  )
}
