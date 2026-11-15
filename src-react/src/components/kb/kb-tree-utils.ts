import type { KbFolder, KbItem } from '@/lib/api'

/**
 * 知识库左栏单树的纯函数工具（S1 批次，tasks 2.1）。
 *
 * 设计要点（design D1/D2/D10）：
 * - 树在前端由 `folders` + `items` 合成，不新增后端查询命令（O(n log n)）；
 * - 文件夹排序 `pinned desc → order asc（null 视为 Infinity）→ name localeCompare('zh-CN')`；
 * - 条目排序 `pinned desc → order asc → updatedAt desc → title localeCompare('zh-CN')`；
 * - 脏数据（parentId 断链 / 自引用 / 成环）一律提升到顶层兜底，保证节点不从 UI 消失；
 * - 过滤态用扁平结果 + 路径面包屑，路径解析同样对断链兜底（返回能走到的部分）。
 *
 * 本文件只放纯逻辑，不依赖 React / DOM，便于 `node src-react/test/kb-tree.test.mjs` 直接跑。
 */

/** 左栏固定「未分类」虚拟行在展开态 Set 里的 key（不是真实文件夹 id） */
export const UNFILED_EXPAND_KEY = '__unfiled__'

/** 未归属（含悬空 folderId）条目的路径文案 */
export const UNFILED_LABEL = '未分类'

/**
 * 文件夹树上的选择上下文：全部 / 未分类 / 具体文件夹。
 * 单树模式下它只决定「新建条目/文件夹的默认父级」与高亮，不再过滤树（过滤态由搜索/标签决定）。
 */
export type FolderSelection =
  | { kind: 'all' }
  | { kind: 'unfiled' }
  | { kind: 'folder'; id: string }

/** 排序比较器的输入：文件夹取 name，条目取 title / updatedAt */
export interface KbSortable {
  pinned: boolean
  /** null / 缺失视为 Infinity（旧数据未生成 order 时排在同级最后） */
  order: number | null
  /** 条目更新时间（本地语义字符串，可直接字典序比较）；文件夹无 */
  updatedAt?: string
  name: string
}

/**
 * 同级排序：pinned desc → order asc（null 视为 Infinity）→ updatedAt desc → name('zh-CN')。
 * 文件夹没有 updatedAt，天然落到 name 比较。
 */
export function compareKbNodes(a: KbSortable, b: KbSortable): number {
  const pinnedDiff = Number(b.pinned) - Number(a.pinned)
  if (pinnedDiff !== 0) return pinnedDiff

  const aOrder = a.order ?? Number.POSITIVE_INFINITY
  const bOrder = b.order ?? Number.POSITIVE_INFINITY
  if (aOrder !== bOrder) return aOrder < bOrder ? -1 : 1

  if (a.updatedAt && b.updatedAt) {
    // 本地语义时间字符串（YYYY-MM-DDTHH:mm）字典序 = 时间序，倒序即最新在前
    const updatedDiff = b.updatedAt.localeCompare(a.updatedAt)
    if (updatedDiff !== 0) return updatedDiff
  }

  return a.name.localeCompare(b.name, 'zh-CN')
}

/** 合成后的文件夹节点：子文件夹与直属条目的两段容器（渲染时子夹在前、条目在后） */
export interface KbTreeFolderNode {
  kind: 'folder'
  /** 原始文件夹对象（重命名/删除等操作直接复用，不丢字段） */
  folder: KbFolder
  id: string
  name: string
  parentId: string | null
  pinned: boolean
  order: number | null
  /** 子文件夹（已排序） */
  folders: KbTreeFolderNode[]
  /** 直属条目（已排序） */
  items: KbItem[]
}

export interface KbTree {
  /** 顶层文件夹（已排序） */
  roots: KbTreeFolderNode[]
  /** 未分类条目：folderId 为空 / 指向已删除或非法文件夹（与旧筛选口径一致） */
  unfiled: KbItem[]
  totalItems: number
}

/** id → folder；重复 id 只保留第一条，避免 React key 冲突与同夹重复渲染 */
function folderMap(folders: KbFolder[]): Map<string, KbFolder> {
  const byId = new Map<string, KbFolder>()
  for (const folder of folders) {
    if (!folder || !folder.id || byId.has(folder.id)) continue
    byId.set(folder.id, folder)
  }
  return byId
}

