# Design

## Context

本 change 的五项改动都发生在外壳 / 编辑器 / 便签边界上，彼此可独立实施，但共享同一批在途文件。以下证据取自当前工作树（含 `kb-file-tree`、`refine-frontend-visual-language` 等并行 change 的未提交改动，行号可能漂移；实施时以当时文件为准）。

**标题下拉现状**
- `src-react/src/rendering/tiptap-editor.tsx:69-76`：`HEADING_OPTIONS` 为「正文 + H1–H4」五项。
- `src-react/src/rendering/tiptap-editor.tsx:1051-1071`：以原生 `<select className="kb-editor-heading-select">` 渲染，`onChange` 内 `next === 'p'` 走 `chain().setParagraph().run()`，否则 `chain().toggleHeading({level}).run()`。
- `src-react/src/rendering/styles/prosemirror.css:52-91`：触发按钮样式（`.kb-editor-heading`、`::after` CSS 三角 caret、`.on` 高亮）已具备，但展开列表由 OS 绘制，无法套用主题 token。

**便签现状**
- `src-react/src/App.tsx:33-40`：`ROUTES` 仅 today / inbox / planned / review / kb / settings；`:44-48` `routeFromHash` 不匹配时回落 `#/today`；`:157-178` 主区按 route 条件渲染。
- `src-react/src/lib/api.ts:243-253`：`Bootstrap` 不含便签数据，侧栏无法直接得到便签计数。
- `src-react/src/views/SettingsView.tsx:470-575`：唯一便签管理入口是设置页内的纯文本行列表（打开 / 展开·缩小 / 两步确认删除），并含可见上限与新建入口。
- `src-react/src/sticky/sticky.css:6-63`、`:289-293`：便签窗样式自带 `html, body` 全局规则与 `height: 100vh`，不可被主窗页面直接复用。
- `src-react/src/views/kb.css:218-255`：宣纸 / 墨色 token（`--paper`、`--ink-1/2/3`、`--seal`）目前仅作用域在 `.kb-detail`，页面外不可用。

**免打扰现状**
- `src-tauri/src/models.rs:18-19`、`:266-283`、`:314`、`:343-363`、`:366-408`：`Dnd` 结构（默认开启、23:00–07:30）、`Settings.dnd` 字段、默认值与 coerce 校验。
- `src-tauri/src/models.rs:738-765`：`DndPayload` 与 `SettingsPayload.dnd`。
- `src-tauri/src/store.rs:1917-1933`：`patch.dnd` 落库及两句中文校验错误。
- `src-tauri/src/scheduler.rs:135`、`:205-216`、`:228-234`、`:486-504`：`in_dnd` 判定、免打扰期静默入账、窗口结束合并补发；`:738-770` 单测 `dnd_silences_then_merges_after_window_ends`。
- `src-react/src/views/SettingsView.tsx:625-647`：「提醒」区块的免打扰开关与起止时间；`src-react/src/lib/api.ts:148-152`、`:171`、`:191` 的 `Dnd` 类型与字段。
- `src-tauri/src/telemetry.rs:371-372`：单测样例含 `reason: "dnd"`。

**托盘现状**
- `src-tauri/src/runtime.rs:1266-1286`：`tray_menu` 共 10 个可点项 + 4 条分隔；`:1326-1396` `handle_tray_click` 分发。
- `src-tauri/src/main.rs:206`：启动时 `runtime::build_tray(todo_visible)`；`:352-354` 托盘事件分发入口。
- `test/main-window-close.test.mjs:63-65`：断言 `handle_tray_click` 函数体不得出现 `let _ = show_main`（`open_main` 分支必须保留）。
- 移除项的替代入口：快速记录走全局热键 + `CaptureWindow`；提醒管理在「计划」视图；周汇总导出在 `src-react/src/views/ReviewView.tsx:173-199`；便签显隐在设置页清单与便签窗。`/planned/reminders` 与 `/review/weekly` 两个托盘深链在主窗路由中没有对应处理，属历史遗留。

