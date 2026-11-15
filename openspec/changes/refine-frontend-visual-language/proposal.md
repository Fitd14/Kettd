# Proposal

## Why

界面的视觉命题只活在一个窗口里：捕获条上那张「描图纸」（`src-react/src/index.css:42-53`）是全应用唯一有来处的材质，而六个一级页面退化成「暖白底 + 琥珀」的默认脸——同一档字号硬写八次，hover 一律变成浅底块，和任何一套模板没有区别。

更麻烦的是界面在向用户解释实现：今天页副行写「三处同源」（`src-react/src/views/TodayView.tsx:113`），收件箱副行写「越久滞留越醒目（琥珀，非红）」（`InboxView.tsx:126`），输入框把快捷键塞进 placeholder（`TodayView.tsx:123`）。排版决策缺位让层级倒挂——页面标题 18px/600（`app.css:186-190`）小于 KPI 数字 22px/700（`app.css:557`）；知识库详情自带第二套皮肤，8 个硬编码 hex（`src-react/src/views/kb.css:216-223`）与 AGENTS.md §2.2「颜色一律 `hsl(var(--xxx))`，禁止硬编码色值」正面冲突。

现在改的理由：全部落在 CSS 与文案层，一行业务逻辑不用动；再叠功能，字号和皮肤会像债务一样滚起来。

## What Changes

- 新增视觉语言契约：4 档 chrome 字号 + 1 档阅读字号，颜色只从令牌取，一级页面只用 `.view` 外壳。
- 材料语法二分：**描图纸**给「立刻离开的浮层」，**宣纸**给「停留阅读的面」；用哪张纸由这条规则决定，不靠手感。
- 确立唯一用力处：同一时刻只有一张纸浮起来——当前行。其余行平贴账本，去掉默认 hover 底色。
- 行尾动作改为「⋯」常驻 + 次要动作 hover/focus 展开，可发现且不挤动行宽。
- 三处实现词汇文案改为用户语言，快捷键移进空态提示。
- 知识库详情并入共享令牌：8 个 hex 收编为 2 个正式令牌 + 4 处映射。
- **BREAKING**（仅视觉层）：`#/kb` 的 `padding: 1rem` 与宣纸面底色改用令牌，知识库页外观会变。

## Capabilities

### New Capabilities

- `visual-language`: 界面视觉语言契约——字号阶梯、颜色令牌、两种纸的材料语法、页面外壳、当前行浮起规则。
- `interface-copy`: 用户可见文案口径——只说用户的事，不解释实现；快捷键只出现在可学到的地方。

### Modified Capabilities

（无。`openspec list --specs` 返回空，主 spec 目录尚无任何 capability，本次两项均为新增。）

## Impact

- 代码：`src-react/src/index.css`（新增 `--fs-*`、`--seal`）、`src-react/src/styles/app.css`（字号、行外壳、hover、row-actions）、`src-react/src/views/kb.css`（去第二皮肤）、`src-react/src/rendering/styles/prosemirror.css`（`--ink-3` 回落值）、`src-react/src/views/TodayView.tsx:111-123`、`InboxView.tsx:126`、`KbView.tsx:464`。
- 文档：`README.md`（技术架构下新增「视觉语言」）、`docs/ARCHITECTURE.md:23`（`spark-output/design/` 已不存在）、`docs/capability/01-任务与四视图.md:37`（副行与实现漂移）、`docs/capability/06-知识库.md`（材质与令牌）、`AGENTS.md` §2.2（新增字号/外壳硬约束，需人工批准后提交）。逐条清单见 `design.md` §6。
- 不改：命令契约（64 命令四方同步不受影响）、依赖清单、Rust 侧、数据格式与门禁脚本。
- 验证：`npm run check`（12 项，含 `test/docs.test.mjs` 章节契约）、`npm run build`；L4 真机至少覆盖今天页与知识库页。