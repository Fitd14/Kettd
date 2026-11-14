# 缺陷复现证据 — 知识库编辑器（#1 / #2 / #10 / #9）

> 只读调查产物。未修改项目源码、未执行 git 写命令、未启动应用、未运行 `tauri build` / 前端构建。无头实验脚本全部位于 `%TEMP%`，调查结束已删除；未新增任何依赖。文中「推断」= 本次无法无头验证、需真机确认的结论。

## 环境

| 项 | 值 |
|---|---|
| 项目根 | `C:\Users\simy\project\mywork\kettd` |
| 调查时间 | 2026-11-13 23:00–23:30（Asia/Shanghai） |
| git HEAD | `2167a48`（完整 `2167a48c238bb1a86c88cd33dd79157809ca3eea`，2026-11-13 22:38:47 +0800，msg「20261113-1438-c285-处理-createwebview-错误并指定可写用户数据目录：记进展」） |
| Node | v22.23.3（Windows x64） |
| Tiptap | `@tiptap/core` / `@tiptap/react` / `@tiptap/starter-kit` / `@tiptap/extension-heading` 均 3.31.3；`tiptap-markdown` 0.9.0；`prosemirror-markdown` 1.13.7 |
| React / 构建 | react/react-dom 19.2.8；vite 8.2.2；jsdom 24.1.3（`src-react/node_modules` 内既有 transitive 依赖） |
| 后端 | tauri 1.8.3（`src-tauri/Cargo.lock`） |
| 工作树 | 不干净：`git status --porcelain` = `M src-react/src/rendering/styles/prosemirror.css`、`M src-react/src/rendering/tiptap-editor.tsx`、`M src-tauri/Cargo.toml`、`M src-tauri/src/kb.rs` + 未跟踪 `openspec/`；`git diff --stat` = 3 files changed, 217 insertions(+), 36 deletions(-)（prosemirror.css +21/−2、tiptap-editor.tsx +173/−32、kb.rs +23/−2；Cargo.toml 仅 LF/CRLF 提示，无内容 diff）。**本报告行号 = 当前工作树行号**；`KbView.tsx`、`kb-item-detail.tsx`、`App.tsx`、`commands.rs`、`store.rs`、`fs_store.rs`、`kb.css` 无未提交改动，行号同 HEAD。 |

行号自查：`tiptap-editor.tsx` 的未提交改动主要是斜杠菜单定位/命令链 refactor（diff hunk 如 `@@ -69,0 +72,15 @@`、`@@ -123 +145,32 @@`、`@@ -422,0 +529,24 @@`），**未新增标题分级入口**（SLASH_ITEMS 仍只有 h1/h2，工具栏仍只有 H1/H2 两个标题按钮）；`prosemirror.css` 改动为列表 marker 等样式；`kb.rs` 改动为 `normalize_body_md`（只裁首尾换行、保留行尾空格）。#10 的「入库归一化」按当前工作树实现复刻；若按 HEAD 的 `body_md.trim()`，行尾空格会被额外裁掉（见 #10 备注）。

取证方式：逐行阅读 + 只读 grep / git 查询；#1 用 jsdom 挂载真实 `KbView`（Vite 程序化打包）；#10 用 jsdom + 与 `tiptap-editor.tsx` 相同的扩展栈跑 Markdown 往返；#2 用 Node/V8 测序列化耗时与扩展构建；#9 用 `@tiptap/extension-heading` 源码 + jsdom 输入规则模拟 + CSS 规则核对。临时脚本与 JSON 结果位于 `C:\Users\simy\AppData\Local\Temp\dsh-yx3Fg2\kettd-repro`，调查结束已删除（含指向 `src-react/node_modules` 的 junction，未触及项目依赖）。

需求映射（对照 `proposal.md`「What Changes / 知识库编辑器」与 `specs/knowledge-base/editor/spec.md`；编号取自任务清单）：

| 编号 | proposal 条目 | spec 需求 |
|---|---|---|
| #1 | 消除「保存后闪烁」 | 保存过程不打断编辑 |
| #2 | 提升输入流畅度 | 输入过程保持流畅 |
| #10 | 收敛 Markdown 格式漂移 | Markdown 保存保真 |
| #9 | 标题分级 / 链接插入 | 标题级别可分级设置 / 行内格式与链接插入 |

---

## #1 保存时界面闪烁（知识库左栏）

### 现象

在知识库条目里保存（标题失焦保存、标签保存、正文 800ms 自动保存）后，左栏列表在保存请求期间**顶部插入一整块「📚 知识库空空如也」空态**，已有条目被整体下推；请求返回后空态消失、列表弹回 → 视觉上闪一下。保存越慢（IPC/写盘慢、条目多）空态停留越久。

### 证据

1. **刷新时无条件置 loading**：`src-react/src/views/KbView.tsx:47-70` 的 `refresh`：`:48 setLoading(true)` → `:51 await getKbItems()` → `:63 setItems(...)` → `:67-68 finally setLoading(false)`；`:72 useEffect(() => { void refresh() }, [refresh])`。不是仅首屏 loading，而是每次刷新都走。
2. **loading 空态与列表同时渲染（闪烁的根因）**：`KbView.tsx:173-190` 左清单：
   - `:174 {loading && <KbEmptyState variant="empty" />}`
   - `:175 {!loading && error && <KbEmptyState variant="error" … />}`
   - `:176-181` 空列表/无结果分支（有 `onCreate`）
   - `:182-189 {filteredItems.map(item => <KbItemRow … />)}`

   `:174` 与 `:182-189` 是并列的兄弟节点，**没有互斥**：`loading===true` 时既有 `.kb-empty` 又有旧的行；其余分支都用 `!loading` 保护。`KbEmptyState` 未传 `onCreate` 时只渲染「📚 + 知识库空空如也」。