**移出淡化现状**
- `src-react/src/sticky/StickyWindow.tsx:52`、`:113-117`：`faded` state 与 `fadeOn` / `fadeOpacity` 读取。
- `src-react/src/sticky/StickyWindow.tsx:160-191`（mini）、`:224-263`（知识便签展开态）、`:266-314`（自由便签展开态）：仅用 `onMouseLeave` / `onMouseEnter` 切换 `faded`，mini 态只 `setFaded` 不加 `.faded` class。
- `src-react/src/sticky/sticky.css:29-31`：`.sticky-note.faded { opacity: var(--sticky-fade-opacity, 38%) }`，过渡 180ms（`:6-36`）。
- 当前没有 `compositionstart` / `compositionend`、`window` blur/focus 或 `document.hasFocus()` 判定，也没有去抖。

**视觉语言约束（来自并行 change `refine-frontend-visual-language`）**
- 一级页面根节点统一 `.view`（`src-react/src/styles/app.css:179-185`，padding 24px、gap 16px、纵向滚动）。
- 字号四档 `--fs-*` 令牌，颜色只允许 `hsl(var(--token))` / `var(--token)`，材料二分（描图纸用于 <2s 浮层，纸面用于停留面），动效只动 transform/opacity。
- 本 change 新 UI 复用这些令牌，不新增第二套 token。

## Goals / Non-Goals

**Goals**
1. 给便签一个独立的、可搜索的卡片墙管理页 `#/sticky`，复用现有命令，不新增后端命令。
2. 标题下拉改为应用自绘的下拉浮层（radix-ui `Select`，DOM 与主题 token 仍由本仓掌握），主题一致、键盘可用、可访问，且不抢编辑器选区。
3. 全链路移除免打扰（UI / 模型 / 调度 / 类型 / 文档 / 遥测），旧数据平滑兼容。
4. 托盘精简为五个高频入口，被移除项的功能入口不丢失，文档同步。
5. 修复中文输入法下「指针仍在便签内却淡化」的问题：淡化条件收紧为「指针在窗外 + 无焦点 + 非组合输入 + 无浮层」，并加去抖。

**Non-Goals**
- 不新增 / 删除 Tauri 命令，命令契约维持现状（parity 门禁当前基线 71；68 是并行的 `kb-storage-backup` 落地前的旧值，实施时以门禁实际断言为准）。
- 不做数据破坏性迁移，不引入迁移脚本、不 bump 数据版本。
- 不重构便签窗样式体系，不把四纸色提升为全局主题。
- 不改动知识库搜索、任务视图、阅读字体等其它在途 change 的范围。
- 不新增第三方依赖，也不新增 UI 组件库：本 change 只使用既有依赖 `radix-ui`（`src-react/package.json` 已有），不为标题下拉安装新包。
- H5 / H6 已按用户确认加入标题选择器（「正文 + H1–H6」七项），不再是待定项。

## Decisions

### D1 便签卡片页：新增 `#/sticky` 路由 + 卡片墙

**做法**
- 在 `src-react/src/App.tsx` 的 `ROUTES` 末尾（设置之前）新增 `{hash: '#/sticky', label: '便签', icon: StickyNote}`，主区条件渲染新视图 `src-react/src/views/StickyView.tsx`；页面根节点用 `.view` 骨架，标题风格与其它一级页面一致。
- 视图数据：挂载时 `listStickies()`，并用 `getKbItems()` 建立 `kbRef → 标题` 映射；订阅 `onEvent('store-changed', reload)`，本窗口内编辑、隐藏、删除后自动刷新；`visible !== false` 判定可见性（Rust serde 省略 `visible: true`）。
- 卡片内容：正文纯文本摘要（剥 Markdown、约 120 字，超出省略）、更新时间、状态徽标（已显示 / 已隐藏、已缩小、知识来源 + 知识标题，失效引用显示「知识条目已删除」）、空正文占位「（空）」。
- 卡片操作：打开（复用 `openSticky`；隐藏便签先走既有 `updateSticky({visible:true})`，受 5 张可见上限约束，失败以中文短句提示）、删除（沿用设置页两步确认：删除 → 确认删除 / 取消）、新建（页头按钮，复用 `createSticky()`）。失败提示一律中文短句，不含路径与堆栈。
- 搜索：本地过滤输入，匹配正文纯文本与知识标题，大小写不敏感；无匹配显示空结果提示，清空恢复全部。
- 空态：`listStickies()` 为空时渲染 `.empty` 风格空态 + 新建引导按钮。
- 页面顶部展示「可见 N/5 · 隐藏 M」计数（由列表本地计算），但不加侧栏计数（Bootstrap 不含便签数据，且不为计数新增命令）。
- 卡片使用全局语义 token（`--card` / `--border` / `--muted` / `--foreground` / `--pri-med` / `--accent`），不引入纸色 token；纸色是全局设置 `stickyPaper` 而非单条数据，`listStickies` 也不返回 paper，故卡片不呈现纸色。不 `import` `src-react/src/sticky/sticky.css`（会带 `html, body` 全局规则与 `100vh`）。
- 新增共享纯函数 `src-react/src/lib/sticky-text.ts`（`plainPreview(content, maxLen)`），卡片页与设置页清单复用；便签窗内已有的本地实现保持不动，避免跨窗口 bundle 改动扩大。
- 设置页便签区保留外观设置（纸色 / 花纹 / 移出淡化）与清单，并提供一个跳转到便签页的入口；便签页成为主管理面。

