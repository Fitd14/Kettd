# Tasks

> 说明：吸顶条、portal 工具栏、页边大纲、复制、排版弹层在 2026-11-16 已先落盘（`kb-item-detail.tsx` / `tiptap-editor.tsx` / `views/kb.css`），但当时没有对应规格；下面按新规格逐条复验，未真机确认前一律不勾。
>
> **实施状态（2026-11-16 21:40）**：代码全部到位，含本轮新增的窄窗抽屉与 `--font-reading-title`。
> 已用静态手段验过的条目标了 `[x]`（8 条）；其余条目的验证子句要求真机操作，本机无 GUI 自动化，
> 统一留待 5.4 真机跑完后勾选。

## 1. 令牌与标题衬线

- [x] 1.1 `src-react/src/index.css` 的 `:root` 新增 `--font-reading-title: "Noto Serif SC", "Songti SC", SimSun, serif;`，注释写明「不含 Kettd Serif，避免子集与回落混排」。
  验证：`node src-react/test/md-fidelity.test.mjs` 不受影响；`grep -n "font-reading-title" src-react/src/index.css` 有且仅有一处定义。
- [x] 1.2 `src-react/src/views/kb.css` 的 `.kb-detail-title` 与 `.kb-title-input` 改用 `font-family: var(--font-reading-title)`（当前是 `var(--font-serif)`）。
  验证：`npx tsc -b`；`grep -n "font-family: var(--font-serif)" src-react/src/views/kb.css` 在 `.kb-detail-title`/`.kb-title-input` 两处不再命中。
- [x] 1.3 全表检查：`src-react/src/styles/fonts.css` 与 `index.css` 之外，样式表不得出现裸字族名（`"Noto Serif SC"` 之类）。
  验证：`grep -rn "\"Noto Serif SC\"\|'Noto Serif SC'" src-react/src --include=*.css` 只命中 `:root` 令牌定义。

## 2. 结构对齐规格

- [x] 2.1 `.kb-detail-strip` 的负外边距仍为 `calc(var(--kb-gutter) * -1)`，左右各一份；吸顶偏移为 `top: 0`。（已被 kb-workspace-layout 取代：工具条改 grid 两行，不再用负外边距吸顶）
  验证：真机拖动窗口宽度，工具条左右边缘始终与纸面边缘重合。
- [x] 2.2 `.kb-detail-main` 承担限宽（`width: min(100%, var(--kb-measure-cap))` + `margin-inline: auto`），`.kb-detail-inner` 只做布局。
  验证：`grep -n "kb-measure-cap" src-react/src/views/kb.css` 只在 `.kb-detail-main` 与列宽两档覆盖处命中。
- [x] 2.3 大纲显示时标题仍居中对齐，页边栏不覆盖正文。（已被 kb-workspace-layout 取代：大纲改恒占 188px 轨道）
  验证：真机 1440px 窗口开大纲，量标题左缘与正文左缘是否同一条线。

## 3. 大纲

- [x] 3.1 页边栏仅在 `paneWidth >= 1120` 时以第三列出现；否则整列不占位（`with-outline` 类移除）。（已被 kb-workspace-layout 取代：判据改为 innerWidth − 188 ≥ 700）
  验证：真机在 1119px 与 1120px 两侧各看一次，正文宽度只在 1120px 跳一次。
- [x] 3.2 **窄窗抽屉**：`paneWidth < 1120` 且大纲打开时，改为右侧抽屉（`role="dialog"`、`aria-modal="true"`、宽 `min(78vw, 320px)`），遮罩点击与 `Esc` 关闭，关闭后焦点回到大纲按钮。
  验证：真机窄窗点大纲 → 抽屉滑入 → `Esc` 关闭 → 键盘 `Tab` 一次落在大纲按钮上。
- [x] 3.3 滚动时当前标题高亮（`getBoundingClientRect().top <= 容器顶 + 96px` 的最后一条），点击条目跳到对应标题。
  验证：真机滚动到第二、三节，高亮项逐段切换；点条目后标题滚到吸顶带下方。
- [x] 3.4 无标题时显示空态文案「用标题分段后，这里会出现目录。」
  验证：新建空文档 → 打开大纲 → 看到空态，且不出现空白列表。

