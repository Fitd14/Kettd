# Tasks

> 批次依赖：S1（外壳与托盘）→ S2（便签卡片页）→ S3（标题下拉）→ S4（淡化修复）→ 集成验收。每批独立可编译、可回滚，提交策略见文末 Workflow follow-up。命令数契约：本 change 不新增、不删除 Tauri 命令，parity 以实施时门禁实际基线为准（当前工作树为 71；68 是并行的 `kb-storage-backup` 落地前的旧值，用户简报的 64 更早）。

## 1. S1 外壳与托盘（移除免打扰 + 托盘精简）

- [x] 1.1 删除后端免打扰模型：`src-tauri/src/models.rs` 移除 `Dnd`、`DEFAULT_DND_FROM` / `DEFAULT_DND_TO`、`Settings.dnd`、`Default` 与 `coerce` 中的 dnd 分支、`DndPayload`、`SettingsPayload.dnd`；验证：`cargo check --manifest-path src-tauri/Cargo.toml` 通过，全仓搜索 `Dnd` / `dnd` 仅剩历史注释或旧数据兼容测试。
- [x] 1.2 删除落库与调度分支：`src-tauri/src/store.rs` 移除 `patch.dnd` 及两句中文校验错误；`src-tauri/src/scheduler.rs` 移除 `in_dnd`、`NightState.last_dnd` / `quiet`、静默入账、窗口结束合并批次，保留其它错过原因；验证：`cargo check` 通过，`cargo test` 中原 DND 测试已删除或改写。
- [x] 1.3 更新后端单测并补旧数据兼容：删除 / 改写 `scheduler.rs` 的 `dnd_silences_then_merges_after_window_ends` 为「无 DND 时到点提醒正常入队」；新增「含旧 `settings.dnd` 的 JSON 仍能反序列化」兼容测试（确认 `Settings` 无 `deny_unknown_fields`）；`telemetry.rs` 删除 `reason: "dnd"` 样例；验证：`cargo test --manifest-path src-tauri/Cargo.toml` 全绿，新测试名在输出中可见。
- [x] 1.4 删除前端免打扰类型与设置 UI：`src-react/src/lib/api.ts` 移除 `Dnd`、`Settings.dnd`、`SettingsPayload.dnd`；`src-react/src/views/SettingsView.tsx` 移除免打扰开关 / 起止时间行及相关状态与 patch 调用，「提醒」区块只留每小时上限等；验证：`npm --prefix src-react run typecheck` 与 `npm --prefix src-react run lint` 通过；设置页提醒区目视无免打扰。
- [x] 1.5 同步免打扰相关文档：`docs/ARCHITECTURE.md` §4 第 5 条、§2 的 `Settings` 模型与 dnd 校验错误、通知规则段落；`docs/capability/03-提醒与通知.md`、`docs/capability/08-设置.md`、`docs/requirements/总体需求.md` 的现行表述；验证：全仓搜索「免打扰」只在历史 / 已废弃说明中出现，`npm run check` 中文档契约项通过。
- [x] 1.6 精简托盘菜单：`src-tauri/src/runtime.rs` 的 `tray_menu` 只保留「打开主界面 / 待办悬浮窗（勾选）/ 新建便签 / 打开数据文件夹 / 退出」并按组分隔；`handle_tray_click` 删除 `capture`、`sticky_show_all`、`sticky_hide_all`、`reminders`、`weekly` 五个分支，保留 `open_main`；`note_snapshots` 若仅被 show_all / hide_all 使用则一并删除；验证：`cargo check` 通过，`npm run dev` 后托盘菜单目视只有五项（L4）。
- [x] 1.7 托盘契约测试与文档 §5：更新 / 新增测试断言托盘菜单含五项、不含已移除五项；保留 `open_main` 形态以通过 `test/main-window-close.test.mjs`；同步 `docs/ARCHITECTURE.md` §5 托盘段落与 README / capability 中提及托盘项的文字；验证：`node test/main-window-close.test.mjs` 与新增契约测试通过，全仓搜索五个移除项文案只在历史说明中出现。
- [x] 1.9 托盘移除项的功能入口门禁：托盘菜单本身由「托盘菜单契约」锁住，但 spec 还要求「被移除项 MUST NOT 让对应功能失去入口」——补 `test/tray-entry-parity.test.mjs`，逐条锁住快速记录（设置页热键输入 + 换绑）、便签显示 / 隐藏（卡片页 `updateSticky({visible})` 与打开窗口）、提醒查看（计划页 `boot.reminders` 与下一条计算）、本周汇总导出（回顾页 `exportWeekly` 调用与按钮文案），并断言 README 有对应功能说明；验证：新门禁 5 项全过并接入 `scripts/check.ps1`「托盘入口契约」。
- [ ] 1.8 S1 真机冒烟：启动应用验证托盘五项、待办悬浮窗勾选态切换、新建便签、打开数据文件夹、退出；并验证一条到点提醒正常通知（不再被免打扰吞掉）；验证：真机操作记录 / 截图，数值与状态逐项核对。

