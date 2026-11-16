# visual-language Specification

## Purpose
定义 Kettd 主界面的视觉语言：一眼能认出的字号阶梯、只从令牌取的颜色、两种纸的材料语法，以及「同一时刻只有一张纸浮起」的层级规则。

## Requirements

### Requirement: 字号阶梯限定为四档

界面 chrome 字号 SHALL 只使用四档：`--fs-meta` 11px、`--fs-sub` 12.5px、`--fs-body` 14px、`--fs-masthead` 22px；新增样式 MUST NOT 引入第五档。

#### Scenario: 新增一条列表样式

- **WHEN** 开发者为新的行元素写 `font-size`
- **THEN** 只能引用上述四个变量之一，`src-react/src/styles/app.css` 与 `src-react/src/views/kb.css` 中不再出现裸数值字号

#### Scenario: 知识库列表折算

- **WHEN** 打开 `#/kb`
- **THEN** 搜索框、条目标题、标签、空态文案分别落在 12.5 / 14 / 11 / 14 档，不出现 `0.72em`…`1.5em` 的 em 阶梯

### Requirement: 阅读字号仅限编辑器正文

阅读面字号 SHALL 在 `12–24px` 之间逐像素取档（共 13 档，默认 15px），由用户在文档工具条的排版弹层里决定；该档 MUST NOT 出现在 chrome 元素（页面标题、按钮、列表行、空态）上。知识库详情标题 SHALL 与该档同档，标题层级 MUST 按既有倍率随之等比缩放。

#### Scenario: 正文与 chrome 并存

- **WHEN** 知识库详情同时显示标题、标签与编辑器正文
- **THEN** 标题与正文同档，标签仍为 chrome 的 11 / 12.5 档，两层行高互不干扰

#### Scenario: 未设置时的默认档

- **WHEN** 用户从未调整过阅读字号
- **THEN** 正文与标题均为 15px，行高 1.85，比本次变更前（16px）略紧凑、每行多容纳约一个字

#### Scenario: 逐像素调档

- **WHEN** 用户在排版弹层里把阅读字号从 15px 调到 16px
- **THEN** 正文与标题立即变为 16px，H1–H6 按 `1.35 / 1.2 / 1.05 / 1 / .93 / .88` 倍率等比变化，侧栏、列表行、按钮字号不变

#### Scenario: 越界值被拒

- **WHEN** 设置里出现 `12–24` 之外的数值（如 11px 或 30px）
- **THEN** 系统回退到 15px 并正常显示正文，界面不出现异常字号

#### Scenario: 便签窗不受影响

- **WHEN** 用户把知识库阅读字号设为 24px 后打开知识便签
- **THEN** 便签正文仍为 16px，便签标题不放大

### Requirement: 页面外壳唯一

所有一级页面根节点 SHALL 使用 `.view` 外壳（padding 24px、gap 16px、纵向滚动）；页面 MUST NOT 自定义根 padding 或 gap。

#### Scenario: 知识库与其他页面左缘对齐

- **WHEN** 依次打开 `#/today` 与 `#/kb`
- **THEN** 页面标题的上边距与左缘横坐标差不超过 1px

### Requirement: 层级不倒挂

页面报头 SHALL 使用 `--fs-masthead`（22px / 600），MUST NOT 小于同页任何内容数字的字号；KPI 数字与报头同档时 SHALL 以 700 字重加 `tabular-nums` 区分。

#### Scenario: 回顾页

- **WHEN** 打开 `#/review`
- **THEN** 页面标题为 22px / 600，KPI 数字为 22px / 700 且等宽对齐，二者不同屏竞争

### Requirement: 字体族只多一族

界面默认字族 SHALL 为 `--font-ui`（系统黑体栈）；自绘字体 SHALL 只有一个报头衬线子集（`Kettd Serif`，61 字 / 约 12 KB），MUST 只用于 `.view-title` 与侧栏品牌字两处；样式表 MUST NOT 出现裸字族名（如 `"Noto Sans SC"`），字族一律经 `--font-*` 令牌取值。

知识库文档标题 SHALL 使用系统衬线，并经独立令牌 `--font-reading-title` 取值；该令牌 MUST NOT 包含 `Kettd Serif`，以免同一标题里部分字走 61 字子集、其余字回落到系统衬线而出现两种衬线混排。

#### Scenario: 新增文案落到正文

- **WHEN** 新功能在列表行、按钮或副行里新增中文
- **THEN** 这些字由 `--font-ui` 渲染，不出现同一行里部分宋体、部分黑体的混排

#### Scenario: 报头出现子集外的字

- **WHEN** 报头渲染了子集字表之外的字符
- **THEN** 该字符回落到 `"Noto Serif SC"` → `"Songti SC"` → `SimSun` → `serif`，仍是衬线，不出现黑体断档

#### Scenario: 文档标题不混用子集

- **WHEN** 用户打开标题含子集外字符的知识条目
- **THEN** 整条标题由 `--font-reading-title` 一族的字体渲染，同一条标题内不出现两种衬线

#### Scenario: 授权可追溯

- **WHEN** 检查 `src-react/src/assets/fonts/`
- **THEN** 同时存在子集字体、来源说明 `NOTICE.md` 与 OFL 1.1 全文 `OFL-1.1.txt`

