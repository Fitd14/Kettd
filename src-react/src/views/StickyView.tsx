import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link2, Plus, Search } from 'lucide-react'
import {
  createSticky,
  deleteSticky,
  getKbItems,
  listStickies,
  onEvent,
  openSticky,
  updateSticky,
} from '@/lib/api'
import type { StickyNoteItem } from '@/lib/api'
import { stickyMatches } from '@/lib/sticky-filter'
import { plainPreview } from '@/lib/sticky-text'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import './sticky-wall.css'

/** 可见性兼容读取：旧数据 / 旧桥接可能缺 visible 字段，缺失按「可见」（对齐 models.rs 默认值） */
const isStickyVisible = (st: StickyNoteItem) => st.visible !== false

/** 更新时间展示：后端给本地语义串（YYYY-MM-DDTHH:mm），只把 T 换空格，禁止 UTC 换算 */
function updatedText(value: string | null | undefined): string {
  const raw = typeof value === 'string' ? value : ''
  return raw ? raw.replace('T', ' ') : '未知时间'
}

/**
 * 便签卡片墙（OpenSpec sticky-shell-polish D1 / tasks 2.1–2.7）。
 * 数据自取：list_stickies + get_kb_items，store-changed 广播即刷新；
 * 样式只走全局语义令牌，不呈现纸色/花纹（纸色与花纹只属于便签窗）。
 */