3. **三种保存入口都会触发刷新**：`src-react/src/components/kb/kb-item-detail.tsx`
   - `:36-41 saveTitle`：标题变化 → `await updateKbItem(id,{title})` → `onRefresh()`
   - `:46-49 saveTags`：标签变化 → `updateKbItem(id,{tags})` → `onRefresh()`
   - `:61-68 handleSave(bodyMd)`：`await updateKbItem(id,{bodyMd})` → `setSaveState` → `onRefresh()`

   KbView 把 `refresh` 作为 `onRefresh` 传入 `:211-229 <KbItemDetail …>`。
4. **正文自动保存每 800ms 触发一次**：`src-react/src/rendering/tiptap-editor.tsx:263-277` `onUpdate` → 800ms 防抖 → `:270 getMarkdown(editor)` → `:271-273` 内容变化时 `onSave(md)` → `kb-item-detail.tsx:61-68 handleSave` → `onRefresh()`。停笔 800ms 后即触发一次上屏闪烁。
5. **无头复现（真实组件）**：jsdom + Vite 程序化 build 打包真实 `KbView.tsx`（`@/lib/api` 换成 deferred Promise 替身、`@/rendering/tiptap-editor` 换成保留 `markdown` 的渲染替身；#1 验证的是保存→onRefresh 链路，与编辑器实现无关）；按真实路径执行：点行选中 → `.kb-title-input` 改值 → `focusout`（触发 `saveTitle`）→ `updateKbItem` → `onRefresh`。关键输出（原样，stderr 仅有 act 警告）：

   ```text
   react.version=19.2.8 typeof act=function
   {"label":"1-mount-pending-ipc","rows":[],"emptyInList":"📚知识库空空如也","kbListFirstChildClass":"kb-empty","bothEmptyStateAndRows":false,"getKbItemsCalls":1,"updateKbItemCalls":0}
   {"label":"2-initial-loaded","rows":["条目 A","条目 B"],"emptyInList":null,"kbListFirstChildClass":"kb-item-row","bothEmptyStateAndRows":false,"getKbItemsCalls":1,"updateKbItemCalls":0}
   {"label":"3-after-select","rows":["条目 A","条目 B"],"emptyInList":null,"kbListFirstChildClass":"kb-item-row active","bothEmptyStateAndRows":false,"getKbItemsCalls":1,"updateKbItemCalls":0}
   {"label":"4-DURING-SAVE-refresh-pending","rows":["条目 A","条目 B"],"emptyInList":"📚知识库空空如也","kbListFirstChildClass":"kb-empty","bothEmptyStateAndRows":true,"getKbItemsCalls":2,"updateKbItemCalls":1}
   lastUpdatePatch={"id":"a","patch":{"title":"条目 A（改名）"}}
   {"label":"5-save-refresh-done","rows":["条目 A","条目 B"],"emptyInList":null,"kbListFirstChildClass":"kb-item-row active","bothEmptyStateAndRows":false,"getKbItemsCalls":2,"updateKbItemCalls":1}
   DONE
   ```

   第 4 行是缺陷瞬间：保存触发第 2 次 `getKbItems` 且其 pending 时，左清单首子元素变成 `.kb-empty` 空态，**同时**两条列表行仍在 DOM（`bothEmptyStateAndRows=true`）；第 5 行说明 refresh 完成后空态消失。
6. **空态块的实际布局影响**：`src-react/src/views/kb.css:290-305` `.kb-empty{display:flex;flex-direction:column;align-items:center;justify-content:center;gap:.75rem;padding:3rem;…}`、`.kb-empty .empty-icon{font-size:2.5rem}`；`kb.css:114-121` `.kb-list{display:flex;flex-direction:column;gap:.25rem;overflow-y:auto}`。空态块按 CSS 约 3rem×2 padding + 2.5rem 图标 + 文案/间距 ≈ 180–200px 高（**推断**：按 16px 计），插在 `.kb-list` 最前面 → 已有行被推下，loading 结束后推回，构成可见闪烁。
7. **放大因素（全 App 重渲染，推断增加卡顿但非空态来源）**：保存后 Rust `update_kb_item` 发 `store-changed`（`src-tauri/src/commands.rs:853-855`；`emit` 见 `:26-28` `app.emit_all("store-changed", event)`）；Tauri v1.8.3 的 `AppHandle::emit_all` 过滤器恒 `true`，**包含发起窗口自身**（`C:\Users\simy\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\tauri-1.8.3\src\lib.rs:646-649`）；`src-react/src/App.tsx:59-62` 监听 `store-changed` → `:48-57 getBootstrap()` → `setBoot` → 整 App 重渲染。KbView 自身没有 `store-changed` 监听（grep 无匹配），所以它不会因此多拉一次列表；闪烁来源仍是 KbView 本地 `loading` 空态。
8. **排除项**：`KbItemDetail` 的 `key={selected.id}`（`KbView.tsx:211`）在保存期间不变 → 编辑器/详情不会重挂载；闪烁不是「列表 key 重建」导致，而是 `:174` 的 loading 空态被插入。

