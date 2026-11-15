// 知识库单树纯函数测试（tasks 2.1 + S2 3.1，design D11/D4/D5/D6）。
// 直接 `node src-react/test/kb-tree.test.mjs` 运行（刻意不用 node:test / --test：
// Windows 沙箱下 node --test 的子进程管道会 EPERM）。Node 22 默认开启 type stripping，
// 可以直接 import 源码 .ts；utils 里 `import type` 会在加载前被擦除，不触发 @ 别名解析。
import assert from 'node:assert/strict'
import {
  applyFolderDropLocally,
  applyItemDropLocally,
  buildKbTree,
  collectPinnedNodes,
  compareKbNodes,
  compareKbSiblingOrder,
  computeSiblingIndex,
  findKbFolderNode,
  flattenFolderOptions,
  flattenResults,
  folderPathLabel,
  folderSubtreeIds,
  KB_DND_MIME,
  localKbMatch,
  planAppendDrop,
  planKbDrop,
  resolveDropTarget,
  resolveFolderPath,
  sortKbSiblings,
  UNFILED_EXPAND_KEY,
  UNFILED_LABEL,
} from '../src/components/kb/kb-tree-utils.ts'

// ─── 测试夹具 ─────────────────────────────────────────────────────────────

const folder = (id, name, parentId = null, extra = {}) => ({
  id,
  name,
  parentId,
  order: 0,
  pinned: false,
  ...extra,
})

const item = (id, title, folderId = null, extra = {}) => ({
  id,
  title,
  bodyMd: '',
  tags: [],
  folderId,
  order: null,
  pinned: false,
  createdAt: '2026-01-01T00:00',
  updatedAt: '2026-01-01T00:00',
  ...extra,
})

/** 前序遍历整棵树，收集所有文件夹 id（用于断言每个节点恰好出现一次、无环展开） */
function walkFolderIds(tree) {
  const ids = []
  const visit = (node) => {
    ids.push(node.id)
    for (const child of node.folders) visit(child)
  }
  for (const root of tree.roots) visit(root)
  return ids
}

/** 中文名参考序：不写死 ICU 结果，用同一 locale 参考排序避免环境差异导致的脆断 */
const zhSorted = (names) => [...names].sort((a, b) => a.localeCompare(b, 'zh-CN'))

// ─── 极简测试运行器 ───────────────────────────────────────────────────────

let passed = 0
const failures = []

function test(name, fn) {
  try {
    fn()
    passed += 1
    console.log(`✓ ${name}`)
  } catch (error) {
    failures.push({ name, error })
    console.error(`✗ ${name}\n    ${error.message}`)
  }
}

// ─── 1. 层级合成 ─────────────────────────────────────────────────────────

test('buildKbTree：文件夹树 + 条目叶子 + 未分类归并', () => {
  const folders = [
    folder('f1', '设计'),
    folder('f2', '前端', 'f1'),
    folder('f3', '后端', 'f1'),
  ]
  const items = [
    item('i1', '按钮规范', 'f2'),
    item('i2', '设计原则', 'f1'),
    item('i3', '随手记', null),
    item('i4', '悬空归属', 'ghost-folder'), // 指向已删除文件夹 → 未分类兜底
    item('i5', '接口文档', 'f3'),
  ]
  const tree = buildKbTree(folders, items)

  assert.equal(tree.totalItems, 5)
  assert.equal(tree.roots.length, 1)
  const design = tree.roots[0]
  assert.equal(design.name, '设计')
  assert.equal(design.items.length, 1)
  assert.equal(design.items[0].id, 'i2')
  assert.deepEqual(design.folders.map((node) => node.name).sort(), ['前端', '后端'].sort())
  const frontend = design.folders.find((node) => node.name === '前端')
  assert.equal(frontend.id, 'f2')
  assert.deepEqual(frontend.items.map((i) => i.id), ['i1'])
  assert.deepEqual(tree.unfiled.map((i) => i.id).sort(), ['i3', 'i4'])
})

test('buildKbTree：空数据返回空树', () => {
  const tree = buildKbTree([], [])
  assert.deepEqual(tree.roots, [])
  assert.deepEqual(tree.unfiled, [])
  assert.equal(tree.totalItems, 0)
})

test('buildKbTree：文件夹对象字段完整保留（folder 原样引用）', () => {
  const folders = [folder('f1', '设计', null, { order: 7, pinned: true })]
  const tree = buildKbTree(folders, [])
  assert.equal(tree.roots[0].folder, folders[0])
  assert.equal(tree.roots[0].order, 7)
  assert.equal(tree.roots[0].pinned, true)
})

// ─── 2. 断链 / 自引用 / 成环兜底 ─────────────────────────────────────────