export function StickyView() {
  const [stickies, setStickies] = useState<StickyNoteItem[]>([])
  const [kbTitles, setKbTitles] = useState<Map<string, string>>(() => new Map())
  /** 知识标题映射的加载态：失败时不能把全部 kbRef 误标成「条目已删除」 */
  const [kbStatus, setKbStatus] = useState<'loading' | 'ready' | 'error'>('loading')
  const [error, setError] = useState<string | null>(null)
  const [creating, setCreating] = useState(false)
  const [busyId, setBusyId] = useState<string | null>(null)
  const [confirmDel, setConfirmDel] = useState<string | null>(null)
  const [query, setQuery] = useState('')

  const loadStickies = useCallback(async () => {
    const r = await listStickies()
    if (r.err) { setError(r.err); return }
    setError(null)
    setStickies(r.data ?? [])
  }, [])

  const loadKbTitles = useCallback(async () => {
    const r = await getKbItems()
    if (r.err || !r.data) { setKbStatus('error'); return }
    setKbTitles(new Map(r.data.map((item) => [item.id, item.title])))
    setKbStatus('ready')
  }, [])

  const reload = useCallback(async () => {
    await Promise.all([loadStickies(), loadKbTitles()])
  }, [loadStickies, loadKbTitles])

  useEffect(() => {
    void reload()
    // 跨窗唯一广播源；订阅前到达的事件由 api 的补发队列回放
    const off = onEvent('store-changed', () => { void reload() })
    return off
  }, [reload])

  // 删除确认可 Esc 取消（Tauri WebView 没有原生 confirm）
  useEffect(() => {
    if (!confirmDel) return
    const onKey = (event: KeyboardEvent) => { if (event.key === 'Escape') setConfirmDel(null) }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [confirmDel])

  const visibleCount = useMemo(() => stickies.filter(isStickyVisible).length, [stickies])
  const hiddenCount = stickies.length - visibleCount

  const filtered = useMemo(
    () => stickies.filter((st) =>
      stickyMatches(query, st.content, st.kbRef ? kbTitles.get(st.kbRef) : null),
    ),
    [stickies, query, kbTitles],
  )

  const doCreate = async () => {
    if (creating) return
    setCreating(true)
    setError(null)
    const r = await createSticky()
    setCreating(false)
    if (r.err) { setError(r.err); return }
    await loadStickies()
  }

  /** 打开：隐藏便签先恢复可见再建窗（上限 5，超限显示后端中文文案） */
  const doOpen = async (st: StickyNoteItem) => {
    if (busyId) return
    setBusyId(st.id)
    setError(null)
    try {
      if (!isStickyVisible(st)) {
        const revived = await updateSticky(st.id, { visible: true })
        if (revived.err) { setError(revived.err); return }
      }
      const opened = await openSticky(st.id)
      if (opened.err) { setError(opened.err); return }
      await loadStickies()
    } finally {
      setBusyId(null)
    }
  }

  const doDelete = async (st: StickyNoteItem) => {
    const r = await deleteSticky(st.id)
    if (r.err) { setError(r.err); return }
    setConfirmDel(null)
    setError(null)
    await loadStickies()
  }

  return (
    <div className="view sticky-wall">
      <header className="sticky-wall-head">
        <div className="sticky-wall-title-row">
          <div>
            <div className="view-title">便签</div>
            <div className="view-sub">可见 {visibleCount}/5 · 隐藏 {hiddenCount}</div>
          </div>
          <Button
            size="sm"
            onClick={() => { void doCreate() }}
            disabled={creating}
          >
            <Plus size={14} aria-hidden="true" />
            {creating ? '新建中…' : '新建便签'}
          </Button>
        </div>
        <div className="sticky-wall-toolbar">
          <label className="sticky-wall-search">
            <Search size={14} aria-hidden="true" />
            <Input
              type="search"
              value={query}
              placeholder="搜索便签正文或知识标题"
              aria-label="搜索便签"
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
          {query.trim().length > 0 && (
            <Button variant="ghost" size="xs" onClick={() => setQuery('')}>清空搜索</Button>
          )}
        </div>
        {error && <div className="inline-err" role="alert">{error}</div>}
      </header>

      {stickies.length === 0 ? (
        <div className="empty">
          <div>还没有便签</div>
          <div className="tiny text-muted">随手记一张纸，常驻置顶；关闭只是隐藏，删除才不可恢复。</div>
          <Button size="sm" onClick={() => { void doCreate() }} disabled={creating}>
            新建第一张便签
          </Button>
        </div>
      ) : filtered.length === 0 ? (
        <div className="empty">
          <div>没有匹配的便签</div>
          <div className="tiny text-muted">换个关键词试试，或清空搜索。</div>
          <Button size="sm" onClick={() => setQuery('')}>清空搜索</Button>
        </div>
      ) : (
        <ul className="sticky-wall-grid" aria-label="便签卡片">
          {filtered.map((st) => {
            const visible = isStickyVisible(st)
            const title = st.kbRef ? kbTitles.get(st.kbRef) : undefined
            const preview = plainPreview(st.content) || '（空）'
            const sourceFallback = kbStatus === 'ready'
              ? '知识条目已删除'
              : kbStatus === 'error' ? '知识来源读取失败' : '知识来源读取中…'
            return (
              <li key={st.id} className={`sticky-card${visible ? '' : ' is-hidden'}`}>
                <div className="sticky-card-top">
                  <span className={`badge${visible ? '' : ' sticky-status-hidden'}`}>
                    {visible ? '已显示' : '已隐藏'}
                  </span>
                  {st.mini && <span className="badge sticky-status-mini">已缩小</span>}
                  <time className="sticky-card-time" dateTime={st.updatedAt || undefined}>
                    {updatedText(st.updatedAt)}
                  </time>
                </div>
                <p className="sticky-card-preview">{preview}</p>
                {st.kbRef && (
                  <div className={`sticky-card-source${title ? '' : ' is-missing'}`}>
                    <Link2 size={12} aria-hidden="true" />
                    <span className="sticky-card-source-text" title={title ?? sourceFallback}>
                      {title ?? sourceFallback}
                    </span>
                  </div>
                )}
                <div className="sticky-card-actions">
                  {confirmDel === st.id ? (
                    <>
                      <Button variant="danger" size="xs" onClick={() => { void doDelete(st) }}>
                        确认删除
                      </Button>
                      <Button variant="ghost" size="xs" onClick={() => setConfirmDel(null)}>取消</Button>
                    </>
                  ) : (
                    <>
                      <Button
                        size="xs"
                        onClick={() => { void doOpen(st) }}
                        disabled={busyId === st.id}
                        aria-label={`打开便签：${preview}`}
                      >
                        {busyId === st.id ? '打开中…' : '打开'}
                      </Button>
                      <Button variant="ghost" size="xs" onClick={() => setConfirmDel(st.id)}>删除</Button>
                    </>
                  )}
                </div>
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}