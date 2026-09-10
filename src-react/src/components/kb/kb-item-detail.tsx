import { useState, useCallback, useEffect } from 'react'
import type { KbItem } from '@/lib/api'
import { updateKbItem, trackEvent } from '@/lib/api'
import { Pin, ClipboardList, Trash2, X, Plus } from 'lucide-react'
import { TipTapEditor } from '@/rendering/tiptap-editor'

interface Props {
  item: KbItem
  /** 被任务引用数（删除确认提示用，qa-8） */
  refCount?: number
  onDeleted: (id: string) => void
  /** 可选：贴到桌面 */
  onPin?: (item: KbItem) => void
  /** 可选：转为待办 */
  onCreateTask?: (item: KbItem) => void
  /** 刷新父列表 */
  onRefresh: () => void
}

/**
 * KB 右详情面板（editor-polish-spec：单一即时渲染画布，无阅读/编辑模式）。
 * 布局：动作栏（贴桌面/转待办/删除）→ 标题就地编辑 + 保存指示 + 标签增删 → 常驻 WYSIWYG 画布。
 * bodyMd 仍是唯一真相——输入防抖 800ms 写回 updateKbItem。
 */
export function KbItemDetail({ item, refCount = 0, onDeleted, onPin, onCreateTask, onRefresh }: Props) {
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false)
  const [saveState, setSaveState] = useState<{ saved: boolean; time: string }>({
    saved: true,
    time: formatTime(item.updatedAt),
  })

  // 标题就地编辑
  const [title, setTitle] = useState(item.title)
  useEffect(() => { setTitle(item.title) }, [item.id, item.title])

  const saveTitle = useCallback(async () => {
    const next = title.trim()
    if (!next || next === item.title) return
    await updateKbItem(item.id, { title: next })
    onRefresh()
  }, [item.id, item.title, title, onRefresh])

  // 标签增删
  const [tagInput, setTagInput] = useState('')
  const [tagEditing, setTagEditing] = useState(false)
  const saveTags = useCallback(async (tags: string[]) => {
    await updateKbItem(item.id, { tags })
    onRefresh()
  }, [item.id, onRefresh])

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
      await updateKbItem(item.id, { bodyMd })
      setSaveState({ saved: true, time: formatTime(new Date().toISOString()) })
      onRefresh()
    },
    [item.id, onRefresh],
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

  return (
    <div className="kb-detail">
      {/* 动作栏 */}
      <div className="kb-detail-toolbar">
        {onPin && (
          <button className="btn xs ghost" onClick={() => onPin(item)} title="贴到桌面">
            <Pin size={13} strokeWidth={1.7} /> 贴到桌面
          </button>
        )}
        {onCreateTask && (
          <button className="btn xs ghost" onClick={() => onCreateTask(item)} title="转为待办">
            <ClipboardList size={13} strokeWidth={1.7} /> 转为待办
          </button>
        )}
        <span style={{ flex: 1 }} />
        {!showDeleteConfirm ? (
          <button className="btn xs ghost" onClick={() => setShowDeleteConfirm(true)} title="删除">
            <Trash2 size={13} strokeWidth={1.7} />
          </button>
        ) : (
          <>
            <span className="kb-del-confirm">
              {refCount > 0 ? `有 ${refCount} 处引用将悬空，` : ''}确认删除？
            </span>
            <button className="btn xs danger" onClick={handleDelete}>确认</button>
            <button className="btn xs ghost" onClick={() => setShowDeleteConfirm(false)}>取消</button>
          </>
        )}
      </div>

      {/* 标题（就地编辑）+ 保存指示 */}
      <div className="kb-meta">
        <div className="kb-title-row">
          <input
            className="kb-title-input"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            onBlur={saveTitle}
            onKeyDown={(e) => {
              if (e.key === 'Enter') { e.currentTarget.blur() }
              if (e.key === 'Escape') { setTitle(item.title); e.currentTarget.blur() }
            }}
            aria-label="条目标题"
          />
          <span className={'kb-save-pill' + (saveState.saved ? '' : ' editing')}>
            <span className="dot" />{saveState.saved ? `已保存 ${saveState.time}` : '编辑中…'}
          </span>
        </div>

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
            <input
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
        <div className="kb-meta-sep" />
      </div>

      {/* 常驻即时渲染画布（无模式） */}
      <div className="kb-detail-body">
        <TipTapEditor
          markdown={item.bodyMd}
          onSave={handleSave}
          onDirty={handleDirty}
          placeholder="写点什么…（输入 / 唤起插入菜单）"
        />
      </div>
    </div>
  )
}

function formatTime(iso: string): string {
  try {
    return String(iso).slice(11, 16)
  } catch {
    return ''
  }
}
