# Design

## Context

动机见 `proposal.md`。这里只记决定做法的事实与约束。

**令牌是单源的，但只覆盖颜色与圆角。** `src-react/src/index.css:8-112` 定义了双态颜色、`--r-lg/md/sm/pill`、三档阴影、`--sp-1..8`、分类/优先级语义色、热力 4 档，以及只给捕获条用的硫酸纸令牌（`:42-53`）。**没有字号令牌**，字号全部写死在样式里。

**字号散成了九档数值。** `grep -c font-size src-react/src` 命中 138 处，散在 7 个样式文件：`styles/app.css` 48 处（10/11/12/13/14/15/18/22/34 九档，另有 `font-size: 0` 一处技巧值与 `0.85em` 一处），`views/kb.css` 34 处（`0.72em`…`1.5em`、`2.5rem` 的 em 阶梯），`rendering/styles/prosemirror.css` 34 处（`0.78em`…`1.5em`），其余在 `capture/`、`sticky/`、`todo/`、`views/settings-*.css`。同一角色在不同文件里取不同值，是「不像一个产品」的主要来源。

**外壳是统一的，只有知识库例外。** `src-react/src/App.tsx:154` 渲染 `<main className="shell-main">`，六个视图各自渲染根节点。`.view`（`styles/app.css:179-185`）负责 `padding: var(--sp-6)`（24px）、`gap: var(--sp-4)`（16px）、纵向滚动；宽度约束在上层，`.view` 本身不设 `max-width`。`src-react/src/views/KbView.tsx:464` 用的是 `.kb-view`，`views/kb.css:3-9` 给它 `padding: 1rem`、`gap: .75rem` —— 知识库因此比其它页更贴边、行距更紧。

**知识库详情是第二套皮肤。** `views/kb.css:214-238` 给 `.kb-detail` 定义了 8 个硬编码 hex：`--paper #FAF6EE`、`--paper-deep #F2EBDC`、`--ink-1 #4A3A36`、`--ink-2 #7A655F`、`--ink-3 #A59089`、`--seal #C0574F`、`--seal-soft #F9EDEA`、`--line-warm #EADFD2`，外加 SVG `feTurbulence` 纸纹与 inset 阴影。注释写明这是「朱砂宣纸面（qa board 选型 D）」。`rendering/styles/prosemirror.css:221/227/254` 等处还把这个命名空间当回落值用（`var(--ink-3, #A59089)`）。

**材料只有一个屋子在用。** 描图纸令牌只在 capture 窗生效；主界面没有任何材料表达，hover 一律是浅底块。

**可达性基线已经存在，别改坏。** `styles/app.css` 有 4 处 `prefers-reduced-motion`（`:157`、`:240`、`:830` 附近）、4 处 `:focus-visible`（`:14`、`:18`、`:75`、`:810`）、`tabular-nums`（`:790`，`views/kb.css:542` 另有一处）。语义色用琥珀不用红（`--pri-med: 32 85% 34%`，注释标明 5.2:1）。

**约束：** 离线单机、不引图标库；字体只允许一个**报头衬线子集**（12 KB，见 D8），不引第二族；Windows 优先、1366 宽可用；命令契约与 Rust 侧零改动；`test/docs.test.mjs` 锁死 README 8 章与 AGENTS 4 章的结构。

**未验证项：** 沙箱内拉不起 Tauri GUI，本设计全部基于代码，没有真机截图（详见 `ledger/worklog/202611/14-1042-26b5-优化前端设计.md`）。

## Goals / Non-Goals

**Goals:**

- 一眼能认出是 Kettd：不是「暖白 + 琥珀」的通用脸，而是「一本账本 + 一张会浮起来的纸」。
- 层级只剩四档字号，且报头不再小于内容数字。
- 两种纸、一条规则，说清哪个面用哪种。
- 知识库回到同一套令牌与同一个外壳，视觉差异只来自纸样，不来自第二套色板。

**Non-Goals:**

- 不改信息架构、导航、快捷键、拖拽与键盘流程。
- 不重做配色（色相保留，只收编取值与来源）。
- 不引图标库、不换图标；字体只引一个报头衬线子集，不动正文黑体。
- 不动便签窗、悬浮窗、捕获条的既有材质——它们继续用同一套令牌，只是被引用。
- 不做响应式重排或 1366 以下适配（当前不在范围内）。