test('buildKbTree：parentId 断链的文件夹提升顶层', () => {
  const folders = [folder('f1', '孤儿', 'missing-parent')]
  const tree = buildKbTree(folders, [])
  assert.deepEqual(tree.roots.map((node) => node.id), ['f1'])
})

test('buildKbTree：祖先中途断链时保留直接父级（不误伤层级）', () => {
  const folders = [
    folder('fA', '中途断链', 'missing-parent'),
    folder('fB', '子级', 'fA'),
  ]
  const tree = buildKbTree(folders, [])
  assert.deepEqual(tree.roots.map((node) => node.id), ['fA'])
  assert.deepEqual(tree.roots[0].folders.map((node) => node.id), ['fB'])
})

test('buildKbTree：自引用文件夹提升顶层', () => {
  const folders = [folder('f1', '自引用', 'f1')]
  const tree = buildKbTree(folders, [])
  assert.deepEqual(tree.roots.map((node) => node.id), ['f1'])
  assert.deepEqual(tree.roots[0].folders, [])
})

test('buildKbTree：父子成环 / 长环提升顶层且节点不丢、不无限递归', () => {
  const folders = [
    folder('a', 'A', 'b'),
    folder('b', 'B', 'a'), // a ↔ b 互指
    folder('c', 'C', 'd'),
    folder('d', 'D', 'e'),
    folder('e', 'E', 'c'), // c → d → e → c 三节点长环
  ]
  const tree = buildKbTree(folders, [])
  const ids = walkFolderIds(tree).sort()
  assert.deepEqual(ids, ['a', 'b', 'c', 'd', 'e'])
  assert.equal(new Set(ids).size, ids.length, '每个文件夹节点必须恰好出现一次')
  // 环上节点全部提升顶层，避免任何节点的父子链残留成环
  assert.equal(tree.roots.length, 5)
})

test('buildKbTree：重复 id 只渲染一次且不挂重复子树', () => {
  const folders = [folder('f1', '设计'), folder('f1', '设计副本')]
  const tree = buildKbTree(folders, [])
  assert.equal(walkFolderIds(tree).length, 1)
})

// ─── 3. 排序：pinned desc → order asc(null=Infinity) → updatedAt desc → name ──

test('排序：pinned 优先于 order', () => {
  const folders = [
    folder('normal', '普通', null, { order: 0 }),
    folder('pinned', '置顶', null, { order: 99, pinned: true }),
  ]
  const tree = buildKbTree(folders, [])
  assert.deepEqual(tree.roots.map((node) => node.id), ['pinned', 'normal'])
})

test('排序：order asc，null 视为 Infinity 排同级最后', () => {
  const folders = [
    folder('null', '无序号', null, { order: null }),
    folder('two', '第二', null, { order: 2 }),
    folder('zero', '第一', null, { order: 0 }),
  ]
  const tree = buildKbTree(folders, [])
  assert.deepEqual(tree.roots.map((node) => node.id), ['zero', 'two', 'null'])
})

test('排序：条目 updatedAt desc，更新时间相同回落到标题 zh-CN', () => {
  const folders = [folder('f1', '设计')]
  const items = [
    item('old', '旧文', 'f1', { updatedAt: '2026-10-01T08:00' }),
    item('new', '新文', 'f1', { updatedAt: '2026-11-01T08:00' }),
    item('tie-b', '乙', 'f1', { updatedAt: '2026-09-01T08:00' }),
    item('tie-a', '甲', 'f1', { updatedAt: '2026-09-01T08:00' }),
  ]
  const tree = buildKbTree(folders, items)
  const order = tree.roots[0].items.map((i) => i.id)
  assert.equal(order[0], 'new')
  assert.equal(order[1], 'old')
  // 时间相同的两条按标题排序（参考同一 locale 的比较结果）
  const tieOrder = order.slice(2)
  const expectedTie = ['乙', '甲']
    .map((name) => ({ name, id: name === '乙' ? 'tie-b' : 'tie-a' }))
    .sort((x, y) => x.name.localeCompare(y.name, 'zh-CN'))
    .map((x) => x.id)
  assert.deepEqual(tieOrder, expectedTie)
})

test('排序：同级文件夹 name 按 zh-CN 排序（同等 pinned/order）', () => {
  const names = ['设计', '前端', '后端', '阿波罗']
  const folders = names.map((name, index) => folder(`f${index}`, name, null, { order: null }))
  const tree = buildKbTree(folders, [])
  assert.deepEqual(tree.roots.map((node) => node.name), zhSorted(names))
})

