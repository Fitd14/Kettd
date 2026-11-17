# Design

## Context

- 现状见 `proposal.md`。这里只补实现相关的事实。
- `src-react/components.json` 已按 shadcn 配置（new-york、alias `@/components/ui`），`src-react/src/index.css` 首行 `@import "tailwindcss"`，`@tailwindcss/vite` 在 devDependencies —— Tailwind v4 在构建里生效，utilities 可用。
- `components/ui/button.tsx` 现用 shadcn 的 Tailwind 皮肤（`bg-primary`、`h-9 px-4 text-sm`）；而页面侧按钮全部是项目自有类：`className="btn sm"` / `"btn xs ghost"` / `"btn on"`，皮肤定义在 `src-react/src/styles/app.css:65` 起（`.btn` / `.ghost` / `.danger` / `.sm` / `.xs`），字号走 `--fs-meta` / `--fs-sub`，边框走 `hsl(var(--input))`。输入框同理：`app.css:96` 的 `.input` / `.input.xs` / `.input.invalid`。
- 两者皮肤不一致，是 `ui/button.tsx` 全仓零引用的直接原因：套上去视觉就变。

## Goals / Non-Goals

**Goals:**

- 原语层替换后视觉零漂移：新 `ui/*` 渲染出来的类与现在页面写的类一一对应。
- 迁移可机械执行、可分批回滚：单文件替换即可验证。
- 弹层、菜单、标签页的键盘行为一次性补齐，不再逐页面补丁。

**Non-Goals:**

- 不改视觉设计本身（不借迁移调整配色、圆角、字号）。
- 不重写 `.btn` / `.input` 的样式实现，只把它们包进原语。
- 不做组件单测与静态门禁（本轮只改代码与规则；后续若需要单独立变更）。
- 不迁移 listbox 两处（radix 无原语，见 Open Questions）。

## Decisions

### D1 原语皮肤复用项目 CSS 类，不套 shadcn 视觉

`buttonVariants` 改为输出项目类组合（`btn`、`btn ghost`、`btn danger`、`btn sm`、`btn xs`），`ui/input` 复用 `input` / `input xs` / `input invalid`；Tailwind utilities 只用于布局性补充（`inline-flex`、`gap-*`）。

**为什么不是直接上 shadcn 皮肤**：shadcn 的 `h-9 px-4 text-sm` 会把 12.5px 的 `.btn` 顶到 14px，且引入 `--radius` / `--primary` 另一套观感，与 `visual-language` 规范（四档字号、令牌取色）正面冲突。迁移的第一诉求是行为升级、视觉不变。

**代价**：`ui/button.tsx` 现在是"shadcn 样板"，改造后不再是可从 shadcn 直接 copy 的文件；`components.json` 保留（新增原语仍可 copy 骨架再改皮肤）。

### D2 变体名对齐现有类名

`variant`: `default`（`.btn`）、`ghost`、`danger`、`outline`（`.btn` + 自定语义，供第 4 批弹层按钮用）；`size`: `sm`、`xs`、`icon`（无文字的图标钮，`aspect-square` + 现有 padding）。页面替换是 `className="btn xs ghost"` → `<Button size="xs" variant="ghost">` 的等价改写，不是重新设计。

### D3 弹层行为用 radix，皮肤保留

Dialog / Popover / DropdownMenu / Tabs 用 `radix-ui` 统一包导入，`Portal` 挂到 `body`；遮罩与面板沿用现有浮现层样式（描图纸 / 纸面），过渡动画复用 `tw-animate-css`。手写 `role="dialog"` 里的 `useEffect` 焦点逻辑删除，由 `Dialog.Content` 的默认行为接替。

**替代方案**：自写 focus-trap（否决：重复 radix，且 Esc / 点击外部 / 焦点归还三条都要各自测试）。

### D4 迁移顺序与"一批一提交"

按 `components/ui/` → `views/` → `components/` → `rendering/` 推进；每个文件一次提交，替换后跑 `npx tsc -b`，涉及视觉的文件在 L4 冒烟里点一遍。这样任一批出问题，`git revert` 单个提交即可。

### D5 例外写进任务而非规则

