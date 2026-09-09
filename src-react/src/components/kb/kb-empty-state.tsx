interface Props {
  variant: 'empty' | 'no-results' | 'limit' | 'error'
  onCreate?: () => void
  errorMessage?: string
}

const messages = {
  empty:     { icon: '📚', text: '知识库空空如也', action: '新建第一条知识' },
  'no-results': { icon: '🔍', text: '没找到', action: null },
  limit:     { icon: '⚠️', text: '知识条目已达上限（5000），请先清理', action: null },
  error:     { icon: '⚠️', text: '加载出错', action: null },
}

export function KbEmptyState({ variant, onCreate, errorMessage }: Props) {
  const { icon, text, action } = messages[variant]

  return (
    <div className="kb-empty">
      <span className="empty-icon">{icon}</span>
      <span>{variant === 'error' && errorMessage ? errorMessage : text}</span>
      {action && onCreate && (
        <button className="btn primary" onClick={onCreate}>{action}</button>
      )}
    </div>
  )
}