test('排序：未分类条目同样走 pinned/order/updatedAt 规则', () => {
  const items = [
    item('plain', '普通', null, { order: 0, updatedAt: '2026-10-01T08:00' }),
    item('pinned', '置顶', null, { order: 99, pinned: true, updatedAt: '2026-01-01T08:00' }),
  ]
  const tree = buildKbTree([], items)
  assert.deepEqual(tree.unfiled.map((i) => i.id), ['pinned', 'plain'])
})

test('compareKbNodes：order null 与数字混排时 null 永远靠后', () => {
  const mk = (order) => ({ pinned: false, order, name: 'x' })
  assert.ok(compareKbNodes(mk(0), mk(null)) < 0)
  assert.ok(compareKbNodes(mk(null), mk(3)) > 0)
  assert.equal(compareKbNodes(mk(null), mk(null)), 0)
})

// ─── 4. 路径面包屑 ───────────────────────────────────────────────────────

test('resolveFolderPath：嵌套文件夹返回根→叶', () => {
  const folders = [
    folder('root', '设计'),
    folder('child', '前端', 'root'),
    folder('leaf', '组件', 'child'),
  ]
  const path = resolveFolderPath(folders, 'leaf')
  assert.deepEqual(path.map((f) => f.id), ['root', 'child', 'leaf'])
  assert.equal(folderPathLabel(folders, 'leaf'), '设计 / 前端 / 组件')
})

test('resolveFolderPath：无归属 / 未知 id 返回空数组', () => {
  const folders = [folder('root', '设计')]
  assert.deepEqual(resolveFolderPath(folders, null), [])
  assert.deepEqual(resolveFolderPath(folders, undefined), [])
  assert.deepEqual(resolveFolderPath(folders, 'ghost'), [])
})

test('resolveFolderPath：父链断链只返回能走到的部分', () => {
  const folders = [folder('orphan', '孤儿', 'missing-parent')]
  const path = resolveFolderPath(folders, 'orphan')
  assert.deepEqual(path.map((f) => f.id), ['orphan'])
  assert.equal(folderPathLabel(folders, 'orphan'), '孤儿')
})

test('resolveFolderPath：环上节点不挂死，返回有限路径', () => {
  const folders = [folder('a', 'A', 'b'), folder('b', 'B', 'a')]
  const path = resolveFolderPath(folders, 'a')
  assert.ok(path.length <= 2, '环路径必须有界')
  assert.ok(path.some((f) => f.id === 'a'))
  assert.ok(path.some((f) => f.id === 'b'))
})

test('folderPathLabel：无归属统一为「未分类」', () => {
  assert.equal(folderPathLabel([], null), UNFILED_LABEL)
  assert.equal(folderPathLabel([folder('f1', '设计')], 'ghost'), UNFILED_LABEL)
})

// ─── 5. 过滤态扁平结果 ───────────────────────────────────────────────────

test('flattenResults：只保留命中项，带标题 + 路径面包屑', () => {
  const folders = [
    folder('f1', '设计'),
    folder('f2', '前端', 'f1'),
    folder('f3', '后端'),
  ]
  const items = [
    item('i1', '按钮规范', 'f2', { tags: ['前端', '组件'] }),
    item('i2', '接口文档', 'f3'),
    item('i3', '随手记', null),
    item('i4', '悬空条目', 'ghost'),
  ]
  const hitIds = new Set(['i1', 'i3', 'i4'])
  const results = flattenResults(items, folders, (entry) => hitIds.has(entry.id))

  assert.deepEqual(results.map((r) => r.item.id), ['i1', 'i3', 'i4'], '命中项顺序 = 输入顺序')
  assert.equal(results[0].title, '按钮规范')
  assert.equal(results[0].pathLabel, '设计 / 前端')
  assert.equal(results[0].folderId, 'f2')
  assert.equal(results[1].pathLabel, UNFILED_LABEL)
  assert.equal(results[1].folderId, null)
  assert.equal(results[2].pathLabel, UNFILED_LABEL, '悬空 folderId 兜底为未分类')
  assert.equal(results[2].folderId, null)
})

test('flattenResults：顶层文件夹条目不出现空面包屑', () => {
  const folders = [folder('f1', '设计')]
  const items = [item('i1', '设计原则', 'f1')]
  const results = flattenResults(items, folders, () => true)
  assert.equal(results[0].pathLabel, '设计')
})

test('flattenResults：matcher 全拒绝时返回空数组（真无命中，不冒充空库）', () => {
  const results = flattenResults([item('i1', 'x')], [], () => false)
  assert.deepEqual(results, [])
})

test('flattenResults：重复 folderId 的路径解析复用缓存且结果一致', () => {
  const folders = [folder('f1', '设计'), folder('f2', '前端', 'f1')]
  const items = [item('i1', 'a', 'f2'), item('i2', 'b', 'f2')]
  const results = flattenResults(items, folders, () => true)
  assert.deepEqual(results.map((r) => r.pathLabel), ['设计 / 前端', '设计 / 前端'])
})