## 2. S2 便签卡片页

- [x] 2.1 新增路由与侧栏入口：`src-react/src/App.tsx` 的 `ROUTES` 增加 `#/sticky`（label「便签」、图标 `StickyNote`），主区条件渲染 `StickyView`；验证：`npm --prefix src-react run typecheck` 通过，手工把 hash 改为 `#/sticky` 刷新直达、侧栏折叠态仍可点击（L4）。
- [x] 2.2 抽取摘要纯函数：新增 `src-react/src/lib/sticky-text.ts`（`plainPreview(content, maxLen)`，剥离 Markdown、去空行、截断），并让设置页清单复用；验证：新增 `src-react/test/sticky-text.test.mjs` 覆盖 Markdown 剥离 / 截断 / 空串 / 多行，`node src-react/test/sticky-text.test.mjs` 通过。
- [x] 2.3 实现 `StickyView` 数据层：挂载时 `listStickies()` + `getKbItems()` 建 `kbRef → 标题` 映射；订阅 `onEvent('store-changed')` 刷新；可见性判定 `visible !== false`；验证：`typecheck` / `lint` 通过；在便签窗编辑、隐藏、删除后卡片自动更新（L4）。
- [x] 2.4 渲染卡片墙与空态：每张卡展示纯文本摘要（约 120 字）、更新时间、已显示 / 已隐藏 / 已缩小徽标、知识来源与标题（失效显示「知识条目已删除」）、空正文「（空）」；页面用 `.view` 骨架并显示「可见 N/5 · 隐藏 M」；列表为空时渲染空态 + 新建引导；验证：手工新增 / 删除便签后卡片与计数正确（L4），`typecheck` 通过。
- [x] 2.5 卡片操作：打开（隐藏便签先 `updateSticky({visible:true})` 再 `openSticky`，达 5 张上限显示「便签最多同时打开 5 张，请先关闭一张」）、两步确认删除（`deleteSticky`，取消保留）、页头新建（`createSticky`）；验证：手工覆盖打开成功 / 达上限 / 取消删除 / 新建四条路径（L4），无控制台报错。
- [x] 2.6 本地搜索过滤：新增纯函数（正文纯文本 + 知识标题、大小写不敏感）并在页面接入搜索框，无匹配显示空结果、清空恢复；验证：新增 `src-react/test/sticky-filter.test.mjs` 用 node 直跑通过；手工输入中文 / 英文关键词各验一次（L4）。
- [x] 2.7 设置页加入口：在设置页便签区提供「管理便签」入口跳转 `#/sticky`，保留纸面 / 花纹 / 移出淡化外观设置；验证：`typecheck` 通过，手工从设置页跳转成功（L4）。
- [x] 2.8 样式与视觉约束：新增便签页 CSS 只使用 `hsl(var(--token))` / `var(--token)`，字号走 `--fs-*` 四档，卡片 hover / `:focus-within` 才浮起、动效 ≤160ms，不 `import` `src-react/src/sticky/sticky.css`；验证：新增文件搜索无裸 hex / rgb 色值，亮色与暗色主题各目视一次（L4）。
- [x] 2.9 便签文档更新：`docs/capability/04-便签系统.md` 管理入口改为便签页并补卡片墙 / 搜索说明，同步 `docs/ARCHITECTURE.md` 与 README 中的相关描述；验证：`npm run check` 中文档契约项通过，`docs/capability/04-便签系统.md` 与实现一致。
- [ ] 2.10 S2 真机验收：卡片墙扫读、搜索、空态、上限提示、删除确认、深链刷新、窄窗自适应逐项走查；验证：真机操作记录 + 亮色 / 暗色两张截图。
- [x] 2.11 知识来源接缝门禁：卡片页用 `get_kb_items` 的 `id → title` 建映射、`kbRef` 取标题、条目缺失 / 读取失败 / 加载中三态分列（失败不得冒充「条目已删除」），后端 `get_kb_items` 缺省回元数据（`body_md` 置空、其余字段完整）与 `create_sticky` 的 `kbRef` 顶层参数一并锁住——这条接缝同时被并行的 `kb-file-tree`（条目标识）与 `kb-storage-backup`（列表收成元数据）改动；验证：`test/sticky-kbref-seam.test.mjs` 4 项全过并接入 `scripts/check.ps1`「便签知识来源契约」。

## 3. S3 标题下拉