## Decisions

**D1 · 四档 human-picked 阶梯，不用模数比例。** 中文字距与字重补偿比拉丁字母强，1.25 / 1.333 这类比例阶梯在 11–22px 区间会产出 13.4px、16.8px 这类不可交付值，还要连带重算全部行高与 4px 网格。四档是穷举出来的、能被 review 的。**备选：** 模数阶梯（12 / 15 / 18.75 / 23.4）—— 否决，重写成本高于收益。

**D2 · 报头与 KPI 同为 22px，靠字重区分。** 倒挂的实质是「标题比内容数字小」；今天页没有 KPI 卡，回顾页没有任务行，两者不共屏竞争，同档即可消除。**备选：** 报头提到 26px —— 否决，1366 宽下中文大标题压迫感过强，且要重算行高与空态留白。

**D3 · 材料二分，不做全站统一。** 宣纸面是已有选型且有真实收益（长文阅读不刺眼），统一成半透明会牺牲对比度稳定性。规则按**停留时长**划：< 2 秒的浮层用描图纸，> 10 秒的阅读面用纸，两者之间不放第三种。**备选：** 全站描图纸 —— 否决，长文阅读对比度不稳；**备选：** 全站宣纸 —— 否决，捕获条会失去「透光」这一物理意义。

**D4 · 当前行浮起用白页，不用加深底色。** 「一张纸从桌面抽起来」与账本母题同源，且天然只能有一张；加深底色只是通用 SaaS hover。**备选：** 保留 `--accent` 浅底 hover —— 否决，六页一个样、没有来处。

**D5 · 行尾常驻一个「⋯」。** 纯 hover 动作在触屏与纯键盘下不可发现，全常显又会制造四个按钮的噪音。折中为「一个常驻入口 + 次要动作在 hover/focus 展开」，且用固定列位避免展开时挤动行宽。**备选：** 全常显 —— 否决；**备选：** 维持现状 —— 否决（见 `views/TodayView.tsx` 行尾动作 `opacity: 0`）。

**D6 · 知识库用令牌映射，不重画。** 8 个 hex 收编成 2 个正式令牌（`--seal` 保留朱砂身份）+ 4 处映射，视觉变化可控、对比度只升不降。**备选：** 把 KB 改成纯白页面 —— 否决，丢掉宣纸的阅读收益与既有选型。

**D7 · 文案只删不加。** 三处 P0 都是「界面解释实现」，删掉即成立；唯一新增是空态里的快捷键提示，因为快捷键需要有一个学得到的地方。**备选：** 把「三处同源」改写成用户话术 —— 否决，用户不需要知道，删比换好。

**D8 · 只让报头上衬线，其余保持系统黑体。** 全站字族是 `-apple-system, "Segoe UI", "Microsoft YaHei", system-ui`，Windows 上实际落到「Segoe UI + 微软雅黑」，即系统默认脸——这正是页面「没有时代与性格」的底层原因，但整站换黑体（思源黑体）体积 6–9 MB、屏显收益近零。折中：**只裁一个报头衬线子集**（Noto Serif SC，OFL 1.1，61 字 / 12.3 KB），用在品牌字与 `.view-title` 两处；正文、行、按钮全部维持系统黑体。这样同一屏只有一处不是默认脸，且这处正好是「书页」隐喻的落点。**备选：** 全站思源黑体 —— 否决，体积大、与雅黑差异小；**备选：** 报头也保持黑体 —— 否决，D2 把报头提到 22px 后，黑体 22/600 与 KPI 22/700 只剩字重差别，层级仍不成立。

## 设计规格

### 4.1 字号令牌（`index.css` 新增）

| 令牌 | 值 | 角色 | 备注 |
| --- | --- | --- | --- |
| `--fs-meta` | 11px | 时间戳、标签、键盘提示、行内 meta | 与 `tabular-nums` 搭配 |
| `--fs-sub` | 12.5px | 副行、说明、按钮、Tab、段落标题 | 取代原 12/13 两档 |
| `--fs-body` | 14px | 行标题、输入框、列表正文 | 正文唯一档 |
| `--fs-masthead` | 22px | 页面报头；KPI 数字同档 | 报头 600，KPI 700 + `tabular-nums` |
| `--fs-reading` | 16px | 仅编辑器正文（行高 1.85） | 禁止用于 chrome |