// ─── 6. 本地检索兜底（与后端 kb.rs 同口径：标题 + 祖先文件夹名）────────────

test('localKbMatch：标题命中，大小写不敏感且裁剪空白', () => {
  const entry = item('i1', 'Button Guide', null, {
    tags: ['React'],
    bodyMd: '这是一段关于按钮的说明',
  })
  assert.equal(localKbMatch(entry, 'button'), true, '标题命中')
  assert.equal(localKbMatch(entry, ' BUTTON '), true, '查询两侧空白裁剪')
  assert.equal(localKbMatch(entry, '不存在的词'), false)
  assert.equal(localKbMatch(entry, '  '), true, '空查询视为全命中')
})

test('localKbMatch：匹配所有祖先文件夹名（父级 + 祖父级）', () => {
  const folders = [folder('f1', '设计'), folder('f2', '前端', 'f1')]
  const entry = item('i1', 'Button Guide', 'f2')
  assert.equal(localKbMatch(entry, '前端', folders), true, '直接父级文件夹命中')
  assert.equal(localKbMatch(entry, ' 设计 ', folders), true, '祖父级文件夹命中且裁剪空白')
  assert.equal(localKbMatch(entry, '不存在的词', folders), false)
  assert.equal(localKbMatch(entry, 'button', folders), true, '文件夹路径不影响标题命中')
})

test('localKbMatch：不再匹配标签与正文（D5 收窄）', () => {
  const folders = [folder('f1', '设计')]
  const entry = item('i1', 'Button Guide', 'f1', {
    tags: ['React'],
    bodyMd: '这是一段关于按钮的说明',
  })
  assert.equal(localKbMatch(entry, 'react', folders), false, '标签不再命中')
  assert.equal(localKbMatch(entry, '按钮', folders), false, '正文不再命中')
  assert.equal(localKbMatch(entry, 'button', folders), true, '标题仍命中')
})

test('localKbMatch：断链 / 未归属的兜底', () => {
  const broken = item('i1', 'Button Guide', 'missing')
  assert.equal(localKbMatch(broken, 'button', []), true, '标题仍可命中')
  assert.equal(localKbMatch(broken, '设计', [folder('f1', '设计')]), false, '断链文件夹不命中')

  const unfiled = item('i2', '未归属笔记')
  assert.equal(localKbMatch(unfiled, '未归属'), true, '未分类条目按标题匹配')
  assert.equal(localKbMatch(unfiled, '设计', [folder('f1', '设计')]), false, '未归属不吃任何文件夹名')
})

test('localKbMatch：成环与深度上限安全终止', () => {
  const loop = [folder('a', '循环A', 'b'), folder('b', '循环B', 'a')]
  const cyclic = item('i1', 'Cycle', 'a')
  assert.equal(localKbMatch(cyclic, '循环A', loop), true, '自身文件夹名命中')
  assert.equal(localKbMatch(cyclic, '不存在的词', loop), false, '成环查询安全终止')

  const folders = []
  for (let i = 0; i < 70; i += 1) {
    folders.push(folder(`d${i}`, `层级${i}`, i === 0 ? null : `d${i - 1}`))
  }
  const deep = item('i2', 'Deep', 'd69')
  assert.equal(localKbMatch(deep, '层级60', folders), true, '深度上限内命中祖先')
  assert.equal(localKbMatch(deep, '层级0', folders), false, '超过深度上限不再向上匹配')
})

test('UNFILED_EXPAND_KEY 不与真实文件夹 id 冲突（虚拟行 key 约定）', () => {
  assert.equal(typeof UNFILED_EXPAND_KEY, 'string')
  assert.ok(UNFILED_EXPAND_KEY.startsWith('__'))
  assert.equal(UNFILED_LABEL, '未分类')
})

// ─── 7. 「移动到…」菜单选项与禁选集合（tasks 2.5）──────────────────────────

test('flattenFolderOptions：先序遍历，depth / pathLabel 与树一致', () => {
  const folders = [
    folder('f1', '设计', null, { order: 0 }),
    folder('f2', '前端', 'f1', { order: 0 }),
    folder('f5', '样式', 'f2', { order: 0 }),
    folder('f3', '后端', null, { order: 1 }),
  ]
  const options = flattenFolderOptions(folders)
  assert.deepEqual(options.map((o) => o.id), ['f1', 'f2', 'f5', 'f3'], '根→叶先序')
  assert.deepEqual(options.map((o) => o.depth), [0, 1, 2, 0], '深度 = 祖先数')
  assert.deepEqual(
    options.map((o) => o.pathLabel),
    ['设计', '设计 / 前端', '设计 / 前端 / 样式', '后端'],
  )
  assert.equal(options[0].name, '设计')
})