Tiptap 工具条（`rendering/tiptap-editor.tsx:1311` 起）保留 `onMouseDown` 的 `preventDefault`；listbox 两处（`views/KbView.tsx:1308`、`components/kb/kb-tree.tsx:646`）保持现状。例外写进 `tasks.md` 的执行备注，不进 `AGENTS.md`，避免规则被个例稀释。

### D6 规则落点分工

`AGENTS.md` §2.2 只写"必须遵守什么"（四条约束），`README.md` §目录结构 只写"东西放哪"（三层描述），本变更的解释性内容留在 OpenSpec。三条文档不互相抄。

### D7 分级滑块换 radix Slider

`components/ui/step-slider.tsx` 的手写 `role="slider"` 改为 radix `Slider`（`Root` + `Track` + `Range` + `Thumb`），`steps` 映射为 `min=0` / `max=steps-1` / `step=1`，`aria-valuetext` 透传 `format(current)`；轨道、刻度、读数外观保持不变。

**约束**：组件只有一处调用（`components/kb/typo-popover.tsx`，字号与字距各七档），改动面小；刻度 radix 不提供，继续由本组件按 `(i/max)*100%` 渲染；`Range` 的宽度与 `Thumb` 的 `left` / `translateX(-50%)` 都由 radix 计算，CSS 不能再补 `margin-left`（会双重偏移）。

**实现时修正的口径**：原计划「轨道 `pointerdown` 继续 `stopPropagation`，否则浮层把按滑块当成点了外面」在 1.7 之后已不成立 —— 浮层换成 radix `Popover` 后，点外判定看的是 `event.target` 是否落在内容里（拖动起点在内容内），而文档级捕获监听也无法被 Root 上的冒泡处理器拦住。继续保留只会挡住无关的上层逻辑，故删除；`step-slider` 的交互完全交给 radix。

**替代方案**：保留手写（否决 —— 用户 2026-11-16 明确要求换 radix；且能白得 `PageUp` / `PageDown`、触摸与方向键的标准化处理）。

### D8 listbox 保持现状

知识库左侧列表的 `role="listbox"` / `<button role="option" aria-selected>` 本轮不动（`views/KbView.tsx:1308` 空态容器、`components/kb/kb-tree.tsx:646` 检索结果）。用户 2026-11-16 决定保持现状；原本并列的零引用行组件 `components/kb/kb-item-row.tsx` 已按用户 2026-11-17 的指示删除，不再列入白名单。

理由：radix 无 Listbox 原语，`RadioGroup` 会把语义换成单选按钮组、上百条目的朗读负担明显；改 `aria-current` 又要连带改 `sticky/StickyWindow.tsx:44-51` 的 `OVERLAY_SELECTOR`（它用 `[role="listbox"]` 判断窗口内是否有浮层）。本轮目标是原生控件收口，listbox 不是原生控件，因此记为不做，非「待定」。

### D9 按钮统一外观：3 档尺寸 × 6 变体，先出原型再动代码

`\.btn\.outline`（22 处）与 `\.btn\.primary`（4 处）在项目 CSS 中没有定义，只有 `rendering/styles/capture.css:114` 的 `.cap-frame .btn.outline` 与 `settings/settings-backup.css:203` 的 `.backup-dialog .btn.primary` 两处页面作用域补丁 —— 这些按钮现在多数渲染成默认描边样式，类名空转。同时 12 组专用按钮类（`.sticky-btn`、`.titlebar-btn`、`.kb-find-btn`、`.kb-folder-caret`、`.kb-editor-tool`、`.kb-strip-btn`、`.kb-folder-add`、`.kb-folder-action`、`.kb-tag-x`、`.kb-pin-row`、`.todo-mini-btn`、`.sticky-md-btn`）各自写高度（19/20/21/22/24/28/32）、圆角（4/6/8）与 hover 底色。

**决策**：`ui/button` 固定 3 档尺寸（`xs` 22 / `sm` 26 / `md` 30，字号 12.5 / 12.5 / 14）× 6 个变体（`default` / `primary` / `ghost` / `soft` / `danger` / 用 `aria-pressed` 表达的按下态）；图标钮复用同一档位（`icon-xs` / `icon-sm` / `icon-md`）；保留两个特例：`size="window"`（44×32、无圆角、close 走 destructive 实心）与 `size="caret"`（18×20，树折叠）。粘贴面只换色不改尺寸：便签内用 `[data-surface="sticky"]` 覆写 `--sticky-*`。