### 复现步骤（真机）

1. 启动 Kettd → 侧栏「知识库」；确保左栏至少有 2 条知识。
2. 左栏点选一条，右侧编辑标题后点击别处（失焦触发保存），或在正文里输入任意字符后停手 ≥ 800ms（触发自动保存）。
3. 注视左栏：保存请求在途期间，列表顶部会出现「📚 知识库空空如也」，已有条目整体下移；请求完成后空态消失、条目弹回。
4. 可重复：连续修改标题/正文，每次保存都会出现一次；条目越多、数据越大（`get_kb_items` 返回全量）越明显。

### 是否可无头验证

**可（已做）**。真实 `KbView` + 真实 `KbItemDetail` 保存链路在 jsdom + Vite 打包下复现出 `loading` 空态与列表行并存（上方第 5 条输出）。唯一替身是 `TipTapEditor` 渲染组件（jsdom 中挂载真实 Tiptap 会触发 React `Maximum update depth exceeded`，属 jsdom 环境伪影，未定位）；保存入口用标题失焦，不经过编辑器，因此该替身不影响本项结论。视觉层面的「闪」强弱需真机。

### 需真机确认点

- 空态块的实际高度/闪烁幅度（本报告 180–200px 为按 CSS 计算的推断）及是否伴随滚动位置跳动。
- IPC + 写盘耗时下空态停留时长；真实数据量（KB 上限 5000 条）下的可感知程度。
- `store-changed` → `getBootstrap` 全 App 重渲染是否叠加额外卡顿（本次未测真实帧）。

---

## #2 Markdown 编辑不流畅

### 现象

- 每输入一个字符，编辑器组件都会重渲染整个工具栏 / BubbleMenu（现象级：输入延迟，长文档更明显）。
- 停笔 800ms 后保存瞬间卡顿：整篇 Markdown 被重新序列化，耗时随文档增大**超线性**增长（2k 段量级实测 100–250ms 主线程计算，推断在 WebView2 上同样存在）。
- 保存完成后还会全量重拉 KB 列表并整 App 重渲染（见 #1 证据 7），放大保存瞬间的卡顿感。

### 证据

**A. 每键入触发全组件重渲染（每次击键）**

- `src-react/src/rendering/tiptap-editor.tsx:377-382`：`editor.on('transaction', rerender)`，`rerender = () => bumpRender((k) => k + 1)`；ProseMirror 每个输入/选区事务都更新组件 state → 整个 `TipTapEditor` 函数组件重渲染。
- 重渲染时整段重建：`tools` 数组（`:394-405`，10 项，每项调用 `editor.isActive(...)`）、工具栏 JSX（`:440-509`）、2 个 `BubbleMenu`（`:511-551`）、扩展/options 对象（`:239-258`）。
- 每次重渲染后 `useEditor` 的副作用还会再执行：`@tiptap/react` 3.31.3 `dist/index.js`：`:453 useEffect(instanceManager.onRender(deps))`（**未给依赖数组 → 每次 render 后都执行**）→ `:396-411 onRender`：当 `deps.length===0` 且 editor 存在时，若 `compareOptions` 不通过就 `editor.setOptions({...this.options.current, editable})`；`compareOptions`（`:364-390`）对 `onUpdate` 等回调键直接返回 true，但 `extensions` 按**元素身份**比较（`:379-385`）——Kettd 每次 render 都新建 `StarterKit.configure(...)` 等扩展实例（`tiptap-editor.tsx:240-258`），身份必然不同 → **每次 render 都触发 `editor.setOptions`**。
- `editor.setOptions` 本身不重建扩展，但会强制视图更新：`@tiptap/core` 3.31.3 `dist/index.js:6299-6307`：合并选项 → `if (this.options.editorProps) this.view.setProps(...)` → `this.view.updateState(this.state)`。即每次击键除了 React 重渲染外，还额外做一次 `setProps` + 全量 `updateState`。
- 线索核实（原线索「`useEditor` 无 deps 导致每次 render 重建 extensions/回调」）：**部分不成立**。`useEditor`（@tiptap/react `dist/index.js:447-463`）用 `useState` 持有 EditorInstanceManager，deps 默认 `[]` 且不变 → **不会重建 Editor 实例**；但扩展数组每次 render 新构造（分配成本实测 ≈0.0147ms/次，300 次均值，非瓶颈，**推断**真正开销在 React 重渲染 + 上述 setOptions/视图更新，而非扩展构建）。

**B. 保存时整篇序列化，且随文档增大超线性**