/**
 * 解析文件夹应挂载的父级：
 * - 无父 / 父不存在（断链）/ 自引用 / 父链成环 → null（提升顶层兜底）；
 * - 祖先链只是中途断掉（父夹本身存在、更上层缺失）→ 保留直接父级，不误伤层级。
 */
function resolvedParentId(
  folder: { id: string; parentId: string | null },
  byId: Map<string, KbFolder>,
): string | null {
  const direct = folder.parentId ?? null
  if (!direct || direct === folder.id || !byId.has(direct)) return null

  const seen = new Set<string>([folder.id])
  let cursor: string | null = direct
  while (cursor) {
    if (seen.has(cursor)) return null // A↔B 互指 / 更长环：整环提升顶层，防止节点消失
    seen.add(cursor)
    const parent = byId.get(cursor)
    if (!parent) break // 祖先断链：当前层级仍合法
    if (parent.parentId === parent.id) return null // 父夹自引用
    cursor = parent.parentId ?? null
  }
  return direct
}

function folderSortable(node: KbTreeFolderNode): KbSortable {
  return { pinned: node.pinned, order: node.order, name: node.name }
}

function itemSortable(item: KbItem): KbSortable {
  return {
    pinned: item.pinned === true,
    order: typeof item.order === 'number' ? item.order : null,
    updatedAt: item.updatedAt,
    name: item.title,
  }
}

const compareFolderNodes = (a: KbTreeFolderNode, b: KbTreeFolderNode) =>
  compareKbNodes(folderSortable(a), folderSortable(b))

const compareTreeItems = (a: KbItem, b: KbItem) =>
  compareKbNodes(itemSortable(a), itemSortable(b))

/**
 * 把文件夹与条目合成一棵树（tasks 2.1 / design D1）。
 *
 * 文件夹节点只持有「直属条目」，悬空 folderId 的条目归入 `unfiled`；
 * 排序在合成时一次完成，渲染层直接按序输出。
 */
export function buildKbTree(folders: KbFolder[], items: KbItem[]): KbTree {
  const byId = folderMap(folders)
  const nodesById = new Map<string, KbTreeFolderNode>()

  for (const folder of folders) {
    if (!folder || !folder.id || nodesById.has(folder.id)) continue
    nodesById.set(folder.id, {
      kind: 'folder',
      folder,
      id: folder.id,
      name: folder.name,
      parentId: folder.parentId ?? null,
      pinned: folder.pinned === true,
      order: typeof folder.order === 'number' ? folder.order : null,
      folders: [],
      items: [],
    })
  }

  const roots: KbTreeFolderNode[] = []
  for (const node of nodesById.values()) {
    const parentId = resolvedParentId(node, byId)
    const parent = parentId ? nodesById.get(parentId) : undefined
    if (parent) parent.folders.push(node)
    else roots.push(node)
  }

  const unfiled: KbItem[] = []
  for (const item of items) {
    const folderId = item.folderId ?? null
    const node = folderId ? nodesById.get(folderId) : undefined
    if (node) node.items.push(item)
    else unfiled.push(item)
  }

  for (const node of nodesById.values()) {
    node.folders.sort(compareFolderNodes)
    node.items.sort(compareTreeItems)
  }
  roots.sort(compareFolderNodes)
  unfiled.sort(compareTreeItems)

  return { roots, unfiled, totalItems: items.length }
}

/**
 * 文件夹面包屑：根 → 叶返回 `KbFolder[]`。
 * 断链 / 环兜底：只返回实际能走到的部分；folderId 不存在返回空数组。
 */
export function resolveFolderPath(
  folders: KbFolder[],
  folderId: string | null | undefined,
): KbFolder[] {
  if (!folderId) return []
  const byId = folderMap(folders)
  const path: KbFolder[] = []
  const seen = new Set<string>()
  let cursor: string | null = folderId
  while (cursor && !seen.has(cursor)) {
    const folder = byId.get(cursor)
    if (!folder) break // 断链：丢弃不存在的祖先，保留已走到的层级
    seen.add(cursor)
    path.unshift(folder)
    cursor = folder.parentId ?? null
  }
  return path
}

/** 文件夹路径文案（如「设计 / 前端」）；无归属 / 悬空返回「未分类」 */
export function folderPathLabel(
  folders: KbFolder[],
  folderId: string | null | undefined,
): string {
  const path = resolveFolderPath(folders, folderId)
  if (path.length === 0) return UNFILED_LABEL
  return path.map((folder) => folder.name).join(' / ')
}

