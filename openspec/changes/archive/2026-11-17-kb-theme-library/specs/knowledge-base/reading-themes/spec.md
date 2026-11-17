# Spec Delta

## Purpose

让用户用一个 CSS 文件替换整个应用的外观：主题可安装、可切换、可删除，可同时声明亮暗两套取值，覆盖外壳配色（背景、卡片、弹层、主色、次级色、静音色、边框、输入、聚焦环、链接）与阅读区观感（纸面、墨色、字族、行高、标题、引用、代码、表格、链接、图片）；四个窗口一致生效，不动字号/字距/列宽档位与页面布局。

## ADDED Requirements

### Requirement: 主题文件格式与声明白名单

一个主题 MUST 是单个 UTF-8 编码的 `.css` 文件，首部 MUST 用注释块声明主题名（`name`），可选声明作者（`author`）与说明（`description`）；正文 MUST 只通过白名单内的自定义属性给值，白名单之外的声明 MUST 被忽略并计入「已忽略」数。白名单为 59 个变量、分十一组（清单见下），`--paper-deep` MUST NOT 被受理（有声明无消费者）；字号、正文字距、列宽与留白令牌 MUST NOT 被主题触达，语义色（`--cat-*`、`--pri-*`、`--destructive`）与快速捕获气泡的固定深底（`--bubble-*`）MUST NOT 被主题改写。

#### Scenario: 白名单变量清单

- **WHEN** 主题声明下列任一组变量
- **THEN** 该组变量全部受理：外壳配色 `--background` `--foreground` `--card` `--card-foreground` `--popover` `--popover-foreground` `--primary` `--primary-foreground` `--secondary` `--secondary-foreground` `--muted` `--muted-foreground` `--accent` `--accent-foreground` `--border` `--input` `--ring` `--link` `--seal`（19 个，取值必须是**不带 `hsl()` 的 HSL 三元组**，如 `40 33% 98%`，与 `index.css` 既有令牌同形）；纸面墨色 `--paper` `--ink-1` `--ink-2` `--ink-3` `--line-warm` `--seal-track` `--seal-ink` `--seal-soft`；纸面形态 `--kb-paper-radius` `--kb-paper-shadow` `--kb-paper-texture`；字族 `--font-reading` `--font-reading-title`；段落 `--kb-line-height`；标题 `--kb-h1`…`--kb-h6` `--kb-head-tracking` `--kb-h1-rule` `--kb-h2-rule`；引用 `--kb-quote-bg` `--kb-quote-line` `--kb-quote-ink` `--kb-quote-radius`；代码 `--kb-code-bg` `--kb-code-ink` `--kb-code-radius` `--kb-inline-code-bg` `--kb-inline-code-ink` `--kb-inline-code-radius`；表格 `--kb-table-line` `--kb-table-head-bg` `--kb-table-zebra` `--kb-table-radius`；链接 `--kb-link-ink` `--kb-link-underline`；图片 `--kb-img-radius`

#### Scenario: 合法主题被接受

- **WHEN** 用户导入一个首部含 `name: 我的主题`、正文只声明白名单属性的 `.css` 文件
- **THEN** 主题以「我的主题」出现在设置页列表，选中后外壳（背景、卡片、边框、按钮）与阅读区（纸色、字族、正文元素）在同一帧一起换装，四个窗口一致

#### Scenario: 缺主题名被拒

- **WHEN** 文件首部没有任何 `name` 声明
- **THEN** 导入被拒绝，提示「主题文件缺少名称」，列表不新增条目

#### Scenario: 白名单外声明被忽略

- **WHEN** 主题里写了 `position: fixed` 或 `font-size: 40px` 这类白名单外声明
- **THEN** 该声明不生效，其余白名单声明照常生效，界面不出现浮层错位或异常字号，导入结果提示「忽略 M 条」

### Requirement: 正文元素装饰随主题

主题给出的装饰变量 MUST 作用于阅读区的对应元素，且编辑器与只读渲染保持一致：引用块（底色、左线、墨色、圆角）、代码块与行内代码（底色、墨色、圆角）、表格（边线、表头底色、斑马纹、圆角）、链接（墨色、下划线粗细）、图片圆角、纸面（圆角、投影、纸纹强度）、标题（字距、h1/h2 底线）。