- 保存路径：`tiptap-editor.tsx:263-277` 800ms 防抖 → `:270 getMarkdown(editor)`；实现为 `:26-33 editor.storage.markdown.getMarkdown()` → `tiptap-markdown` 0.9.0 `dist`：`:991-992 serializer.serialize(this.editor.state.doc)`（每次新建 `MarkdownSerializerState`、重建 nodes/marks 映射，见 `dist:752-757`、`:759-772`）。
- 实测（Node v22 V8，纯 JS 序列化，不涉 DOM；每档独立 editor，15 次取 median，`--expose-gc`）：

  | 文档规模 | 正文字节数 | getMarkdown median |
  |---|---|---|
  | 250 段 × 40 字 | 9.7 KB | 2.18 ms |
  | 500 段 | 19.4 KB | 3.92 ms |
  | 1000 段 | 39 KB | 12.19 ms |
  | 1500 段 | 59 KB | 24.12 ms |
  | 2000 段 | 79 KB | 102.02 ms |
  | 2500 段 | 99 KB | 219.00 ms |

  对照：同 editor `getJSON()` 0.2–1.5ms、`state.doc.textContent` 0.4–2.3ms（遍历文档本身不慢），说明瓶颈在 Markdown 序列化。
- 超线性根因（第三方库）：`prosemirror-markdown` 1.13.7 `dist/index.js:613-615`：
  `atBlank() { return /(^|\n)$/.test(this.out) }`，而 `text()`（`:645-655`）→ `write()`（`:628-634`）→ `flushClose()` 中调用 `atBlank()`，每次写文本都对**已累积的整个输出字符串**跑一次未锚定正则 → O(n²)；实测把 tiptap 序列化器换回 `prosemirror-markdown` 原生序列化器耗时几乎相同（2000 段×40 字：135.92ms vs 131.59ms；2500 段：262.14 vs 261.29ms），排除 tiptap 映射层嫌疑。
- 单次键入后若触发保存的量级参考：20KB 文档一次 `insertText` 11.266ms、随后序列化 7.366ms（另一轮 perf 脚本数据；非应用每键路径，仅表示保存量级）。

**C. 保存链路的后端与刷新开销**

- 保存正文 = `update_kb_item`（`src-tauri/src/commands.rs:836-857`）→ `save_notes()`：`src-tauri/src/store.rs:965-970` 对**整个知识库数组** `serde_json::to_string_pretty(&self.kb)` → `src-tauri/src/infra/fs_store.rs:288-291 write_notes` → `write_data_at`（`:37-49` tmp+rename 原子替换）。KB 上限 5000 条（`src-tauri/src/kb.rs:12 NOTES_FILE_MAX_ITEMS=5000`），条数越多写盘序列化越久（未实测耗时）。
- 保存返回后：`KbView.tsx:47-70` 全量 `get_kb_items`（`commands.rs:809-812` 返回 `store.kb.clone()` 全量）→ `:182-189` 重渲染所有 `KbItemRow`（`src-react/src/components/kb/kb-item-row.tsx:19` `export function KbItemRow(...)`，无 `memo`）；同时 `commands.rs:855` emit `store-changed` → `App.tsx:62` → `:48-57 getBootstrap()` 全量 + `setBoot` 整 App 重渲染。
- 结果：一次自动保存 = 整篇 Markdown 序列化 + IPC + 整库 pretty 序列化/写盘 + 全量列表重渲染 + 整 App 重渲染（Rust 写盘/IPC/帧耗时未测，**推断**这是「保存后不流畅」的组成）。

**D. 解析/回填开销（参考，jsdom 放大）**

- `setContent`（打开条目 / 外部同步 `tiptap-editor.tsx:366-374` 时）在 jsdom 中 median：2KB 33.82ms、20.9KB 121.707ms、106.3KB 544.783ms。jsdom 的 DOM 实现显著放大解析成本，WebView2 会更快，只能看增长趋势（**推断**）。

### 复现步骤（真机）

1. 打开知识库一个较长条目（或准备接近 5000 字的正文；spec 场景为「接近 5000 字」）。
2. 连续快速输入文字，感受每个字符的响应（每键均触发组件重渲染 + `setOptions`/视图更新，长文档下更明显）。
3. 停手 ≥800ms 触发自动保存，观察保存瞬间是否出现可感知停顿；进入多段落/多列表的长文档（千段量级）时停顿更明显（序列化实测 100–250ms 量级，**推断** WebView2 中相近）。
4. 保存完成瞬间观察左栏列表与整窗口是否有刷新/重绘（见 #1）。

### 是否可无头验证

**部分可（已做）**。序列化耗时与超线性根因、`setOptions` 每次 render 触发的机制、保存链路代码路径均在无头环境验证；输入延迟/掉帧、WebView2 真实耗时、Rust 写盘耗时无法无头验证。

### 需真机确认点

- 实际输入延迟/掉帧（尤其 2k+ 段落、99KB 级文档）与「接近 5000 字」场景的体验。
- 保存瞬间主线程阻塞时长、Rust 写盘 + IPC 延迟。
- 每次击键的额外 `view.updateState` 在真实 WebView2 中的成本（本报告未测帧）。

---

## #10 保存后 Markdown 格式变化

### 现象