**原型**：`design-review/button-system.html`（高保真、单文件、明暗可切、可开尺寸标注），含 4 组真按钮的 before/after 对照与「现类名 → variant/size」映射表。视觉基线以原型为准，`ui/button.tsx` 照它实现；原型再改须重走一次评审。

**落点**：CSS 写在 `src-react/src/styles/app.css` 的 `.btn` 段之后，用 `.btn[data-variant="…"]` / `.btn[data-size="…"]` 属性选择器，不新造第二套类名；`cva` 只负责拼 `btn` 与 `data-*`。旧修饰类（`.ghost` / `.danger` / `.sm` / `.xs`）在迁移期间保留，收口时按 5.6 删除。

**评审结论（2026-11-17，用户确认）**：四项全部通过 —— ① 允许尺寸上移（`xs` 约 19→22、字号 11→12.5，工具条 24→26）；② `primary` 实心补成全局变体，不再靠页面作用域补丁；③ 便签按钮沿用 `[data-surface="sticky"]` 只换色，尺寸与圆角跟随全局；④ 窗口按钮保持 44×32、无圆角、close 走 destructive 实心 `size="window"`，不参与三档阶梯。原型 `design-review/button-system.html` 自此为按钮视觉基线，实现与评审都以它为准。

**映射缺口（2026-11-17，迁移 `SettingsView` 时发现）**：`.font-chip-name`（字体名）与 `.font-chip-del`（chip 内 × 删除）是 chip 里的纯文字钮，不在 12 组专用类内，D9 没有给出对应变体。按「先补表再改代码」本轮跳过这 2 处，待补一条 chip/文字钮口径后单独迁。

**另两处待补口径（2026-11-17，迁移 `components/kb/` 时确认）**：① `.kb-typo-seg-btn`（`typo-popover.tsx:73` 起，列宽三档）是 26px 高的排印分段钮，选中态用 `--seal` 强调色（`hsl(var(--seal)/.07)` + `--seal-ink`），套 `ghost` 会丢掉这层色；需要像便签那样补一个 `data-accent="seal"` 口径再迁。② `.kb-tag-x`（`kb-item-detail.tsx` 标签 chip 内的 ×，原生约 11px）抬到 `icon-xs` 的 22px 会把 chip 撑高，D9 虽列了 `icon-xs`，实测尺寸冲突，待 chip 尺寸口径。

**无外观控件不换原语（2026-11-17，批次 3 清 `views/` 时确定）**：`SettingsView.tsx` 还剩 3 个原生 `<input>`，都不该进 `ui/input`——两个是隐藏的 `type="file"`（`.font-file`、`.backup-file-input`，CSS 已 `display:none`，由相邻按钮触发，换原语反而会把 `.input` 的边框/内边距带进来），一个是 `type="range"` 的 `.fade-slider`（不是文本输入，按 D9 映射表归 `ui/slider`，随批次 4/5 处理）。口径：**没有可见外形的控件（隐藏 file input、隐藏桥接输入）不算「原生控件」**，归入 5.1 白名单；`range` 归滑块批次，不计入输入框批。

**专用图标钮的过渡写法（2026-11-17）**：迁 `kb-folder-caret` / `kb-folder-add` / `kb-folder-action` / `kb-pin-row` 这类树内图标钮时，`size` 决定盒子（`caret` 18×20、`icon-xs` 22、`icon-sm` 26），**布局类仍留在 `className`**——`.kb-folder-caret` 的 `flex:none`、`.is-empty{visibility:hidden}`、`.kb-pin-row` 的行内定位都靠它，删掉会让树缩进错位；被 `size` 取代的 `width`/`height`/`hover` 段留到 5.6 统一删。这一条同样适用于 `.titlebar-btn`（保留 44px 宽由 `size="window"` 管）。

**替代方案**：只补 `.btn.outline` / `.btn.primary` 两个空转类的定义（否决 —— 12 组专用类仍各自漂移，用户 2026-11-17 要求「专用样式按钮统一视觉体验」）。

### D10 菜单拆两步：先落 `dropdown-menu`，`kb-context-menu` 并入批次 4