未给出某变量时，该元素 MUST 保持变更前的默认计算值（逐像素一致），不得因接线而改变默认观感。

#### Scenario: 代码块换成上游底色

- **WHEN** 主题声明 `--kb-code-bg: #ECECEE` 与 `--kb-code-radius: 8px`
- **THEN** 编辑器与只读视图的代码块都变为该底色与圆角，行内代码仍按自己的 `--kb-inline-code-*` 取值

#### Scenario: 表格斑马纹

- **WHEN** 主题声明 `--kb-table-zebra: #ECECEE` 与 `--kb-table-head-bg: #ECECEE`
- **THEN** 表格偶数行与表头铺该底色，表头文字仍用 `--ink-1`，不出现不可读组合

#### Scenario: 引用块圆角与左线

- **WHEN** 主题声明 `--kb-quote-bg: #F3EDE5`、`--kb-quote-line: 3px solid #C58C64`、`--kb-quote-radius: 0 8px 8px 0`
- **THEN** 两处引用块（编辑器与只读）同为暖底、3px 陶土左线、右侧两角 8px 圆角

#### Scenario: 未声明装饰时保持原样

- **WHEN** 用户切回「默认」主题
- **THEN** 引用、代码、表格、链接、图片的计算样式与本变更前逐项一致（用同一份基线记录比对）

### Requirement: 几何与结构取值的区间钳制

装饰类变量的取值 MUST 落在允许区间内，越界或不合语法 MUST 丢弃该条并计入「已忽略」，MUST NOT 整份拒收（区间见下）。

#### Scenario: 区间表被严格执行

- **WHEN** 主题声明圆角、数值或结构类变量
- **THEN** 按下列区间校验：圆角 `--kb-paper-radius` 0–24px，`--kb-quote-radius` / `--kb-code-radius` / `--kb-table-radius` / `--kb-img-radius` 0–16px，`--kb-inline-code-radius` 0–8px，允许 1–4 个 px 值且每值在区间内；数值 `--kb-line-height` 1.3–2.4、`--kb-h1..h6` 0.8–2.0、`--kb-paper-texture` 0–0.12、`--kb-head-tracking` -0.02em–0.2em、`--kb-link-underline` 0–2px；结构 `--kb-h1-rule` / `--kb-h2-rule` 取 `none` 或 `<0–4px> <solid|dashed|dotted> <颜色>`，`--kb-quote-line` 取 `<0–6px> <solid|dashed|dotted> <颜色>`，`--kb-paper-shadow` 取 `none` 或 `box-shadow` 形态的值

#### Scenario: 超大圆角被丢弃

- **WHEN** 主题写 `--kb-code-radius: 400px`
- **THEN** 该条被忽略，代码块保持默认圆角，导入提示出现「忽略 1 条」，其余变量照常生效

#### Scenario: 引用线语法不合

- **WHEN** 主题写 `--kb-quote-line: 5px double red`
- **THEN** 该条被忽略（只接受 solid / dashed / dotted），引用块保持默认左线

#### Scenario: 纸面投影越界

- **WHEN** 主题写 `--kb-paper-shadow: 0 0 400px 100px #000`
- **THEN** 该条被丢弃，纸面保持默认内阴影，不影响布局与滚动

### Requirement: 主题来源三态

阅读主题设置 SHALL 只有三种取值：空串（默认外观）、`builtin:<id>`（内置示例主题）、`file:<文件名>`（用户导入，存于数据目录 `themes/`）；系统 MUST NOT 把主题 CSS 正文写入 `data.json`，设置里只保存标识。

#### Scenario: 选内置主题

- **WHEN** 用户选中内置示例主题
- **THEN** 设置落库为 `builtin:<id>`，重启应用后仍是该主题

#### Scenario: 导入用户主题

- **WHEN** 用户导入 `我的主题.css`
- **THEN** 文件被复制到 `<数据目录>/themes/我的主题.css`，设置可落为 `file:我的主题.css`，源文件删除后主题仍可用

#### Scenario: 删除正在使用的主题

- **WHEN** 用户删除当前选中的用户主题
- **THEN** 设置先切回默认外观，再删除文件，阅读区不留半套样式

### Requirement: 主题作用于全应用

