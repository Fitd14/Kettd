# ui-components Specification

## Purpose
定义前端交互控件的唯一来源与行为契约：所有按钮、输入框、弹层、菜单、标签页都经 `components/ui/` 原语渲染，键盘与焦点行为符合 WAI-ARIA 实践，组件按表现形式分层，存量原生控件分批收口到零。

## Requirements

### Requirement: 交互控件单一出口

页面与业务组件 SHALL 只通过 `components/ui/` 原语渲染可交互控件；MUST NOT 直接写原生 `<button>` / `<input>` / `<select>` / `<textarea>`，MUST NOT 自建弹层、菜单、标签页的开关与焦点逻辑。

#### Scenario: 新页面需要按钮

- **WHEN** 开发者在 `views/` 下新增一个页面并加入操作按钮
- **THEN** 该按钮来自 `components/ui/button`，文件里不出现 `<button`

#### Scenario: 业务组件需要输入框

- **WHEN** `components/kb/` 下的组件需要一行可编辑文本
- **THEN** 使用 `components/ui/input`，不出现 `<input>`，也不为它单独写样式的 `:focus` 分支

#### Scenario: 审计页面侧原生控件

- **WHEN** 对 `src-react/src/views/**` 与 `src-react/src/components/**`（`ui/` 除外）检索 `<button` / `<input` / `<select` / `<textarea`
- **THEN** 除本条 Requirement 的例外（整行可点击的行容器、无外观的隐藏输入、`Input` 的 `bare` 用法）与 `design.md` 已登记的映射缺口外，结果为零；缺口清单与行号记在 `tasks.md` 5.1 的审计段

### Requirement: 弹层与菜单的键盘行为

弹层、菜单、标签页、提示 SHALL 具备 WAI-ARIA Authoring Practices 规定的键盘与焦点行为，由原语统一实现，不由各页面各自补。

#### Scenario: 对话框关闭

- **WHEN** 知识库文件夹弹窗打开后按 `Esc` 或点击遮罩
- **THEN** 弹窗关闭，焦点回到打开它的按钮，弹窗打开期间焦点不逃到背景页面

#### Scenario: 菜单键盘导航

- **WHEN** 任务行上下文菜单打开后用 `↑` / `↓` / `Home` / `End` 操作
- **THEN** 高亮项按顺序循环移动，`Enter` 触发当前项，`Esc` 关闭并把焦点还给触发按钮

#### Scenario: 标签页键盘切换

- **WHEN** 回顾页或计划页的标签页获得焦点后按 `←` / `→`
- **THEN** 选中项切换且焦点跟随，未选中面板不进入 tab 序列

### Requirement: 原生元素只允许出现在原语内部

`components/ui/` 之外 MUST NOT 渲染原生交互元素；每个控件在原语层只有一个实现，页面只传 props，不复制行为。

例外三类：① **整行可点击的列表 / 树 / 菜单行**（外观由行容器自身承担、没有按钮轮廓，整行即命中区域）不属于控件，允许保留原生 `<button>`；② **无外观的隐藏输入**（`display:none` 的 `type="file"`，由按钮代理点击）；③ `design.md` 映射缺口里尚未补齐 D9 口径的单点钮，补齐后必须迁移。判定口径见 `design.md` D12 与 D9 的映射缺口段。

#### Scenario: 新增一个控件

- **WHEN** 需要项目里尚不存在的控件（例如复选框）
- **THEN** 先在 `components/ui/` 建封装，再由页面引用；不得在页面里就地手写一份

#### Scenario: 整行可点击的列表项

- **WHEN** 一行本身即点击目标（如知识树行、文件夹行、扁平结果行、右键菜单项）
- **THEN** 保留原生 `<button>` 承载行语义，不套用 `ui/button`，也不让 `.btn` 的边框与内边距进入行布局

### Requirement: 组件按表现形式分层

组件 SHALL 分三层：`components/ui/` 为无业务原语；`components/<domain>/` 为业务域（如 `kb/`、`editor/`）；`components/` 根为跨视图壳。新增层级 MUST 先写入 `README.md` §目录结构 再落目录。

#### Scenario: 新业务组件落位

- **WHEN** 新增一个只服务知识库的组件
- **THEN** 落在 `components/kb/`，不放进 `components/` 根，也不放进 `ui/`

#### Scenario: 原语不含业务

- **WHEN** 检查 `components/ui/` 下任一文件
- **THEN** 它不 import `kernel/`、`lib/api` 或任何业务模块，只接收 props 与 className

### Requirement: radix 依赖纪律

radix 能力 SHALL 只从统一包 `radix-ui` 引入；MUST NOT 引入 `@radix-ui/react-*` 分包，MUST NOT 为控件行为引入第二套弹层、菜单或标签页库。

#### Scenario: 新增提示气泡

- **WHEN** 需要 tooltip 行为
- **THEN** 从 `radix-ui` 取 `Tooltip` 并在 `components/ui/` 封装，`package.json` 不新增依赖

#### Scenario: 存量分包入线