## 4. 工具条动作

- [x] 4.1 复制 Markdown 成功时按钮换成 `Check` 并保持 1.6s，剪贴板内容 = 落盘形态（不含 `[@`）。
  验证：真机复制后粘贴到记事本，`grep "\[@"` 无命中。
- [x] 4.2 剪贴板 API 不可用时回退隐藏 `textarea` + `execCommand('copy')`。
  验证：真机用 DevTools 覆盖 `navigator.clipboard.writeText` 抛错，复制仍成功。
- [x] 4.3 两条路径都失败时显示错误提示，**不得**显示「已复制」。
  验证：同时让 `writeText` 与 `execCommand` 返回 false，观察按钮不进入勾选态。
- [x] 4.4 排版弹层拖动即时生效（`applyReadingPrefs`），停手 220ms 后写回 `setSettings`。
  验证：真机拖字号滑块，正文立即变化；`git status` 中 `data.json` 的 `readingSize` 与松手值一致。
- [x] 4.5 排版入口不外移：工具条三个动作在大纲隐藏、宽度 < 720px 时仍在条上可见。
  验证：真机缩到 640px，三个按钮仍在 `strip` 内且未被换行裁掉。
- [x] 4.6 `SettingsView` 不再提供字号/字距/列宽控件，只留指向文档工具条的说明。
  验证：`grep -n "readingSize\|readingTracking\|readingMeasure" src-react/src/views/SettingsView.tsx` 无命中。

## 5. 文档与门禁

- [x] 5.1 `docs/capability/06-知识库.md` 补抽屉行为（阈值两侧、Esc/遮罩/焦点）与 `--font-reading-title` 令牌说明。
  验证：`node test/docs.test.mjs` pass。
- [x] 5.2 静态门禁：`npx tsc -b` 与 `npm run lint`（在 `src-react/` 下）。
  验证：`tsc` exit 0；`Found N warnings and 0 errors`。
- [x] 5.3 契约门禁：`node test/command-parity.test.mjs`、`node test/packaging.test.mjs`、`node src-react/test/md-roundtrip.test.mjs`。
  验证：三者 fail 0；round-trip 报 `FAILURES=0`。
- [x] 5.4 真机四条：宽窗并排、窄窗抽屉、复制、排版弹层不被吸顶带裁切。（用户 2026-11-17 真机核验通过）
  验证：每条留一张截图或一句操作记录；弹层被裁切即视为失败。
- [x] 5.5 归档顺序：先归档 `kb-writing-experience`（等 14 条真机），再归档本变更；归档后人工改 `openspec/specs/knowledge-base/writing-layout/spec.md` 的 `Purpose` 句（现写「标题与工具栏和正文对齐」）。（按顺序执行完毕）
  验证：`openspec archive kb-writing-experience --yes` 成功后再执行本变更归档；`openspec validate --all --strict` 通过。
## 6. 二次实测修订（2026-11-16 晚，用户实记八条）

- [x] 6.1 大纲首进不自动弹：`outlinePref: boolean | null`（null=按留白自动），`railFits` 由 `.kb-detail-inner` 与 `.kb-detail-main` 实测宽度差判定（每侧 ≥150px）。
  验证：`npx tsc -b` exit 0；真机见 6.9。
- [x] 6.2 大纲不推正文：`.kb-detail-inner > .kb-outline` 改绝对定位在右侧留白（`width: clamp(0px, calc((100% - var(--kb-measure-cap)) / 2 - 12px), 176px)`），删掉 `with-outline` 三列 grid；左右留白允许不对称。（此条已被 7.1 取代：改为两列 grid、允许吃留白。**7.1 本身又被 `kb-workspace-layout` 取代**：轨道改为常驻 188px、开关只切可见性，正文不再因开合平移。）
  验证：同上。
- [x] 6.3 大纲独立滚动：`.kb-outline`（外层占位）→ `.kb-outline-inner`（`position: sticky; max-height: calc(100vh - 160px); overflow-y: auto; overscroll-behavior: contain`），`doc-outline.tsx` 加包裹层。
  验证：同上。
