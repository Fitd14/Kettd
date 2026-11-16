# Tasks

> 实施状态（2026-11-16）：代码已全部落盘，静态门禁已过。除「真机复核」外均已在本地验证；真机项待用户在应用内确认后再勾。

## 1. 数据与保存链路

- [x] 1.1 标题仍只读写 `KbItem.title`：改名复用 `updateKbItem(item.id, { title })`，回包经 `onSaved` 局部回填，不新增字段、不新增命令。 验证：`npx tsc -b` exit 0；`grep -n "updateKbItem(item.id, { title" src-react/src/components/kb/kb-item-detail.tsx` 命中 `saveTitle`。
- [x] 1.2 空标题不落盘：失焦时若文本为空，`paintTitle(item.title)` 复原并 `setTitle(item.title)`，不发请求。 验证：`commitDocTitle` 中 `if (!raw) { paintTitle(...); setTitle(...); return }`。
- [x] 1.3 外部改名同步：树里重命名导致 `item.title` 变化时，渲染期用 `syncedTitle` 校正状态并由 effect 重画 DOM。 验证：`grep -n "syncedTitle" src-react/src/components/kb/kb-item-detail.tsx` 命中 4 处；lint `set-state-in-effect` 计数未增加（34 warnings）。

## 2. 纸面标题

- [x] 2.1 `.kb-doc-title` 排在 `.kb-detail-main` 顶端（标签行之上、正文之前），随 `.kb-detail-scroll` 滚动。 验证：`grep -n "kb-doc-title" src-react/src/components/kb/kb-item-detail.tsx` 命中 JSX 与 `className`。
- [x] 2.2 点进即全选：`focusDocTitle()` 用 `document.createRange()` + `selectNodeContents` 选中整条。 验证：同上文件 `focusDocTitle` 实现；`npx tsc -b` exit 0。
- [x] 2.3 `Enter` 落盘并进正文：`titleElRef.current?.blur()` + `apiRef.current?.focusStart()`；`EditorApi` 新增 `focusStart()` 走 `editor.chain().focus('start')`。 验证：`grep -n "focusStart" src-react/src/rendering/tiptap-editor.tsx src-react/src/components/kb/kb-item-detail.tsx`。
- [x] 2.4 `Esc` 复原：`paintTitle(item.title)` 后 `blur()`，失焦回调因 `raw === item.title` 提前返回不写盘。 验证：`onDocTitleKeyDown` 的 `Escape` 分支。
- [x] 2.5 粘贴只取单行纯文本：`preventDefault()` 后取 `text/plain`、压空白、`execCommand('insertText')`。 验证：`onDocTitlePaste` 实现。
- [x] 2.6 占位符「未命名文档」：`data-placeholder` + `:empty::before { content: attr(data-placeholder) }`。 验证：`grep -n "kb-doc-title:empty" src-react/src/views/kb.css`。

## 3. 顶部只读镜像

- [x] 3.1 工具条左端 `<input class="kb-strip-title">` 换成 `.kb-strip-ro`（锁图标 + 单行省略文本 + `只读` 标签），不可聚焦、不可编辑。 验证：`grep -n "kb-strip-title" src-react/src` 零命中。
- [x] 3.2 镜像跟同一份状态：打字时随 `title` 状态实时更新，落盘后由 `saveState` 变「已保存 HH:mm」。 验证：`kb-strip-ro-text` 绑定 `title.trim() || '未命名文档'`。

## 4. 复制 Markdown

- [x] 4.1 复制时在正文前拼 `# 标题`，标题为空则只复制正文。 验证：`copyMarkdown` 内 `heading ? \`# ${heading}\n\n${body}\`.trimEnd() : body`；`npx tsc -b` exit 0。
- [x] 4.2 落盘的 `bodyMd` 不含标题行（标题不在文档树内）。 验证：`node src-react/test/md-roundtrip.test.mjs` → `TOTAL=22 CHANGED=10 NOT_IDEMPOTENT=0 TEXT_LOST=0 FAILURES=0`。

## 5. 样式与字号

- [x] 5.1 `.kb-doc-title` 用 `--font-reading-title`（不含 `Kettd Serif`），字号 `calc(var(--fs-reading) * 2)`，随阅读档位缩放。 验证：`grep -n "kb-doc-title" src-react/src/views/kb.css` 命中 `font-family` 与 `font-size`。
- [x] 5.2 静置态无边框无背景、悬停淡底、聚焦更淡的底；选中色走 `hsl(var(--seal) / 0.2)`（颜色只用令牌）。 验证：`node scripts/css-parse.mjs` → 12 张全 OK。
- [x] 5.3 删除 `.kb-strip-title` 三条输入框样式，不留下未消费的选择器。 验证：`grep -n "kb-strip-title" src-react/src/views/kb.css` 零命中。

## 6. 文档与门禁

- [x] 6.1 `docs/capability/06-知识库.md` 新增「文档标题」一节，并改写编辑器、工具条两段与已实现清单。 验证：`node test/docs.test.mjs` → `# pass 4 / # fail 0`。
- [x] 6.2 门禁：类型、lint、CSS 解析、契约测试、Markdown 往返全过。 验证：`npx tsc -b` exit 0；`npm run lint` → 34 warnings / 0 errors；`node scripts/css-parse.mjs` → 12 张 0 失败；`node test/command-parity.test.mjs` → 6/0；`node src-react/test/md-roundtrip.test.mjs` → FAILURES=0。
- [x] 6.3 真机复核：点进全选、`Enter` 进正文、`Esc` 复原、中文输入法组合态、长标题两行折行、空标题占位符、树里重命名同步、复制出的 Markdown 首行。 验证：用户在应用内逐条确认后勾选。（用户 2026-11-17 真机核验通过）
- [x] 6.4 归档：本变更归档后主规格 `knowledge-base/editor` 增加两条 Requirement。 验证：`openspec archive kb-doc-title --yes` 后 `openspec list --specs` 中 `knowledge-base/editor` 计数由 6 增至 8。（本变更已归档）
## 7. 顶部左端修正（2026-11-17）

- [x] 7.1 去掉工具条左端的锁图标与「只读」标签，只留灰度文件名镜像。 验证：`kb-item-detail.tsx` 中 `Lock` 与 `kb-strip-ro-tag` 零命中；`grep -n "kb-strip-ro-lock\|kb-strip-ro-tag" src-react/src` 零命中。
- [x] 7.2 修左右不在同一水平线：`.kb-strip-head` 改 `align-items: center`，新增内层 `.kb-strip-headline`（`inline-flex` + `align-items: baseline`）保住文件名与保存时间的基线。 验证：`node scripts/css-parse.mjs` → 12 张 0 失败；`npx tsc -b` exit 0。
- [x] 7.3 文档同步：`docs/capability/06-知识库.md` 的工具条段与已实现清单去掉锁图标/标签并写明居中与基线分工。 验证：`node test/docs.test.mjs` → `# pass 4 / # fail 0`。
- [x] 7.4 真机复核：第一行左端文字与右侧图标同一中线、长文件名省略不挤压保存时间。 验证：用户在应用内确认后勾选。（用户 2026-11-17 真机核验通过）