- **WHEN** 检查 `src-react/package.json` 的 dependencies
- **THEN** 只有统一包 `radix-ui`，没有 `@radix-ui/react-*` 条目

### Requirement: 原语遵现有视觉令牌

原语 SHALL 用 `cva` 组织变体，颜色与间距取现有令牌（`hsl(var(--*))`、`--sp-*`），字号落在四档之内；MUST NOT 内联硬编码色值或裸像素字号，浮层材质延用描图纸、停留面延用纸面。

#### Scenario: 新增输入框样式

- **WHEN** 编写 `components/ui/input.tsx`
- **THEN** 边框、底色、焦点环全部来自令牌变量，文件中检索不到裸 hex 与 `font-size: 13px`

### Requirement: 按钮尺寸与变体统一

`components/ui/button` SHALL 只暴露三档尺寸与五个变体：尺寸 `xs`（22px）/ `sm`（26px）/ `md`（30px），图标钮用同档位的 `icon-xs` / `icon-sm` / `icon-md`；变体 `default` / `primary` / `ghost` / `soft` / `danger`，按下态用 `aria-pressed` 表达而非新增类。另有一个不成档的 `bare` 变体，只清盒子（无边框、无底色、无内边距），专供 chip 内部的文字与 × 钮，颜色和宽度仍由所在域的类决定；页面 MUST NOT 用 `bare` 给普通按钮去描边。页面 MUST NOT 为单处需要扩尺寸表或变体表，MUST NOT 使用未定义的按钮修饰类。

#### Scenario: 页面需要次操作按钮

- **WHEN** 页面行内需要一个「编辑 / 取消」类按钮
- **THEN** 使用 `variant="ghost" size="xs"`，不新增类名，也不写内联 padding 与字号

#### Scenario: 对话框主操作

- **WHEN** 弹窗需要主操作按钮
- **THEN** 使用 `variant="primary" size="md"`，其底色来自 `hsl(var(--primary))`，不依赖页面作用域补丁

#### Scenario: 便签内的按钮

- **WHEN** 便签窗口内需要按钮（置顶 / 展开 / 删除）
- **THEN** 使用 `variant="ghost" size="xs"` 并置于 `[data-surface="sticky"]` 下，尺寸与圆角随全局，只把前景色与 hover 底色换为 `--sticky-*`

#### Scenario: 窗口控制按钮

- **WHEN** 渲染标题栏的最小化 / 最大化 / 关闭
- **THEN** 使用 `size="window"`（44×32、无圆角），关闭键用 `variant="window-close"`（默认透明、悬停呈破坏性实心）；这一段不参与 22/26/30 三档阶梯

#### Scenario: 审计未定义的按钮类

- **WHEN** 在 `src-react/src/styles/**` 检索 `.btn.` 修饰类，或在 JSX 里检索 `className="btn …"`
- **THEN** 每个被使用的修饰类都有全局定义与消费者；`outline` / `primary` 这类只存在于页面作用域补丁的写法不再出现

#### Scenario: 收口时清掉被取代的专用类

- **WHEN** 五批迁移完成
- **THEN** 只服务旧写法的声明（`width` / `height` / `hover` / 选中色）已删除，且 `grep` 无引用的整段规则一并删除；仍被 JSX 引用的专用类只保留盒子、配色与定位职责，清单与理由记在 `tasks.md` 5.6，不得再用来重复表达按钮尺寸或变体

### Requirement: 迁移不得让既有交互倒退

迁移现有控件 SHALL 保持当前键盘与焦点行为；原本依赖事件顺序的地方 MUST 保留原事件。

#### Scenario: 编辑器工具条

- **WHEN** 把 `rendering/tiptap-editor.tsx` 工具条按钮换成 `ui/button`
- **THEN** 按钮仍带 `onMouseDown` 的 `preventDefault`，点按钮后编辑器保持焦点、不收起选区

#### Scenario: 尚无原语的控件

- **WHEN** 迁移到 `KbView` 与 `kb-tree` 的两处 listbox 行为
- **THEN** 在选定替代原语前保持现状，不临时手写第二套

### Requirement: 存量收口到零

迁移 SHALL 分批交付且每批可独立运行；全部完成后页面侧原生交互元素与手写弹层、菜单、标签页计数 SHALL 为零——「原生元素只允许出现在原语内部」那条 Requirement 列出的三类例外不计入。

#### Scenario: 中途批次可交付

- **WHEN** 只完成按钮与输入框两批
- **THEN** 应用正常启动，未迁移的页面维持现状，不存在半迁移导致的样式或焦点缺陷

#### Scenario: 收口验收

- **WHEN** 五批全部完成
- **THEN** 页面侧 `<button>` / `<input>` / `<select>` / `<textarea>` 只余例外清单里的行容器与隐藏输入，手写 `role="menu|tab|popover"` 计数为零，`components/ui/` 之外没有第二份控件行为；`role="dialog"` 只余 D14 记录的窄窗大纲抽屉一处（依赖祖先定位与 `--ink-*` 作用域，portal 会铺满视口），在支持 `container` 前保持原生
