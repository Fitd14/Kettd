# KB Phase 2 补充设计 — 知识便签交互细节

> 2026-09-09 · 基于 kb-spec.md §2/§3/§6 拍板结论，头脑风暴补全边缘场景与交互流程。
> 本文档与 kb-spec.md + docs/superpowers/plans/2026-09-09-kb-sticky-task.md 配套，用于后续核查。

## 1. 知识便签创建流程

### 1.1 入口
- **KB 视图详情工具栏**：「📌 贴到桌面」按钮（kb-item-detail.tsx toolbar）
- 点击→调用 `createSticky({ kbRef: item.id })`→后端建 note:* 动态窗

### 1.2 创建后行为
- 窗口出现在桌面（位置：若已有便签，级联 +32px 偏移；首次=屏幕中央偏右上）
- 窗口标题显示 KB 条目标题（非「便签」）
- 内容区显示 KB 条目 bodyMd（WYSIWYG 编辑态）
- 窗口 always_on_top=true（常驻置顶）
- 埋点：`kb_pin_desktop { itemId }`

### 1.3 重复贴到桌面
- 同一条目贴两次→后端创建第二个 note:* 窗（两个视口指向同一 KB 条目）
- 两个窗口独立编辑→都写回同一 KB 数据→最终以最后保存者为准（last-write-wins，不冲突——单用户场景）
- **设计决策**：不做「已贴提示」（简单可靠；用户贴两张说明需要两个视角）

## 2. 知识便签窗口行为

### 2.1 窗口尺寸
- 展开态：380×456（与自由便签一致）
- 缩小态：380×40（mini 置顶条，显示条目标题）

### 2.2 数据流（实时视口语义）
```
KB 条目 bodyMd (唯一真相)
    ↕ 防抖 800ms 双向同步
知识便签窗 TipTapEditor
    ↕ 同一 store-changed 广播
主窗 KB 视图 MdStaticRenderer / TipTapEditor
```

- 便签内编辑→防抖 800ms→`updateKbItem(kbRef, { bodyMd })`→store-changed 广播→主窗 KB 视图刷新
- 主窗 KB 编辑→updateKbItem→store-changed 广播→便签窗 TipTapEditor 外部 sync（`useEffect` on `markdown` prop）
- **无锁**：单用户场景，last-write-wins；800ms 防抖减少写入频率

### 2.3 缩小态（mini）
- mini 条显示：条目标题（非正文第一行，与自由便签区分）
- 点击标题→展开（`updateSticky({ mini: false })`）
- 拖动→记位（notePos）
- ✕→销毁便签（不删 KB 数据）

### 2.4 关闭行为
- ✕ 或 Alt+F4→`deleteSticky(id)`→只销毁窗口
- KB 条目数据不受影响（`kb_ref` 只在 StickyNote 上，KB 侧无反向引用）
- 关闭后如需重新贴→在 KB 视图再点「贴到桌面」

## 3. 失效处理

### 3.1 KB 条目被删除时
- store-changed 广播→便签窗 refresh→`getKbItems()` 查不到 kbRef 对应条目→`kbInvalid=true`
- 渲染「已失效」状态：
  ```
  ┌─────────────────────────┐
  │ ⚠️ 已失效（原资料已删除） │
  │ [拆除便签]               │
  └─────────────────────────┘
  ```
- 点击「拆除便签」→`deleteSticky(id)`→窗口关闭
- **不自动关闭**：用户可能还想看快照内容

### 3.2 KB 条目被重命名
- 便签窗口标题跟随刷新（store-changed→refresh→kbItem.title 更新）
- 无失效问题（id 不变）

## 4. 分池上限

### 4.1 计数规则
- 自由便签：`stickies.filter(s => !s.kbRef).length`（现有 STICKY_CAP=6）
- 知识便签：`stickies.filter(s => s.kbRef).length`（STICKY_KB_CAP=4）
- 互不影响：贴第 5 张知识便签→报错「知识便签已达上限（4张），请先拆掉不需要的」
- 自由便签到 6 张→报错不变

### 4.2 设置页显示
- 便签管理区显示：`自由便签 N/6 · 知识便签 M/4`
- 知识便签行显示：条目标题 + KB 标签 +「拆除」按钮

## 5. 视觉区分