- [x] 6.4 滚动区与工具条分离：`.kb-detail` 改 `overflow: hidden; padding: 0`，新增 `.kb-detail-scroll`（唯一滚动容器，`padding: 10px var(--kb-gutter) var(--kb-bottom)`）；滚动条只出现在正文区。
  验证：同上。
- [x] 6.5 工具条左端补文件名 + 保存时间：`.kb-strip-head > .kb-strip-title`（衬线、就地改名）+ `.kb-strip-saved`；标题输入框从正文移出，正文只留标签行。
  验证：同上。
- [x] 6.6 字号 12–24 逐像素 13 档（默认 15）、字距收敛为三档（常规 / 宽松 / 较宽 = 0 / 0.03em / 0.06em）：前后端档位表、CSS 档位规则、排版弹层文案、`Settings` 默认值与 normalize 测试同步。
  验证：`cargo test reading` → `8 passed; 0 failed`；`npx tsc -b` exit 0。
- [x] 6.7 复制图标改 `FileCode2`；链接内联条点外收起加 3 秒宽限（`OUTSIDE_GRACE_MS = 3000`）。
  验证：静态门禁通过；行为需真机。
- [x] 6.8 底部留白加大：`--kb-bottom: max(140px, 34vh)`。
  验证：真机见 6.9。
- [x] 6.9 真机复核八条：首进不出大纲、字号逐档、字距三档、大纲不推正文、链接条 3 秒、工具条左侧标题与保存时间、底部留白、滚到底大纲不动。（用户 2026-11-17 真机核验通过）
  验证：逐条口头或截图确认；`npm run build` 在本机仍因 `@tailwindcss/oxide` 的 `.node` UTF-8 / `spawn EPERM` 失败，L2 证据缺失。
## 7. 三次实测修订（2026-11-16 夜，用户实记四条）

- [x] 7.1 大纲改为「从留白里让位」，并限定在文档内（用户裁决 m01787「允许大纲占用正文留白」）：`.kb-detail-inner.with-outline` 改 `grid-template-columns: minmax(0, 1fr) 176px` + `column-gap: 12px`；判定由「两侧各 ≥150px」改为 `innerWidth - 188 >= 700`（常量 `OUTLINE_TRACK = 188`、`MIN_TEXT_WIDTH = 700`）；抽屉层 `position: fixed` → `absolute`（`.kb-detail` 已是 `position: relative` + `overflow: hidden`），抽屉宽 `min(78vw, 320px)` → `min(88%, 320px)`；spec 的「宽窗在页边显示大纲」「窄窗用右侧抽屉」两条重写。**⚠ 已被 `kb-workspace-layout` 取代**：轨道改为常驻 188px、开关只切 `.kb-outline.is-hidden`，不再「让位」。
  验证：`npx tsc -b` exit 0；`npm run lint` → `Found 34 warnings and 0 errors.`；`openspec validate kb-doc-shell --strict` 通过（见 7.1 落地记录）。
- [x] 7.2 顶部拆两行：`.kb-detail-strip` 改 `display: grid; grid-template-areas: 'head actions' / 'tools tools'`（不动 JSX 顺序），第一行文件名 + 保存时间 + 右侧动作（排版 / 复制 / 大纲 / 贴桌面 / 删除），第二行格式工具栏独占；朱线留在第二行下缘；strip 补 `position: relative` 让排版弹层仍能盖住正文。
  验证：`npx tsc -b` exit 0；`npm run lint` → `Found 34 warnings and 0 errors.`；真机截图待 7.4。
- [x] 7.3 底部留白收小：`--kb-bottom` 由 `max(140px, 34vh)`（1050 高窗口 357px）改为 `clamp(64px, 14vh, 160px)`（同窗口约 147px）；`scroll-padding-bottom` 由同一个大值改为固定 `32px`，避免与 `padding-bottom` 叠加。
  验证：静态门禁同上；真机待 7.4。
- [x] 7.4 真机复核四条：顶部两行、底部留白、宽敞档大纲并排、抽屉只覆盖文档区。（用户 2026-11-17 真机核验通过）
  验证：逐条截图或操作记录；`npm run build` 在本机仍因 `@tailwindcss/oxide` 的 `.node` UTF-8 / `spawn EPERM` 失败。
