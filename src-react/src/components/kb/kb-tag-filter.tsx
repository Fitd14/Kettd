interface Props {
  tags: string[]
  activeTag: string | null
  onSelect: (tag: string | null) => void
}

export function KbTagFilter({ tags, activeTag, onSelect }: Props) {
  if (tags.length === 0) return null

  return (
    <div className="kb-tag-filter" role="radiogroup" aria-label="标签筛选">
      <button
        className={`kb-tag-chip ${activeTag === null ? 'active' : ''}`}
        onClick={() => onSelect(null)}
      >
        全部
      </button>
      {tags.map((tag) => (
        <button
          key={tag}
          className={`kb-tag-chip ${activeTag === tag ? 'active' : ''}`}
          onClick={() => onSelect(activeTag === tag ? null : tag)}
        >
          {tag}
        </button>
      ))}
    </div>
  )
}
