# Design

## Context

标题此前是工具条里的 `<input>`：它是「文件的属性」，不是文档的第一行。用户要求把标题移到纸面上、顶部只留只读镜像，并明确选**方案 A**——标题不进 Markdown 文档树。

现状约束：

- 数据侧只有 `KbItem.title` 一个字段，`bodyMd` 只存正文；改名走 `updateKbItem({ id, title })`，回包经 `onSaved` 局部回填父列表（不触发全量重拉，避免「保存闪烁」）。
- 详情页的滚动容器是 `.kb-detail-scroll`，工具条 `.kb-detail-strip` 在其外；标题必须落在滚动区内，才会随文滚动。
- AGENTS 冻结字号：chrome 只有四档，编辑器正文只有 `--fs-reading`，禁止新增字号值。
- 内置字体 `Kettd Serif` 只有 61 字报头子集，标题必须走 `--font-reading-title` 的系统衬线栈。

## Goals / Non-Goals

**Goals**

- 标题出现在纸面首行，点进即改，随文滚动；顶部退化为一行只读镜像。
- 一份数据、一条保存链路：树/搜索/标签页与纸面永远同一份名字。
- Markdown 保持干净：`bodyMd` 里没有标题行。

**Non-Goals**

- 不做逐条目的独立标题字段，不做标题历史，不做重命名撤销栈。
- 不改正文 `bodyMd` 的存储格式，不动备份/恢复链路。
- 本轮不处理「导入含 `# 标题` 首行的 Markdown」的回填（见 Open Questions）。

## Decisions

### D1 标题不进文档树（方案 A）

标题是一个独立的可编辑块 `.kb-doc-title`，排在 `.kb-detail-main` 顶端、标签行之上，**不是** ProseMirror 文档的第一个节点。

备选 B（标题就是文档首个 H1）被否，理由三条：每次保存都要把首个 H1 提回条目名，两个真相源；正文里用户自己再插 H1 时与文档标题打架；删标题块要做一次「文档结构手术」。

### D2 用 `contentEditable` 而不是 `<input>` / `<textarea>`

`<input>` 单行不换行，长标题只能横向滚动或截断；`<textarea>` 自带原生滚动条与 resize 把手，且换行是真换行（要另做行数限制与 `Enter` 拦截）。`contentEditable` 的换行来自文本折行，视觉上就是「文章第一行」。代价是必须自己处理受控同步（D3）与粘贴（D6）。

### D3 非受控 DOM + 单向绘制

React 不管理 `contentEditable` 的 children。实现上用 `paintTitle(value)` 单向把文本写回 DOM，且只在**焦点不在标题上**时刷（`document.activeElement !== titleElRef.current`），避免打字时被回灌。

外部改名（树里重命名）与切条目走**渲染期就地校正**：`syncedTitle` 与 `item.title` 不一致时直接 `setState`，不用 effect（否则多一轮渲染，React Compiler 的 `set-state-in-effect` 也会报）。

### D4 `Enter` / `Esc` 的语义

- `Enter`：`preventDefault` → `blur()`（触发失焦落盘）→ `apiRef.current.focusStart()`（光标进正文第一段）。不插入换行。
- `Esc`：`preventDefault` → `paintTitle(item.title)` 复原 → `blur()`。失焦回调里读到的是原名，`saveTitle` 提前返回，不写盘。

### D5 空标题不落盘

清空后失焦时恢复原名，而不是把名字存成空。存空会让左侧文件树出现无名条目、搜索结果无法定位。占位符「未命名文档」只在条目本来没有名字时出现（`:empty::before { content: attr(data-placeholder) }`），与原型文案一致但触发条件更窄。

### D6 复制时补首行标题

`bodyMd` 不含标题，所以「复制 Markdown」由详情页组装：`# ${title}\n\n${bodyMd}`，标题为空时只复制正文。编辑器只回吐 `bodyMd`（`EditorApi.getMarkdown()` 仍是「与落盘同一口径」）。

### D7 字号跟随阅读档位

标题字号 `calc(var(--fs-reading) * 2)`：阅读档位（12–24px）变化时标题一起长，不新增字号令牌、不违反 AGENTS 的字号冻结。

## Risks / Trade-offs

- **`contentEditable` 的边界未真机验证**：中文输入法组合态、WebView2 下的 `:empty` 占位符（浏览器可能在清空后留下 `<br>`）、粘贴行为都需要开应用确认。缓解：粘贴显式拦截并走 `execCommand('insertText')`；失焦即复原，空态只在编辑中短暂出现。
- **`document.execCommand` 已废弃**：WebView2 仍支持；若未来失效，粘贴退化为不响应（不会损坏内容）。
- **标题不再在工具条可改**：老用户可能找不到入口。缓解：树里右键「打开·重命名」会把焦点送到纸面标题并全选；标题悬停有淡底、光标为文本光标。
- **方案 A 的固有代价**：导出/复制要靠调用方补首行，任何新的导出路径（未来 PDF/Word）都得再拼一次。

## Migration Plan

无数据迁移。旧条目的 `title` 原样显示在纸面首行；`bodyMd` 不变，已有正文不会被插入或剥离标题行。

回滚：还原 `kb-item-detail.tsx`、`tiptap-editor.tsx`、`kb.css`、`docs/capability/06-知识库.md` 四个文件即可，无持久化副作用。

## Open Questions

1. 正文里是否还允许用户自插一级标题？允许的话，它与文档标题视觉上会很接近。
2. 导入一篇首行是 `# 标题` 的 Markdown 时，要不要把这一行提回条目名（而不是留在正文里）？
3. 顶部只读镜像是否需要点击跳回标题、或复制标题的小动作？