/** 过滤态扁平结果行：标题 + 路径面包屑（tasks 2.3） */
export interface KbFlatResult {
  item: KbItem
  title: string
  /** 根→叶路径文案；无归属 / 断链为「未分类」 */
  pathLabel: string
  /** 解析后的父级 id；无归属 / 断链为 null */
  folderId: string | null
}

/**
 * 过滤态扁平结果（tasks 2.3 / design D2）。
 * - `items` 的传入顺序即结果顺序（搜索命中按 search_kb 相关度排序，标签过滤按现有列表序）；
 * - `matcher` 只决定保留哪些项（搜索命中集合 / 标签 / 本地兜底都从外部注入，保持纯函数）；
 * - 路径用 `folderPathLabel` 解析，保证断链条目也有稳定文案。
 */
export function flattenResults(
  items: KbItem[],
  folders: KbFolder[],
  matcher: (item: KbItem) => boolean,
): KbFlatResult[] {
  const byId = folderMap(folders)
  const labelCache = new Map<string, string>()
  const pathLabelOf = (folderId: string | null): string => {
    if (!folderId || !byId.has(folderId)) return UNFILED_LABEL
    const cached = labelCache.get(folderId)
    if (cached !== undefined) return cached
    const label = folderPathLabel(folders, folderId)
    labelCache.set(folderId, label)
    return label
  }

  const out: KbFlatResult[] = []
  for (const item of items) {
    if (!matcher(item)) continue
    const rawFolderId = item.folderId ?? null
    const folderId = rawFolderId && byId.has(rawFolderId) ? rawFolderId : null
    out.push({ item, title: item.title, pathLabel: pathLabelOf(folderId), folderId })
  }
  return out
}

/**
 * 本地检索兜底（kb-storage-backup D5 收窄口径，与后端 `kb::search` 一致）：
 * 只匹配「标题 + 所有祖先文件夹名」，大小写不敏感、去首尾空白；**不匹配 tags / 正文**。
 * search_kb 请求未返回或失败时用它即时出结果，避免过滤态空窗；`folders` 缺省时只匹配标题。
 * 防环 / 深度上限 64 与 `resolveFolderPath`、后端实现保持一致。
 */
export function localKbMatch(item: KbItem, query: string, folders: KbFolder[] = []): boolean {
  const q = query.trim().toLowerCase()
  if (!q) return true
  if ((item.title ?? '').toLowerCase().includes(q)) return true
  if (!item.folderId) return false
  const byId = folderMap(folders)
  const seen = new Set<string>()
  let cursor: string | null = item.folderId
  let depth = 0
  while (cursor && !seen.has(cursor) && depth < 64) {
    const folder = byId.get(cursor)
    if (!folder) break // 断链：丢弃不存在的祖先（与面包屑兜底一致）
    seen.add(cursor)
    depth += 1
    if ((folder.name ?? '').toLowerCase().includes(q)) return true
    cursor = folder.parentId ?? null
  }
  return false
}

/**
 * 「移动到…」二级菜单的文件夹选项（tasks 2.5 / design D3）。
 * 顺序 = 树的先序（根→叶）；`depth` 供菜单缩进，`pathLabel` 供 title 与调试。
 */
export interface FolderOption {
  id: string
  name: string
  /** 树深度：顶层文件夹为 0 */
  depth: number
  /** 根→叶路径文案（含自身，如「设计 / 前端」） */
  pathLabel: string
}

/**
 * 按树序（先序）展开全部文件夹为扁平选项。
 * 复用 `buildKbTree` 的排序与脏数据兜底，保证菜单里的层级顺序与左树一致。
 */
export function flattenFolderOptions(folders: KbFolder[]): FolderOption[] {
  const tree = buildKbTree(folders, [])
  const out: FolderOption[] = []
  const walk = (nodes: KbTreeFolderNode[], names: string[]) => {
    for (const node of nodes) {
      const path = [...names, node.name]
      out.push({
        id: node.id,
        name: node.name,
        depth: names.length,
        pathLabel: path.join(' / '),
      })
      walk(node.folders, path)
    }
  }
  walk(tree.roots, [])
  return out
}

