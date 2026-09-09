import { useState, useCallback } from 'react'
import type { KbItem } from '@/lib/api'
import { updateKbItem } from '@/lib/api'
import { MdStaticRenderer } from '@/rendering/md-static-render'
import { TipTapEditor } from '@/rendering/tiptap-editor'

interface Props {
  item: KbItem
  onDeleted: (id: string) => void
  /** 可选：贴到桌面（Phase 2 接线） */
  onPin?: (item: KbItem) => void
  /** 可选：转为待办（Phase 1 Task 6 接线） */
  onCreateTask?: (item: KbItem) => void
  /** 刷新父列表 */
  onRefresh: () => void
}

/**
 * KB 右详情面板。
 *
 * 双态：阅读态（MdStaticRenderer 全宽渲染）⇄ 编辑态（TipTap WYSIWYG）。
 * 工具栏：阅读/编辑切换 + 贴到桌面(Phase 2) + 转为待办 + 删除（二次确认）。
 * bodyMd 是唯一真相——编辑态防抖 800ms 写回 updateKbItem。
 */
export function KbItemDetail({ item, onDeleted, onPin, onCreateTask, onRefresh }: Props) {
  const [mode, setMode] = useState<'read' | 'edit'>('read')
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false)

  const handleSave = useCallback(
    async (bodyMd: string) => {
      await updateKbItem(item.id, { bodyMd })
      onRefresh()
    },
    [item.id, onRefresh],
  )

  const handleDelete = useCallback(async () => {
    const { deleteKbItem } = await import('@/lib/api')
    await deleteKbItem(item.id)
    onDeleted(item.id)
    setShowDeleteConfirm(false)
  }, [item.id, onDeleted])

  return (
    <div className="kb-detail">
      {/* 工具栏 */}
      <div className="kb-detail-toolbar">
        <button
          className={`btn xs ${mode === 'read' ? 'primary' : 'ghost'}`}
          onClick={() => setMode('read')}
        >
          阅读
        </button>
        <button
          className={`btn xs ${mode === 'edit' ? 'primary' : 'ghost'}`}
          onClick={() => setMode('edit')}
        >
          编辑
        </button>

        <span style={{ flex: 1 }} />

        {onCreateTask && (
          <button className="btn xs ghost" onClick={() => onCreateTask(item)} title="转为待办">
            📝 转为待办
          </button>
        )}

        {onPin && (
          <button className="btn xs ghost" onClick={() => onPin(item)} title="贴到桌面">
            📌 贴到桌面
          </button>
        )}

        {!showDeleteConfirm ? (
          <button
            className="btn xs ghost"
            onClick={() => setShowDeleteConfirm(true)}
            title="删除"
          >
            🗑
          </button>
        ) : (
          <>
            <span style={{ fontSize: '0.8em', color: 'var(--muted-foreground)' }}>确认？</span>
            <button className="btn xs danger" onClick={handleDelete}>确认</button>
            <button className="btn xs ghost" onClick={() => setShowDeleteConfirm(false)}>取消</button>
          </>
        )}
      </div>

      {/* 标题 */}
      <h3 className="kb-detail-title">{item.title}</h3>

      {/* 标签 */}
      {item.tags?.length > 0 && (
        <div style={{ display: 'flex', gap: '0.3em', flexWrap: 'wrap' }}>
          {item.tags.map((t) => (
            <span key={t} className="kb-tag-chip">{t}</span>
          ))}
        </div>
      )}

      {/* 内容区：阅读态 ⇄ 编辑态 */}
      <div className="kb-detail-body">
        {mode === 'edit' ? (
          <TipTapEditor
            markdown={item.bodyMd}
            onSave={handleSave}
            placeholder="开始写知识..."
          />
        ) : (
          <div
            className="kb-detail-reading"
            onClick={() => setMode('edit')}
            title="点击进入编辑"
            style={{ cursor: 'pointer', minHeight: 100 }}
          >
            {item.bodyMd ? (
              <MdStaticRenderer markdown={item.bodyMd} />
            ) : (
              <p style={{ color: 'var(--muted-foreground)', fontStyle: 'italic' }}>
                空白 — 点击编辑
              </p>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