主题的令牌集 MUST 同时作用于应用外壳与知识库阅读区：`document.documentElement` MUST 是唯一的变量落点，未选中主题时所有令牌取值 MUST 与变更前逐像素一致。主题 MUST NOT 改变页面布局（面板尺寸、列宽、留白、滚动容器结构）与用户档位（阅读字号、正文字距、列宽上限），MUST NOT 改写语义色与快速捕获气泡的固定深底。

主窗、快速捕获、便签、待办悬浮四个窗口是独立 WebView，MUST 各自读取同一份设置并应用同一套令牌；任一窗口 MUST NOT 因主题出现不可读的文字/背景组合。弹层与菜单虽然 portal 到 `body`，但仍继承 `:root` 令牌，因此 MUST 跟随主题换装。

#### Scenario: 全应用换装

- **WHEN** 用户选中一个深色纸面主题并停留在知识库详情
- **THEN** 侧栏、列表行、顶部工具条、设置页按钮、卡片与弹层都换到该主题的外壳配色，阅读区同时换纸面与正文元素样式

#### Scenario: 四个窗口一致

- **WHEN** 主窗选中 Paperglow 后打开快速捕获、便签与待办悬浮
- **THEN** 三个独立窗口使用同一套令牌（同一背景、卡片、主色与边框），不出现某个窗口仍为默认外观

#### Scenario: 离开详情页仍生效

- **WHEN** 用户从知识库切到「今天」页面再切回
- **THEN** 两个页面都保持主题外观，切换过程不闪回默认配色

#### Scenario: 大圆角不改变布局

- **WHEN** 主题把 `--kb-paper-radius` 设为 `24px`
- **THEN** 面板内容宽度、正文列宽、滚动条位置均不变，只有四角被裁切；正文不因圆角被裁掉或溢出

#### Scenario: 语义色不被主题改写

- **WHEN** 主题写了 `--destructive`、`--pri-high` 或 `--cat-work`
- **THEN** 这些声明不在白名单，被忽略并计入「已忽略」；危险操作、优先级与分类色保持系统取值

### Requirement: 亮暗两套取值与三级回退

主题文件 MAY 为同一属性分别给出亮色与暗色两套值；取值 MUST 按「当前明暗对应套 → 另一套 → 内置默认」三级回退，回退 MUST NOT 让同一属性出现未定义的空值。

#### Scenario: 切换明暗自动换纸

- **WHEN** 主题同时声明亮色纸面 `#FBF7EF` 与暗色纸面 `#1B1B1D`，用户把应用切到暗色
- **THEN** 全应用纸面变为暗色值，正文墨色、代码块底色与表格线同步切换，无需重新选主题

#### Scenario: 只给一套值时

- **WHEN** 主题只声明亮色值，应用处于暗色
- **THEN** 全应用沿用该亮色套（主题作者的选择），出现不可读时可在设置页一键切回默认

### Requirement: 内置示例主题

应用 SHALL 内置 `paperglow` 与 `mdmdt` 两套示例主题，ID MUST 稳定并被 `READING_THEME_BUILTINS` 白名单收录；每套 MUST 同时提供 `.kb-theme-light` 与 `.kb-theme-dark` 两块取值，覆盖全部 59 个白名单变量，作为用户主题的完整样板。内置主题 MUST NOT 允许删除，格式与校验与用户主题一致；文件头 MUST 声明主题名、作者、取材来源与许可（第三方风格注明上游仓库与 Apache-2.0），MUST NOT 整份复制上游 CSS。

#### Scenario: 两套内置主题各自的签名特征

- **WHEN** 用户分别选中两套内置主题
- **THEN** `paperglow` 呈暖纸面、陶土强调、16px 纸面圆角与轻投影、暖底圆角引用、16px 代码块，行高 1.7、标题倍率 1.90/1.65/1.42/1.24/1.12/1.00；`mdmdt` 呈冷白纸、皇家蓝 `#3E69D7`、h1 底边、0.06em 标题字距、蓝底行内代码、斑马表格、扁平纸面，行高 1.7、倍率 2.00/1.75/1.50/1.25/1.12/1.00，暗色强调蓝提亮为 `#4F76DF`

#### Scenario: 开箱选内置主题

- **WHEN** 全新安装的用户打开设置页「阅读主题」
- **THEN** 列表含「默认」「Paperglow」「Mdmdt」，选中任一即生效，不出现空列表

#### Scenario: 内置主题随明暗切换

