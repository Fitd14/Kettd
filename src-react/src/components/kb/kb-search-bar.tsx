interface Props {
  query: string
  onChange: (q: string) => void
  resultCount: number
}

export function KbSearchBar({ query, onChange, resultCount }: Props) {
  return (
    <div className="kb-search-bar">
      <input
        type="search"
        value={query}
        onChange={(e) => onChange(e.target.value)}
        placeholder="搜索知识库..."
        aria-label="搜索知识库"
      />
      {query && (
        <span className="kb-result-count">{resultCount} 条结果</span>
      )}
    </div>
  )
}