test('flattenFolderOptions：置顶夹排在同级前面，断链夹兜底到顶层', () => {
  const folders = [
    folder('f1', '设计', null, { order: 0 }),
    folder('f2', '置顶的', null, { order: 9, pinned: true }),
    folder('f3', '孤儿', 'ghost'),
  ]
  const options = flattenFolderOptions(folders)
  assert.equal(options[0].id, 'f2', '置顶优先')
  const orphan = options.find((o) => o.id === 'f3')
  assert.ok(orphan, '断链夹不能丢')
  assert.equal(orphan.depth, 0, '断链夹提到顶层')
  assert.equal(orphan.pathLabel, '孤儿')
})

test('flattenFolderOptions：空列表返回空数组', () => {
  assert.deepEqual(flattenFolderOptions([]), [])
})

test('folderSubtreeIds：含自身与全部后代，叶子只有自身', () => {
  const folders = [
    folder('f1', '设计'),
    folder('f2', '前端', 'f1'),
    folder('f5', '样式', 'f2'),
    folder('f3', '后端'),
  ]
  assert.deepEqual([...folderSubtreeIds(folders, 'f1')].sort(), ['f1', 'f2', 'f5'])
  assert.deepEqual([...folderSubtreeIds(folders, 'f2')].sort(), ['f2', 'f5'])
  assert.deepEqual([...folderSubtreeIds(folders, 'f5')], ['f5'], '叶子只含自身')
  assert.deepEqual([...folderSubtreeIds(folders, 'f3')], ['f3'], '无后代只含自身')
  assert.deepEqual(
    [...folderSubtreeIds(folders, 'ghost')],
    ['ghost'],
    '未知 id 也返回自身（调用方据此禁选，不会把目标搬进自己）',
  )
})

test('folderSubtreeIds：环 / 自引用不会死循环', () => {
  const folders = [
    folder('a', 'A', 'b'),
    folder('b', 'B', 'a'),
    folder('c', 'C', 'c'),
  ]
  assert.deepEqual([...folderSubtreeIds(folders, 'a')].sort(), ['a', 'b'])
  assert.deepEqual([...folderSubtreeIds(folders, 'c')], ['c'])
})

test('flattenFolderOptions 与 folderSubtreeIds 配合：搬进自身子树被禁用', () => {
  const folders = [
    folder('f1', '设计', null, { order: 0 }),
    folder('f2', '前端', 'f1', { order: 0 }),
  ]
  const blocked = folderSubtreeIds(folders, 'f1')
  const allowed = flattenFolderOptions(folders).filter((o) => !blocked.has(o.id))
  assert.deepEqual(allowed, [], '自身与后代全部禁选')
})

// ─── S2 拖拽落点 / 同级排序 / 置顶（tasks 3.1–3.4，design D4/D5/D6）──────

/** S2 综合夹具：f1(设计) 下 c1/c2 与条目 i1/i2；f2(前端) 下条目 i3；f3(备忘)；i4 未分类 */
function dndTree() {
  return buildKbTree(
    [
      folder('f1', '设计', null, { order: 0 }),
      folder('f2', '前端', null, { order: 1 }),
      folder('f3', '备忘', null, { order: 2 }),
      folder('c1', '头条', 'f1', { order: 0 }),
      folder('c2', '卡片', 'f1', { order: 1 }),
    ],
    [
      item('i1', '甲', 'f1', { order: 0, updatedAt: '2026-01-02T00:00' }),
      item('i2', '乙', 'f1', { order: 1, updatedAt: '2026-01-01T00:00' }),
      item('i3', '丙', 'f2', { order: 0 }),
      item('i4', '丁', null, { order: 0 }),
    ],
  )
}

const drag = (kind, id) => ({ kind, id })

test('拖拽协议：MIME 是自定义类型，不接受外部文件类型', () => {
  assert.equal(KB_DND_MIME, 'text/kettd-kb')
})

test('resolveDropTarget：上 25% before / 中 50% inside / 下 25% after', () => {
  const rect = { top: 100, height: 40 }
  assert.equal(resolveDropTarget(rect, 100), 'before', '顶边')
  assert.equal(resolveDropTarget(rect, 109), 'before')
  assert.equal(resolveDropTarget(rect, 110), 'before', '25% 边界归 before')
  assert.equal(resolveDropTarget(rect, 111), 'inside')
  assert.equal(resolveDropTarget(rect, 120), 'inside', '正中')
  assert.equal(resolveDropTarget(rect, 129), 'inside')
  assert.equal(resolveDropTarget(rect, 130), 'after', '75% 边界归 after')
  assert.equal(resolveDropTarget(rect, 140), 'after', '底边')
})

