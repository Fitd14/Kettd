# Proposal

## Why

知识库详情页仍然是「一列正文 + 一列塞在正文里的工具栏」：窗口一宽，工具栏浮在半空；窗口一窄，它换行成两层。想改字号要退出文档去设置页，想看结构得自己滚动，想把正文带走只能复制粘贴。文档页缺少一个自己的外壳。

现在改最省：排版档位（字号 / 字距 / 列宽）上一轮已经落地，只差把控件搬到文档里；大纲与复制都是只读派生，不碰存储与命令契约。

## What Changes

- **工具条独立成条**：工具栏从正文列里取出，移到标题之上，成为横贯整栏、吸顶、随滚动常驻的文档工具条（`.kb-detail-strip`）；正文列本身不再承担工具栏。
- **标题、标签、正文共用一条中线**：正文列挪进 `.kb-detail-main`，宽度仍是 `min(可用宽, 用户上限)`；右侧腾出页边栏位。
- **排版入口进文档**：字号 / 字距 / 列宽控件从设置页移入工具条右侧的「排版」弹层，改动即时预览、松手后合并写回设置（字段仍是全局 `Settings`，不引入逐条字段）。
- **复制 Markdown**：工具条右侧新增复制按钮，把当前文档的 Markdown 写进剪贴板，成功给短暂反馈；失败回退到隐藏 textarea + `execCommand('copy')`。
- **页边大纲（新能力）**：宽窗（详情面板 ≥1120px）在右侧页边显示一、二、三级标题大纲，随滚动高亮、点击跳转、无标题时给空态；窄窗不挤占正文，改为从右侧滑出的抽屉，由同一按钮开合。
- **标题衬线化（**BREAKING** for 视觉规则）**：文档标题改用系统衬线栈，并为此新增 `--font-reading-title` 令牌——现有 `--font-serif` 首族是 `Kettd Serif`（61 字子集），直接复用会让同一标题里部分字走子集、其余回落，出现两种衬线混排。
- **规格修正**：`knowledge-base/writing-layout` 的「标题与工具栏对齐正文」不再成立，改为「标题与正文对齐、工具条横贯整栏」；「居中正文列与留白下限」补上页边栏位。「`visual-language`」的「字体族只多一族」补出阅读面标题的例外与令牌约束。
- **存量冲突修正**：`kb-writing-experience` 的 writing-layout delta 第 7 行把工具栏写进了正文列，需改成「标题、标签与正文」，否则归档后与新条款打架。

## Capabilities

### New Capabilities

- `knowledge-base/doc-shell`: 知识库详情页的文档外壳——吸顶工具条的位置与行为、工具条上的单次动作（复制 Markdown、排版入口）、动作的反馈与失败回退。
- `knowledge-base/outline`: 文档大纲——数据来源（一至三级标题）、宽窗页边列与窄窗右侧抽屉两种呈现、滚动高亮、点击跳转与空态。

### Modified Capabilities

- `knowledge-base/writing-layout`: 「标题与工具栏对齐正文」改为「标题与正文对齐、工具条横贯整栏」；「居中正文列与留白下限」补页边栏位与大纲出现时的列宽口径。
- `visual-language`: 「字体族只多一族」补充阅读面标题可用系统衬线，但必须经 `--font-reading-title` 令牌取值，且该令牌 MUST NOT 含 `Kettd Serif` 子集。

## Impact

- **改动**：`src-react/src/components/kb/kb-item-detail.tsx`（吸顶条、动作、排版弹层挂载、大纲开合）、`src-react/src/rendering/tiptap-editor.tsx`（`toolbarHost` / `onReady` / `onOutline`、链接点击拦到 click 阶段）、`src-react/src/views/kb.css`（`.kb-detail-strip`、`.kb-detail-main`、`.kb-outline*`、`.kb-typo-*`、`.step-slider*`）、`src-react/src/views/SettingsView.tsx`（移出三行档位）、`src-react/src/index.css`（`--font-reading-title`）。
- **新增**：`src-react/src/components/kb/doc-outline.tsx`、`src-react/src/lib/doc-outline.ts`、`src-react/src/components/kb/typo-popover.tsx`、`src-react/src/components/ui/step-slider.tsx`。
- **文档**：`docs/capability/06-知识库.md`（工具条、大纲、复制的结构与口径）。
- **依赖 / 命令**：无新增依赖，无新增 Tauri 命令，不触 `parity` 四方同步；剪贴板走 `navigator.clipboard` + 回退，不需要后端。
- **与在办线的关系**：新控件（`typo-popover`、`step-slider`）后续由 `radix-component-layer` 统一收口到 `components/ui/`，本变更不等待它完成，也不与它抢同一批文件。