角色映射（按角色，不按数值就近）：

| 角色 | 现在 | 改后 |
| --- | --- | --- |
| `.view-title` | 18 / 600 | 22 / 600 |
| `.view-sub` | 12 | 12.5 |
| `.section-head` | 13（内层 `b` 14） | 12.5（内层 `b` 14） |
| 行标题 | 13 上下 | 14 |
| `.tmeta` / mono | 11 | 11（= meta） |
| `.btn` | 12 | 12.5 |
| `.input.sm` | 13 | 14 |
| 标签 chip | 10 | 11 |
| `.kpi` | 22 / 700 | 22 / 700（不动） |
| `.celebrate .mark` | 34 | 34（装饰符号，不算文本） |
| 编辑器正文 | 16 | 16（`--fs-reading`） |

知识库的 em 阶梯（`0.72/0.75/0.8/0.82/0.85/0.9/0.95/1.15/1.5em`、`2.5rem`）折算到 11 / 12.5 / 14 三档；`2.5rem` 那个空态大字形保留为装饰符号。

### 4.2 颜色令牌（六色命名，取值不变）

| 名 | 令牌 | 值 | hex | 用途 |
| --- | --- | --- | --- | --- |
| 纸 | `--background` | `40 33% 98%` | `#fcfaf8` | 账本页底 |
| 墨 | `--foreground` | `240 6% 10%` | `#18181b` | 正文 |
| 铅笔灰 | `--muted-foreground` | `240 4% 46%` | `#71717a` | 次级文字 |
| 格线 | `--border` | `40 15% 88%` | `#e5e2dc` | 分隔与行边 |
| 琥珀 | `--pri-med` | `32 85% 34%` | `#a05c0d` | 逾期 / 滞留（温和压力，非红） |
| 朱砂 | `--seal`（新增） | `4 47% 53%` | `#c0574f` | 知识库印章、危险动作 |

新增 `--seal` 是唯一新增的颜色令牌，取值即原 `--seal: #C0574F`，只换成 hsl 三元组以符合「颜色一律 `hsl(var(--xxx))`」。

### 4.3 外壳与行度量

- 一级页面根节点一律 `.view`：`padding: var(--sp-6)`、`gap: var(--sp-4)`；`#/kb` 去掉 `.kb-view` 的 `padding: 1rem` / `gap: .75rem`。
- 行高统一 32px（`padding: 6px 10px` + 20px 内容），浮起只用 `transform`（不参与布局），因此行高与行距不变。
- 行尾列固定 25px，`⋯` 常驻，其余动作在同一列内展开，展开不改变行宽。

### 4.4 当前行纸片（唯一用力处）

```css
.trow { background: transparent; border: 1px solid transparent; }
.trow:hover,
.trow:focus-within {
  background: hsl(var(--card));
  border-color: hsl(var(--border));
  box-shadow: var(--shadow-sm);
  transform: translateY(-1px);
}
@media (prefers-reduced-motion: reduce) {
  .trow:hover, .trow:focus-within { transform: none; }
}
```

规则：任一帧内浮起的行不超过 1；`.trow-aged` 的左侧 3px 琥珀竖条保留在最外层，纸片不画左边，二者不抢同一位置；暗色下纸片取 `--card`（`#1f1f23`），不叠 `--accent`，避免被读成「选中」。

### 4.5 材料语法

| 面 | 纸 | 令牌 | 停留时长 |
| --- | --- | --- | --- |
| 捕获浮条 | 描图纸（半透明、上缘 1px 高光、三段渐变） | `--vellum-*`（`index.css:42-53`） | < 2s |
| 当前行 | 白页（不透明、微阴影） | `--card` + `--shadow-sm` | hover 期间 |
| 知识库详情 / 编辑器 | 宣纸（不透明、纸纹、暖墨） | `--background` + `--seal` + 纹层 | > 10s |

一条规则：**看得越久，纸越实。** 描图纸不实心、宣纸不透明，白页是宣纸的未着色纸样；一个面只用一种纸。

### 4.6 知识库令牌映射（`views/kb.css:216-223`）

