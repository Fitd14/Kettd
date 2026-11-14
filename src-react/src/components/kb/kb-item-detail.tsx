import { useState, useCallback, useEffect, useMemo } from 'react'
import type { KbFolder, KbItem } from '@/lib/api'
import { updateKbItem, trackEvent } from '@/lib/api'
import { Folder, Pin, ClipboardList, Trash2, X, Plus } from 'lucide-react'
import { TipTapEditor } from '@/rendering/tiptap-editor'
import { flattenFolders } from '@/components/kb/kb-folder-tree'

interface Props {
  item: KbItem
  /** 被任务引用数（删除确认提示用，qa-8） */
  refCount?: number
  /** 文件夹树（任务 #4「所属文件夹」下拉用；缺失时只显示「未分类」） */
  folders?: KbFolder[]
  onDeleted: (id: string) => void
  /** 可选：贴到桌面 */
  onPin?: (item: KbItem) => void
  /** 可选：转为待办 */
  onCreateTask?: (item: KbItem) => void
  /** 保存成功回填：父级就地更新 items，不触发全量重拉（#1 保存闪烁） */
  onSaved?: (item: KbItem) => void
}

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
 * 布局：动作栏（贴桌面/转待办/删除）→ 标题就地编辑 + 保存指示 + 标签增删 → 常驻 WYSIWYG 画布。
 * bodyMd 仍是唯一真相——输入防抖 800ms 写回 updateKbItem；保存回包经 onSaved 局部回填父列表。
 */
export function KbItemDetail({ item, refCount = 0, folders = [], onDeleted, onPin, onCreateTask, onSaved }: Props) {
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false)
  const [saveState, setSaveState] = useState<{ saved: boolean; time: string }>({
    saved: true,
    time: formatTime(item.updatedAt),
  })

  // 所属文件夹（任务 #4 条目归属）：选中即时写回；失败显示局部错误，select 由受控值回弹
  const [folderSaving, setFolderSaving] = useState(false)
  const [folderError, setFolderError] = useState<string | null>(null)
  const currentFolderId = item.folderId ?? ''
  const folderOptions = useMemo(() => flattenFolders(folders), [folders])
  const currentFolderKnown =
    currentFolderId === '' || folderOptions.some((f) => f.id === currentFolderId)

  const saveFolder = useCallback(
    async (folderId: string) => {
      if (folderSaving) return
      setFolderSaving(true)
      setFolderError(null)
      const result = await updateKbItem(item.id, { folderId })
      if (result.err) {
        setFolderSaving(false)
        setFolderError(result.err)
        return
      }
      setFolderSaving(false)
      onSaved?.(
        result.data ?? withLocalPatch(item, { folderId: folderId === '' ? null : folderId }),
      )
    },
    [folderSaving, item, onSaved],
  )

  // 标题就地编辑
  const [title, setTitle] = useState(item.title)
  useEffect(() => { setTitle(item.title) }, [item.id, item.title])

  const saveTitle = useCallback(async () => {
    const next = title.trim()
    if (!next || next === item.title) return
    const result = await updateKbItem(item.id, { title: next })
    onSaved?.(result.data ?? withLocalPatch(item, { title: next }))
  }, [item, title, onSaved])

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

        {/* 所属文件夹（任务 #4：条目归属；"" = 未分类，选中即写回） */}
        <div className="kb-folder-field">
          <span className="kb-folder-field-label">
            <Folder size={12} strokeWidth={1.7} aria-hidden="true" />
            所属文件夹
          </span>
          <select
            className="kb-folder-select"
            value={currentFolderId}
            disabled={folderSaving}
            aria-label="所属文件夹"
            onChange={(e) => { void saveFolder(e.target.value) }}
          >
            <option value="">未分类</option>
            {!currentFolderKnown && (
              <option value={currentFolderId} disabled>
                未知文件夹
              </option>
            )}
            {folderOptions.map((opt) => (
              <option key={opt.id} value={opt.id}>
                {'\u3000'.repeat(opt.depth) + opt.name}
              </option>
            ))}
          </select>
          {folderSaving && <span className="kb-folder-saving">保存中…</span>}
        </div>
        {folderError && (
          <div className="kb-folder-field-err" role="alert">{folderError}</div>
        )}

        <div className="kb-meta-sep" />
      </div>

      {/* 常驻即时渲染画布（无模式） */}
      <div className="kb-detail-body">
        <TipTapEditor
          markdown={item.bodyMd}
          onSave={handleSave}
          onDirty={handleDirty}
          placeholder="写点什么…（输入 / 唤起插入菜单）"
          allowImages
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