`components/kb/kb-context-menu.tsx` 是 429 行自建实现，承担四类右键区域（文件夹 / 条目 / 空白 / 输入框编辑菜单）、坐标弹出与四边 clamp、子菜单「移动到…」、以及**选中后不自动关闭**（删除的「第一次确认、3 秒回退」依赖菜单保持打开）。radix `ContextMenu` 要求用 `Trigger` 包住右键区域、且默认选中即关，直接替换会同时改掉关闭语义与四类区域的挂载方式。

**决策**：1.6 只交付 `components/ui/dropdown-menu.tsx`（落点 `components/task-row.tsx:152` 的「⋯」菜单，radix 负责 portal、翻转、↑↓ / Home / End / Enter / Esc 与 typeahead）与 `components/ui/context-menu.tsx` 原语（含 `Sub` / `SubTrigger` / `SubContent`），供批次 4 使用；`kb-context-menu.tsx` 本轮保留，批次 4 迁移时把四类区域改成 `ContextMenu.Trigger` 包裹，并把「删除二次确认」改由 `AlertDialog` 或 `Popover` 承载，菜单本身回到「选中即关」的标准语义。

**替代方案**：本轮硬迁 `kb-context-menu`（否决 —— 一次改动同时触及右键分区、子菜单定位与删除确认，且无法在本轮做真机键盘验收）。

### D11 Popover 只做内容层，portal 后按同名变量重新映射纸面令牌

排版弹层原来自己管两件事：面板长什么样，以及「什么时候关」（`open` 由 `kb-item-detail` 的 `typoOpen` 维护，面板内部再挂捕获阶段的 `mousedown` / `keydown`）。radix `Popover` 把这两件事绑在 `Root` + `Trigger` + `Content` 三个部件上，`Content` 不能单独拥有开关状态。

**决策**：`TypoPopover` 退化为纯内容组件（props 只剩 `value` / `onChange`），`Popover` / `PopoverTrigger` 上移到 `components/kb/kb-item-detail.tsx` 的触发按钮处；`aria-haspopup` 与 `aria-expanded` 交给 radix Trigger，删掉手写值与 `setTypoOpen((v) => !v)`。好处是关闭语义（点外、Esc、焦点归还、再点触发器重开）由原语统一，调用方不再各写一份。

**portal 带来的副作用**：内容渲染到 `body` 后脱离了 `.kb-detail` 子树，读不到该作用域定义的 `--paper-deep` / `--line-warm` / `--ink-1..3` / `--seal-track`，滑块与分段钮会退化成默认色。处理办法是在 `.kb-typo-pop` 上按同一批变量名重新映射一次共享令牌（`--paper` ← `--background`、`--line-warm` ← `--border`、`--ink-*` ← `--foreground` / `--muted-foreground`、`--seal-*` ← `--seal`），内部子样式一行不改。**这也是后续所有 portal 型浮层的通用注意点：子样式引用了作用域变量时，必须在浮层根上补映射。**

**替代方案**：把 portal 容器指回 `.kb-detail`（否决 —— 该子树有 overflow 与层叠上下文，弹层可能被裁切；且要为每个调用点传容器引用）。

### D12 整行可点击的列表/树/菜单行不迁 `ui/button`

清点 `components/kb/` 的 13 个原生按钮时发现，其中 10 个是**整行命中**的容器：`kb-tree-item`（知识树行）、`kb-folder-row`（文件夹行，4 处）、`kb-folder-caret`（折叠箭头所在的整行按钮）、`kb-item-row kb-flat-row`（扁平结果行）、`kb-context-menu-item`（右键菜单项）、`kb-tag-chip`（标签筛选 chip）、`kb-pin-row`（置顶列表行：`min-height:26px`、`padding:.2rem .35rem`、`text-align:left`，点整行跳转）。另 `views/ReviewView.tsx:136` 的 `heat-cell` 是周回顾热力格子（`lv-${level}` 决定底色、点格子下钻），同属整格命中。它们的外观、行高、hover 与缩进全部由行容器类决定，本身没有按钮轮廓。

**决策**：这类行保留原生 `<button>`，只把**控制钮**迁到 `ui/button`——即带 `.btn*` 的普通按钮，以及 D9 映射表里明确列出的图标钮（`kb-folder-add` / `kb-folder-action` / `kb-tag-x` / `kb-strip-btn` / `kb-find-btn`）。理由：`ui/button` 会带上 `.btn` 的 `border` / `padding` / `height`，塞进行布局会破坏行高与缩进；且行的可访问语义来自行本身（`aria-current`、`aria-selected`、`role="option"`），套按钮原语反而把两层语义叠在一起。判定口径：**看外观**——有按钮轮廓的迁，整行即命中区域的留。