/**
 * 文件夹自身 + 全部后代 id（tasks 2.5：移动菜单里禁选，避免自嵌套 / 成环）。
 * 用 parentId 正向建邻接表再迭代遍历，重复 id 与环都能收敛（每个 id 只出一次）。
 */
export function folderSubtreeIds(folders: KbFolder[], folderId: string): Set<string> {
  const childrenOf = new Map<string, string[]>()
  const seen = new Set<string>()
  for (const folder of folders) {
    if (!folder || !folder.id || seen.has(folder.id)) continue
    seen.add(folder.id)
    const parentId = folder.parentId ?? null
    if (!parentId || parentId === folder.id) continue
    const siblings = childrenOf.get(parentId)
    if (siblings) siblings.push(folder.id)
    else childrenOf.set(parentId, [folder.id])
  }

  const out = new Set<string>()
  const stack: string[] = [folderId]
  while (stack.length > 0) {
    const id = stack.pop() as string
    if (out.has(id)) continue
    out.add(id)
    const children = childrenOf.get(id)
    if (children) for (const child of children) stack.push(child)
  }
  return out
}

// ─── S2 拖拽 / 同级排序 / 置顶（tasks 3.1–3.4，design D4/D5/D6）───────────
//
// order 写端语义（design D4，必须与 store.rs 的 place / update_folder 一致）：
// 传入的 order 是「先把被拖节点从原同级列表移除后」的目标同级索引，后端再 clamp 到
// [0, len] 并对目标同级做 dense 重排 0..n-1；跨父级移动且不带 order 时追加末尾。
// 因此前端算索引时必须用「移除自身后的同级序列」，否则同父向下拖动会差一位。

/** 树内拖拽协议 MIME（design D5）：只接受这一个自定义类型，避免把外部文件拖进来 */
export const KB_DND_MIME = 'text/kettd-kb'

/** 拖拽载荷：文件夹或条目，id 为原始实体 id */
export interface KbDragPayload {
  kind: 'folder' | 'item'
  id: string
}

/** 落点三区：上 25% 插入前 / 中 50% 移入 / 下 25% 插入后 */
export type KbDropZone = 'before' | 'inside' | 'after'

/**
 * 行矩形 + 指针 y → 落点三区（tasks 3.1 / design D5）。
 * - 上 25% → before，中 50% → inside，下 25% → after；
 * - y 在矩形上方/下方时按最近边界处理；
 * - 空区（树底部空白）不是行，调用方用 planAppendDrop 单独解析；
 * - 高度非正（未布局/隐藏）视为 inside，交调用方决定是否允许。
 */
export function resolveDropTarget(
  rect: { top: number; height: number },
  y: number,
): KbDropZone {
  const height = rect.height
  if (!(height > 0)) return 'inside'
  const ratio = (y - rect.top) / height
  if (ratio <= 0.25) return 'before'
  if (ratio >= 0.75) return 'after'
  return 'inside'
}

/**
 * 同级目标索引（tasks 3.1 / design D4）：
 * `siblingsWithoutDragged` 是「已移除被拖节点」的目标同级序列（按后端落库序），
 * `targetId` 是被拖节点将落在其前/后的那个锚点节点。
 * 返回 before 取锚点索引、after 取锚点索引 + 1，并 clamp 到序列长度（内联列表末尾）。
 * 锚点不存在返回 -1（调用方视为不可放置）。
 */
export function computeSiblingIndex(
  siblingsWithoutDragged: ReadonlyArray<{ id: string }>,
  targetId: string,
  zone: 'before' | 'after',
): number {
  const targetIndex = siblingsWithoutDragged.findIndex((sibling) => sibling.id === targetId)
  if (targetIndex < 0) return -1
  const index = zone === 'after' ? targetIndex + 1 : targetIndex
  return Math.min(Math.max(index, 0), siblingsWithoutDragged.length)
}

/** 后端同级落库序的键（与 compareKbNodes 的差异：pinned 只影响展示，不参与落库序列） */
export interface KbSiblingOrderKey {
  id: string
  /** null / 缺失视为 Infinity（旧数据未生成 order 时排同级最后） */
  order: number | null
  /** 条目更新时间（本地语义字符串）；文件夹无，并列时回落到名称 */
  updatedAt?: string
  /** 名称（文件夹名；条目并列时不用标题，收口到 id 与后端一致） */
  name?: string
}