**理由**
- 设置页是低频配置面，便签管理是高频动作，且卡片墙与搜索需要整页宽度；独立路由与现有六个一级页面同构，改造成本最低。
- 复用既有命令意味着后端零改动、命令契约不变、parity 不受影响；`store-changed` 事件已能让多窗口状态一致。
- 不新增 token、不引入第二套主题，满足视觉刷新的材料与颜色约束。

**备选方案**
- 设置页内嵌卡片区（不新增路由）：否决——低频页面承载高频管理，且设置页已有纸色 / 花纹 / 淡化配置，再放卡片墙会拥挤。
- 独立管理器窗口（新 WebView）：否决——需新增窗口生命周期与命令、跨窗同步成本高，收益仅是窗口大小。
- 新增 `list_sticky_cards` 命令返回卡片投影：否决——`list_stickies` 已返回完整内容（≤500 字），前端投影足够，新增命令会破坏「命令数不变」的约束。

### D2 标题下拉：radix-ui `Select` 浮层替代原生 select

**做法**
- 重写 `src-react/src/components/editor/heading-select.tsx`，由 `tiptap-editor.tsx:1053` 的 `<HeadingSelect>` 承接；组件对外签名不变（`value` / `onSelect` / `disabled` / `ariaLabel`），内部改用既有依赖 `radix-ui` 的 `Select`（`import { Select } from "radix-ui"`，与 `components/ui/button.tsx` 同一引入方式）。
- 选项仍来自 `src-react/src/lib/heading-options.ts` 的 `HEADING_OPTIONS`：无 DOM 依赖的纯逻辑（选项表、下标循环、value → 命令映射）继续与 Node 直跑测试共用，扩为「正文 + H1–H6」（用户 2026-11-14 确认补 H5/H6；H5/H6 的编辑态与阅读态样式已存在，见 `prosemirror.css:170-172`、`:334-339`）。
- 触发按钮沿用 `.kb-editor-heading` 外观，caret 改成 lucide `ChevronDown`（不再用 CSS `::after` 三角，`radix-ui` 的 `data-state` 驱动打开态底色）；按钮内显示 `<Select.Value>` 的当前级别文案。
- 浮层结构：`Select.Portal` → `Select.Content`（`position="popper"`、`side="bottom"`、`align="start"`、`sideOffset={4}`、`collisionPadding={8}`）→ `Select.Viewport` → 七个 `Select.Item`（选中项用 `Select.ItemIndicator` 打勾）。定位交给 floating-ui：视口内自动翻转与夹取，可用高度通过 `--radix-select-content-available-height` 交给 `.kb-editor-heading-viewport` 的 `max-height`。
- 层级：`Content` 自身 `z-index: 121`（高于知识库右键菜单 120 与详情弹窗 100）。不选默认的 `position="item-aligned"`：那会按选中项对齐并额外包一层 `position: fixed` 的层叠上下文，在便签窗 1200px 的窄容器里页内变高时层级关系更难把握。
- 键盘：radix 用真实 DOM 焦点在选项间移动（roving focus，`←/→` 只在 H1–H6 分档里生效）；radix 自带的 ↑↓ 走到端点会停住，组件在 `Content` 的 `onKeyDown` 里只截「已停在端点还要继续按」，改走 `nextIndex()` 的首尾回绕（spec「方向键循环」）；Enter / Space 应用、Esc 关闭并由 `FocusScope.onUnmountAutoFocus` 把焦点还到触发按钮；Tab 在 radix 里被 `preventDefault()`，浮层不关闭。带 Ctrl / Alt / Meta 的组合键一律放行，编辑器快捷键不被吞。
- 选区：组件不自己拦 `mousedown` / `pointerdown`——radix 在选项上登记的是 `pointerup` 选中，若在 `pointerdown` 上 `preventDefault()`，浏览器会取消后续 `mousedown`，`pointerup` 的选区判断随之失效，点选项反而不生效。radix 自身已在 `Trigger` 的 `pointerdown` 与 `Item` 的 `pointermove` 上用 `preventScroll` 聚焦，不折叠编辑器选区。应用选项沿用现有语义：`'p'` → `chain().setParagraph().run()`，`'1'..'6'` → `chain().toggleHeading({level}).run()`。
- 可访问性：`Select.Trigger` 渲染 `role="combobox"` + `aria-haspopup` + `aria-expanded` + `aria-controls`，并保留 `aria-label="标题级别：当前级别"` 与 `title`；`Select.Content` 由 radix 渲染 `role="listbox"`，选项为 `role="option"` + `data-state="checked"` + `data-highlighted`；焦点环与高亮背景只走既有 token（对比度 ≥3:1）。
- 样式落在 `src-react/src/rendering/styles/prosemirror.css` 的 `.kb-editor-heading` / `.kb-editor-heading-caret` / `.kb-editor-heading-menu` / `.kb-editor-heading-viewport` / `.kb-editor-heading-option` / `.kb-editor-heading-option-label` / `.kb-editor-heading-check`，状态选择器全部换成 radix 的 `data-*`（`data-state` / `data-highlighted` / `data-disabled`）；颜色只用 `hsl(var(--token))` / `var(--token)`，字号走 `--fs-*` 档位。
- 已知口径变化（相对自绘方案）：① 焦点模型从「焦点留在触发按钮 + `aria-activedescendant`」改为 radix 的「焦点进入选项、roving focus」，`aria-selected` 只在聚焦项上为真，当前选中级别靠选中项的 `aria-selected="true"` 与 `data-state="checked"` 勾号表达；② Tab 不再关闭浮层（radix 拦住 Tab），此时在 `Content` 的 `onCloseAutoFocus` 里下一帧检查「浮层内是否还有焦点」，没有就把编辑器焦点还原，避免「浮层已关、焦点悬空、编辑器敲不进字」；③ 删除原本手写的 createPortal / 矩形测量 / 视口 clamp / 外部 pointerdown / blur / resize / scroll 关闭逻辑，统一由 radix 的 `DismissableLayer` 与 floating-ui 承担；④ radix 自带字母 type-ahead，在中文选项上按 `H` 会定位到 H1。