test('resolveDropTarget：越界按最近边界、高度非正兜底 inside', () => {
  const rect = { top: 100, height: 40 }
  assert.equal(resolveDropTarget(rect, 10), 'before', '指针在行上方')
  assert.equal(resolveDropTarget(rect, 999), 'after', '指针在行下方')
  assert.equal(resolveDropTarget({ top: 100, height: 0 }, 100), 'inside')
  assert.equal(resolveDropTarget({ top: 100, height: -5 }, 50), 'inside')
})

test('computeSiblingIndex：before 取锚点索引、after 取 +1、缺失 -1', () => {
  const siblings = [{ id: 'a' }, { id: 'b' }, { id: 'c' }]
  assert.equal(computeSiblingIndex(siblings, 'a', 'before'), 0)
  assert.equal(computeSiblingIndex(siblings, 'b', 'before'), 1)
  assert.equal(computeSiblingIndex(siblings, 'b', 'after'), 2)
  assert.equal(computeSiblingIndex(siblings, 'c', 'after'), 3, '末尾 after = 追加')
  assert.equal(computeSiblingIndex(siblings, 'missing', 'before'), -1)
})

test('planKbDrop 同父下移：索引按「移除自身后」计算（不会多一位）', () => {
  // 原序 [i1(order0), i2(order1)]，拖 i1 到 i2 之后：移除 i1 → [i2]，插入 index 1 → [i2, i1]
  assert.deepEqual(planKbDrop(dndTree(), drag('item', 'i1'), drag('item', 'i2'), 'after'), {
    type: 'reorder',
    parentId: 'f1',
    index: 1,
  })
  // 文件夹同父下移：f1 到 f3 之后 → [f2, f3] 的末尾 index 2 → [f2, f3, f1]
  assert.deepEqual(planKbDrop(dndTree(), drag('folder', 'f1'), drag('folder', 'f3'), 'after'), {
    type: 'reorder',
    parentId: null,
    index: 2,
  })
})

test('planKbDrop 同父上移：索引不受自身位移影响', () => {
  // 拖 i2 到 i1 之前：移除 i2 → [i1]，index 0
  assert.deepEqual(planKbDrop(dndTree(), drag('item', 'i2'), drag('item', 'i1'), 'before'), {
    type: 'reorder',
    parentId: 'f1',
    index: 0,
  })
  assert.deepEqual(planKbDrop(dndTree(), drag('folder', 'f3'), drag('folder', 'f1'), 'before'), {
    type: 'reorder',
    parentId: null,
    index: 0,
  })
})

test('planKbDrop 跨父插入：parentId 取锚点父级，索引按目标同级算', () => {
  // 未分类 i4 插到 f2 的 i3 之前 → 父级变成 f2
  assert.deepEqual(planKbDrop(dndTree(), drag('item', 'i4'), drag('item', 'i3'), 'before'), {
    type: 'reorder',
    parentId: 'f2',
    index: 0,
  })
  // 未分类 i4 插到根层级文件夹 f2 之后 → 条目组首位（条目渲染在子文件夹之后）
  assert.deepEqual(planKbDrop(dndTree(), drag('item', 'i4'), drag('folder', 'f2'), 'after'), {
    type: 'reorder',
    parentId: null,
    index: 0,
  })
})

test('planKbDrop 中区：条目/文件夹移入目标文件夹', () => {
  assert.deepEqual(planKbDrop(dndTree(), drag('item', 'i4'), drag('folder', 'f2'), 'inside'), {
    type: 'into',
    parentId: 'f2',
  })
  assert.deepEqual(planKbDrop(dndTree(), drag('folder', 'f3'), drag('folder', 'f1'), 'inside'), {
    type: 'into',
    parentId: 'f1',
  })
})

test('planKbDrop 拒绝：自身、条目中区、同父中区 no-op', () => {
  assert.equal(planKbDrop(dndTree(), drag('item', 'i1'), drag('item', 'i1'), 'before'), null)
  assert.equal(planKbDrop(dndTree(), drag('folder', 'f1'), drag('folder', 'f1'), 'after'), null)
  // 条目没有「移入」语义
  assert.equal(planKbDrop(dndTree(), drag('item', 'i1'), drag('item', 'i2'), 'inside'), null)
  // 已经在该文件夹下：inside 不产生变化（避免后端追加到末尾）
  assert.equal(planKbDrop(dndTree(), drag('item', 'i1'), drag('folder', 'f1'), 'inside'), null)
  assert.equal(planKbDrop(dndTree(), drag('folder', 'c2'), drag('folder', 'f1'), 'inside'), null)
})