/**
 * 后端落库顺序口径（store.rs：order asc(null=MAX) → 条目 updatedAt desc → 文件夹 name → id）。
 * 前端本地乐观重排用它算 dense 0..n-1，保证刷新前后顺序与服务端一致。
 */
export function compareKbSiblingOrder(a: KbSiblingOrderKey, b: KbSiblingOrderKey): number {
  const aOrder = a.order ?? Number.POSITIVE_INFINITY
  const bOrder = b.order ?? Number.POSITIVE_INFINITY
  if (aOrder !== bOrder) return aOrder < bOrder ? -1 : 1

  if (a.updatedAt && b.updatedAt) {
    const updatedDiff = b.updatedAt.localeCompare(a.updatedAt)
    if (updatedDiff !== 0) return updatedDiff
  }
  if (a.name && b.name) {
    const nameDiff = a.name.localeCompare(b.name, 'zh-CN')
    if (nameDiff !== 0) return nameDiff
  }
  return a.id.localeCompare(b.id)
}

/** 按落库序排好的同级副本（不修改入参） */
export function sortKbSiblings<T extends KbSiblingOrderKey>(nodes: readonly T[]): T[] {
  return [...nodes].sort(compareKbSiblingOrder)
}

/** 拖放计划：移入某文件夹 / 在目标同级插入到某个索引 */
export type KbDropPlan =
  | { type: 'into'; parentId: string | null }
  | { type: 'reorder'; parentId: string | null; index: number }

/** 树结构索引（WeakMap 缓存：dragover 高频调用时不重复遍历整棵树） */
interface KbTreeIndex {
  folderNodes: Map<string, KbTreeFolderNode>
  /** 树展示的父级（脏数据已兜底，根为 null） */
  folderParents: Map<string, string | null>
  /** 原始 folder.parentId（与后端校验口径一致，用于自嵌套/成环预判） */
  rawFolderParents: Map<string, string | null>
  itemParents: Map<string, string | null>
  /** 条目对象（置顶状态影响展示序，落点索引需要用到） */
  itemNodes: Map<string, KbItem>
}

const treeIndexCache = new WeakMap<KbTree, KbTreeIndex>()

function buildTreeIndex(tree: KbTree): KbTreeIndex {
  const folderNodes = new Map<string, KbTreeFolderNode>()
  const folderParents = new Map<string, string | null>()
  const rawFolderParents = new Map<string, string | null>()
  const itemParents = new Map<string, string | null>()
  const itemNodes = new Map<string, KbItem>()
  const walk = (nodes: KbTreeFolderNode[], parentId: string | null) => {
    for (const node of nodes) {
      folderNodes.set(node.id, node)
      folderParents.set(node.id, parentId)
      rawFolderParents.set(node.id, node.folder?.parentId ?? null)
      walk(node.folders, node.id)
    }
  }
  walk(tree.roots, null)
  for (const item of tree.unfiled) {
    itemParents.set(item.id, null)
    itemNodes.set(item.id, item)
  }
  for (const node of folderNodes.values()) {
    for (const item of node.items) {
      itemParents.set(item.id, node.id)
      itemNodes.set(item.id, item)
    }
  }
  return { folderNodes, folderParents, rawFolderParents, itemParents, itemNodes }
}

function treeIndex(tree: KbTree): KbTreeIndex {
  const cached = treeIndexCache.get(tree)
  if (cached) return cached
  const built = buildTreeIndex(tree)
  treeIndexCache.set(tree, built)
  return built
}

/** 按 id 找树上的文件夹节点（600ms 悬停展开用） */
export function findKbFolderNode(tree: KbTree, folderId: string): KbTreeFolderNode | null {
  return treeIndex(tree).folderNodes.get(folderId) ?? null
}

/** 判断 candidateId 是否沿原始 parentId 链位于 ancestorId 的子树内（含自身，防自嵌套/成环） */
function isRawDescendant(
  index: KbTreeIndex,
  candidateId: string,
  ancestorId: string,
): boolean {
  if (candidateId === ancestorId) return true
  const seen = new Set<string>([candidateId])
  let cursor = index.rawFolderParents.get(candidateId) ?? null
  while (cursor) {
    if (cursor === ancestorId) return true
    if (seen.has(cursor)) return false // 原始数据成环：无法作为落点，留后端兜底报错
    seen.add(cursor)
    cursor = index.rawFolderParents.get(cursor) ?? null
  }
  return false
}