条目保存后，正文的 Markdown 原文被 tiptap-markdown 序列化器重写：等价语法被归一化（`*`→`-`、`1)`→`1.`、`~~~`→``` 等），**部分信息有损甚至数据丢失**：Markdown 图片整体消失、表格转义竖线 `\|` 在第二次保存时变成列分隔符并丢内容、表格对齐标记 `:---:` 丢失、HTML 被转义为实体、下划线格式无法表达；且保存链路没有「有损转换」提示。

### 证据

1. **保存内容以序列化结果为准**：`src-react/src/rendering/tiptap-editor.tsx:263-277`：800ms 防抖 → `getMarkdown(editor)`（tiptap-markdown 把 ProseMirror doc 序列化为 Markdown）→ `:271-273` 变化时 `onSave(md)`；`kb-item-detail.tsx:61-68` → `updateKbItem({bodyMd})` → Rust `kb.rs:53-81 update`：`body_md: normalize_body_md(...)`（工作树 `kb.rs:14-22`：**只裁首尾 `\n`/`\r`，保留行尾空格与缩进**）→ 落盘。即编辑器里显示的 doc 决定 Markdown，原文不经保留。
2. **序列化 schema 缺少图片节点**：实测同一扩展栈的 schema：`SCHEMA_NODES=paragraph,blockquote,bulletList,doc,hardBreak,heading,horizontalRule,listItem,orderedList,text,codeBlock,taskList,taskItem,table,tableRow,tableHeader,tableCell`（无 `image`）；`SCHEMA_MARKS=link,bold,code,italic,strike,underline`。`@tiptap/starter-kit` 3.31.3 内含扩展清单已核对：bold/blockquote/bulletList/code/codeBlock/document/dropcursor/gapcursor/hardBreak/heading/undoRedo/horizontalRule/italic/listItem/listKeymap/link/orderedList/paragraph/strike/text/underline/trailingNode，**不含 Image**；`tiptap-editor.tsx:240-258` 也未添加 Image 扩展。
3. **试验方法（可复现）**：jsdom（`pretendToBeVisual:true`，window/document 挂 global）+ 与 `tiptap-editor.tsx:239-258` 相同的扩展栈（`StarterKit.configure({codeBlock:false})`、`CodeBlockLowlight`、`TaskList`、`TaskItem.configure({nested:true})`、`Table`、`TableRow`、`TableHeader`、`TableCell`、`Placeholder`、`Markdown.configure({html:false,linkify:true,breaks:true})`；slashExtension 不含 schema 节点，省略）；59 个 Markdown 样例：`editor.commands.setContent(md,{emitUpdate:false})` → `getMarkdown(editor)` 得 out1 → `stored = normalize(out1)`（复刻工作树 `kb.rs:14-22`，只裁首尾 `\n\r`）→ 再 `setContent(stored)` → out2，比较 out2 与 stored 判断幂等。统计：**TOTAL=59、CHANGED=31、NOT_IDEMPOTENT=1、ERRORS=0**。
4. **31 条变更清单（原文 → 首次保存后；内容为脚本原样输出，`\n` 为换行转义）**：

   ````text
   ul-star        "* 星号列表项一\n* 星号列表项二" -> "- 星号列表项一\n- 星号列表项二"
   ul-plus        "+ 加号列表项一\n+ 加号列表项二" -> "- 加号列表项一\n- 加号列表项二"
   ol-paren       "1) 第一项\n2) 第二项" -> "1. 第一项\n2. 第二项"
   nested-ul-4sp  "- 父\n    - 子（4空格）" -> "- 父\n  - 子（4空格）"
   task-upper-x   "- [X] 已完成大写" -> "- [x] 已完成大写"
   task-mixed     "- [ ] 未完成\n- [x] 已完成\n  - [ ] 嵌套" -> "- [ ] 未完成\n\n- [x] 已完成\n\n  - [ ] 嵌套"
   table-align    "| 左 | 中 | 右 |\n|:---|:---:|---:|\n| a | b | c |" -> "| 左 | 中 | 右 |\n| --- | --- | --- |\n| a | b | c |\n"   （对齐冒号全丢）
   table-plain    "| a | b |\n|---|---|\n| 1 | 2 |" -> "| a | b |\n| --- | --- |\n| 1 | 2 |\n"
   table-no-edge-pipes  "a | b\n--- | ---\n1 | 2" -> "| a | b |\n| --- | --- |\n| 1 | 2 |\n"
   table-escaped-pipe   "| a | b |\n|---|---|\n| x \\| y | z |" -> "| a | b |\n| --- | --- |\n| x | y | z |\n"   （\\| 变列分隔；二次保存 -> "| a | b |\n| --- | --- |\n| x | y |"，数据丢失，非幂等）
   fence-tilde    "~~~js\nconst x = 1\n~~~" -> "```js\nconst x = 1\n```"
   fence-indented "    indented code line1\n    line2" -> "```\nindented code line1\nline2\n```"
   setext-h1      "一级标题\n========" -> "# 一级标题"
   setext-h2      "二级标题\n--------" -> "## 二级标题"
   heading-closing  "## 标题 ##" -> "## 标题"
   softbreak-single "第一行\n第二行" -> "第一行\\\n第二行"
   hardbreak-two-spaces "第一行  \n第二行" -> "第一行\\\n第二行"
   blockquote     ">引用第一行\n>引用第二行" -> "> 引用第一行\\\n> 引用第二行"
   bare-url       "请看 https://example.com/path?q=1 这个链接" -> "请看 <https://example.com/path?q=1> 这个链接"
   www-url        "访问 www.example.com 看看" -> "访问 [www.example.com](http://www.example.com) 看看"
   ref-link       "[引用][1]\n\n[1]: https://example.com" -> "[引用](https://example.com)"   （引用定义被删）
   image          "![一张图](https://example.com/a.png)" -> ""   （图片 + alt 全部丢失）
   inline-html    "这是 <b>粗体</b> 文本" -> "这是 &lt;b&gt;粗体&lt;/b&gt; 文本"
   html-block     "<div class=\"x\">\nhello\n</div>" -> "&lt;div class=\"x\"&gt;\\\nhello\\\n&lt;/div&gt;"
   underline-html "<u>下划线</u>" -> "&lt;u&gt;下划线&lt;/u&gt;"
   em-underscore  "_斜体_ 与 __粗体__" -> "*斜体* 与 **粗体**"
   hr-star        "***" -> "---"
   hr-underscore  "___" -> "---"
   mulit-blank    "第一段\n\n\n\n第二段" -> "第一段\n\n第二段"
   trailing-spaces  "行尾有空格   \n\n下一段" -> "行尾有空格\n\n下一段"
   mixed-doc      "## 小节\n\n- [ ] 任务一\n\n> 引用\n\n```ts\nconst a = 1\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |" -> "## 小节\n\n- [ ] 任务一\n\n> 引用\n\n```ts\nconst a = 1\n```\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n"
   ````

   （注：31 条中 `storedEqualsOut1=false` 的 5 条均为表格类（out1 末尾多一个换行，入库前被 `normalize_body_md` 裁掉）；`mixed-doc` 同理；其余 26 条 out1 与入库值一致。）