| 原 | 值 | 映射到 | 说明 |
| --- | --- | --- | --- |
| `--paper` | `#FAF6EE` | `hsl(var(--background))` `#fcfaf8` | 同为暖纸白，差一档亮度 |
| `--paper-deep` | `#F2EBDC` | `hsl(var(--secondary))` `#f3f1ed` | 次级纸面色 |
| `--ink-1` | `#4A3A36` | `hsl(var(--foreground))` `#18181b` | 对比升，正文更清楚 |
| `--ink-2` | `#7A655F` | `hsl(var(--muted-foreground))` `#71717a` | 5.2:1 → 4.9:1，仍 ≥ 4.5:1 |
| `--ink-3` | `#A59089` | `hsl(var(--muted-foreground) / 0.75)` | 弱化不靠新令牌；`prosemirror.css` 的回落值同步改 |
| `--seal` | `#C0574F` | `hsl(var(--seal))`（新增，同值） | 朱砂身份保留 |
| `--seal-soft` | `#F9EDEA` | `hsl(var(--seal) / 0.12)` | 底纹由主色派生 |
| `--line-warm` | `#EADFD2` | `hsl(var(--border))` | 格线统一 |

纸纹（`feTurbulence`）与 inset 阴影保留，它们不是色值。

### 4.7 文案改写（P0）

| 位置 | 现在 | 改成 |
| --- | --- | --- |
| `views/TodayView.tsx:111-113` | `{today} · 今天计划 N 项 · 剩 N · 做完 N · 三处同源` | `{today} · 计划 N · 剩 N · 做完 N` |
| `views/TodayView.tsx:123` | placeholder `记一条，回车放进今天（或按 Alt+Shift+A）` | placeholder `记一条，回车放进今天`；空态补 `按 Alt+Shift+A 随时呼出` |
| `views/TodayView.tsx:140` | 空态 `今天还没有挑出来的事项。` | `今天还没有挑出来的事项 · 按 Alt+Shift+A 随时呼出` |
| `views/InboxView.tsx:126` | `…· 越久滞留越醒目（琥珀，非红）` | `…· 越久滞留越醒目` |

### 4.8 今天页：改前 / 改后

```
改前                                          改后
今天                       18/600             今天                        22/600
2026-11-14 · 今天计划 6 项 · 剩 3 ·          2026-11-14 · 计划 6 · 剩 3 ·
做完 3 · 三处同源                            做完 3
[ 记一条…（或按 Alt+Shift+A） ] [记下]       [ 记一条，回车放进今天 ] [记下]
今天要做的 3 件                              今天要做的 3 件
 ☐ 写周报    工作  →今天 排期 ⋯              ┌────────────────────────────┐
 ☐ 买电池    生活  →今天 排期 ⋯              │ ☐ 写周报    工作        ⋯ │ ← 唯一一张浮起
 ☐ 交房租    生活  →今天 排期 ⋯              └────────────────────────────┘
                                               ☐ 买电池    生活        ⋯  ← 平贴
ℹ hover 才出动作、且是浅底块                  ℹ 动作常驻一个、纸片只有一张
```

### 4.9 字体族（唯一自绘字体：报头衬线子集）

| 令牌 | 值 | 用在哪 |
| --- | --- | --- |
| `--font-ui` | `-apple-system, "Segoe UI", "Microsoft YaHei", system-ui, sans-serif` | 全站正文、行、按钮、副行、便签、编辑器 chrome |
| `--font-serif` | `"Kettd Serif", "Noto Serif SC", "Songti SC", SimSun, serif` | **只有两处**：`.view-title`、`.shell-side .brand` |
| `--font-mono` | `ui-monospace, "Cascadia Mono", Consolas, "Courier New", monospace` | 时间、行内 meta、`kbd` |
| `--font-reading` | 默认 `var(--font-ui)`；由 `settings.readingFont` 覆写成 `"<族名>", var(--font-ui)` | 编辑器正文（`--paper-font` 的默认值） |

**阅读面可切换（2026-11-14 追加）。** 沙箱内拿不到霞鹜文楷的源 TTF（`npm pack` 60 MB 级包反复超时、无本地代理），预置大体积 CJK 字体既不可验证也不该替用户决定；改为把选择权交给用户，`--font-reading` 由设置驱动，三态：

| `settings.readingFont` | 含义 | 落地 |
| --- | --- | --- |
| `""`（默认） | 跟随正文 | 移除变量，回落 `var(--font-ui)` |
| `system:<键>` | 本机已装字体，键即 CSS 族名（`SimSun` 宋体 / `KaiTi` 楷体 / `FangSong` 仿宋 / `Microsoft YaHei` 微软雅黑） | `"<键>", var(--font-ui)` |
| `file:<文件名>` | 用户上传，落在 `<数据目录>/fonts/` | 取回字节 → `FontFace('Kettd Reading')` → `"Kettd Reading", var(--font-ui)` |