test('planKbDrop 拒绝：文件夹移入自身后代 / 插入自身子树内', () => {
  assert.equal(planKbDrop(dndTree(), drag('folder', 'f1'), drag('folder', 'c1'), 'inside'), null)
  // c1 的父级就是 f1 自己 → 插入后仍等于把 f1 塞进自身子树
  assert.equal(planKbDrop(dndTree(), drag('folder', 'f1'), drag('folder', 'c1'), 'before'), null)
  assert.equal(planKbDrop(dndTree(), drag('folder', 'f1'), drag('folder', 'c2'), 'after'), null)
})

test('planKbDrop 置顶优先：置顶节点拖到非置顶锚点后，索引折到置顶组末尾', () => {
  const tree = buildKbTree(
    [
      folder('p1', '甲', null, { order: 0, pinned: true }),
      folder('p2', '乙', null, { order: 1, pinned: true }),
      folder('n1', '丙', null, { order: 2 }),
    ],
    [],
  )
  // 展示序：置顶组 [p1, p2] → 非置顶组 [n1]；拖 p1 到 n1 之后
  // 落库 basis（不含 p1）=[p2(pinned), n1]，按普通 computed 是 2，会把 p1 塞到 p2 后 → 折到置顶组末尾 = 1
  assert.deepEqual(planKbDrop(tree, drag('folder', 'p1'), drag('folder', 'n1'), 'after'), {
    type: 'reorder',
    parentId: null,
    index: 1,
  })
  // 两个都置顶时按普通锚点索引算（仍在置顶组内微调）
  assert.deepEqual(planKbDrop(tree, drag('folder', 'p1'), drag('folder', 'p2'), 'after'), {
    type: 'reorder',
    parentId: null,
    index: 1,
  })
})

test('planKbDrop：置顶条目仍允许拖拽排序（pinned 只影响展示优先级）', () => {
  const tree = buildKbTree([folder('f', '夹', null)], [
    item('a', '甲', 'f', { order: 0, pinned: true }),
    item('b', '乙', 'f', { order: 1, pinned: true }),
  ])
  assert.deepEqual(planKbDrop(tree, drag('item', 'a'), drag('item', 'b'), 'after'), {
    type: 'reorder',
    parentId: 'f',
    index: 1,
  })
})

test('planAppendDrop 空区追加：不传 order，落点父级由调用方给定', () => {
  assert.deepEqual(planAppendDrop(dndTree(), drag('item', 'i1'), 'f2'), { type: 'into', parentId: 'f2' })
  assert.deepEqual(planAppendDrop(dndTree(), drag('item', 'i1'), null), { type: 'into', parentId: null })
  assert.deepEqual(planAppendDrop(dndTree(), drag('folder', 'f3'), 'f1'), { type: 'into', parentId: 'f1' })
  assert.deepEqual(planAppendDrop(dndTree(), drag('folder', 'f1'), null), { type: 'into', parentId: null })
})

test('planAppendDrop 拒绝：文件夹追加进自身子树 / 自身 / 目标不存在', () => {
  assert.equal(planAppendDrop(dndTree(), drag('folder', 'f1'), 'c1'), null)
  assert.equal(planAppendDrop(dndTree(), drag('folder', 'f1'), 'f1'), null)
  assert.equal(planAppendDrop(dndTree(), drag('folder', 'f3'), 'nope'), null)
})

test('applyItemDropLocally 同父：dense 重排 0..n-1，不改其它字段', () => {
  const items = [
    item('a', '甲', 'f', { order: 0 }),
    item('b', '乙', 'f', { order: 1 }),
    item('c', '丙', 'f', { order: 2 }),
  ]
  const next = applyItemDropLocally(items, 'c', 'f', 0)
  const byId = (id) => next.find((candidate) => candidate.id === id)
  assert.equal(byId('c').order, 0)
  assert.equal(byId('a').order, 1)
  assert.equal(byId('b').order, 2)
  assert.equal(items[0].order, 0, '入参不可变')
  assert.notEqual(byId('a'), items[0], '变更项返回新对象')
})

test('applyItemDropLocally 跨父：旧父重排 + 新父插入', () => {
  const items = [
    item('a', '甲', 'f1', { order: 0 }),
    item('b', '乙', 'f1', { order: 1 }),
    item('x', '丙', 'f2', { order: 0 }),
  ]
  const next = applyItemDropLocally(items, 'a', 'f2', 0)
  const byId = (id) => next.find((candidate) => candidate.id === id)
  assert.equal(byId('a').folderId, 'f2')
  assert.equal(byId('a').order, 0)
  assert.equal(byId('x').order, 1)
  assert.equal(byId('b').folderId, 'f1')
  assert.equal(byId('b').order, 0, '旧父剩余条目 dense 重排')
})

