import { useState, useCallback, useEffect, useLayoutEffect, useRef } from 'react'
import type { KeyboardEvent as ReactKeyboardEvent, ClipboardEvent as ReactClipboardEvent } from 'react'
import type { KbItem } from '@/lib/api'
import { updateKbItem, trackEvent, getSettings, setSettings } from '@/lib/api'
import { Pin, Trash2, X, Plus, Type, FileCode2, Check, PanelRight } from 'lucide-react'
import { TipTapEditor, type EditorApi } from '@/rendering/tiptap-editor'
import { applyReadingPrefs } from '@/lib/reading-prefs'
import type { OutlineItem } from '@/lib/doc-outline'
import { TypoPopover } from './typo-popover'
import { Popover, PopoverTrigger } from '@/components/ui/popover'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { DocOutline } from './doc-outline'

interface Props {
  item: KbItem
  /** 被任务引用数（删除确认提示用，qa-8） */
  refCount?: number
  onDeleted: (id: string) => void
  /** 可选：贴到桌面 */
  onPin?: (item: KbItem) => void
  /** 保存成功回填：父级就地更新 items，不触发全量重拉（#1 保存闪烁） */
  onSaved?: (item: KbItem) => void
  /**
   * 新建即命名 / 右键「打开·重命名」的焦点信号（tasks 2.6）：
   * 每次自增即请求把焦点放进标题并全选；只消费大于已消费值的信号，不抢用户后续焦点。
   */
  focusTitleSignal?: number
}

/** 页边大纲占位：176px 栏 + 12px 间距。宽度从这里让，正文列随之收窄，而不是被大纲盖住。 */
const OUTLINE_TRACK = 188

/** 让出大纲后正文仍低于这个宽度（700px ≈ 87 字符）就不并排，改用抽屉。 */
const MIN_TEXT_WIDTH = 700

