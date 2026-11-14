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
import { TipTapEditor } from '@/rendering/tiptap-editor'
import './sticky.css'

/**
 * Markdown → 迷你条纯文本摘要：去行首标记、去行内语法、取首个非空行、截 60 字。
 * 不引依赖：迷你条只有 40px，`#` / `**` / 链接语法原样露出会很难看。
 */
function plainPreview(md: string): string {
  const firstLine = (md || '').split(/\r?\n/).find((line) => line.trim().length > 0) ?? ''
  const text = firstLine
    .replace(/^\s*#{1,6}\s*/, '') // 标题
    .replace(/^\s*>\s?/, '') // 引用
    .replace(/^\s*(?:[-*+]|\d+[.)])\s+/, '') // 列表
    .replace(/^\s*\[[ xX]\]\s*/, '') // 任务清单勾选标记
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1') // 图片 → alt
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1') // 链接 → 链接文字
    .replace(/(\*\*|__|~~)(.*?)\1/g, '$2') // 粗体 / 删除线
    .replace(/(\*|_)(.*?)\1/g, '$2') // 斜体
    .replace(/`{1,3}([^`]*)`{1,3}/g, '$1') // 行内码 / 代码
    .replace(/\s+/g, ' ')
    .trim()
  return text.length > 60 ? `${text.slice(0, 60)}…` : text
}

/**
 * 便签窗（sticky-separation 定稿）：纯自由便签，一张纸，多实例，常驻置顶无开关。
 * - 身份由后端 sticky_self 给出（note:* 动态窗）
 * - 展开态 = 纸片：点击即写、防抖自动保存（≤500 字由编辑器计数把关），不进今天不参与提醒
 * - 缩小态 = 置顶悬浮文本条：正文第一行（Markdown 剥成纯文本），点文本展开、⠿ 拖动、✕ 关闭
 * - 关闭 = 隐藏不删（visible=false）：窗口收走、内容保留，设置页可重新打开；可见便签合计上限 5
 */
export default function StickyWindow() {
  const [boot, setBoot] = useState<Bootstrap | null>(null)
  const [self, setSelf] = useState<StickySelf | null>(null)
  const [faded, setFaded] = useState(false)
  const [freeText, setFreeText] = useState('')
  const [saveErr, setSaveErr] = useState<string | null>(null)
  const [kbItem, setKbItem] = useState<KbItem | null>(null)
  const [kbInvalid, setKbInvalid] = useState(false)
  const freeBodyRef = useRef<HTMLDivElement>(null)
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
      const active = document.activeElement
      const inFreeEditor = active instanceof Node && freeBodyRef.current?.contains(active)
      if (!inFreeEditor) setFreeText(s.data.content)
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

  /** ✕ = 关闭（隐藏不删）：窗口收走、内容保留，设置 → 便签里可重新打开；可见上限 5 由后端把关 */
  const close = useCallback(async () => {
    if (!self) return
    await updateSticky(self.id, { visible: false })
  }, [self])

  /** 知识便签失效态的「拆除便签」= 显式销毁（与 ✕ 的隐藏不删区分：残缺便签没有保留价值） */
  const removeBroken = useCallback(async () => {
    if (!self) return
    void trackEvent('kb_unpin_desktop', { itemId: self.kbRef ?? '', reason: 'invalid' })
    await deleteSticky(self.id)
  }, [self])

  /**
   * TipTapEditor 防抖落盘（入参为序列化后的 Markdown）。
   * 失败时把后端文案（如「便签最多 500 字」）留在窗口内，不静默丢内容；下次键入时清掉。
   */
  const commitFree = useCallback(async (md: string) => {
    if (!self) return
    const next = md.trim()
    if (next === self.content.trim()) return
    const r = await updateSticky(self.id, { content: next })
    if (r.err) {
      setSaveErr(r.err)
      return
    }
    setSaveErr(null)
    // metric：自由便签是否真被写（字数桶）
    const bucket = next.length === 0 ? '0' : next.length <= 50 ? '1-50' : next.length <= 200 ? '51-200' : '200+'
    void trackEvent('sticky_free_edit', { len_bucket: bucket })
    await refresh()
  }, [self, refresh])

  /* ---- 缩小态：置顶悬浮文本条（380×40）---- */
  if (mini) {
    const displayName = isKnowledge
      ? (kbInvalid ? '⚠️ 已失效的知识便签' : (kbItem?.title ?? '加载中...'))
      : (plainPreview(self?.content ?? '') || '（空便签）')
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
        <button
          className="sticky-btn"
          title="关闭（内容保留，可在设置中重新打开）"
          aria-label="关闭便签（内容保留）"
          onClick={() => { void close() }}
        >
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
          <button
            className="sticky-btn"
            title="拆除便签（销毁数据）"
            aria-label="拆除便签"
            onClick={() => { void removeBroken() }}
          >
            <X size={14} aria-hidden />
          </button>
        </div>
        <div className="sticky-body">
          <div className="sticky-invalid-body">
            <span>此知识条目已被删除</span>
            <button className="sticky-md-btn on" onClick={() => { void removeBroken() }}>
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
            title="关闭（内容保留，可在设置中重新打开）"
            aria-label="关闭便签（内容保留）"
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

  // 自由便签展开态：TipTap 所见即所得，防抖自动保存（字数上限由编辑器 maxChars 计数把关）
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
          便签
        </span>
        <button className="sticky-btn" title="缩小为悬浮条" aria-label="缩小便签" onClick={() => { void setMini(true) }}>
          <Minus size={14} aria-hidden />
        </button>
        <button
          className="sticky-btn"
          title="关闭（内容保留，可在设置中重新打开）"
          aria-label="关闭便签（内容保留）"
          onClick={() => { void close() }}
        >
          <X size={14} aria-hidden />
        </button>
      </div>

      <div className="sticky-body" ref={freeBodyRef}>
        {saveErr && (
          <div
            className="sticky-save-err"
            role="alert"
            style={{ color: 'hsl(var(--destructive))', fontSize: 12, lineHeight: 1.6, padding: '0 2px 6px' }}
          >
            ⚠ {saveErr}
          </div>
        )}
        <TipTapEditor
          markdown={freeText}
          onSave={(md) => { void commitFree(md) }}
          onDirty={() => setSaveErr(null)}
          autoSaveMs={600}
          maxChars={500}
          allowTables={false}
          placeholder="写点什么…"
        />
      </div>
      <i className="sticky-fold" aria-hidden />
    </div>
  )
}