- 只动 `--font-reading`：不碰 `--font-ui` 与 `--font-serif`，报头衬线不受设置影响。
- 后端 4 个命令（`list/save/delete/read_reading_font`，命令总数 64 → 68）：文件名走 `is_safe_font_file_name`（禁路径与控制字符、扩展名白名单），字节 ≤ 16 MB 且校验文件头（`wOFF`/`wOF2`/`OTTO`/`0x00010000`/`true`/`typ1`），先写临时文件再 rename。
- 传输走 base64（`infra::text_codec`，自实现、零依赖，符合依赖冻结线），不经 JSON 数字数组。
- 删掉正在用的字体会先退回 `""`，启动时 `read_reading_font` 失败也静默退回正文——字体坏了不能让正文消失。

- 子集文件 `src-react/src/assets/fonts/kettd-masthead-serif.woff2`：**61 字 / 12.3 KB**，`@font-face` 写在 `src-react/src/styles/fonts.css`，`font-weight: 600`、`font-display: swap`。
- 只声明 600 一档，不声明 400/700——缺档时浏览器会合成加粗，反而破坏宋体的笔画对比。
- 授权与再生成：`src-react/src/assets/fonts/NOTICE.md`，OFL 1.1 全文同目录 `OFL-1.1.txt`。
- 落地取值：`.view-title` 22 / 600 / `letter-spacing: .01em`（原 18 / 600 / `.02em`）；`.shell-side .brand` 17 / 600（原 15 / 700）。
- **禁止扩用**：这个家族的字表只有 61 个字，用到第三处会立刻出现「一半宋体、一半黑体」的混排；缺字虽然会回落，但同一行里两种骨架并置比纯黑体更糟。

## 文档影响（需要修改的文档）

| # | 文件:位置 | 现在写的 | 应改成 | 原因 | 门禁 / 审批 |
| --- | --- | --- | --- | --- | --- |
| 1 | `README.md:74` `## 技术架构` 下 | 无视觉语言小节 | 新增 `### 视觉语言`：四档字号、两种纸、令牌位置 | README 是事实层，新契约要有事实入口 | `test/docs.test.mjs` 只锁已有 8 章，新增小节不影响 |
| 2 | `README.md:119` `### 关键文件索引` | 13 行全是 Rust/TS，没有样式文件 | 加 `src-react/src/index.css`（设计令牌）与 `src-react/src/styles/app.css`（视图骨架与行样式） | 索引缺样式入口，新同学找不到令牌 | 同上 |
| 3 | `docs/ARCHITECTURE.md:23` | 「具体 UI 规格（在 `spark-output/design/` 五份规格里）」 | 指向本 change 的 `design.md`；归档后指向 `openspec/specs/visual-language/spec.md` | `spark-output/` 目录已不存在（`glob spark-output/**/*.md` 无结果），引用失效 | 无自动门禁，人工核对 |
| 4 | `docs/capability/01-任务与四视图.md:37` | 副标题「今天计划 N · 剩 N · 做完 N + 下一条提醒」 | 与改后实现一致；补「当前行浮起为纸片」「空态显示 `Alt+Shift+A`」 | 文档与实现已漂移（实现多一个「三处同源」，文档没记；改后要反向同步） | 行为层文档，随实现更新 |
| 5 | `docs/capability/06-知识库.md` | 无材质 / 令牌段（`grep 宣纸\|令牌\|ink` 无命中） | 补「宣纸面 + 令牌映射，见 design.md §4.6」 | 详情面板的材质与色值没有出处可查 | 同上 |
| 6 | `AGENTS.md` §2.2 前端 | 4 条硬约束 | 加 2 条：字号只许四档；一级页面只许 `.view` | 规则层越早钉住越省返工 | **文档类交付，按 AGENTS §4.6 需人工批准后才提交** |
| 7 | `docs/design/README.md:3` | 已声明「非当前实现规格」 | 加一行：当前视觉语言以本 change 的 `design.md` 为准 | 防止有人拿 v1 归档当规格 | 无 |
| — | `docs/design/**` 其余 9 份、`docs/requirements/**`、`docs/capability/{02,03,04,05,07,08,09,10}.md` | — | 不改 | v1 历史归档，或与本 change 无关 | — |

