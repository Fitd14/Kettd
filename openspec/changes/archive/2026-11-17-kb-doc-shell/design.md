# Design

## Context

`kb-item-detail.tsx` 目前的渲染层级是 `.kb-detail-inner > (.kb-detail-toolbar, .kb-meta, .kb-detail-body)`：动作栏、标题、编辑器全在一列里，宽度受 `--kb-measure-cap`（96ch / 112ch）限制。上一轮把排版档位做进了 `Settings`（`reading_size` / `reading_tracking` / `reading_measure`）并由 `src-react/src/lib/reading-prefs.ts` 写 `data-reading-*`，但控件留在设置页。

本轮把「文档自己的外壳」补上：工具条独立、页边大纲、复制、排版入口内嵌。约束来自 AGENTS.md：颜色只走 `hsl(var(--xxx))`、字号只走四档令牌（正文另有 `--fs-reading`）、组件后续统一由 `radix-component-layer` 收口。

## Goals

- 工具条横贯整栏、吸顶、位于标题之上；标题与正文仍共用一条中线。
- 大纲在宽窗是页边栏、窄窗是右侧抽屉，两者共用同一份数据。
- 复制 Markdown 与排版入口不再离开文档页。
- 不新增 Tauri 命令、不新增依赖、不改存储结构。

## Non-Goals

- 不做逐条目排版（档位仍是全局设置）。
- 不做标题字号放大：受 `visual-language`「字号阶梯限定为四档」约束，标题继续用 `--fs-masthead`（22px）；原型图里的 30px 只是示意。
- 不改工具栏按钮集合（用户 2026-11-16 决定暂不精简）。
- 不迁移到 `components/ui/` 原语（由 `radix-component-layer` 负责）。

## Decisions

### D1 结构：一条吸顶带 + 一个三列容器

`.kb-detail` 保持纸张与滚动容器不变，内部变成：

```
.kb-detail
├─ .kb-detail-strip          position: sticky; top: 0
│  ├─ .kb-strip-tools        ← 编辑器工具栏 portal 到这里
│  └─ .kb-strip-actions      A 排版 / 复制 / 大纲 / 贴桌面 / 删除
└─ .kb-detail-inner          with-outline 时 grid: minmax(0,1fr) 176px
   ├─ .kb-detail-main        width: min(100%, var(--kb-measure-cap)); margin-inline: auto
   └─ .kb-outline            右列；让出 188px 后正文仍 ≥700px 才出现，否则改用抽屉
```

吸顶带用负的左右外边距吃掉 `--kb-gutter`，所以它与纸面同宽，而不是与正文列同宽——这是「工具条拉宽」的机械实现。正文列的限宽从 `.kb-detail-inner` 移到 `.kb-detail-main`，让页边栏在没有大纲时也不占位。

### D2 工具栏用 portal 挂进吸顶带，而不是复制一份

`TipTapEditor` 新增 `toolbarHost?: HTMLElement | null`，有值就 `createPortal(toolbar, toolbarHost)`，无值仍内联渲染。便签窗（`StickyWindow`）不传这个属性，行为完全不变——同一条工具栏只有一份实例，不会出现两套状态（`useEditorState` 的 flags 只算一次）。

挂载顺序：`setToolbarHost` 在 `useLayoutEffect` 里执行，`TipTapEditor` 的 portal 依赖 ref 非空后才渲染；因此首帧可能没有工具栏，但不会有布局抖动（`.kb-strip-tools` 预留 `min-height: 24px`）。

### D3 大纲数据从编辑器文档派生，不走存储

`collectOutline(doc)` 遍历 `doc.descendants`，只收 `heading`（`level ≤ 3`），文本取 `textContent.trim()`，遇到标题即返回 `false` 跳过子树。`TipTapEditor` 在 `editor.on('update')` 时回调 `onOutline(items)`，并在 mounted 时推一次首版；`KbItemDetail` 只是把结果存进 state。没有任何持久化，也不进 `getMarkdown()`。

代价：每次输入都会重算一次大纲。文档最多几百行标题，`descendants` 是 O(n)，实测可接受；若将来大文档出现卡顿，改成 300ms 节流即可，不改接口。

### D4 宽窗页边栏、窄窗抽屉，共用一份数据

判据改成「让出大纲后还剩多少正文」：`railFits = innerWidth - 188 >= 700`（188 = 176px 栏 + 12px 间距，700px ≈ 87 字符）。成立时用页边栏（grid 右列），否则用抽屉。这个判据不读 `window.innerWidth`、也不读列宽档位，只观察 `.kb-detail-inner` 的实测宽度——树折叠、字号档位、列宽档位都会反映到这一个数上，所以「切到宽敞档就掉进抽屉」的旧毛病（旧判据要求两侧各留 150px，切宽敞后余量从 162 掉到 98）自然消失。

布局上大纲**从留白里让位**，不是叠在留白上：`.kb-detail-inner.with-outline` 变成 `grid-template-columns: minmax(0, 1fr) 176px`，正文列在左列里按「可用宽与上限取小」居中。留白 ≥188px 时正文宽度与每行字数不变，只是右侧被大纲占掉、左留白变窄（左右不对称是有意为之）；留白不足 188px 时正文列跟着收窄，这是 m01787 明确接受的代价。