**理由**
- 原生 `<option>` 列表由 OS 绘制，无法套用主题（暗色主题下白底列表是当前直接缺陷）。
- radix 已是本项目依赖，`Select` 渲染的是应用自绘 DOM（不是 OS 弹层），满足 spec「展开列表 MUST 由应用自绘」；主题 token、类名与字号档位仍由本仓掌握，不引入第二套主题。
- 键盘、焦点还位、外部点击关闭、翻转与裁剪这些容易漏的边界由成熟实现承担，减少自维护的交互代码；`heading-options.ts` 的纯逻辑与契约测试原样保留。

**备选方案**
- 保留原生 `<select>` 仅改 CSS：否决——展开列表样式不可控，暗色主题无解。
- 手写 listbox（`createPortal` + `position: fixed` + 自算矩形与 clamp + `aria-activedescendant`）：曾按此实现并被 `heading-select.test.mjs` 之外的代码评审接受，2026-11-15 用户确认改用 radix（方案 B）——自维护的定位与焦点代码是持续的回归面，而 radix 已在依赖树上。
- 复用 `KbContextMenu` 组件并加 listbox 模式：否决——语义混杂（菜单 vs 列表选择），且会让本 change 依赖并行 change 的在途文件。
- 用 `size` 属性把原生 select 展开成大列表：否决——仍是 OS 绘制，且交互不符合下拉预期。