- **WHEN** 用户选中 Paperglow 后把应用切到暗色
- **THEN** 全应用使用该主题的暗色套（外壳转暗、阅读区暖褐纸面转暗棕、引用线与代码块同步转暗），无需重新选择

#### Scenario: 内置主题不可删

- **WHEN** 用户尝试删除内置示例主题
- **THEN** 界面不提供删除入口，内置主题始终可选

#### Scenario: 内置清单与资源一致

- **WHEN** 新增、改名或移除内置主题
- **THEN** `READING_THEME_BUILTINS`、`src-react/src/assets/themes/<id>.css` 与设置页选项同步更新，一致性测试通过；只改其一会使测试失败

#### Scenario: 内置主题零忽略

- **WHEN** 用解析器读取两个内置主题
- **THEN** 每套亮暗块各识别 59 条声明、忽略 0 条、错误 0 条；任一变量拼错会使一致性测试失败

### Requirement: 导入安全校验与失败回退

导入 MUST 校验扩展名、大小上限（512 KB）、UTF-8 可解码性与禁用规则：含 `@import`、远程 `url(http…)`、`expression(`、`javascript:` 的文件 MUST 被拒收并给出中文短句；已安装主题文件缺失或损坏时系统 MUST 静默回退默认外观并保留其余设置，MUST NOT 出现空白正文或错误页。

#### Scenario: 远程资源被拒

- **WHEN** 主题中含 `@import url("https://…")` 或 `background: url(http://…)`
- **THEN** 导入失败，提示「主题包含不允许的外部资源」，不写入数据目录

#### Scenario: 超大文件被拒

- **WHEN** 用户选择 2 MB 的 CSS 文件
- **THEN** 导入失败并提示「主题文件过大」，设置与已安装列表不变

#### Scenario: 主题文件被手工删除

- **WHEN** 设置里仍是 `file:我的主题.css`，但该文件已不存在
- **THEN** 全应用按默认外观渲染，正文完整可读，设置页提示该主题不可用并允许切回

### Requirement: 设置页主题区与即时切换

设置页 SHALL 提供「阅读主题」区，列出默认、内置与用户主题，支持导入与删除；切换主题 MUST 立即生效并持久化，MUST NOT 要求重启应用或重新打开条目。

#### Scenario: 选中即预览

- **WHEN** 用户在一个已打开的条目上切换主题
- **THEN** 全应用在当帧刷新为新的外壳配色与元素装饰，滚动位置与编辑内容不变

#### Scenario: 导入后自动选中

- **WHEN** 用户导入一个通过校验的主题
- **THEN** 该主题被加入列表并立即选中，全应用同步更新

### Requirement: 主题不覆盖用户档位

主题 MUST NOT 改变阅读字号（12–24px 十三档）、正文字距三档、列宽上限与正文列布局；这些仍由用户在排版弹层与设置中决定。主题 MAY 提供正文行高系数、H1–H6 倍率、标题字距与标题底线；标题字距 MUST 与用户字距档位**叠加**而非替换，正文字距 MUST 完全不受主题影响。未提供时 MUST 使用既有默认值（行高 1.85 / 只读 1.65，倍率 1.35 / 1.2 / 1.05 / 1 / .93 / .88 与 1.5 / 1.3 / 1.15 / 1.05 / .98 / .92）。

#### Scenario: 换主题后字号不变

- **WHEN** 用户阅读字号为 18px、字距「宽松」，随后切换主题
- **THEN** 正文仍为 18px 与 0.03em，只有颜色、字族、行高与元素装饰按主题变化

#### Scenario: 标题字距叠加

- **WHEN** 用户正文字距为「标准」（0em），主题声明 `--kb-head-tracking: 0.06em`
- **THEN** 标题在该主题字距基础上再叠加用户档位值；正文与列表字距保持用户档位，不随标题变化

#### Scenario: 主题提供标题倍率

- **WHEN** 主题声明 H2 倍率为 1.3 倍，阅读字号为 18px
- **THEN** H2 渲染为 23.4px，正文仍为 18px；切回默认主题后 H2 回到 1.2 倍

#### Scenario: 主题不得改列宽

- **WHEN** 主题尝试写 `--kb-measure-cap: 200ch`
- **THEN** 该声明因不在白名单被忽略，正文列宽仍由用户档位与面板宽度决定