# Proposal

## Why

知识条目的标题一直放在顶部工具条里，做成一个输入框。位置错了：那是「文件的属性」该待的地方，而用户读一篇文档时，第一眼要找的是「这篇叫什么」——它应该和正文在同一张纸上、同一列里，随文滚动，而不是钉在工具条里当控件。

这个错位带来三个具体问题：工具条第一行被一个可输入框占着，长标题挤掉「已保存」与文档动作；标题与正文各在一处，改完名字视线要跳回顶部确认；复制出去的 Markdown 没有标题（`bodyMd` 只有正文），粘到别处是一篇没头没尾的文档。

## What Changes

- 顶部工具条左端的**可编辑输入框改为只读镜像**：锁图标 + 标题 + `只读` 标签，单行省略，不再接收焦点与光标。
- 正文首行新增**可编辑文档标题**（`.kb-doc-title`）：点进即全选整条，`Enter` 落盘并把光标交给正文第一段，`Esc` 复原放弃本次改名，粘贴只取单行纯文本。
- 标题仍只存**条目名一个字段**：改名走原有的 `updateKbItem({ title })`，树、搜索结果、标签页随之刷新，不新增第二份命名。
- 空标题**不落盘**：失焦时恢复原名；条目本来没有名字时显示占位符「未命名文档」。
- 标题**不进 Markdown 文档树**（方案 A）：`bodyMd` 里没有 `# 标题` 这一行；复制 Markdown 时由详情页在首行拼 `# 标题`，粘出去才是一篇完整文档。
- 文档标题字号取 `calc(var(--fs-reading) * 2)`，跟随排版弹层的字号档位；字体沿用 `--font-reading-title`（不含 `Kettd Serif` 子集）。
- 新建条目 / 右键「打开·重命名」的焦点信号改为聚焦正文标题并全选。
- 同步 `docs/capability/06-知识库.md`（新增「文档标题」一节，改写编辑器与工具条两段）。

## Capabilities

### New Capabilities

无。这是既有编辑能力的一部分，不新开能力。

### Modified Capabilities

- `knowledge-base/editor`：新增「文档标题在正文首行编辑」一条 Requirement——标题是文档第一行而不是工具条属性，且不进 Markdown 文档树。

## Impact

- `src-react/src/components/kb/kb-item-detail.tsx`：工具条左端改只读镜像；正文顶端新增 `.kb-doc-title`；复制 Markdown 拼首行标题；焦点信号改指正文标题。
- `src-react/src/rendering/tiptap-editor.tsx`：`EditorApi` 增加 `focusStart()`。
- `src-react/src/views/kb.css`：新增 `.kb-doc-title` 与 `.kb-strip-ro*`，删除 `.kb-strip-title` 输入框样式。
- `docs/capability/06-知识库.md`：新增「文档标题」一节，改写编辑器与工具条两段，已实现清单补一条。
- 无新依赖、无新 Tauri 命令、不触碰命令契约四方同步与 parity 门禁；数据结构不变（仍是 `title` 一个字段），无迁移。