5. **二次保存稳定性（幂等）**：59 例中 58 例在「入库值 → 再解析 → 再序列化」后与入库值一致；唯一不稳定是 `table-escaped-pipe`，且第二次保存丢失单元格 `z`（上表）。未被改动的 28 个样例（列表 `-`/`1.`、``` 代码块、`###` 标题、`*斜体*`、`~~删除线~~`、`[点击](url "标题")`、`<url>` autolink、转义 `\*`/`\#`、emoji、行内码、2 空格嵌套列表、`[ ]` 空列表项、引用内列表等）的 id 为：`ul-dash,ol-start3,ol-dot,fence-backtick,fence-lang-cn,hardbreak-backslash,autolink,md-link,md-link-title,em-star,em-both,strike,hr-dash,blockquote-nested,blockquote-empty,nested-ul-2sp,nested-ol,loose-list,empty-li-dash,empty-li-ol,escape-star,escape-hash,unicode-emoji,inline-code,code-block-empty,heading-h3,heading-raw-hash-no-space,quote-with-list`。
6. **触发条件（重要，避免误判影响面）**：这些改写发生在「Markdown 文本被解析进编辑器文档」之后——即打开已存条目（`tiptap-editor.tsx:259 content: markdown` 由 tiptap-markdown 解析）、或未来的粘贴/导入/图片入口。编辑器内用 UI 新建的内容一开始就是序列化器风格，因此不会持续漂移（幂等 58/59 佐证）。但只要文档中存在上述语法，**任意一次正文编辑**（触发 800ms 自动保存，`onUpdate` → 整篇重新序列化并 `onSave`）就会把整篇重写：图片被删、表格对齐被删、HTML 被转义、`~~~`/setext 等风格被改、`*`/`+` 变 `-`。这是「静默改变/丢失用户内容」的缺陷面。
7. **有损无提示**：`Markdown.configure({html:false})`（`tiptap-editor.tsx:252-256`）意味着 HTML/underline 等无法用 Markdown 表达的内容被转义或丢弃；序列化时 stderr 出现 `Tiptap Markdown: "underline" mark is only available in html mode`（见 #9 证据），但 **UI 上无任何「有损转换」提示**（spec「有损转换必须提示」未满足）。

### 复现步骤（真机）

1. 准备一条其 `body_md` 已含上述语法的知识条目（可从外部写入 `notes.json` 后启动，或粘贴/导入含这些语法的 Markdown——注意：直接在编辑器里键入 `*`/`![]()` 等文本不会生成对应文档节点，需以「Markdown 解析」路径载入正文才复现；打开已有条目即走解析路径）。
2. 在该条目正文里随便输入一个字符（触发整篇重新序列化与 800ms 自动保存）；仅改标题不会触发正文 `onUpdate`。
3. 停手 ≥800ms，等保存完成；关闭并重开该条目（或直接查看 `%APPDATA%\todo-list\notes.json` 中该条的 `bodyMd`）。
4. 对比原文：图片、表格对齐、HTML、`~~~`、setext 等发生变化/丢失；`*`/`+` 列表变 `-`；`[X]` 变 `[x]`。
5. 含转义竖线的表格：保存两次后观察单元格内容被吞（`z` 丢失）。

### 是否可无头验证

**可（已做）**。round-trip 使用真实 tiptap 扩展栈 + 与保存链路一致的归一化函数，得到 59 例统计与完整变更清单；无头无法验证的是「真实用户入口（粘贴/导入/未来图片）」以及真实 `notes.json` 落盘结果。

### 需真机确认点

- 真实粘贴/导入路径是否能让 Markdown 语法进入编辑器文档（本次只验证「解析后的 doc → 保存」这一段）。
- 保存后 `%APPDATA%\todo-list\notes.json` 中该条 `body_md` 的实际内容（本报告未读写生产数据目录）。
- 图片缺失对用户数据的实际影响面（现有数据中是否已有图片 Markdown）。
- 重建/重开条目后的渲染表现（`.kb-md-render` 只读渲染）。

