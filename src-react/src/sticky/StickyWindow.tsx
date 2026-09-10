import { useCallback, useEffect, useRef, useState } from 'react'
import { GripHorizontal, Minus, X } from 'lucide-react'
import {
  applyTheme,
  deleteSticky,
  getBootstrap,
  getKbItems,
  onEvent,
  stickySelf,
  trackEvent,
  updateKbItem,
  updateSticky,
  type Bootstrap,
  type KbItem,
  type StickySelf,
} from '@/lib/api'
import { PaperPattern } from './paper-patterns'
import { MdStaticRenderer } from '@/rendering/md-static-render'
import { TipTapEditor } from '@/rendering/tiptap-editor'
import './sticky.css'

const FREE_MAX = 500

/**
 * 便签窗（sticky-separation 定稿）：纯自由便签，一张纸，多实例，常驻置顶无开关。
 * - 身份由后端 sticky_self 给出（note:* 动态窗）
 * - 展开态 = 纸片：点击即写、失焦保存（≤500 字），不进今天不参与提醒
 * - 缩小态 = 置顶悬浮文本条：正文第一行，点文本展开、⠿ 拖动、✕ 销毁
 * - 关闭即销毁（一次性工具语义）：✕ 直接 delete_sticky，无确认无撤销，
 *   误关靠写前轮转备份兜底
 */