test('applyItemDropLocally：index 缺省 = 追加末尾（空区追加语义）', () => {
  const items = [
    item('a', '甲', 'f1', { order: 0 }),
    item('x', '丙', 'f2', { order: 0 }),
  ]
  const next = applyItemDropLocally(items, 'a', 'f2')
  assert.equal(next.find((candidate) => candidate.id === 'a').order, 1)
  assert.equal(next.find((candidate) => candidate.id === 'x').order, 0)
})

test('applyFolderDropLocally 同父排序：按 order → name 落库序 dense 重排', () => {
  const folders = [
    folder('f1', '设计', null, { order: 0 }),
    folder('f2', '前端', null, { order: 1 }),
    folder('f3', '备忘', null, { order: 2 }),
  ]
  const next = applyFolderDropLocally(folders, 'f3', null, 0)
  const byId = (id) => next.find((candidate) => candidate.id === id)
  assert.equal(byId('f3').order, 0)
  assert.equal(byId('f1').order, 1)
  assert.equal(byId('f2').order, 2)
  assert.equal(folders[0].order, 0, '入参不可变')
})

test('applyFolderDropLocally 跨父：parentId 改写 + 旧/新同级 dense 重排', () => {
  const folders = [
    folder('f1', '设计', null, { order: 0 }),
    folder('c1', '头条', 'f1', { order: 0 }),
    folder('c2', '卡片', 'f1', { order: 1 }),
  ]
  const next = applyFolderDropLocally(folders, 'c2', null, 0)
  const byId = (id) => next.find((candidate) => candidate.id === id)
  assert.equal(byId('c2').parentId, null)
  assert.equal(byId('c2').order, 0)
  assert.equal(byId('f1').order, 1, '目标同级 dense 重排')
  assert.equal(byId('c1').parentId, 'f1')
  assert.equal(byId('c1').order, 0)
})

test('sortKbSiblings / compareKbSiblingOrder：order 升序、null 末尾、更新时间倒序', () => {
  const sorted = sortKbSiblings([
    { id: 'late', order: null },
    { id: 'older', order: 0, updatedAt: '2026-01-01T00:00' },
    { id: 'newer', order: 0, updatedAt: '2026-02-01T00:00' },
    { id: 'tail', order: 2 },
  ])
  assert.deepEqual(sorted.map((node) => node.id), ['newer', 'older', 'tail', 'late'])
  assert.ok(compareKbSiblingOrder({ id: 'a', order: null }, { id: 'b', order: 0 }) > 0)
})

test('findKbFolderNode：按 id 在合成树里定位（含嵌套），缺失返回 null', () => {
  const tree = dndTree()
  assert.equal(findKbFolderNode(tree, 'c1')?.name, '头条')
  assert.equal(findKbFolderNode(tree, 'f2')?.name, '前端')
  assert.equal(findKbFolderNode(tree, 'nope'), null)
})

test('collectPinnedNodes：只收置顶项，排序同 compareKbNodes，带路径提示', () => {
  const folders = [
    folder('f1', '设计', null, { order: 0, pinned: true }),
    folder('f2', '备忘', null, { order: 2, pinned: true }),
    folder('f3', '普通', null, { order: 0 }),
  ]
  const items = [
    item('i1', '甲', 'f1', { order: 1, pinned: true }),
    item('i2', '乙', null, { order: 3, pinned: true }),
    item('i3', '丙', 'f1', { order: 0, pinned: false }),
  ]
  const pinned = collectPinnedNodes(folders, items)
  assert.deepEqual(pinned.map((node) => node.id), ['f1', 'i1', 'f2', 'i2'])
  assert.equal(pinned[0].kind, 'folder')
  assert.equal(pinned[0].pathLabel, '设计', '文件夹路径为自身')
  assert.equal(pinned[1].pathLabel, '设计', '条目路径为所在文件夹')
  assert.equal(pinned[3].pathLabel, UNFILED_LABEL, '无归属条目路径为未分类')
  assert.equal(pinned[1].item.title, '甲', '原始对象随行返回，供右键菜单复用')
})

test('collectPinnedNodes：空数据/全未置顶时为空数组（快捷区不渲染）', () => {
  assert.deepEqual(collectPinnedNodes([], []), [])
  assert.deepEqual(collectPinnedNodes([folder('f', '甲')], [item('i', '乙', 'f')]), [])
})

// ─── 汇总 ────────────────────────────────────────────────────────────────

console.log('')
if (failures.length > 0) {
  console.error(`失败 ${failures.length} 项 / 通过 ${passed} 项`)
  process.exit(1)
}
console.log(`全部通过：${passed} 项`)