### 备注（工作树归一化差异）

本报告幂等结论使用当前工作树 `kb.rs:14-22`（只裁首尾换行、保留行尾空格）。HEAD 版本此处为 `body_md.trim()`（`git diff` 显示 `- body_md: body_md.trim().to_string()` 被替换）；若按 HEAD 实现，行尾空格会被裁掉——例如空列表项 `"- "` 会变成 `"-"`，重开时被解析成普通段落（列表凭空消失）。该行为在本次 59 例中未触发（空列表项样例 `empty-li-dash` 在 worktree 归一化下幂等），但属于 HEAD 保存链路的潜在额外漂移（**推断**，未单独跑 HEAD 版脚本）。

---

## #9 标题分级与行内格式现状

### 现状摘要

| 能力 | 现状 | 证据 |
|---|---|---|
| 工具栏标题 | 只有 H1、H2 两个按钮；**无标题下拉**、无「正文」/H3–H6 按钮 | `tiptap-editor.tsx:394-405`（:398 H1、:399 H2，共 10 个按钮） |
| 斜杠菜单标题 | 只有 `/h1`、`/h2`；无 h3–h6 条目 | `tiptap-editor.tsx:48-67`（:61-64 h1/h2） |
| 输入式标题 | 生效：Heading 扩展默认 `levels:[1,2,3,4,5,6]`，行首 `### ` 变 H3（无头实测） | `@tiptap/extension-heading` 3.31.3 `src/heading.ts:52`、`:141-151`；实测见下 |
| 编辑态标题样式 | 仅有大标题档：h1 1.35em / h2 1.2em / h3 1.05em；**h4/h5/h6 无任何规则** | `src-react/src/rendering/styles/prosemirror.css:80-84` |
| 阅读态标题样式 | h1–h4 有 margin（`:216-223`），字号仅 h1 1.5em / h2 1.3em / h3 1.15em（`:225-227`）；**h4 无字号、h5/h6 无规则** | 同上 `:209-227` |
| 斜体 | 已支持（按钮 + BubbleMenu + Markdown `*…*`） | `tiptap-editor.tsx:396`、BubbleMenu `:522-525`；实测序列化 `*格式文本*` |
| 删除线 | 已支持（按钮 + BubbleMenu + Markdown `~~…~~`） | `tiptap-editor.tsx:397`、BubbleMenu `:525`；实测序列化 `~~格式文本~~` |
| 下划线 | schema 里有（StarterKit 自带 underline），但**无 UI 入口**，且非 html 模式序列化丢失（stderr 警告） | 实测 `getMarkdown` 输出纯文本 + 警告 `Tiptap Markdown: "underline" mark is only available in html mode` |
| 链接插入 | 仍是浏览器原生 `window.prompt`，未用居中对话框 | `tiptap-editor.tsx:483-497`（:490 `window.prompt('链接地址（留空取消链接）')`） |

### 证据

1. **工具栏只有 H1/H2、无下拉**：`tiptap-editor.tsx:394-405` `tools` 数组 10 项：Bold、Italic、Strikethrough、Heading1（:398）、Heading2（:399）、List、ListOrdered、ListTodo、TextQuote、Code；工具栏渲染 `:440-509` 中没有任何 `<select>`/下拉组件（只有按钮与表格网格选择器 `:463-481`）。H1/H2 为独立按钮，不能表达「正文/H3/H4」，且点击 H1 再点 H2 之类的切换语义由 `toggleHeading` 决定。
2. **SLASH_ITEMS 只有 h1/h2**：`tiptap-editor.tsx:48-67`：table、ul、ol、task、quote、code、h1（:61-62）、h2（:63-64）、hr；无 h3–h6。
3. **输入式标题生效（`### ` 等）**：Kettd 未 `configure` Heading 扩展（`tiptap-editor.tsx:240-258` 无 Heading 配置，StarterKit 默认）：
   - `@tiptap/extension-heading` 3.31.3 `src/heading.ts:52` `levels: [1, 2, 3, 4, 5, 6]`；
   - `:72-77` `parseHTML` 映射 h1–h6（`tag: 'h1'` … 到 h6）；
   - `:131-139` 注册快捷键 `Mod-Alt-${level}`（1–6 全部）；
   - `:141-151` `addInputRules` 对每个 level 生成 `textblockTypeInputRule({ find: new RegExp(\`^(#{${Math.min(...levels)},${level}})\\s$\`), type: this.type, getAttributes: { level } })` → 实测生成的 6 条正则：level1 `^(#{1,1})\s$`，level2 `^(#{1,2})\s$`，level3 `^(#{1,3})\s$`，…，level6 `^(#{1,6})\s$`。
   - 无头实测（jsdom，模拟 ProseMirror 文本输入 `view.someProp('handleTextInput', f => f(view, 4, 4, ' '))`，以及直接匹配输入规则）：`###` + 空格后首节点为 `{type:'heading', attrs:{level:3}}`；`####`..`######` 解析为 level 4/5/6 并原样 round-trip（`parsedLevels 1..6`）。注：这是规则引擎实测，非真实键盘事件。