- [x] 3.1 抽取标题选项纯逻辑：新增可测试模块（选项集合「正文 + H1–H6」、`nextIndex` 循环、value → 编辑命令参数映射），供组件与测试复用；验证：新增 `src-react/test/heading-select.test.mjs` 用 node 直跑，覆盖首尾循环、边界、`p` 与 `1..6` 映射。
- [x] 3.2 落地下拉组件：重写 `src-react/src/components/editor/heading-select.tsx`，改用既有依赖 `radix-ui` 的 `Select`——`Select.Portal` → `Select.Content`（`position="popper"`、`sideOffset={4}`、`collisionPadding={8}`，翻转与夹取交给 floating-ui）→ `Select.Viewport` → 七个 `Select.Item`（选中项 `Select.ItemIndicator` 打勾）；`Select.Trigger` 提供 `role="combobox"` + `aria-haspopup` + `aria-expanded`，`Content` 提供 `role="listbox"`、选项 `role="option"`；↑↓ 走到端点时由组件补首尾回绕（`nextIndex()`），Enter 应用、Esc 关闭并还焦触发按钮、Tab 不关闭浮层（用 `onCloseAutoFocus` 还原编辑器焦点）、点击外部关闭由 radix `DismissableLayer` 负责；（组件不自行拦 `pointerdown`，否则 radix 的 `pointerup` 选中会被浏览器取消）验证：`typecheck` / `lint` 通过，键盘全流程手工走查（L4）。
- [x] 3.3 替换编辑器原生 select：`src-react/src/rendering/tiptap-editor.tsx` 用新组件替换 `<select>`，保留「正文 → `setParagraph`、H1–H6 → `toggleHeading`」语义与当前级别回显；验证：手工切换正文 ⇄ H1–H6、光标在各级别间移动时显示值同步、再次选同级回正文（L4）。
- [x] 3.4 下拉样式：`src-react/src/rendering/styles/prosemirror.css` 的 `.kb-editor-heading*` 类改写为 radix 的 `data-*` 状态选择器（`data-state` / `data-highlighted` / `data-disabled`），颜色只用 `hsl(var(--token))` / `var(--token)`，字号走 `--fs-*`，高亮与焦点环对比度 ≥3:1；验证：新增样式无裸色值，暗色主题展开不出现白底 OS 列表（L4）。
- [ ] 3.5 选区保护：选中编辑器文字后打开下拉并选 H2，标题作用于原选区且选区不被重置；打开 / 关闭 / 悬停不改编辑器选区；验证：L4 手工三步操作 + 无控制台报错。
- [ ] 3.6 可访问性验证：读屏（Narrator）识别「标题级别」按钮与七项选项、播报当前选中级别，键盘焦点可见；验证：L4 读屏记录（若环境不可用，在交付报告中标注未验证项与原因）。
- [x] 3.7 组件接线门禁：纯函数门禁（3.1）覆盖不到组件本身，补 `test/heading-select-wiring.test.mjs` 静态锁住 radix `Select` 接线（既有依赖、无原生 `<select>` 回退）、`Select.Portal` + `position="popper"` + `collisionPadding`、`onValueChange → onSelect`、七项来自 `HEADING_OPTIONS`、可读名称与回显、编辑器侧 `applyHeadingCommand` 与 `value`/`onSelect` 成对、样式无裸色值与手写 `.is-active` 残留；运行态行为（焦点 / Tab / Esc / 点选项）仍归 5.5 真机；验证：新门禁 6 项全过并接入 `scripts/check.ps1`「标题下拉接线契约」。

## 4. S4 淡化修复

- [x] 4.1 抽取淡化决策纯函数：新增 `shouldFade({enabled, pointerOutside, hasFocus, composing, overlayOpen})` 与去抖常量（250ms），四条件任一不满足即不淡化；验证：新增 `src-react/test/sticky-fade-guard.test.mjs` 用 node 直跑，覆盖四条件全组合与去抖常量。
- [x] 4.2 接入 `StickyWindow` 守卫：用 `mousemove` 追踪指针坐标并与窗口边界比较（`mouseleave` 仅作兜底）、`window` blur / focus + `document.hasFocus()`、`compositionstart` / `compositionend`、窗口内浮层检测、250ms 去抖；`keydown` / `mousemove` / `mouseenter` / `mousedown` / `focus` / `compositionstart` 立即取消待执行淡化；知识与自由便签两个展开态共用，mini 态与关闭设置时行为不变；验证：`typecheck` / `lint` 通过，指针移出 + 切应用后 250ms 淡化、移回 / 点回立即恢复（L4）。
- [ ] 4.3 中文输入法真机回归（L4 必做）：开启「移出淡化」→ 指针放在便签正文内 → 用微软拼音输入（候选窗出现、移动、长句、空格上屏）→ 全程观察便签不淡化；再用第三方输入法（搜狗或 QQ 拼音）重复一次；记录输入法版本与 WebView2 版本；验证：真机操作记录 / 录屏，失败则附复现步骤与现象。
- [ ] 4.4 淡化边界回归：关闭移出淡化不淡化、mini 条不淡化、系统「减少动态效果」下无过渡动画、快速移出又移回不闪、自定义不透明度（10 / 38 / 60 / 100）生效；验证：L4 逐项手工核对。
- [x] 4.5 更新便签文档：`docs/capability/04-便签系统.md` 补充淡化触发条件（指针在窗外 + 无焦点 + 非组合输入 + 无浮层）与 250ms 去抖说明；验证：`npm run check` 中文档契约项通过。