export default function StickyWindow() {
  const [boot, setBoot] = useState<Bootstrap | null>(null)
  const [self, setSelf] = useState<StickySelf | null>(null)
  const [faded, setFaded] = useState(false)
  const [freeText, setFreeText] = useState('')
  const [mdPreview, setMdPreview] = useState(false)
  const [kbItem, setKbItem] = useState<KbItem | null>(null)
  const [kbInvalid, setKbInvalid] = useState(false)
  const freeRef = useRef<HTMLTextAreaElement>(null)
  const knowledgeBodyRef = useRef<HTMLDivElement>(null)

  // 知识便签：加载 KB 条目（实时视口）。
  // 编辑中（焦点在本窗编辑器内）跳过重拉——防止正在打的字被外部刷新回灌覆盖。
  const isKnowledge = !!self?.kbRef
  const kbRef = self?.kbRef
  const loadKbItem = useCallback(async () => {
    if (!kbRef) return
    const active = document.activeElement
    if (active instanceof Node && knowledgeBodyRef.current?.contains(active)) return
    try {
      const r = await getKbItems()
      const items = r.data ?? []
      const found = items.find((i) => i.id === kbRef)
      if (found) {
        setKbItem(found)
        setKbInvalid(false)
      } else {
        setKbInvalid(true)
      }
    } catch {
      setKbInvalid(true)
    }
  }, [kbRef])

  useEffect(() => {
    setKbItem(null)
    setKbInvalid(false)
    void loadKbItem()
  }, [loadKbItem])

  const refresh = useCallback(async () => {
    const [b, s] = await Promise.all([getBootstrap(), stickySelf()])
    if (!b.err && b.data) {
      setBoot(b.data)
      applyTheme(b.data.settings.theme)
    }
    if (!s.err && s.data) {
      setSelf(s.data)
      // 编辑中不回灌内容（避免打字被刷新打断）
      if (document.activeElement !== freeRef.current) setFreeText(s.data.content)
    }
    // 知识便签：store-changed（主窗编辑 KB）→ 同步视口内容
    void loadKbItem()
  }, [loadKbItem])

  useEffect(() => {
    void refresh()
    const off = onEvent('store-changed', () => { void refresh() })
    const offTheme = onEvent('theme-changed', (payload: unknown) => applyTheme(payload as string))
    return () => { off(); offTheme() }
  }, [refresh])

  const paper = boot?.settings.stickyPaper ?? 'warm'
  const fadeOn = boot?.settings.stickyFade ?? true
  const fadeOpacity = boot?.settings.stickyFadeOpacity ?? 38
  const pattern = boot?.settings.stickyPattern ?? 'none'
  const mini = self?.mini ?? false

  /** 缩小 ↔ 展开：形态记在便签数据里，重启按原样恢复 */
  const setMini = useCallback(async (next: boolean) => {
    if (!self) return
    const r = await updateSticky(self.id, { mini: next })
    if (!r.err) await refresh()
  }, [self, refresh])

  /** ✕ = 关闭即销毁（一次性工具语义）：数据与窗口一并回收 */
  const close = useCallback(async () => {
    if (!self) return
    await deleteSticky(self.id)
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

  /* ---- 缩小态：置顶悬浮文本条（380×40）---- */
  if (mini) {
    const displayName = isKnowledge
      ? (kbItem?.title ?? '加载中...')
      : ((self?.content ?? '').split('\n')[0].trim() || '（空便签）')
    return (
      <div
        className={`sticky-note is-mini paper-${paper}${isKnowledge ? ' knowledge-sticky' : ''}`}
        onMouseLeave={() => setFaded(true)}
        onMouseEnter={() => setFaded(false)}
      >
        <span className="sticky-mini-grip" data-tauri-drag-region title="按住拖动">
          <GripHorizontal size={14} aria-hidden />
        </span>
        <button
          className="sticky-mini-text"
          title="点击展开"
          aria-label={`展开便签：${displayName}`}
          onClick={() => { void setMini(false) }}
        >
          {isKnowledge && '📎 '}{displayName}
        </button>
        <button className="sticky-btn" title="关闭并删除" aria-label="关闭并删除便签" onClick={() => { void close() }}>
          <X size={12} aria-hidden />
        </button>
      </div>
    )
  }

  /* ---- 展开态：纸片 ---- */
  // 知识便签失效态
  if (isKnowledge && kbInvalid) {
    return (
      <div className={`sticky-note paper-${paper} knowledge-sticky is-invalid`}>
        <PaperPattern pattern={pattern} />
        <div className="sticky-strip">
          <span className="sticky-head" data-tauri-drag-region title="按住拖动">
            ⚠️ 已失效（原资料已删除）
          </span>
          <button className="sticky-btn" title="拆除便签" aria-label="拆除便签" onClick={() => { void close() }}>
            <X size={14} aria-hidden />
          </button>
        </div>
        <div className="sticky-body">
          <div className="sticky-invalid-body">
            <span>此知识条目已被删除</span>
            <button
              className="sticky-md-btn on"
              onClick={() => {
                void trackEvent('kb_unpin_desktop', { itemId: self?.kbRef ?? '', reason: 'invalid' })
                void close()
              }}
            >
              拆除便签
            </button>
          </div>
        </div>
      </div>
    )
  }

  // 知识便签正常态
  if (isKnowledge && kbItem) {
    return (
      <div
        className={`sticky-note paper-${paper} knowledge-sticky${faded && fadeOn ? ' faded' : ''}`}
        style={{ '--sticky-fade-opacity': `${fadeOpacity}%` } as React.CSSProperties}
        onMouseLeave={() => setFaded(true)}
        onMouseEnter={() => setFaded(false)}
      >
        <PaperPattern pattern={pattern} />
        <div className="sticky-strip">
          <span className="sticky-head" data-tauri-drag-region title="按住拖动">
            📎 {kbItem.title}
          </span>
          <button className="sticky-btn" title="缩小为悬浮条" aria-label="缩小便签" onClick={() => { void setMini(true) }}>
            <Minus size={14} aria-hidden />
          </button>
          <button
            className="sticky-btn"
            title="关闭便签（数据保留在知识库）"
            aria-label="关闭便签"
            onClick={() => {
              void trackEvent('kb_unpin_desktop', { itemId: kbItem.id, reason: 'user' })
              void close()
            }}
          >
            <X size={14} aria-hidden />
          </button>
        </div>

        <div className="sticky-body" ref={knowledgeBodyRef}>
          <TipTapEditor
            markdown={kbItem.bodyMd}
            onSave={(md) => { void updateKbItem(kbItem.id, { bodyMd: md }) }}
            placeholder="开始写知识..."
          />
        </div>
        <i className="sticky-fold" aria-hidden />
      </div>
    )
  }

  // 自由便签展开态
  return (
    <div
      className={`sticky-note paper-${paper}${faded && fadeOn ? ' faded' : ''}`}
      style={{ '--sticky-fade-opacity': `${fadeOpacity}%` } as React.CSSProperties}
      onMouseLeave={() => setFaded(true)}
      onMouseEnter={() => setFaded(false)}
    >
      <PaperPattern pattern={pattern} />
      <div className="sticky-strip">
        <span className="sticky-head" data-tauri-drag-region title="按住拖动">
          便签{freeText.trim() && ' · 已保存'}
        </span>
        <button
          className={`sticky-md-btn${mdPreview ? ' on' : ''}`}
          title={mdPreview ? '返回编辑' : '切换 Markdown 预览'}
          aria-pressed={mdPreview}
          onClick={() => {
            const next = !mdPreview
            setMdPreview(next)
            void trackEvent('sticky_md_toggle', { on: next })
          }}
        >
          MD
        </button>
        <button className="sticky-btn" title="缩小为悬浮条" aria-label="缩小便签" onClick={() => { void setMini(true) }}>
          <Minus size={14} aria-hidden />
        </button>
        <button className="sticky-btn" title="关闭并删除（一次性便签）" aria-label="关闭并删除便签" onClick={() => { void close() }}>
          <X size={14} aria-hidden />
        </button>
      </div>

      <div className="sticky-body">
        {mdPreview ? (
          <div
            className="sticky-free md-preview"
            onClick={() => setMdPreview(false)}
            title="点击回到编辑"
            style={{ cursor: 'pointer', minHeight: 80 }}
          >
            <MdStaticRenderer markdown={freeText} />
          </div>
        ) : (
          <>
            <textarea
              ref={freeRef}
              className="sticky-free"
              value={freeText}
              maxLength={FREE_MAX}
              placeholder="自由写点什么…（失焦自动保存）"
              aria-label="便签内容"
              onChange={(e) => setFreeText(e.target.value.slice(0, FREE_MAX))}
              onBlur={() => { void commitFree() }}
            />
            <span className="sticky-count tiny">{freeText.length}/{FREE_MAX}</span>
          </>
        )}
      </div>
      <i className="sticky-fold" aria-hidden />
    </div>
  )
}