/** 目标同级成员（树展示序 = 落库序，二次排序用 sortKbSiblings 保证确定性） */
function siblingMembers(
  index: KbTreeIndex,
  tree: KbTree,
  parentId: string | null,
): { folders: KbTreeFolderNode[]; items: KbItem[] } | null {
  if (parentId === null) return { folders: tree.roots, items: tree.unfiled }
  const node = index.folderNodes.get(parentId)
  if (!node) return null
  return { folders: node.folders, items: node.items }
}

/**
 * 拖放落点 → 落库计划（tasks 3.1 / 3.2，纯函数，树内拖拽与单测共用）。
 *
 * 拒绝（返回 null）：
 * - 拖到自身；
 * - 文件夹拖到自身或自身后代（含中区移入、以及插入后父级落在自身子树内）；
 * - 中区落在条目行上（条目没有「移入」语义，调用方先把中区折算成 before/after）；
 * - 目标/父级在树上找不到。
 *
 * inside：parentId = 目标文件夹 id；before/after：parentId = 锚点节点所在父级，
 * index 已按「移除被拖节点后的同级序列」计算（design D4）。
 * 跨类型相邻（文件夹 ↔ 条目）：条目固定渲染在子文件夹之后，文件夹插到该父级文件夹组
 * 末尾，条目插到条目组首位。
 */
export function planKbDrop(
  tree: KbTree,
  dragged: KbDragPayload,
  target: KbDragPayload,
  zone: KbDropZone,
): KbDropPlan | null {
  if (dragged.kind === target.kind && dragged.id === target.id) return null
  if (zone === 'inside' && target.kind !== 'folder') return null

  const index = treeIndex(tree)
  const draggedFolder = dragged.kind === 'folder' ? index.folderNodes.get(dragged.id) : undefined
  if (dragged.kind === 'folder' && !draggedFolder) return null

  const targetParent = (): string | null | undefined => {
    if (target.kind === 'folder') {
      const node = index.folderNodes.get(target.id)
      return node ? index.folderParents.get(node.id) ?? null : undefined
    }
    return index.itemParents.get(target.id)
  }

  if (zone === 'inside') {
    if (target.kind !== 'folder') return null
    if (dragged.kind === 'folder') {
      // 后端二次校验同样拒绝：不能把文件夹移进自身或后代
      if (isRawDescendant(index, target.id, dragged.id)) return null
      // 已经在该夹下：inside 不产生任何变化，别让后端把它追加到末尾
      if ((index.folderParents.get(dragged.id) ?? null) === target.id) return null
    } else if ((index.itemParents.get(dragged.id) ?? null) === target.id) {
      return null
    }
    return { type: 'into', parentId: target.id }
  }

  const parentId = targetParent()
  if (parentId === undefined) return null
  if (dragged.kind === 'folder' && parentId !== null && isRawDescendant(index, parentId, dragged.id)) {
    return null
  }
  const members = siblingMembers(index, tree, parentId)
  if (!members) return null

  /**
   * 置顶优先于手动顺序（design D9）：展示序是「置顶组 → 非置顶组」，全部按 order 升序；
   * 而后端 dense 重排只按 order（不含 pinned）。所以「置顶节点拖到非置顶锚点前/后」时，
   * 直接把 order 取锚点的落库索引会插进其它置顶节点中间，刷新后顺序会跳。
   * 这里统一折到「置顶组末尾」（basis 里最后一个置顶同级之后），展示上等价于紧随置顶组。
   */
  const pinnedGroupEnd = <T extends { pinned?: boolean }>(siblings: T[]): number => {
    let lastPinned = -1
    siblings.forEach((sibling, position) => {
      if (sibling.pinned) lastPinned = position
    })
    return lastPinned + 1
  }

  if (dragged.kind === 'folder') {
    const draggedNode = index.folderNodes.get(dragged.id) as KbTreeFolderNode
    if (target.kind === 'folder') {
      const targetNode = index.folderNodes.get(target.id)
      if (!targetNode) return null
      const siblings = sortKbSiblings(
        members.folders
          .filter((node) => node.id !== dragged.id)
          .map((node) => ({ id: node.id, order: node.order, name: node.name, pinned: node.pinned })),
      )
      const computed = computeSiblingIndex(siblings, target.id, zone)
      if (computed < 0) return null
      const position = draggedNode.pinned && !targetNode.pinned
        ? pinnedGroupEnd(siblings)
        : computed
      return { type: 'reorder', parentId, index: position }
    }
    // 文件夹落在条目行前/后：插到该父级文件夹组末尾（条目组之前）
    return {
      type: 'reorder',
      parentId,
      index: members.folders.filter((node) => node.id !== dragged.id).length,
    }
  }

  const draggedItem = index.itemNodes.get(dragged.id)
  if (!draggedItem) return null
  if (target.kind === 'item') {
    const targetItem = index.itemNodes.get(target.id)
    if (!targetItem) return null
    const siblings = sortKbSiblings(
      members.items
        .filter((item) => item.id !== dragged.id)
        .map((item) => ({
          id: item.id,
          order: typeof item.order === 'number' ? item.order : null,
          updatedAt: item.updatedAt,
          pinned: item.pinned,
        })),
    )
    const computed = computeSiblingIndex(siblings, target.id, zone)
    if (computed < 0) return null
    const position = draggedItem.pinned && !targetItem.pinned
      ? pinnedGroupEnd(siblings)
      : computed
    return { type: 'reorder', parentId, index: position }
  }

  // 条目落在文件夹行前/后：插到该父级条目组首位（子文件夹之后）
  return { type: 'reorder', parentId, index: 0 }
}