### D3 废弃免打扰：全链路删除 + serde 兼容

**做法**
- 后端删除：`models.rs` 的 `Dnd` 结构、`DEFAULT_DND_FROM/TO`、`Settings.dnd` 字段与 `Default`/coerce 分支、`DndPayload` 与 `SettingsPayload.dnd`；`store.rs:1917-1933` 的 `patch.dnd` 落库分支及两句中文校验错误；`scheduler.rs` 的 `in_dnd`、`NightState.last_dnd` / `quiet`、静默入账分支、窗口结束合并批次及对应单测。
- 前端删除：`api.ts:148-152` `Dnd`、`:171` `Settings.dnd`、`:191` `SettingsPayload.dnd`；`SettingsView.tsx:625-647` 免打扰行及其状态与 patch 调用；`docs/` 中现行描述、`docs/capability/03`、`04`（如提及）、`08`、`docs/requirements/总体需求.md` 的相关表述改为已废弃或移除。
- 遥测：删除 `reason: "dnd"` 的样例与产出；`reminder_missed` 事件本身保留（仍有其它错过原因）。
- 旧数据兼容：确认 `Settings` 未使用 `#[serde(deny_unknown_fields)]`（当前未见）；`serde` 默认忽略未知字段，旧 `settings.dnd` 自然被忽略，首次保存后字段消失；不做破坏性迁移、不写迁移报告。
- 单测：原 DND 测试删除或改写为「无 DND 时到点提醒正常入队 / 不被静默」；新增一条「旧 JSON 含 dnd 仍能反序列化」的兼容测试。
- 命令数不变：不涉及命令注册与签名，parity 维持当前基线（实施时重跑门禁确认，若并行 change 已改变基数以门禁实际值为准）。

**理由**
- 免打扰是完整但独立的功能链路，保留「只隐藏 UI」会让旧数据继续静默吞提醒，行为与用户预期矛盾；整体删除最干净。
- serde 默认忽略未知字段使兼容成本接近零，比迁移脚本、数据版本升级更稳，也符合「无破坏性迁移」约束。

**备选方案**
- 只删设置 UI、保留模型与调度：否决——旧配置仍在生效，问题会以「提醒莫名不响」的形式回归。
- 破坏性迁移清除字段：否决——多余风险，且旧版本回滚后无法恢复用户设置。
- 保留字段并标记 deprecated：否决——死代码与文档负担，且调度分支仍需维护。

### D4 托盘精简：保留五项，删除五项及分发

**做法**
- `runtime.rs:1266-1286` 菜单改为：打开主界面 / 待办悬浮窗（勾选项）/ 新建便签 / 打开数据文件夹 / 退出，配 2–3 条分隔线；删除 capture、sticky_show_all、sticky_hide_all、reminders、weekly 五项。
- `handle_tray_click` 同步删除上述五个分支；`open_main` 分支保留不动（`test/main-window-close.test.mjs:63-65` 依赖其形态）。`sync_tray_todo` 保留（待办悬浮窗勾选态）；`note_snapshots` 若仅被 show_all / hide_all 使用则一并删除，否则保留（实现期确认）。
- `sticky_new` 仍走 `runtime::create_note(app, None)`，与全局热键一致。
- 同步 `docs/ARCHITECTURE.md` §5 托盘段落与相关说明，移除五项描述；检查 README 与 `docs/capability/` 是否提及托盘项。
- 检查 `test/` 下是否有断言托盘菜单项集合的测试，一并更新；新增（或扩展）一条「托盘菜单不含五项」的契约断言，防止回流。

