import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  ChevronDown,
  ChevronRight,
  Folder,
  FolderOpen,
  FolderPlus,
  Inbox,
  Layers,
  Pencil,
  Trash2,
} from 'lucide-react'
import type { KbFolder } from '@/lib/api'

/** 左栏文件夹筛选选择：全部 / 未分类 / 具体文件夹（精确匹配） */
export type FolderSelection =
  | { kind: 'all' }
  | { kind: 'unfiled' }
  | { kind: 'folder'; id: string }

/** 扁平化后的文件夹（含层级深度），供树渲染与详情下拉共用 */
export interface FlatFolder {
  id: string
  name: string
  depth: number
}

export interface KbFolderCounts {
  all: number
  unfiled: number
  /** folderId → 直属条目数（精确计数，不含子文件夹） */
  byFolder: Map<string, number>
}

interface KbFolderTreeProps {
  folders: KbFolder[]
  selection: FolderSelection
  counts: KbFolderCounts
  /** 文件夹列表加载失败的后端文案（显示在树内，不阻塞「全部/未分类」） */
  loadError?: string | null
  onSelect: (selection: FolderSelection) => void
  onCreate: () => void
  onRename: (folder: KbFolder) => void
  onDelete: (folder: KbFolder) => void | Promise<void>
}

const DELETE_CONFIRM_MS = 3000

function compareFolders(a: KbFolder, b: KbFolder): number {
  const orderDiff = (a.order ?? 0) - (b.order ?? 0)
  if (orderDiff !== 0) return orderDiff
  return a.name.localeCompare(b.name, 'zh-CN')
}

/**
 * 把 folder 列表按 parentId 组织成 parentId → 子夹[]（已排序）。
 * 防御脏数据：父夹不存在或自引用时提升为顶层；A↔B 互指成环时也提升为顶层，
 * 保证任何文件夹都不会从 UI 中消失（否则条目归属下拉会缺项）。
 */
function buildChildren(folders: KbFolder[]): Map<string | null, KbFolder[]> {
  const byId = new Map(folders.map((f) => [f.id, f]))

  const resolveParentId = (folder: KbFolder): string | null => {
    const direct = folder.parentId ?? null
    // 无父 / 自引用 / 父夹不存在（断链）→ 提升为顶层
    if (!direct || direct === folder.id || !byId.has(direct)) return null
    // 环检测：沿父链向上走，只有真的回到自己才提升顶层；
    // 走到根（parentId 为空）或祖先断链时，当前层级仍然合法，必须保留。
    const seen = new Set<string>([folder.id])
    let cursor: string | null = direct
    while (cursor) {
      if (seen.has(cursor)) return null // A↔B 互指成环 → 顶层，防止节点消失
      seen.add(cursor)
      const parent = byId.get(cursor)
      if (!parent) break // 祖先断链：保留在 direct 下
      if (parent.parentId === parent.id) return null // 父夹自引用
      cursor = parent.parentId ?? null
    }
    return direct
  }

  const children = new Map<string | null, KbFolder[]>()
  const placed = new Set<string>()
  for (const folder of folders) {
    // 重复 id 只保留第一条，避免 React key 冲突与同夹重复渲染
    if (placed.has(folder.id)) continue
    placed.add(folder.id)
    const parentId = resolveParentId(folder)
    const list = children.get(parentId)
    if (list) list.push(folder)
    else children.set(parentId, [folder])
  }
  for (const list of children.values()) list.sort(compareFolders)
  return children
}

/**
 * 扁平化文件夹树（父 → 子，同级按 order 再按名称）。
 * 详情下拉直接用它，保证缩进层级与左栏树一致。
 */
export function flattenFolders(folders: KbFolder[]): FlatFolder[] {
  const children = buildChildren(folders)
  const out: FlatFolder[] = []
  const visited = new Set<string>()

  const walk = (parentId: string | null, depth: number) => {
    const list = children.get(parentId)
    if (!list) return
    for (const folder of list) {
      if (visited.has(folder.id)) continue
      visited.add(folder.id)
      out.push({ id: folder.id, name: folder.name, depth })
      walk(folder.id, depth + 1)
    }
  }
  walk(null, 0)

  // 兜底（正常不会命中）：极端环数据里仍保证所有文件夹可选
  for (const folder of folders) {
    if (!visited.has(folder.id)) {
      visited.add(folder.id)
      out.push({ id: folder.id, name: folder.name, depth: 0 })
    }
  }
  return out
}

/** 按展开状态排出当前可见行（折叠的子夹整枝跳过） */
function visibleFolders(
  children: Map<string | null, KbFolder[]>,
  collapsed: Set<string>,
): FlatFolder[] {
  const out: FlatFolder[] = []
  const walk = (parentId: string | null, depth: number) => {
    const list = children.get(parentId)
    if (!list) return
    for (const folder of list) {
      out.push({ id: folder.id, name: folder.name, depth })
      if (!collapsed.has(folder.id)) walk(folder.id, depth + 1)
    }
  }
  walk(null, 0)
  return out
}

/**
 * 左栏文件夹树（任务 #4）：
 * 固定项「全部知识」「未分类」+ 可展开/收起的层级文件夹；行内 hover 出现重命名 / 删除。
 * 展开状态仅存在组件 state（切视图重挂载后恢复全展开）。
 * 删除走两步确认：首次点击变「确认删除」，3 秒无后续操作自动回退。
 */
