import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Pin, PinOff, X } from 'lucide-react'
import {
  applyTheme,
  getBootstrap,
  hideFloat,
  onEvent,
  reorderTasks,
  setStickyPinned,
  stickySelf,
  trackEvent,
  updateSticky,
  updateTask,
  type Bootstrap,
  type StickySelf,
  type Task,
} from '@/lib/api'
import { todayList } from '../kernel/selectors.js'
import { localToday } from '../kernel/time.js'
import { useRowNav } from '@/lib/list-nav'
import { PaperPattern } from './paper-patterns'
import './sticky.css'

/** 每张待办便签的分页大小（multi-sticky-spec §2） */
const PAGE_SIZE = 7
const FREE_MAX = 500

/**
 * 便签窗（便签规格 + multi-sticky-spec · 多实例）：
 * - 窗身份由后端 sticky_self 给出（float=1 号待办便签 page0；note:* 动态窗）
 * - 待办便签 = 分页镜像今天 + 钉单条；第 N 张显示第 N 页（分页镜像语义）
 * - 自由便签 = 一张纸：点击即写、失焦保存（≤500 字），不进今天不参与提醒
 * - 共用：固定/收起两键、拖动记位、移出淡化（可关+可调透明度）、纸色/花纹
 */
export default function StickyWindow() {
  const [boot, setBoot] = useState<Bootstrap | null>(null)
  const [self, setSelf] = useState<StickySelf | null>(null)
  const [faded, setFaded] = useState(false)
  const [freeText, setFreeText] = useState('')
  const bodyRef = useRef<HTMLDivElement>(null)
  const freeRef = useRef<HTMLTextAreaElement>(null)

  const refresh = useCallback(async () => {
    const [b, s] = await Promise.all([getBootstrap(), stickySelf()])
    if (!b.err && b.data) {
      setBoot(b.data)
      applyTheme(b.data.settings.theme)
    }
    if (!s.err && s.data) {
      setSelf(s.data)
      // 自由便签：编辑中不回灌内容（避免打字被刷新打断）
      if (document.activeElement !== freeRef.current) setFreeText(s.data.content)
    }
  }, [])

  useEffect(() => {
    void refresh()
    const off = onEvent('store-changed', () => { void refresh() })
    const offTheme = onEvent('theme-changed', (payload: unknown) => applyTheme(payload as string))
    return () => { off(); offTheme() }
  }, [refresh])

  const today = localToday()
  const isFree = self?.kind === 'free'
  const page = self?.page ?? 0
  const mirror: Task[] = useMemo(() => {
    if (!boot || isFree) return []
    return todayList(boot.tasks, today).slice(page * PAGE_SIZE, page * PAGE_SIZE + PAGE_SIZE)
  }, [boot, today, page, isFree])
  const pinned: Task[] = useMemo(() => {
    if (isFree || page > 0) return []
    return (boot?.tasks ?? []).filter(
      (t) => t.stickyPinned && !t.done && !t.deletedAt && !mirror.some((m) => m.id === t.id),
    )
  }, [boot, mirror, isFree, page])

  const { containerProps } = useRowNav({
    containerRef: bodyRef,
    ids: mirror.map((t) => t.id),
    onReorder: async (ids) => {
      await reorderTasks(ids)
      await refresh()
    },
  })

  const paper = boot?.settings.stickyPaper ?? 'warm'
  const pinnedOn = self?.pinned ?? true
  const fadeOn = boot?.settings.stickyFade ?? true
  const fadeOpacity = boot?.settings.stickyFadeOpacity ?? 38
  const pattern = boot?.settings.stickyPattern ?? 'none'

  const togglePin = useCallback(async () => {
    if (!self) return
    // 1 号待办便签（float）的置顶 = 全局设置；其余便签按张记在自身数据里
    const r = self.id === 'sticky'
      ? await setStickyPinned(!pinnedOn)
      : await updateSticky(self.id, { pinned: !pinnedOn })
    if (!r.err) await refresh()
  }, [self, pinnedOn, refresh])

  const toggleDone = useCallback(async (id: string, next: boolean) => {
    const r = await updateTask(id, { done: next })
    if (!r.err) await refresh()
  }, [refresh])

  const unpin = useCallback(async (id: string) => {
    const r = await updateTask(id, { stickyPinned: false })
    if (!r.err) await refresh()
  }, [refresh])

  const openMain = useCallback(() => {
    void import('@/lib/api').then((m) => m.openMainWindow())
    void import('@/lib/api').then((m) => m.emitNavigate('/today'))
  }, [])

  /** ✕ = 收起（非销毁）：动态便签 hidden=true 由后端收窗；1 号走 legacy hideFloat */
  const close = useCallback(async () => {
    if (self && self.id !== 'sticky') {
      const r = await updateSticky(self.id, { hidden: true })
      if (!r.err) return // 后端已收窗
    }
    await hideFloat()
  }, [self])

  const commitFree = useCallback(async () => {
    if (!self) return
    const next = freeText.trim().slice(0, FREE_MAX)
    if (next === self.content) return
    const r = await updateSticky(self.id, { content: next })
    if (r.err) return
    // metric：自由便签是否真被写（字数桶）
    const bucket = next.length === 0 ? '0' : next.length <= 50 ? '1-50' : next.length <= 200 ? '51-200' : '200+'
    void trackEvent('sticky_free_edit', { len_bucket: bucket })
    await refresh()
  }, [self, freeText, refresh])

  const rows = (list: Task[], start: number, removable: boolean) =>
    list.map((t, i) => (
      <div
        key={t.id}
        className="sticky-row"
        role="listitem"
        data-row-index={start + i}
        draggable
        onDragStart={(e) => {
          e.dataTransfer.effectAllowed = 'move'
          e.dataTransfer.setData('text/kettd-order', String(start + i))
        }}
        onDragEnd={(e) => { e.preventDefault() }}
      >
        <input
          type="checkbox"
          className="tcheck"
          aria-label={`完成 ${t.title}`}
          checked={!!t.done}
          onMouseDown={(e) => e.preventDefault()}
          onChange={() => { void toggleDone(t.id, !t.done) }}
        />
        <button className="sticky-title" onClick={openMain} onMouseDown={(e) => e.stopPropagation()}>
          {t.title}
          {(Number(t.carriedFrom) || 0) > 0 && (
            <span className="sticky-carry">顺延 {t.carriedFrom} 天</span>
          )}
        </button>
        {removable && (
          <button
            className="sticky-unpin"
            aria-label={`从便签取下：${t.title}`}
            title="取下"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => { void unpin(t.id) }}
          >
            ×
          </button>
        )}
      </div>
    ))

  const doneToday = boot?.tasks.filter(
    (t) => t.done && (t.plannedDate === today || String(t.dueAt || '').startsWith(today)),
  ).length ?? 0
  const total = mirror.length + pinned.length

  return (
    <div
      className={`sticky-note paper-${paper}${faded && fadeOn ? ' faded' : ''}${pinnedOn ? ' is-pinned' : ''}${isFree ? ' is-free' : ''}`}
      style={{ '--sticky-fade-opacity': `${fadeOpacity}%` } as React.CSSProperties}
      onMouseLeave={() => setFaded(true)}
      onMouseEnter={() => setFaded(false)}
    >
      <PaperPattern pattern={pattern} />
      <div className="sticky-strip">
        <button
          className={`sticky-btn pin${pinnedOn ? ' on' : ''}`}
          title={pinnedOn ? '取消固定（沉到普通层）' : '固定（置顶）'}
          aria-label={pinnedOn ? '取消固定' : '固定置顶'}
          aria-pressed={pinnedOn}
          onClick={() => { void togglePin() }}
        >
          {pinnedOn ? <Pin size={14} aria-hidden /> : <PinOff size={14} aria-hidden />}
        </button>
        <span className="sticky-head" data-tauri-drag-region title="按住拖动">
          {isFree ? (
            <>自由便签{freeText.trim() && ' · 已保存'}</>
          ) : (
            <>今天要做的 · <b>{total}</b>{page > 0 && ` · 第 ${page + 1} 页`}{doneToday > 0 && page === 0 && ` · 做完 ${doneToday}`}</>
          )}
        </span>
        <button className="sticky-btn" title="收起（清单可再展开）" aria-label="收起便签" onClick={() => { void close() }}>
          <X size={14} aria-hidden />
        </button>
      </div>

      {isFree ? (
        <div className="sticky-body">
          <textarea
            ref={freeRef}
            className="sticky-free"
            value={freeText}
            maxLength={FREE_MAX}
            placeholder="自由写点什么…（失焦自动保存）"
            aria-label="自由便签内容"
            onChange={(e) => setFreeText(e.target.value.slice(0, FREE_MAX))}
            onBlur={() => { void commitFree() }}
          />
          <span className="sticky-count tiny">{freeText.length}/{FREE_MAX}</span>
        </div>
      ) : (
        <div className="sticky-body" ref={bodyRef} role={total > 0 ? 'list' : undefined} aria-label="便签待办" {...containerProps}>
          {total === 0 ? (
            <div className="sticky-empty">
              {page > 0 ? '这一页没有更多待办了 ·' : <>今天没有待办 ·<br /></>}
              <kbd>Alt+Shift+A</kbd> 记一条
            </div>
          ) : (
            <>
              {rows(mirror, 0, false)}
              {pinned.length > 0 && (
                <>
                  <div className="sticky-gap" />
                  {rows(pinned, mirror.length, true)}
                </>
              )}
            </>
          )}
        </div>
      )}
      <i className="sticky-fold" aria-hidden />
    </div>
  )
}