### Requirement: 阅读面字族可由用户切换

知识库正文使用的 `--font-reading` SHALL 由 `settings.readingFont` 决定，取值 MUST 限定为三态：空串（跟随 `--font-ui`）、`system:<族名>`（本机已装字体）、`file:<文件名>`（用户上传，存于数据目录 `fonts/`）；切换 MUST 只影响阅读面，MUST NOT 改变 `--font-ui` 与 `--font-serif`。

#### Scenario: 默认无设置
- **WHEN** `readingFont` 为空串
- **THEN** 不输出 `--font-reading` 覆写，正文回落 `var(--font-ui)`，与全站一致

#### Scenario: 选择系统字族
- **WHEN** 用户在设置页选「楷体」
- **THEN** `readingFont` 落库为 `system:KaiTi`，编辑器正文显示为楷体，页面标题、按钮、列表行仍是黑体

#### Scenario: 上传自己的字体
- **WHEN** 用户选择本机一个 `.ttf`（≤16 MB）
- **THEN** 文件被复制进 `<数据目录>/fonts/`，以 `FontFace` 注册为 `Kettd Reading` 并立即生效；重启应用后仍生效

#### Scenario: 字体不可用时不吞正文
- **WHEN** 记录里的字体文件已被删除或损坏
- **THEN** 正文静默回落到 `var(--font-ui)`，MUST NOT 出现空白或错误页

### Requirement: 颜色只从令牌取

所有颜色 SHALL 形如 `hsl(var(--token))` 或 `var(--token)`；样式表 MUST NOT 出现裸 hex 色值（纸纹用的 SVG data URI 除外）。

#### Scenario: 知识库详情材质

- **WHEN** 在 `src-react/src/views/kb.css` 中检索 `#[0-9a-fA-F]{3,8}`
- **THEN** 结果为空，`--paper` / `--ink-*` / `--seal` / `--line-warm` 全部来自 `src-react/src/index.css` 令牌

### Requirement: 材料语法二分

界面 SHALL 只有两种纸：**描图纸**（半透明、上缘 1px 高光，用于「立刻离开的浮层」）与**纸面**（不透明，分白页与宣纸两样，用于「停留的面」）；一个面 MUST NOT 混用两种纸，描图纸面 MUST NOT 有实心背景，纸面 MUST NOT 加半透明模糊。

#### Scenario: 捕获条与知识库详情

- **WHEN** 捕获条与知识库详情同时可见
- **THEN** 捕获条仍为三段渐变描图纸，知识库详情仍为不透明宣纸，两者底色不互换

#### Scenario: 暗色下描图纸仍可读

- **WHEN** 切换到暗色主题
- **THEN** 描图纸等效不透明度不低于 0.78，正文对比度不低于 4.5:1

### Requirement: 同一时刻只有一张纸浮起

列表行 SHALL 默认平贴（无底色、无边框、无阴影）；当且仅当 hover 或 `:focus-within` 时，该行 SHALL 浮起为唯一一张纸（白底、1px 边、微阴影、上移 1px），MUST NOT 同时出现两张浮起的行。

#### Scenario: 鼠标移过两行

- **WHEN** 指针从第一行移到第二行
- **THEN** 第一行回到平贴、第二行浮起，任意一帧内浮起行数不超过 1

#### Scenario: 键盘导航

- **WHEN** 用 `j` / `k` 移动当前行
- **THEN** 当前行同样浮起，焦点环与纸片边缘同时可见

### Requirement: 动效与可达性不回退

纸片浮起 SHALL 只动 `transform` 与 `opacity`，时长不超过 160ms；`prefers-reduced-motion: reduce` 下 MUST 关闭位移只留渐显；焦点指示 MUST 保持可见且对比度不低于 3:1。

#### Scenario: 减弱动效

- **WHEN** 系统开启「减少动态效果」
- **THEN** 行高亮无位移、仅淡入，且 120ms 内完成

#### Scenario: 键盘可见焦点

- **WHEN** 用 `Tab` 遍历页面控件
- **THEN** 每个可聚焦元素都有可见焦点环，焦点环不被行纸片底色吞掉

### Requirement: 阅读字距可调

阅读面字距 SHALL 从三档中取值——常规 `0em`（默认）、宽松 `0.03em`、较宽 `0.06em`——由用户在文档工具条的排版弹层里决定；该档 MUST 以 `em` 表达而非常量像素，MUST NOT 改变 chrome 字号与标题自带的字距常量。

#### Scenario: 默认字距

- **WHEN** 用户从未调整过字距
- **THEN** 正文字距为 0，正文观感与本次变更前一致

#### Scenario: 选中间档

- **WHEN** 用户把字距切到「宽松」（0.03em），正文为 15px
- **THEN** 正文字距约为 0.45px，字与字之间出现可感知但不松散的间隙，标题与列表行不受影响

#### Scenario: 与字号档叠加

- **WHEN** 阅读字号为 24px、字距为「较宽」（0.06em）
- **THEN** 实际字距约为 1.44px，随字号等比放大而不是固定像素

#### Scenario: 不改变列宽

- **WHEN** 用户在任一列宽档位下调大或调小字距
- **THEN** 正文列的像素宽度不变，变的是每行可容纳的字数