/**
 * 空区追加计划（tasks 3.2）：把节点追加到指定父级末尾，不传 order（后端追加语义）。
 * parentId：条目为 null = 未分类；文件夹为 null = 根目录。
 * 校验与 planKbDrop 同口径（不存在 / 自嵌套 / 成环 → null）。
 */
export function planAppendDrop(
  tree: KbTree,
  dragged: KbDragPayload,
  parentId: string | null,
): KbDropPlan | null {
  const index = treeIndex(tree)
  if (dragged.kind === 'folder') {
    if (!index.folderNodes.has(dragged.id)) return null
    if (parentId !== null) {
      if (!index.folderNodes.has(parentId)) return null
      if (isRawDescendant(index, parentId, dragged.id)) return null
    }
  }
  return { type: 'into', parentId }
}

/**
 * 条目拖放的本地乐观重排（tasks 3.2/3.3，与后端 dense 语义一致）：
 * - 跨父级：旧父级剩余条目按落库序重排，新父级插入后整体重排；
 * - 同父：插入后整体重排；
 * - index 缺省 = 追加末尾；
 * - 不改 updatedAt/pinned 等其它字段，失败回滚由调用方持有快照。
 */
export function applyItemDropLocally(
  items: readonly KbItem[],
  draggedId: string,
  targetFolderId: string | null,
  index?: number,
): KbItem[] {
  const dragged = items.find((item) => item.id === draggedId)
  if (!dragged) return [...items]
  const oldFolderId = dragged.folderId ?? null
  const orderKey = (item: KbItem): KbSiblingOrderKey => ({
    id: item.id,
    order: typeof item.order === 'number' ? item.order : null,
    updatedAt: item.updatedAt,
  })
  const sortGroup = (group: KbItem[]): KbItem[] =>
    [...group].sort((a, b) => compareKbSiblingOrder(orderKey(a), orderKey(b)))
  const nextOrders = new Map<string, number>()

  if (oldFolderId !== targetFolderId) {
    const oldGroup = sortGroup(
      items.filter((item) => item.id !== draggedId && (item.folderId ?? null) === oldFolderId),
    )
    oldGroup.forEach((item, position) => nextOrders.set(item.id, position))
  }

  const targetGroup = sortGroup(
    items.filter((item) => item.id !== draggedId && (item.folderId ?? null) === targetFolderId),
  )
  const position = index === undefined
    ? targetGroup.length
    : Math.min(Math.max(index, 0), targetGroup.length)
  targetGroup.splice(position, 0, dragged)
  targetGroup.forEach((item, groupIndex) => nextOrders.set(item.id, groupIndex))

  return items.map((item) => {
    const order = nextOrders.get(item.id)
    if (order === undefined) return item
    if (item.id === draggedId) return { ...item, folderId: targetFolderId, order }
    return { ...item, order }
  })
}