### 5.1 知识便签 vs 自由便签
| 特征 | 自由便签 | 知识便签 |
|------|---------|---------|
| 标题行 | 「便签」 | KB 条目标题 |
| 内容区 | textarea / MD 预览 | TipTap WYSIWYG |
| 字数限制 | ≤500 | 无限制 |
| MD 开关 | 手动切换 | 默认 WYSIWYG（无开关——始终渲染） |
| 编辑写回 | updateSticky(content) | updateKbItem(bodyMd) |
| 关闭语义 | 删除便签数据 | 只拆窗口，KB 数据不动 |

### 5.2 CSS 区分
- 知识便签：`.sticky-note.knowledge-sticky` class
- 标题行加小书签图标：`📎 {title}`
- 可选：纸色略深一档（视觉暗示「这是知识」）

## 6. TaskDetail 挂载区交互

### 6.1 已挂条目 chip
- 显示：📎 条目标题 + [↗ 跳转] + [× 卸载]
- 悬空引用：标题显示「已失效」+ 删除线 + 无跳转按钮
- 跳转：`window.location.hash = '#/kb?id={itemId}'`→KB 视图自动选中

### 6.2 挂载操作
- 点击「+ 挂载」→KbAttachPicker 弹层
- 弹层：搜索框 + 全量条目列表 + 多选 checkbox + 底部「挂载 (N)」按钮
- 提交→`updateTask(id, { kbRefs: [...selected] })`→整体替换（非增量）
- 现有挂载的条目默认勾选

### 6.3 卸载
- 点击 chip 上的 ×→从 kbRefs 中移除该 id→`updateTask(id, { kbRefs: next })`
- 卸载不删 KB 条目（只是解除引用关系）

## 7. 「条目→建待办」流程

### 7.1 触发
- KB 详情工具栏「📝 转为待办」按钮
- 点击→KbNewTaskDialog 弹窗

### 7.2 弹窗内容
- 标题输入框：预填 KB 条目标题（可修改）
- 分类下拉：工作/学习/生活（默认「工作」）
- 说明文字：「创建后自动挂载到此知识条目」
- 按钮：「取消」+「创建并挂载」

### 7.3 创建逻辑
1. `addTask({ title, category, note: '' })`→获取 task.id
2. `updateTask(task.id, { kbRefs: [item.id] })`→挂载 KB 条目
3. 关闭弹窗→刷新 KB 视图
4. 埋点：`kb_item_to_task { itemId }`

## 8. Deep Link

### 8.1 `#/kb?id={itemId}`
- TaskDetail 的「↗ 跳转」按钮设置 `window.location.hash = '#/kb?id={itemId}'`
- KbView 启动时读取 hash 中的 `?id=` 参数→自动选中该条目
- 支持：从任务详情直接跳到 KB 并定位到挂载的资料

## 9. 埋点清单（Phase 2 新增）

| 事件 | 触发时机 | 属性 |
|------|---------|------|
| `kb_pin_desktop` | 点击「贴到桌面」 | `{ itemId }` |
| `kb_unpin_desktop` | 知识便签拆除 | `{ itemId, reason: 'user'|'invalid' }` |
| `kb_item_to_task` | 「转为待办」创建成功 | `{ itemId }` |
| `task_kb_attach` | TaskDetail 挂载资料 | `{ taskId, count }` |
| `task_kb_detach` | TaskDetail 卸载资料 | `{ taskId, count }` |

## 10. 测试清单（Phase 2 新增）

| # | 测试项 | 类型 |
|---|--------|------|
| 1 | StickyNote.kb_ref serde：旧数据无字段→默认 None，零迁移 | Rust 单测 |
| 2 | create_sticky({ kbRef }) 成功→sticky 含 kb_ref | Rust 单测 |
| 3 | create_sticky({ kbRef }) 知识便签达 4 上限→报错 | Rust 单测 |
| 4 | create_sticky({ kbRef }) 自由便签达 6 上限→报错（不受知识便签数影响） | Rust 单测 |
| 5 | 知识便签编辑→update_kb_item 生效（bodyMd 同步） | 真机 |
| 6 | 关闭知识便签→KB 数据仍在 | 真机 |
| 7 | 删除 KB 条目→知识便签显示「已失效」 | 真机 |
| 8 | TaskDetail 挂载/卸载资料→kbRefs 正确更新 | 真机 |
| 9 | 「转为待办」→任务创建+kbRefs 挂载 | 真机 |
| 10 | Deep link #/kb?id=xxx→自动选中条目 | 真机 |