抽屉实现：`role="dialog"` + `aria-modal="true"`，宽 `min(88%, 320px)`，遮罩点击或 `Esc` 关闭；关闭后焦点回到大纲按钮（`ref.focus()`）。打开时若焦点在编辑器内，先把焦点移进抽屉，避免滚动时高亮抢焦点。抽屉层是 `position: absolute`（包含块 `.kb-detail`，已是 `position: relative` + `overflow: hidden`），因此只覆盖文档区，不会像旧版 `position: fixed` 那样盖住整个应用窗口。

### D5 复制走 `EditorApi.getMarkdown()`，失败必须如实报告

`TipTapEditor` 通过 `onReady` 暴露 `{ getMarkdown, scrollToHeading }`。复制按钮调用 `getMarkdown()`，先试 `navigator.clipboard.writeText`，失败回退到隐藏 `<textarea>` + `execCommand('copy')`，两条路都失败时**不显示**「已复制」，改显示一次短暂错误提示。反馈 1.6s 后复位。

注意：`getMarkdown()` 会先跑 `normalizeLinkSyntax()`，所以复制得到的是落盘形态（不含 `[@`），与保存一致——这是刻意的，复制出去的内容应当能直接被 Markdown 解析器识别。

### D6 排版入口搬到文档，但值仍是全局设置

弹层挂在工具条右侧的「A」按钮下，内含两条 `StepSlider`（字号 7 档、字距 7 档）与列宽两段按钮。拖动时先 `applyReadingPrefs()` 即时生效，同时把改动合并进 `pendingPrefs`，220ms 无新改动后才 `setSettings()`。这样拖动不会打出十几次写盘，松手后仍能持久化。

`SettingsView` 里的三行控件删除，留一行说明指向文档工具条——避免两处控件同时改一个字段。

### D7 标题衬线用独立令牌，绝不复用 `--font-serif`

`--font-serif` 的首族是 `Kettd Serif`（61 字子集，只覆盖报头用字）。若 `.kb-detail-title` 直接用它，标题里命中的字走子集、其余回落 `Noto Serif SC`，同一行出现两种衬线。因此新增：

```css
--font-reading-title: "Noto Serif SC", "Songti SC", SimSun, serif;
```

样式表里只允许 `font-family: var(--font-reading-title)`，不出现裸字族名。

### D8 滚动高亮用几何计算，不用 IntersectionObserver

滚动时取「最后一个 `getBoundingClientRect().top <= 容器顶 + 96px` 的标题」作为当前项。IO 需要为每个标题注册观察器并在切换时抖动，几何计算在有 `scroll` 事件的容器里更直接；96px 是吸顶带高度加一点余量。

### D9 与 `radix-component-layer` 的边界

本变更新增的 `typo-popover`、`step-slider`、`doc-outline` 都是手写控件，**会在 radix 线里被替换**。两边都不等对方：本变更先把行为与样式立住，radix 线负责把弹层/滑块换成 `radix-ui` 原语。为避免抢文件，本变更 P0 只改 `kb-detail-*`、`kb-outline*`、`kb-typo-*`、`step-slider*` 四组样式与三个新组件。

### D10 归档顺序

`kb-writing-experience` 与本变更都 MODIFIED `knowledge-base/writing-layout` 的「居中正文列与留白下限」。必须**先归档 `kb-writing-experience`**，再归档本变更；否则本变更的 MODIFIED 会覆盖掉两档列宽那版文本，把 96/112ch 规则写丢。

同理，主规格 `writing-layout` 的 `Purpose` 行（现在写着「标题与工具栏和正文对齐」）无法用 delta 改，归档后需人工改这一句。

## Risks / Tradeoffs

- **吸顶带与便签窗**：便签窗不传 `toolbarHost`，工具栏仍内联；若将来便签窗也要吸顶，需要另开一条（它的容器不是 `.kb-detail`）。
- **portal 的 z-index**：吸顶带 `z-index: 8`，链接内联条 `z-index: 90`、排版弹层在带内绝对定位。若弹层被裁切，是把 `.kb-detail` 的 `overflow` 从 `auto` 改成 `clip` 的连锁反应——真机验收时要专门看一眼弹层是否被切。
- **大纲重算开销**：每次按键一次 `descendants`，大文档（>200 标题）可能掉帧；先测再决定是否节流。
- **抽屉焦点**：`role="dialog"` 但没有焦点陷阱，Tab 可能跑出抽屉。若 radix 线先落地，直接换成 `Dialog` 即可，不必在本变更里手写陷阱。

## Migration Plan

1. 令牌：`index.css` 增 `--font-reading-title`；`.kb-detail-title` 改用它。
2. 结构：`kb-item-detail` 的返回改成 strip + inner 两段；`kbb.css` 的 `.kb-detail-inner` 限宽规则迁到 `.kb-detail-main`。
3. 大纲：`doc-outline.ts` / `doc-outline.tsx` 落地，先只做页边栏；抽屉作为第二步，开关由同一个 `outlineOpen` 驱动。
4. 工具栏：`tiptap-editor` 加 `toolbarHost`；`StickyWindow` 不传，回归便签窗工具栏。
5. 验收：`tsc -b`、`lint`、`test/docs.test.mjs`、`test/command-parity.test.mjs`（确认无命令变化）、真机四条（宽窗并排、窄窗抽屉、复制、排版弹层不被裁切）。

## Open Questions

- 抽屉宽度与动效时长（现取 `min(78vw, 320px)` / 160ms）是否需要随字号档位变化？
- 大纲是否需要支持折叠二级以下（现在固定显示到三级）？
- 复制按钮是否同时提供「复制为纯文本」？