export function KbFolderTree({
  folders,
  selection,
  counts,
  loadError,
  onSelect,
  onCreate,
  onRename,
  onDelete,
}: KbFolderTreeProps) {
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set())
  const [confirmingId, setConfirmingId] = useState<string | null>(null)
  const confirmTimer = useRef<number | null>(null)

  const clearConfirmTimer = useCallback(() => {
    if (confirmTimer.current !== null) {
      window.clearTimeout(confirmTimer.current)
      confirmTimer.current = null
    }
  }, [])

  useEffect(() => clearConfirmTimer, [clearConfirmTimer])

  const children = useMemo(() => buildChildren(folders), [folders])
  const rows = useMemo(() => visibleFolders(children, collapsed), [children, collapsed])

  const toggleCollapse = useCallback((id: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }, [])

  /** 删除按钮：第一次进入确认态，第二次执行 */
  const handleDeleteClick = useCallback(
    (folder: KbFolder) => {
      if (confirmingId === folder.id) {
        clearConfirmTimer()
        setConfirmingId(null)
        void onDelete(folder)
        return
      }
      clearConfirmTimer()
      setConfirmingId(folder.id)
      confirmTimer.current = window.setTimeout(() => {
        confirmTimer.current = null
        setConfirmingId(null)
      }, DELETE_CONFIRM_MS)
    },
    [confirmingId, clearConfirmTimer, onDelete],
  )

  return (
    <div className="kb-folder-tree" aria-label="文件夹筛选">
      <div className="kb-folder-tree-head">
        <span className="kb-folder-tree-title">文件夹</span>
        <button
          type="button"
          className="kb-folder-add"
          title="新建文件夹"
          aria-label="新建文件夹"
          onClick={onCreate}
        >
          <FolderPlus size={13} />
        </button>
      </div>

      {loadError && (
        <div className="kb-folder-error" role="alert">
          {loadError}
        </div>
      )}

      <div className="kb-folder-fixed">
        <button
          type="button"
          className={`kb-folder-row${selection.kind === 'all' ? ' is-active' : ''}`}
          onClick={() => onSelect({ kind: 'all' })}
          aria-pressed={selection.kind === 'all'}
        >
          <Layers size={12} className="kb-folder-ico" />
          <span className="kb-folder-name">全部知识</span>
          <span className="kb-folder-count">{counts.all}</span>
        </button>
        <button
          type="button"
          className={`kb-folder-row${selection.kind === 'unfiled' ? ' is-active' : ''}`}
          onClick={() => onSelect({ kind: 'unfiled' })}
          aria-pressed={selection.kind === 'unfiled'}
        >
          <Inbox size={12} className="kb-folder-ico" />
          <span className="kb-folder-name">未分类</span>
          <span className="kb-folder-count">{counts.unfiled}</span>
        </button>
      </div>

      {rows.length > 0 && (
        <div className="kb-folder-rows">
          {rows.map((row) => {
            const folder = folders.find((f) => f.id === row.id)
            if (!folder) return null
            const hasChildren = (children.get(row.id)?.length ?? 0) > 0
            const expanded = !collapsed.has(row.id)
            const active = selection.kind === 'folder' && selection.id === row.id
            const confirming = confirmingId === row.id
            const count = counts.byFolder.get(row.id) ?? 0
            return (
              <div
                key={row.id}
                className={`kb-folder-row-wrap${active ? ' is-active' : ''}${confirming ? ' is-confirming' : ''}`}
                style={{ paddingLeft: `${row.depth * 14}px` }}
              >
                {hasChildren ? (
                  <button
                    type="button"
                    className="kb-folder-caret"
                    onClick={() => toggleCollapse(row.id)}
                    title={expanded ? '收起子文件夹' : '展开子文件夹'}
                    aria-label={expanded ? '收起子文件夹' : '展开子文件夹'}
                    aria-expanded={expanded}
                  >
                    {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                  </button>
                ) : (
                  <span className="kb-folder-caret is-empty" aria-hidden="true" />
                )}
                <button
                  type="button"
                  className={`kb-folder-row${active ? ' is-active' : ''}`}
                  onClick={() => onSelect({ kind: 'folder', id: row.id })}
                  aria-pressed={active}
                  title={row.name}
                >
                  {expanded && hasChildren ? (
                    <FolderOpen size={12} className="kb-folder-ico" />
                  ) : (
                    <Folder size={12} className="kb-folder-ico" />
                  )}
                  <span className="kb-folder-name">{row.name}</span>
                  <span className="kb-folder-count">{count}</span>
                </button>
                <span className="kb-folder-actions">
                  {confirming ? (
                    <button
                      type="button"
                      className="kb-folder-action is-danger"
                      title="确认删除文件夹"
                      onClick={() => handleDeleteClick(folder)}
                    >
                      确认删除
                    </button>
                  ) : (
                    <>
                      <button
                        type="button"
                        className="kb-folder-action"
                        title="重命名文件夹"
                        aria-label="重命名文件夹"
                        onClick={() => onRename(folder)}
                      >
                        <Pencil size={11} />
                      </button>
                      <button
                        type="button"
                        className="kb-folder-action"
                        title="删除文件夹"
                        aria-label="删除文件夹"
                        onClick={() => handleDeleteClick(folder)}
                      >
                        <Trash2 size={11} />
                      </button>
                    </>
                  )}
                </span>
              </div>
            )
          })}
        </div>
      )}

      {!loadError && folders.length === 0 && (
        <div className="kb-folder-empty">暂无文件夹</div>
      )}
    </div>
  )
}