**替代方案**：给 `ui/button` 加 `variant="row"` 抹掉全部按钮样式（否决 —— 变体越加越多，且行布局仍要依赖调用方的类，等于换个名字继续手写；一旦有人给 row 变体加 padding 又会悄悄改变行高）。

### D14 窄窗大纲抽屉不迁 ui/dialog

`kb-item-detail.tsx:505` 的 `.kb-outline-layer` 是 `position:absolute; inset:0` 的面板内抽屉，铺在 `.kb-detail` 这块相对定位容器上，靠祖先的 `--ink-*` / `--paper` 变量取色。`ui/dialog` 的 portal 落到 `body`，既丢祖先定位（抽屉会铺满视口而不是面板）也丢作用域变量。本轮不迁，留待 `DialogContent` 支持 `container` 或单独立抽屉原语时再做；4.1 的验证范围相应缩到三个视口级弹层。

### D13 自带皮肤的编辑器内输入框暂留原生

批次 3 清到 `rendering/` 与标签 chip 时剩 3 处：`link-inline-editor.tsx` 的 `.kb-link-inline-input`、`tiptap-editor.tsx` 的 `.kb-find-input`（透明无边框的查找框）、`kb-item-detail.tsx` 的 `.kb-tag-input`。它们的皮肤都写在 `prosemirror.css` / `kb.css` 里，而 `.input` 基础皮肤在 `app.css`（import 顺序在视图之后），类选择器同特异性时后者胜出——直接换成 `ui/input` 会把查找框变成描边框、标签 chip 变成圆角描边输入框。本轮不硬迁，留到编辑器/标签批次（批次 4 完成后再处理）：届时的做法是把皮肤并进 `.input[data-skin="inline|find|tag"]`，或抽出无边框的 `bare` 变体，二者都要先补进 D9 表。

## Risks / Trade-offs

- [radix 弹层默认行为与现有手写弹层不一致（关闭时机、滚动锁定、层级）] → 先在 `kb-folder-dialog` 一个点上做对照验证，确认后再批量；皮肤类保持不动，只换行为层。
- [Tailwind preflight 与既有 CSS 已有共存，新原语若混入 utilities 可能与 `.btn` 规则打架] → 原语内只用 `cn()` 合并类，不在组件里写裸 utility 覆盖 `.btn` 的 padding / font-size。
- [161 个按钮逐个替换，人工漏改] → 以 grep 计数作为进度刻度（每阶段记录剩余数），收口时要求计数为零；不引入静态门禁，靠计数与冒烟。
- [`.btn` 类在新原语里被引用，日后改样式会同时影响未迁移页面] → 过渡期内 `.btn` 保持只增不改；全部迁移完成后再决定是否把它降级为原语内部实现。
- [迁移期间两套写法并存，新代码可能继续手写原生控件] → 规则先行（本轮已写入 AGENTS），review 时以规则为准。

## Migration Plan

1. 视觉基线：按钮体系原型 `design-review/button-system.html` 评审通过（D9），结论回填后动 `ui/button.tsx`。
2. 原语批次：改造 `ui/button.tsx` 的皮肤映射与 `ui/step-slider.tsx`（换 radix `Slider`），新增 `input`、`checkbox`、`dialog`、`dropdown-menu`、`context-menu`、`popover`、`tabs`；`textarea` / `switch` / `tooltip` 零使用点，按需再建。
3. 按钮批次：`views/` → `components/` → `rendering/`，单文件提交。
4. 输入框批次：同上顺序。
5. 弹层批次：4 个 dialog、2 个 menu、1 个 popover 换 radix。
6. 标签页批次：`ReviewView`、`PlannedView`。
7. 收口：grep 计数归零，`npx tsc -b` / `npm run lint` / `npm run build` 通过。

**Rollback**：每批是独立提交，`git revert <commit>` 即回到上一批；原语新增文件删除即可，页面未引用前无影响。

## Open Questions

暂无。listbox 按 D8 关闭，step-slider 按 D7 定案，按钮视觉按 D9 以原型为准。