/**
 * 文件夹拖放的本地乐观重排（tasks 3.2/3.3）：与 applyItemDropLocally 同口径，
 * 同级排序口径为父级 → order asc(null=MAX) → name('zh-CN')。
 * targetParentId：null = 根目录。
 */
export function applyFolderDropLocally(
  folders: readonly KbFolder[],
  draggedId: string,
  targetParentId: string | null,
  index?: number,
): KbFolder[] {
  const dragged = folders.find((folder) => folder.id === draggedId)
  if (!dragged) return [...folders]
  const oldParentId = dragged.parentId ?? null
  const orderKey = (folder: KbFolder): KbSiblingOrderKey => ({
    id: folder.id,
    order: typeof folder.order === 'number' ? folder.order : null,
    name: folder.name,
  })
  const sortGroup = (group: KbFolder[]): KbFolder[] =>
    [...group].sort((a, b) => compareKbSiblingOrder(orderKey(a), orderKey(b)))
  const nextOrders = new Map<string, number>()

  if (oldParentId !== targetParentId) {
    const oldGroup = sortGroup(
      folders.filter((folder) => folder.id !== draggedId && (folder.parentId ?? null) === oldParentId),
    )
    oldGroup.forEach((folder, position) => nextOrders.set(folder.id, position))
  }

  const targetGroup = sortGroup(
    folders.filter((folder) => folder.id !== draggedId && (folder.parentId ?? null) === targetParentId),
  )
  const position = index === undefined
    ? targetGroup.length
    : Math.min(Math.max(index, 0), targetGroup.length)
  targetGroup.splice(position, 0, dragged)
  targetGroup.forEach((folder, groupIndex) => nextOrders.set(folder.id, groupIndex))

  return folders.map((folder) => {
    const order = nextOrders.get(folder.id)
    if (order === undefined) return folder
    if (folder.id === draggedId) return { ...folder, parentId: targetParentId, order }
    return { ...folder, order }
  })
}

/** 置顶快捷区行（tasks 3.4 / design D6）：只读副本入口，不参与拖拽 */
export interface KbPinnedNode {
  kind: 'folder' | 'item'
  id: string
  name: string
  /** 路径提示：文件夹为自身路径；条目为所在文件夹路径（无归属/断链为「未分类」） */
  pathLabel: string
  /** 原始对象（右键菜单直接复用最新快照，避免二次查找） */
  folder?: KbFolder
  item?: KbItem
}

function pinnedSortable(node: KbPinnedNode): KbSortable {
  if (node.kind === 'folder') {
    return { pinned: true, order: node.folder?.order ?? null, name: node.name }
  }
  return {
    pinned: true,
    order: typeof node.item?.order === 'number' ? node.item.order : null,
    updatedAt: node.item?.updatedAt,
    name: node.name,
  }
}

/**
 * 置顶快捷区数据（tasks 3.4 / design D6）：
 * - 只收 pinned 的文件夹与条目（重复 id 只保留第一条）；
 * - 排序同 compareKbNodes（pinned 都相等 → order asc(null=MAX) → updatedAt desc → name），
 *   跨类型并列时用 kind/id 收口，保证每次渲染顺序稳定；
 * - 取消置顶后实体 pinned=false，下一次渲染自然从快捷区消失。
 */
export function collectPinnedNodes(folders: KbFolder[], items: KbItem[]): KbPinnedNode[] {
  const nodes: KbPinnedNode[] = []
  const seenFolders = new Set<string>()
  for (const folder of folders) {
    if (!folder || !folder.id || folder.pinned !== true || seenFolders.has(folder.id)) continue
    seenFolders.add(folder.id)
    nodes.push({
      kind: 'folder',
      id: folder.id,
      name: folder.name,
      pathLabel: folderPathLabel(folders, folder.id),
      folder,
    })
  }
  const seenItems = new Set<string>()
  for (const item of items) {
    if (!item || !item.id || item.pinned !== true || seenItems.has(item.id)) continue
    seenItems.add(item.id)
    nodes.push({
      kind: 'item',
      id: item.id,
      name: item.title,
      pathLabel: folderPathLabel(folders, item.folderId ?? null),
      item,
    })
  }
  nodes.sort((a, b) => compareKbNodes(pinnedSortable(a), pinnedSortable(b))
    || a.kind.localeCompare(b.kind)
    || a.id.localeCompare(b.id))
  return nodes
}