**理由**
- 五项均是低频或已有多入口的操作；托盘只留高频与状态项，菜单更短更易点中。
- 逐个删除分发分支可避免「菜单没了、代码还在」的死路径，`show_main(Some("/planned/reminders"))` 这类与主窗路由不匹配的历史深链也一并清理。

**备选方案**
- 保留菜单项但禁用 / 置灰：否决——仍是噪音，用户明确要求去掉。
- 收进「更多」子菜单：否决——托盘菜单本就短，二级菜单增加点击成本。
- 保留快速记录（高频）：否决——全局热键已覆盖，且用户明确要求移除。

### D5 淡化守卫：四条件 + 去抖 + IME / 焦点事件

**做法**
- 在 `StickyWindow.tsx` 抽一个淡化守卫（hook / 工具函数），enabled 时统一决策 `shouldFade()`：
  1. 指针在窗外：用最近一次 `mousemove` 的客户端坐标与窗口边界比较（`clientX ≤ 0 || clientY ≤ 0 || clientX ≥ innerWidth || clientY ≥ innerHeight`），并在 `mouseleave` 时记录事件坐标作为兜底；不再只凭 `mouseleave` 直接置 `faded`。
  2. 窗口无焦点：`window` blur/focus 事件 + 判定时 `document.hasFocus()`。
  3. 非组合输入：`compositionstart` 置组合标记并取消待执行淡化，`compositionend` 清标记后重新判定。
  4. 无浮层：检查窗口内是否有打开的下拉 / 弹层（如链接气泡、命令菜单）；有则视为不可淡化。
- 四个条件全部满足时，启动 250ms 去抖定时器；去抖窗口内任意 `keydown`、`mousemove` / `mouseenter` / `mousedown`、`focus`、`compositionstart` 立即取消定时器并恢复不透明。`window` focus 立即 `setFaded(false)`，不等去抖。
- 设置关闭（`stickyFade === false`）时守卫整体不生效；mini 态保持现状不淡化；不透明度仍用 `stickyFadeOpacity`，过渡仍用现有 `opacity` 过渡，`prefers-reduced-motion` 下仍无动画。
- 知识便签、自由便签两个展开态共用同一守卫，避免两份逻辑漂移。
- L4 真机验收：微软拼音 + 至少一种第三方输入法，覆盖「指针在便签内打字」「候选窗出现 / 移动」「上屏」「切到其它应用」「点回便签」五种操作。

**理由**
- 用户报告的触发场景（指针视觉上仍在便签内、打字中却淡化）说明单纯 `mouseleave` 不足：输入法候选窗是独立 HWND，WebView2 可能把指针进入候选窗视为离开客户区；`composition` 标记 + 焦点 / 坐标复核能覆盖这一类误判，去抖则吸收瞬时抖动。
- 前端 DOM 事件已足够，不必引入全局鼠标钩子或后端窗口事件，改动局限在便签窗 bundle。

**备选方案**
- 只在 `compositionstart` 时禁止淡化：否决——组合结束瞬间候选窗仍可能存在，仍会闪一下；且不覆盖焦点误判。
- 由 Rust 侧全局鼠标钩子 / 窗口焦点事件驱动：否决——跨窗通信与平台差异成本高，DOM 侧已有等价信号。
- 直接移除移出淡化功能：否决——用户明确要求保留设置，只修误触发。
- 把过渡降到 160ms 对齐视觉刷新：本次不改，现有 180ms 属既有外观，视觉刷新统一动效时再收敛（见 Risks）。