## Risks / Trade-offs

- **白页行在暗色主题下可能被读成「选中」** → 纸片取 `--card` + `--border`，不叠 `--accent`；选中态用勾选框与行内文字表达，不靠底色。
- **12.5px 在 100% / 125% 缩放下有次像素渲染风险** → 只用于次级文本；行标题与正文保持 14px 整数；真机在 125% 下核对一次。
- **kb.css 的 em 字号依赖父级，折算会连带漂移** → 先把父级字号固定到 14px，再逐块折算，逐屏目视复核，不做一次性全局替换。
- **`--ink-2` 映射后对比度从 5.2:1 降到 4.9:1** → 仍高于 4.5:1 门槛；若真机偏浅，改用 `color-mix` 而不是新开令牌。
- **纸片与 `.trow-aged` 左侧琥珀竖条同现会打架** → 纸片不画左边界，竖条留在最外层，两者分走「边」与「面」。
- **时间轴卡宽仅 128px**（`.ctl-item`）→ 卡内标题降到 12.5、只留日期与计数用 11，避免 14px 下换行过多。
- **视觉无法自动验证** → 本 change 交付时必须写明「未做真机 / 截图验证」，并由用户执行 L4 五条冒烟后再归档。
- **报头衬线的字表只有 61 字** → 报头与品牌字是固定文案，够用；一旦有人用它渲染用户数据（任务标题、KB 正文），会静默回落到黑体，形成半宋半黑的混排。防控：只在 `.view-title` / `.brand` 两处引用，spec 里写死禁止扩用。
- **子集化属于「修改」，OFL 要求保留许可与标注改动** → 许可全文与来源说明已随字体入库（`assets/fonts/NOTICE.md` + `OFL-1.1.txt`），子集族名改为 `Kettd Serif` 以避开上游家族名。
- **字体文件进 git 会让仓库变大 12 KB、构建产物多一个 hash 文件** → 可接受；不用 CDN，保证离线可用与版本可复现。

## Migration Plan

1. 先只加令牌（`--fs-*`、`--font-*`、`--seal`），全库不引用，跑一次 `npm run build` 确认零影响。
1.5 字体（已先行落地，见 §4.9）：入库 `kettd-masthead-serif.woff2` + `fonts.css` + `NOTICE.md`/`OFL-1.1.txt`，`.view-title` 与 `.brand` 换 `--font-serif`；构建后确认 `dist/assets/` 出现字体文件且体积 ≈ 12 KB。
2. 按文件换字号：`styles/app.css` → `views/kb.css` → `rendering/styles/prosemirror.css` → `capture/`、`sticky/`、`todo/`、`views/settings-*.css`；每换完一个文件目视一次再下一个。
3. 落外壳与行语义：`KbView.tsx` 根节点换 `.view`，`kb.css` 删根 padding；`.trow` 平贴 + 纸片，行尾「⋯」常驻。
4. 知识库令牌映射（§4.6），同步 `prosemirror.css` 的 `--ink-*` 回落值。
5. 文案三处（§4.7）。
6. 文档（见上一节，AGENTS.md 待批准）。
7. 门禁：`npm run check`（12 项）、`npm run build`；真机 L4 五条。

**回滚：** 纯 CSS/JSX 改动，按文件 revert 即可；无数据迁移、无命令契约变化、无依赖变化，回滚后 `data.json` 与旧版本完全兼容。

## Open Questions

1. ~~**知识库阅读面用哪一族字？**~~ **已决（2026-11-14 20:15，用户定）**：不预置霞鹜文楷，也不替用户选思源宋体，改为「跟随正文 / 四种系统字族 / 自己上传」三态可切换（见 §4.9）。原候选保留作背景：**霞鹜文楷（OFL 1.1）** 楷体文人感最贴宣纸，但源 TTF 8–13 MB 且沙箱内拉不到；**思源宋体（本机 Noto Serif SC VF，OFL）** 可离线裁 1.5–2.5 MB，与报头同族最克制。两者都留在「用户自己上传」这条出口里，不再作为默认。
2. 暗色下纸片的精确透明度可在真机上微调，不改变 spec 与任务分解。