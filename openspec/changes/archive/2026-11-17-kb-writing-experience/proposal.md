# Proposal

## Why

知识库详情页的写作面被几条早期约束锁死了：正文列宽固定 `78ch` 封顶、字号写死 16px、字距写死 0，规格里还明文禁止改字号层级。实际写作时用户既不能把长文摊宽，也不能把字调到看得清的大小。同一块界面上还有三处断点：链接在编辑区完全不可辨（被 tailwind preflight 重置成正文色、无下划线），插入链接只有「点按钮 → 弹窗填地址」一条路，斜杠菜单里也没有链接项；表格网格选择器打开后只能再点一次图标才关得掉。这些都在同一个写作面上，一次改完比拆四次省事，也能避免字号规格被反复改动。

另有一处是代码比规格更窄：本机已装思源宋体/黑体，但系统字体白名单只认宋体、楷体、仿宋、雅黑四种，选了会被后端拒绝（「这个系统字体不在可选项里」）。规格本身只说「`system:<族名>` 为本机已装字体」，没有限定四种，因此这是修正实现，不新增能力。

## What Changes

- **正文列宽两档自适应**：`78ch` 定宽改为「可用宽与上限取小」，上限两档（舒适 96ch / 宽敞 112ch，默认舒适）；窄窗自动填满、宽窗撞上限即止，切档只换上限、不改布局结构。
- **阅读字号 7 档**：`8 / 10 / 12 / 14 / 16 / 18 / 20px`，默认 16px。作用于知识库详情正文，标题按既有倍率等比缩放，与正文同档；chrome 四档令牌（`--fs-meta/--fs-sub/--fs-body/--fs-masthead`）与便签窗不受影响。
- **阅读字间距 7 档**：`0 / 0.015em / 0.03em / 0.045em / 0.06em / 0.075em / 0.09em`，默认 0。以 `em` 表达，随字号档位等比缩放；只作用于正文，标题自带的字距常量不动。
- **链接语法化**：正文键入或粘贴 `[@文本](url)` 即转为链接；转换后可见文字不含 `@`，落盘仍是标准 `[文本](url)`，不产生 Markdown 漂移。纯 `[]()` 不转换——`@` 就是用来防止「方括号后跟括号」被误当超链接。
- **链接可见**：新增 `--link` 令牌（亮/暗各一值，落值前实测对比度 ≥ 4.5:1），编辑区与阅读态统一为「链接色 + 下划线」，覆盖 tailwind preflight 的重置。
- **外链行为**：普通点击只把光标落进链接，`Ctrl/⌘ + 点击` 才交给系统浏览器打开；链接不再在 WebView 内导航。
- **BREAKING 移除链接弹窗**：删除 `LinkDialog` 及其浮层/卡片样式，改为「点链接 → 在链接下方弹一条内联输入条（非弹窗）」修改地址、清空即移除；`Ctrl+K` 与工具栏链接按钮改为插入语法模板。
- **斜杠菜单新增 `/link`**：知识库详情与便签窗都可用，插入 `[@文本](url)` 模板并把选区落在文本上。
- **表格网格选择器**：点击选择器外部或按 `Esc` 关闭；悬停行列预览高亮保留。
- **思源字体可选**：系统字体白名单从 4 种扩到 6 种（加 `Noto Serif SC`、`Noto Sans SC`），前后端两处同步并补校验测试。
- 同步文档：`docs/capability/06-知识库.md` 的列宽量算与「不改字号层级」条款。

## Capabilities

### New Capabilities

（无：本次全部落在既有能力上，不新立能力名，避免规格树继续碎片化。）

### Modified Capabilities

- `knowledge-base/writing-layout`：「78ch」定宽改为「可用宽与上限取小」的两档上限；「不改变主题与字号层级」改写为「不改 chrome 四档，但允许用户切换阅读档位」。
- `knowledge-base/editor`：链接插入由弹窗改为 `[@文本](url)` 语法 + 内联输入条；补链接可见样式与 `Ctrl+点击` 外链；`/link` 进斜杠菜单；补表格网格选择器点击外部关闭。
- `visual-language`：阅读字号从「固定 16px」改为 7 档枚举并新增字距档位；修正「标题与标签不超过 14px」的场景描述（现状 `.kb-detail-title` 已是 16px，场景本身已过时）。

## Impact

- 前端：`src-react/src/views/kb.css`、`src-react/src/rendering/styles/prosemirror.css`、`src-react/src/index.css`（`--link` 与字距令牌）、`src-react/src/rendering/tiptap-editor.tsx`（链接语法扩展、点击处理、工具栏、斜杠项、网格选择器）、`src-react/src/views/SettingsView.tsx` 与 `settings-font.css`（两组档位控件）、`src-react/src/App.tsx`、`src-react/src/lib/api.ts`，新增 `src-react/src/lib/reading-prefs.ts`；删除 `src-react/src/rendering/link-dialog.tsx` 及 `src-react/src/sticky/StickyWindow.tsx` 中 `.kb-link-dialog-overlay` 选择器。
- 后端：`src-tauri/src/models.rs`（`Settings.reading_size` / `reading_tracking` / `reading_measure`、白名单校验、系统字体白名单 4→6、单测）、`src-tauri/src/store.rs`（`apply_settings_patch` 校验与 `coerce` 兜底）。
- 命令面：不新增 Tauri 命令（外链走 `window.__TAURI__.shell.open`，`withGlobalTauri` 已开），parity 门禁与 `docs/ARCHITECTURE.md` §2 不动。
- 规格冲突：本变更显式取代 `kb-file-tree` 的「不改变主题与字号层级」与 `refine-frontend-visual-language` 的「阅读字号仅限编辑器正文」两条在办 delta；`fix-known-defects-iteration` 的 design/tasks 里 LinkDialog 记录按历史档案保留，不回改。
- 依赖：不新增第三方依赖。
- 验证：`npm run check`（17 项，SKIP 不算通过）+ `cargo test`（设置校验单测）+ L4 真机（字号/字距 7 档实拍、链接三种输入路径、内联条、网格选择器关闭、思源字体选中）。