## Risks / Trade-offs

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| 与 `refine-frontend-visual-language` 并行改同一批文件（`App.tsx`、`SettingsView.tsx`、`api.ts`、`prosemirror.css`） | 合并冲突、样式回退 | 本 change 只做规划；实施排期在视觉刷新落地后；复用其 `.view` / `--fs-*` / token，不新增 token；冲突时以视觉刷新为基准 |
| 原生 select 替换后的 a11y 回归 | 读屏 / 键盘用户不可用 | radix `Select` 的 combobox + listbox + roving focus（当前项另有 `aria-selected="true"` 与 `data-state="checked"` 勾号）+ 焦点环 ≥3:1；Tab 不关闭浮层时在 `onCloseAutoFocus` 里还原编辑器焦点；实施后用键盘全流程与读屏（Narrator）各验一遍；保留 `title`/`aria-label` |
| 删 DND 的旧数据兼容 | 旧 `settings.dnd` 反序列化失败 → 恢复页 | 确认无 `deny_unknown_fields`；新增旧 JSON 兼容单测；store 加载路径不改 |
| 回滚到旧版本 | 缺 `dnd` 字段时旧默认值（开启、23:00–07:30）会重新生效，夜间提醒可能静默 | 在 Migration Plan 与交付说明中标注回滚注意事项；回滚后首启提示用户检查提醒设置（交付文档层面） |
| 托盘项被脚本 / 文档引用 | `test/main-window-close.test.mjs` 或 docs 门禁失败 | 保留 `open_main` 分支；实施后跑 `npm run check`（含文档契约）；grep 全仓托盘 id 与文案 |
| 托盘深链 `/planned/reminders`、`/review/weekly` 清理 | 若存在未发现的外部入口会断链 | 已确认主窗路由无对应处理；删除前 grep 全仓这两个字符串与 `show_main(Some(` |
| IME 真机差异（微软拼音 / 搜狗 / QQ 输入法、WebView2 版本） | 守卫在某些组合下仍误判 | 守卫覆盖四条件 + 去抖，任一路径不满足都不淡化；L4 至少两种输入法、记录 WebView2 版本；保留手动复现步骤 |
| 卡片页与便签窗样式体系分裂 | 维护两套预览逻辑 | 抽取共享 `sticky-text.ts`；卡片只用全局语义 token；不 import 便签窗 CSS |
| 命令数口径（用户简报 64 / 本文旧值 68 / 门禁现行 71） | 误改 parity 断言 | 本 change 不增删命令；实施时以 `npm run check` 与 `node test/command-parity.test.mjs` 实际基线为准，不硬编码数字（71 = `kb-storage-backup` 落地后的现行值） |

## Migration Plan

- **数据**：无破坏性迁移。旧 `data.json` 的 `settings.dnd` 由 serde 忽略，首次保存后自然消失；5 份写前轮转备份、损坏通道（主数据进恢复页 / runtime.json 静默重建）行为不变；不新增迁移报告、不 bump 数据版本、不改数据目录。
- **配置 / 本地存储**：前端 `localStorage` 无免打扰相关键；新增路由 `#/sticky` 对旧的 `kettd.pending-route` 自动生效（`ROUTES` 内即可跳转）；`shell.sideCollapsed` 等键不动。
- **部署顺序**：S1 外壳与托盘 → S2 便签卡片页 → S3 标题下拉 → S4 淡化修复 → 集成验收。每批独立可编译、可回滚；S1 先落地可让文档与托盘契约尽早稳定。
- **回滚**：按批次 revert 对应提交即可；数据侧无迁移产物需要回退。注意回滚到含 DND 的旧版本时，缺失的 `dnd` 字段会回落旧默认值（开启、23:00–07:30），需在回滚说明中提示检查提醒设置。
- **构建机器约定**：生产构建（`npm run build:front` / `release.ps1`）在另一台机器上执行，dev 沙箱本机的 vite 构建会因 `@tailwindcss/oxide` 原生模块与 `spawn EPERM` 失败，不作为验证环境；本机只负责 `typecheck` / `lint` / Node 契约测试 / `cargo check` / `cargo test`。
- **文档门禁**：文档类改动（`docs/ARCHITECTURE.md`、`docs/capability/`、README）按 AGENTS §4.6 先给人工审核，批准后再提交。

## 决策确认

用户已于 2026-11-14 确认：① 标题选择器**补 H5 / H6**（选项为「正文 + H1–H6」七个；H5/H6 的样式沿用现有规则，见 `prosemirror.css:129-133`、`:287-300`）；② 卡片墙不呈现纸色 / 花纹；③ 删除按钮 hover / `:focus-within` 显示；④ 淡化去抖 250ms；⑤ 托盘顺序为 打开主界面 / 待办悬浮窗 / 新建便签 / 打开数据文件夹 / 退出。

Open Questions：无。