4. **H4–H6 无样式（编辑器）**：`prosemirror.css:80-84` 只定义 `.kb-detail .kb-editor-content h1/h2/h3`（1.35em/1.2em/1.05em）与 `p`；对 `h4|h5|h6` 的全文件 grep 仅命中 `:219 .kb-md-render h4`（阅读态），**编辑器态无 h4/h5/h6 规则**。阅读态 `.kb-md-render` 只给 h1–h4 设 margin（`:216-223`）、字号仅 h1–h3（`:225-227`）→ 阅读态 H4 与正文同字号、H5/H6 无样式。
5. **Tailwind preflight 会清掉标题默认字重/字号**：`src-react/src/index.css:1-3` `@import "tailwindcss"; @import "tw-animate-css"; @import "./rendering/styles/prosemirror.css";`；`src-react/node_modules/tailwindcss/preflight.css:78-86` 对 `h1,h2,h3,h4,h5,h6` 设 `font-size: inherit; font-weight: inherit;`。→ **推断**：编辑器内 H4–H6 与正文完全同样式（同字号同字重），H1–H3 只有字号差异、字重也被 reset 成常规（非粗体）；阅读态 H4 同理。需真机确认视觉效果。
6. **斜体/删除线已存在**：schema marks 实测包含 `italic`、`strike`（`SCHEMA_MARKS=link,bold,code,italic,strike,underline`）；工具栏 `:396-397`；BubbleMenu `:511-527`（:522-525 B/I/行内码/删除线）；Markdown 序列化实测：bold→`**格式文本**`、italic→`*格式文本*`、strike→`~~格式文本~~`、code→`` `格式文本` ``。
7. **下划线不可用**：underline 在 starter-kit 3.31.3 扩展清单中（schema 有该 mark），但 `tiptap-editor.tsx` 工具栏/BubbleMenu 均无入口；且 tiptap-markdown 在 `html:false` 下无法序列化 underline → 实测带 underline mark 的文档 `getMarkdown` 只输出文本并在 stderr 警告 `Tiptap Markdown: "underline" mark is only available in html mode`。
8. **链接插入仍是 prompt**：`tiptap-editor.tsx:483-497` 链接按钮 `onClick` 调用 `window.prompt('链接地址（留空取消链接）')`（:490），非 spec 要求的居中对话框（预填当前链接、确定/取消/移除、Enter/Esc）。BubbleMenu #1 不含链接按钮。
9. **对照 spec 差距**：`specs/knowledge-base/editor/spec.md:42-51`「标题级别可分级设置」要求「正文 / H1–H4 下拉」+ 行首 `#` 到 `####` 输入 + 编辑态/阅读态字号样式；当前缺下拉、缺正文/H3/H4 控件、H4 编辑态无样式（H4–H6 视觉不可辨，**推断**）。`:53-62`「行内格式与链接插入」要求斜体/删除线 + 居中链接对话框；前者已有，后者未实现（仍是 prompt）。`### ` 输入式标题本身已生效（可达 H3–H6），但无样式/入口导致用户难以使用（**推断**）。

### 复现步骤（真机）

1. 启动 Kettd → 知识库 → 选中/新建条目，进入右侧编辑器。
2. 看工具栏：标题相关只有 H1、H2 两个按钮（无下拉、无正文/H3/H4）；输入 `/` 打开斜杠菜单，标题项只有「标题 1」「标题 2」。
3. 行首输入 `### 第三级`，再输入 `#### 第四级`：应即时转为三级/四级标题；对比视觉：H3 有轻微字号差异，H4 与正文几乎无差别（推断，需真机确认）；H5/H6 同理无样式。
4. 选中文字点工具栏「斜体」「删除线」/ 用 BubbleMenu：格式生效；保存后重开，Markdown 为 `*…*`、`~~…~~`。
5. 点工具栏「链接」：弹出浏览器原生 prompt（非居中对话框），确定后插入链接；留空取消链接。
6. 尝试给文字加下划线：无入口（工具栏与浮动菜单均无）。

### 是否可无头验证

**部分可（已做）**。扩展配置（levels/parseHTML/快捷键/输入规则）、输入规则命中 `### `、schema marks、Markdown 序列化结果、CSS 规则缺失均已无头验证；标题实际视觉（字号/字重）、真实键盘输入转换、链接 prompt 的真实 UI 需真机。

### 需真机确认点

- 编辑器内 H1–H3 / H4–H6 的实际视觉层级（字号、字重；preflight reset 后推断 H1–H3 不再加粗、H4–H6 与正文无差别）。
- 真实键盘输入 `### `（含中文输入法状态）是否即时转换、光标行为。
- 工具栏/Slash 菜单之外是否有隐藏的标题入口（本次全量代码检索未发现，判定无）。
- 链接 `window.prompt` 在 WebView2 中的实际表现（是否可用、样式一致性）。

---

## 附：本次无头验证的边界

- 未启动应用、未构建、未运行 Rust 测试；所有运行期/视觉结论均标注「推断」或列入「需真机确认点」。
- 未读写 `%APPDATA%\todo-list` 生产数据；#1/#10 的保存行为均在测试替身/内存 editor 上模拟。
- 未修改任何项目源码；工作树中已有的 4 个 `M` 文件为调查前既有状态，本报告只读引用其行号。
- 临时脚本与输出（含 junction）已从 `%TEMP%\dsh-yx3Fg2\kettd-repro` 删除。