/** 本地日期时间戳（YYYY-MM-DDTHH:mm；全链路本地语义字符串，禁止 UTC/ISO-Z，见 AGENTS.md 4.1） */
function localStamp(): string {
  const d = new Date()
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`
}

/** 保存回包缺失时的本地补丁（#1：保存后局部回填，不再全量 refresh） */
function withLocalPatch(base: KbItem, patch: Partial<KbItem>): KbItem {
  return { ...base, ...patch, updatedAt: localStamp() }
}

/**
 * KB 右详情面板（editor-polish-spec：单一即时渲染画布，无阅读/编辑模式）。
 * 布局：动作栏（贴桌面/删除）→ 标题就地编辑 + 保存指示 + 标签增删 → 常驻 WYSIWYG 画布。
 * bodyMd 仍是唯一真相——输入防抖 800ms 写回 updateKbItem；保存回包经 onSaved 局部回填父列表。
 * 动作收敛（tasks 2.7）：「所属文件夹」下拉与「转为待办」已移除，移动走左树右键菜单。
 */
export function KbItemDetail({ item, refCount = 0, onDeleted, onPin, onSaved, focusTitleSignal }: Props) {
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false)
  const [saveState, setSaveState] = useState<{ saved: boolean; time: string }>({
    saved: true,
    time: formatTime(item.updatedAt),
  })

  // 文档标题（方案 A）：正文首行就地编辑，顶部只保留只读镜像。
  // 标题仍只存条目名一个字段；不进 Markdown 文档树，复制/导出时才在首行拼 `# 标题`。
  const [title, setTitle] = useState(item.title)
  const titleElRef = useRef<HTMLDivElement>(null)
  /** 已消费的焦点信号：只在信号前进时聚焦，避免重渲染/切条目时抢焦点（初始值=挂载时的信号） */
  const consumedFocusSignal = useRef(focusTitleSignal ?? 0)

  /** contentEditable 不吃受控 children：标题文本靠这个单向刷回 DOM */
  const paintTitle = useCallback((value: string) => {
    const el = titleElRef.current
    if (el && el.textContent !== value) el.textContent = value
  }, [])

  /** 外部改名（树里重命名）或切条目：渲染期就地校正，不走 effect（避免多一轮渲染） */
  const [syncedTitle, setSyncedTitle] = useState(item.title)
  if (syncedTitle !== item.title) {
    setSyncedTitle(item.title)
    setTitle(item.title)
  }

  // 编辑中不覆盖用户输入；切条目或外部改名才把 DOM 重画
  useEffect(() => {
    if (document.activeElement !== titleElRef.current) paintTitle(item.title)
  }, [item.id, item.title, paintTitle])

  /** 聚焦并全选整条标题：改名不用三击，落点即替换 */
  const focusDocTitle = useCallback(() => {
    const el = titleElRef.current
    if (!el) return
    el.focus()
    const range = document.createRange()
    range.selectNodeContents(el)
    const sel = window.getSelection()
    sel?.removeAllRanges()
    sel?.addRange(range)
  }, [])

  useEffect(() => {
    if (!focusTitleSignal || focusTitleSignal <= consumedFocusSignal.current) return
    consumedFocusSignal.current = focusTitleSignal
    focusDocTitle()
  }, [focusTitleSignal, focusDocTitle])

  const saveTitle = useCallback(async (raw?: string) => {
    const next = (raw ?? title).trim()
    if (!next || next === item.title) return
    const result = await updateKbItem(item.id, { title: next })
    onSaved?.(result.data ?? withLocalPatch(item, { title: next }))
  }, [item, title, onSaved])

  /** 失焦落盘；空标题不落盘，恢复原名（顶部镜像与树里都不会出现空名） */
  const commitDocTitle = useCallback(() => {
    const el = titleElRef.current
    const raw = (el?.textContent ?? '').replace(/\s+/g, ' ').trim()
    if (!raw) {
      paintTitle(item.title)
      setTitle(item.title)
      return
    }
    paintTitle(raw)
    setTitle(raw)
    void saveTitle(raw)
  }, [item.title, paintTitle, saveTitle])

  const onDocTitleKeyDown = useCallback((e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      titleElRef.current?.blur() // blur 即落盘
      apiRef.current?.focusStart() // 光标进正文第一段
      return
    }
    if (e.key === 'Escape') {
      e.preventDefault()
      paintTitle(item.title)
      setTitle(item.title)
      titleElRef.current?.blur()
    }
  }, [item.title, paintTitle])

  /** 粘贴只取单行纯文本：标题不接收换行与富文本 */
  const onDocTitlePaste = useCallback((e: ReactClipboardEvent<HTMLDivElement>) => {
    e.preventDefault()
    const text = e.clipboardData.getData('text/plain').replace(/\s+/g, ' ').trim()
    if (text) document.execCommand('insertText', false, text)
  }, [])

  // 标签增删
  const [tagInput, setTagInput] = useState('')
  const [tagEditing, setTagEditing] = useState(false)
  const saveTags = useCallback(async (tags: string[]) => {
    const result = await updateKbItem(item.id, { tags })
    onSaved?.(result.data ?? withLocalPatch(item, { tags }))
  }, [item, onSaved])

  const addTag = useCallback(() => {
    const t = tagInput.trim().replace(/^#/, '')
    if (!t) { setTagEditing(false); return }
    if (!item.tags.includes(t)) {
      void saveTags([...item.tags, t])
    }
    setTagInput('')
    setTagEditing(false)
  }, [tagInput, item.tags, saveTags])

  const handleSave = useCallback(
    async (bodyMd: string) => {
      const result = await updateKbItem(item.id, { bodyMd })
      // 优先用服务端更新后的条目；回包为空时用本地已保存 bodyMd 构造补丁
      const saved = result.data ?? withLocalPatch(item, { bodyMd })
      setSaveState({ saved: true, time: formatTime(saved.updatedAt) })
      onSaved?.(saved)
    },
    [item, onSaved],
  )

  const handleDirty = useCallback(() => {
    setSaveState((s) => (s.saved ? { saved: false, time: s.time } : s))
  }, [])

  const handleDelete = useCallback(async () => {
    const { deleteKbItem } = await import('@/lib/api')
    await deleteKbItem(item.id)
    void trackEvent('kb_delete', { itemId: item.id })
    onDeleted(item.id)
    setShowDeleteConfirm(false)
  }, [item.id, onDeleted])

  // ── 文档工具条：工具栏挂载点 / 排版弹层 / 复制反馈 / 大纲 ──
  const stripRef = useRef<HTMLDivElement>(null)
  const scrollRef = useRef<HTMLDivElement>(null)
  const innerRef = useRef<HTMLDivElement>(null)
  const mainRef = useRef<HTMLDivElement>(null)
  const outlineBtnRef = useRef<HTMLButtonElement>(null)
  const drawerRef = useRef<HTMLDivElement>(null)
  const [toolbarHost, setToolbarHost] = useState<HTMLDivElement | null>(null)
  const [typoOpen, setTypoOpen] = useState(false)
  const [copied, setCopied] = useState(false)
  /** null = 还没手动开关过：默认按「有没有页边留白给大纲」自动决定（首进窄窗不会弹抽屉） */
  const [outlinePref, setOutlinePref] = useState<boolean | null>(null)
  const [railFits, setRailFits] = useState(false)
  const [outline, setOutline] = useState<OutlineItem[]>([])
  const [activeHeading, setActiveHeading] = useState(-1)
  const [prefs, setPrefs] = useState({ size: 15, tracking: 0, measure: 'comfortable' })
  const apiRef = useRef<EditorApi | null>(null)
  const copyTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const prefsTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const pendingPrefs = useRef<{ readingSize?: number; readingTracking?: number; readingMeasure?: string }>({})

  // 排版档位是全局设置：这里只读一次，后续改动由本组件的滑块写回
  useEffect(() => {
    let alive = true
    void (async () => {
      const r = await getSettings()
      if (!alive || r.err || !r.data) return
      setPrefs({ size: r.data.readingSize, tracking: r.data.readingTracking, measure: r.data.readingMeasure })
    })()
    return () => { alive = false }
  }, [])

  // 页边大纲：轨道恒占 176px 栏（+12px 间距）= 188px，开关只切可见性，不重排列。
  // 只有余下正文连 700px 都保证不了时，才退回抽屉形态（轨道整体不占位）。
  // 判据是容器实测宽（不看窗口、也不看开关）：树折叠、字号档位、列宽档位都会改这个数。
  useLayoutEffect(() => {
    const inner = innerRef.current
    if (!inner) return
    const measure = () => {
      setRailFits(inner.getBoundingClientRect().width - OUTLINE_TRACK >= MIN_TEXT_WIDTH)
    }
    measure()
    const ro = new ResizeObserver(measure)
    ro.observe(inner)
    return () => ro.disconnect()
  }, [])

  // 大纲高亮跟随滚动：取最后一个越过分隔线的标题（滚动容器是正文区，不含工具条）
  useEffect(() => {
    const scroller = scrollRef.current
    const content = scroller?.querySelector('.kb-editor-content')
    if (!scroller || !content) return
    const onScroll = () => {
      const headings = content.querySelectorAll('h1, h2, h3')
      const line = scroller.getBoundingClientRect().top + 24
      let next = -1
      headings.forEach((el, i) => {
        if (el.getBoundingClientRect().top <= line) next = i
      })
      setActiveHeading(next)
    }
    onScroll()
    scroller.addEventListener('scroll', onScroll, { passive: true })
    return () => scroller.removeEventListener('scroll', onScroll)
  }, [outline, railFits])

  useEffect(() => () => {
    if (copyTimer.current) clearTimeout(copyTimer.current)
    if (prefsTimer.current) clearTimeout(prefsTimer.current)
  }, [])

  // 档位先本地生效（文档立刻变），写入合并 220ms，避免拖滑块时连打后端
  const changePrefs = useCallback((patch: { readingSize?: number; readingTracking?: number; readingMeasure?: string }) => {
    const next = {
      size: patch.readingSize ?? prefs.size,
      tracking: patch.readingTracking ?? prefs.tracking,
      measure: patch.readingMeasure ?? prefs.measure,
    }
    setPrefs(next)
    applyReadingPrefs({ readingSize: next.size, readingTracking: next.tracking, readingMeasure: next.measure })
    pendingPrefs.current = { ...pendingPrefs.current, ...patch }
    if (prefsTimer.current) clearTimeout(prefsTimer.current)
    prefsTimer.current = setTimeout(() => {
      const pending = pendingPrefs.current
      pendingPrefs.current = {}
      void setSettings(pending)
    }, 220)
  }, [prefs])

  const copyMarkdown = useCallback(async () => {
    const body = apiRef.current?.getMarkdown() ?? ''
    const heading = title.trim()
    // 标题不在 bodyMd 里（方案 A）：复制时补首行，粘出去才是一篇完整 Markdown
    const md = heading ? `# ${heading}\n\n${body}`.trimEnd() : body
    if (!md) return
    let ok = false
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(md)
        ok = true
      }
    } catch { ok = false }
    if (!ok) {
      // 剪贴板权限被拒时的兜底：临时 textarea + execCommand
      const ta = document.createElement('textarea')
      ta.value = md
      ta.setAttribute('readonly', 'true')
      ta.style.position = 'fixed'
      ta.style.opacity = '0'
      document.body.appendChild(ta)
      ta.select()
      try { ok = document.execCommand('copy') } catch { ok = false }
      ta.remove()
    }
    if (!ok) return
    setCopied(true)
    if (copyTimer.current) clearTimeout(copyTimer.current)
    copyTimer.current = setTimeout(() => setCopied(false), 1600)
  }, [title])

  /** 用户没手动开合过时：只有页边留白够宽才默认打开（避免首进窄窗弹出抽屉） */
  const outlineOpen = outlinePref ?? railFits
  const drawerOpen = outlineOpen && !railFits

  // 抽屉：打开时把焦点移进去，Esc 关闭，关闭后焦点回到大纲按钮
  const closeOutline = useCallback(() => {
    setOutlinePref(false)
    outlineBtnRef.current?.focus()
  }, [])

  useEffect(() => {
    if (!drawerOpen) return
    const drawer = drawerRef.current
    drawer?.focus()
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation()
        closeOutline()
      }
    }
    document.addEventListener('keydown', onKeyDown, true)
    return () => document.removeEventListener('keydown', onKeyDown, true)
  }, [drawerOpen, closeOutline])

  return (
    <>
      {/* 文档工具条分两行：第一行 文件名 + 保存时间（左）/ 文档动作（右）；第二行 格式工具栏 */}
      <div className="kb-detail-strip" ref={stripRef}>
        {/* 第一行左端只读镜像：标题在正文首行改，这里只回答「现在打开的是哪一条」。
            headline 一层只负责让文件名与保存时间共基线，外层 head 负责把这组居中。 */}
        <div className="kb-strip-head">
          <span className="kb-strip-headline">
            <span className="kb-strip-ro" title={title.trim() || '未命名文档'}>
              <span className="kb-strip-ro-text">{title.trim() || '未命名文档'}</span>
            </span>
            <span className={'kb-strip-saved' + (saveState.saved ? '' : ' editing')} title={saveState.saved ? '已保存' : '编辑中'}>
              <span className="dot" />{saveState.saved ? `已保存 ${saveState.time}` : '编辑中…'}
            </span>
          </span>
        </div>

        <div className="kb-strip-tools" ref={setToolbarHost} />

        <div className="kb-strip-actions">
          <div className="kb-strip-anchor">
            <Popover open={typoOpen} onOpenChange={setTypoOpen}>
              <PopoverTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-pressed={typoOpen}
                  title="排版"
                  aria-label="排版"
                >
                  <Type size={15} strokeWidth={1.7} />
                </Button>
              </PopoverTrigger>
              <TypoPopover
                value={{ size: prefs.size, tracking: prefs.tracking, measure: prefs.measure }}
                onChange={changePrefs}
              />
            </Popover>
          </div>

          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-pressed={copied}
            title={copied ? '已复制' : '复制 Markdown'}
            aria-label="复制 Markdown"
            onClick={() => { void copyMarkdown() }}
          >
            {copied ? <Check size={15} strokeWidth={1.8} /> : <FileCode2 size={15} strokeWidth={1.7} />}
          </Button>

          <Button
            type="button"
            ref={outlineBtnRef}
            variant="ghost"
            size="icon-sm"
            title={railFits ? '大纲（页边）' : '大纲（右侧抽屉）'}
            aria-label="大纲"
            aria-pressed={outlineOpen}
            aria-expanded={drawerOpen}
            onClick={() => setOutlinePref(!outlineOpen)}
          >
            <PanelRight size={15} strokeWidth={1.7} />
          </Button>

          <span className="kb-strip-sep" aria-hidden="true" />

          {onPin && (
            <Button variant="ghost" size="xs" onClick={() => onPin(item)} title="贴到桌面">
              <Pin size={13} strokeWidth={1.7} /> 贴到桌面
            </Button>
          )}
          {!showDeleteConfirm ? (
            <Button variant="ghost" size="xs" onClick={() => setShowDeleteConfirm(true)} title="删除" aria-label="删除">
              <Trash2 size={13} strokeWidth={1.7} />
            </Button>
          ) : (
            <>
              <span className="kb-del-confirm">
                {refCount > 0 ? `有 ${refCount} 处引用将悬空，` : ''}确认删除？
              </span>
              <Button variant="danger" size="xs" onClick={handleDelete}>确认</Button>
              <Button variant="ghost" size="xs" onClick={() => setShowDeleteConfirm(false)}>取消</Button>
            </>
          )}
          <span className="sr-only" aria-live="polite">{copied ? 'Markdown 已复制' : ''}</span>
        </div>
      </div>

      {/* 正文区独立滚动：滚动条从工具条下缘开始，工具条与标题不会被内容盖住 */}
      <div className="kb-detail-scroll" ref={scrollRef}>
        <div className={'kb-detail-inner' + (railFits ? '' : ' no-rail')} ref={innerRef}>
          <div className="kb-detail-main" ref={mainRef}>
            {/* 文档标题：纸面上的第一行，随文滚动。点进即全选，回车进正文，Esc 复原 */}
            <div
              className="kb-doc-title"
              ref={titleElRef}
              contentEditable
              suppressContentEditableWarning
              role="textbox"
              aria-label="文档标题"
              aria-multiline="false"
              data-placeholder="未命名文档"
              spellCheck={false}
              onInput={(e) => setTitle(e.currentTarget.textContent ?? '')}
              onBlur={commitDocTitle}
              onKeyDown={onDocTitleKeyDown}
              onPaste={onDocTitlePaste}
            />
            {/* 标签（就地增删） */}
            <div className="kb-tag-row">
              {item.tags?.map((t) => (
                <span key={t} className="kb-tag-chip">
                  {t}
                  <button
                    className="kb-tag-x"
                    title="删除标签"
                    aria-label={`删除标签 ${t}`}
                    onClick={() => { void saveTags(item.tags.filter((x) => x !== t)) }}
                  >
                    <X size={10} strokeWidth={2} />
                  </button>
                </span>
              ))}
              {tagEditing ? (
                <Input
                  bare
                  className="kb-tag-input"
                  value={tagInput}
                  autoFocus
                  placeholder="标签名，回车确认"
                  onChange={(e) => setTagInput(e.target.value)}
                  onBlur={addTag}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') addTag()
                    if (e.key === 'Escape') { setTagInput(''); setTagEditing(false) }
                  }}
                />
              ) : (
                <button className="kb-tag-add" onClick={() => setTagEditing(true)}>
                  <Plus size={11} strokeWidth={1.7} /> 标签
                </button>
              )}
            </div>

            {/* 常驻即时渲染画布（无模式） */}
            <div className="kb-detail-body">
              <TipTapEditor
                markdown={item.bodyMd}
                onSave={handleSave}
                onDirty={handleDirty}
                placeholder="写点什么…（输入 / 唤起插入菜单）"
                allowImages
                toolbarHost={toolbarHost}
                onReady={(api) => { apiRef.current = api }}
                onOutline={setOutline}
              />
            </div>
          </div>

          {/* 页边批注式大纲：轨道恒占 188px，收起时只隐藏不撤走——开合不移动正文 */}
          {railFits && (
            <DocOutline
              items={outline}
              active={activeHeading}
              hidden={!outlineOpen}
              onPick={(index) => apiRef.current?.scrollToHeading(index)}
            />
          )}
        </div>
      </div>

      {/* 窄窗：同一份大纲换右侧抽屉，遮罩点击 / Esc 关闭，选条目后收起 */}
      {drawerOpen && (
        <div className="kb-outline-layer">
          <div className="kb-outline-mask" onMouseDown={closeOutline} aria-hidden="true" />
          <div
            className="kb-outline-panel"
            role="dialog"
            aria-modal="true"
            aria-label="文档大纲"
            tabIndex={-1}
            ref={drawerRef}
          >
            <DocOutline
              items={outline}
              active={activeHeading}
              onPick={(index) => {
                apiRef.current?.scrollToHeading(index)
                closeOutline()
              }}
              variant="drawer"
              onClose={closeOutline}
            />
          </div>
        </div>
      )}
    </>
  )
}

function formatTime(iso: string): string {
  try {
    return String(iso).slice(11, 16)
  } catch {
    return ''
  }
}