## 5. 集成验收

- [x] 5.1 一键门禁：`npm run check`（`scripts/check.ps1` 实际 **17 步**：本 change 补三条契约门禁（托盘入口 / 便签知识来源 / 标题下拉接线）后由 18 增至 20，AGENTS §4.3 的「17 项」更旧）记录条数与结果（OK / SKIP / FAIL，SKIP 必须列名并说明原因）；验证：日志 `logs/check-<时间戳>.log` 与终端输出。
- [x] 5.2 后端单测：`cargo test --manifest-path src-tauri/Cargo.toml` 全绿，包含旧数据兼容新测试；验证：输出中的测试条数与 pass 结果。
- [x] 5.3 命令契约：`node test/command-parity.test.mjs` 通过，确认本 change 未新增 / 删除命令（当前基线 71 —— `kb-storage-backup` 落地后由 68 上移，2026-11-15 复跑门禁确认）；验证：6 tests pass、0 fail。
- [x] 5.4 生产构建：`npm run build:front`（`tsc -b && vite build`）退出码 0；验证：命令输出末尾构建成功信息。**构建在另一台机器上进行（用户 2026-11-15 确认），本机（WebView2 dev 沙箱）vite 构建被环境阻断，不作为验证环境**：`[UNLOADABLE_DEPENDENCY] Could not load node_modules/@tailwindcss/oxide-win32-x64-msvc/tailwindcss-oxide.win32-x64-msvc.node` + `[plugin externalize-deps] Error: spawn EPERM`。
- [ ] 5.5 全量 L4 真机清单：启动 `npm run dev`，走查 AGENTS §4.2 五条冒烟（快速记录热键、今天视图与侧栏计数一致、提醒到点通知、便签三态、知识库搜索）+ 本 change 专项（便签卡片页、托盘五项、中文输入法淡化、标题下拉）；标题下拉至少覆盖 8 项：正文 ⇄ H1–H6 切换与回显、↑↓ 首尾回绕、Esc 关闭并还焦触发按钮、Tab 后编辑器仍能继续输入、鼠标点选项生效、暗色主题下浮层无白底、`Ctrl+F` 不被浮层吞掉、便签窗窄容器（1200px）里浮层不被裁剪、浮层不被随后打开的详情弹窗 / 链接弹窗压住；成套步骤见同目录 `acceptance-l4.md`；验证：逐项记录通过 / 失败，失败项附现象。
- [x] 5.6 文档与实现一致性终检：`docs/ARCHITECTURE.md`（§2 计数、§4 提醒规则、§5 托盘、§10 命令数）、`docs/capability/03` / `04` / `08`、README；验证：全仓搜索五个移除托盘项与「免打扰」仅剩历史说明，`npm run check` 文档契约项通过。
- [x] 5.7 OpenSpec 校验：`openspec validate sticky-shell-polish --strict` 通过；验证：命令输出 `Change 'sticky-shell-polish' is valid`。

## Workflow follow-up

- 文档类交付（`docs/ARCHITECTURE.md`、`docs/capability/*`、README）先给人工审核，批准前不 commit、不 push（AGENTS §4.6）。
- 分批提交：S1 / S2 / S3 / S4 各自独立提交，中文 conventional commit（如 `feat(shell):`、`feat(sticky):`、`refactor(editor):`、`fix(sticky):`）；每批只提交本批相关文件，提交前确认 `npm run check` 无 FAIL。
- 并行 change（`refine-frontend-visual-language`、`kb-file-tree`）若先落地，合并时以视觉语言文件为基准复核 `App.tsx`、`SettingsView.tsx`、`api.ts`、`prosemirror.css` 的冲突，不覆盖对方改动。
- 跨轮任务在 `ledger/` 登记目的 / 过程 / 结论，结案前更新状态。
- 集成验收通过、文档审核批准后，按项目流程归档 change（`openspec archive sticky-shell-polish`）。
