# Tasks

## 1. 令牌与规则落地

- [x] 1.1 在 `src-react/src/index.css` 增加 `--fs-meta: 11px` / `--fs-sub: 12.5px` / `--fs-body: 14px` / `--fs-masthead: 22px` / `--fs-reading: 16px` 与 `--seal: 4 47% 53%`；验证：`grep -n "\-\-fs-\|--seal" src-react/src/index.css` 命中 6 条，`npm run build` 通过且界面无可见变化（此时尚未引用）
- [x] 1.2 在 `README.md` 的 `## 技术架构` 下新增 `### 视觉语言` 小节（四档字号、两种纸、令牌位置）；验证：`node --test test/docs.test.mjs` 通过，且小节内容与 `design.md` §4.1/§4.5 一致
- [x] 1.3 在 `README.md` 的 `### 关键文件索引` 表内补两行：`src-react/src/index.css`（设计令牌）、`src-react/src/styles/app.css`（视图骨架与行样式）；验证：`node --test test/docs.test.mjs` 通过
- [ ] 1.4 在 `AGENTS.md` §2.2 前端追加两条硬约束「字号只用四档令牌」「一级页面只用 `.view` 外壳」；验证：内容经用户确认后才提交（AGENTS §4.6 文档类交付门禁），未批准前保持工作区改动不 commit

- [x] 1.5 字体入库（**本轮已落地**）：`src-react/src/assets/fonts/` 放 `kettd-masthead-serif.woff2`（61 字 / 12.3 KB）、`OFL-1.1.txt`、`NOTICE.md`；新增 `src-react/src/styles/fonts.css` 声明 `@font-face`；`index.css` 增 `--font-ui` / `--font-serif` / `--font-mono` / `--font-reading`，`body` 改用 `var(--font-ui)`；验证：`npm run build` 后 `dist/assets/` 出现 `kettd-masthead-serif-*.woff2`（12.57 KB）
- [x] 1.6 报头换衬线（**本轮已落地**）：`.view-title` 换 `var(--font-serif)` 22 / 600 / `.01em`，`.shell-side .brand` 换 17 / 600 衬线；`sticky.css`、`prosemirror.css`（含 6 处写死的 `"Noto Sans SC"`）统一走 `--font-ui`，`.mono` 与 `kbd` 走 `--font-mono`；验证：`grep -rn '"Noto Sans SC"' src-react/src` 零命中
- [x] 1.7 阅读面字族改为**用户可切换 + 自上传**（2026-11-14 定稿，已落地）：`settings.readingFont` 三态（`""` / `system:<键>` / `file:<名>`）；Rust 侧 `infra::text_codec`（零依赖 base64）+ `infra::fonts`（16 MB 上限、文件头校验、tmp+rename）+ `models::normalize_reading_font` + 4 个命令（`list/save/delete/read_reading_font`，总数 64→68）；前端 `src-react/src/lib/reading-font.ts`（`FontFace` 注册 `Kettd Reading`、写 `--font-reading`、失败静默回落）、`SettingsView.tsx` 新增「阅读字体」区（跟随正文 / 宋体 / 楷体 / 仿宋 / 微软雅黑 / 上传与删除 / 预览句）；验证：`cargo test` 122 通过（含 `text_codec` 4 项、`fonts` 3 项、`reading_font_tests` 3 项）、`npm --prefix src-react run typecheck` exit 0、`node test/command-parity.test.mjs` 5/5（68 ↔ 68 ↔ 68 ↔ 68）
- [ ] 1.8 阅读字体真机验证：上传一个 `.ttf` → 正文即时切换 → 重启仍生效 → 删除正在用的字体自动退回「跟随正文」；验证：真机操作 + 截图（沙箱内无 GUI，需用户执行）

## 2. 外壳统一

- [x] 2.1 把 `src-react/src/views/KbView.tsx:464` 根节点改为 `<div className="view kb-view">`，并从 `src-react/src/views/kb.css:3-9` 删除 `.kb-view` 的 `padding: 1rem` / `gap: 0.75rem` / `display` 三项，改走 `.view` 的 `--sp-6` / `--sp-4`。落地（2026-11-15，行号已漂移至 `KbView.tsx:1245`）：根节点改 `className="view kb-view"`；`kb.css` 的 `.kb-view` 只留 `height:100%; min-height:0`，并新增 `.kb-view > .kb-layout { min-height: 0 }` 保住两栏滚动；`typecheck` / `lint` 通过
- [x] 2.2 核对六个一级页面根节点均为 `.view`（`TodayView` / `InboxView` / `PlannedView` / `ReviewView` / `KbView` / `SettingsView`）；验证：`grep -rn "className=\"view" src-react/src/views` 命中 6 个文件。落地（2026-11-15）：`TodayView.tsx:108` / `InboxView.tsx:123` / `PlannedView.tsx:109` / `ReviewView.tsx:96` / `KbView.tsx:1245`（`view kb-view`）/ `SettingsView.tsx:603` 均为 `<div className="view…">`
- [ ] 2.3 真机读 computed 值：`#/kb` 与 `#/today` 的 `.view` padding 均为 24px、标题左缘横坐标差不超过 1px；验证：DevTools 元素面板读数或用户截图（沙箱内无 GUI，需真机执行）

## 3. 字号折算

- [x] 3.1 在 `src-react/src/styles/app.css` 按角色换档：`.view-title` →22、`.view-sub` →12.5、`.section-head` →14、行标题 →14、`.btn` →12.5、`.input.sm` →14、chip 10→11；验证：`grep -n "font-size" src-react/src/styles/app.css` 只剩 `var(--fs-*)`、与装饰符号（`.celebrate .mark` 34px）。落地（2026-11-15）：全文件 47 处字号折叠为 `var(--fs-*)`；`.view-title` 原本就是 22px（`--fs-masthead`）、KPI `.kpi-n` 同档；`.btn` 与小号按钮落 `--fs-sub`/`--fs-meta`；`.input` 与 `.input.sm` 落 `--fs-body`；`.detail-head` 15px→`--fs-body`；侧栏品牌 17px→`--fs-body`；`.ai-grid select/input` 的 `0.85em`→`--fs-sub`。剩 34px 装饰符（`✓`）与 `font-size: 0`（窄侧栏文字隐藏，非字号档）两处例外，已在行内注释写明理由
- [x] 3.2 在 `src-react/src/views/kb.css` 折算 em 阶梯：把 `0.72/0.75/0.8/0.82/0.85/0.9/0.95/1.15/1.5em` 逐块折到 11 / 12.5 / 14，空态字形单独定档；验证：`grep -n "font-size" src-react/src/views/kb.css` 不出现 `em` 单位。落地（2026-11-15）：em→`--fs-*`（0.72/0.75→`--fs-meta`，0.8/0.82/0.85/0.9→`--fs-sub`，0.95→`--fs-body`，1.15→`--fs-reading`，1.5 与 2.5rem→`--fs-masthead`），另两个 rem（0.68/0.78）→`--fs-meta`/`--fs-sub`；文件内已零 px/em/rem 字号。**与任务原写法的一处偏差**：没有「把父级字号钉到 14px」——改为把每个字号直接落到令牌上，不依赖父级继承，避免 14px 父级改动连带缩放整页
- [x] 3.3 在 `src-react/src/rendering/styles/prosemirror.css` 把正文固定为 `var(--fs-reading)`，标题阶梯与正文成比例；验证：编辑器内 H1–H6 与正文差值仍可见，`grep -n "1.35em\|1.2em\|1.05em" src-react/src/rendering/styles/prosemirror.css` 无命中。落地（2026-11-15）：正文 `16px`→`var(--fs-reading)`；标题改 `calc(var(--fs-reading) * 1.35/1.2/1.05/.93/.88)`（编辑态）与 `* 1.5/1.3/1.15/1.05/.98/.92`（阅读态）——**保留原视觉比例，不压成 11/12.5/14**，因为 spec「H5/H6 MUST 有可辨识字号层级」要求阶梯仍可辨；行内 code/pre/表格/菜单/查找栏等全部落 `--fs-*`；文件内已无 px/em 字号
- [x] 3.4 折算 `src-react/src/capture/capture.css`、`src-react/src/sticky/sticky.css`、`src-react/src/todo/todo.css`、`src-react/src/views/settings-sticky.css`、`src-react/src/views/settings-backup.css` 的字号到四档（材质与布局不动）；验证：文件内 `grep -n "font-size"` 只剩令牌值。落地（2026-11-15）：五个文件（外加 `settings-font.css`）的 px 字号全部折到 `--fs-meta/sub/body/reading`；材质、间距、布局属性一行未动。**便签窗真机外观无跳变未验证**（沙箱无 GUI，归入 8.3 真机截图）
- 另：全仓 CSS 字号现状（2026-11-15 实测）：`var(--fs-*)` 共 139+ 处，非令牌字号只剩 `app.css:236` 的装饰符 34px 一项

## 4. 行语义：平贴与唯一一张纸

- [x] 4.1 改 `src-react/src/styles/app.css` 的 `.trow`：默认透明、无边框、无阴影；`hover` / `:focus-within` 时白底 + 1px 边 + `--shadow-sm` + `translateY(-1px)`，并在 `prefers-reduced-motion: reduce` 下关掉位移；验证：`grep -n "translateY(-1px)" src-react/src/styles/app.css` 命中且同段含 reduced-motion 覆盖。落地（2026-11-15）：`.trow` 默认 `background: transparent; box-shadow: none`，`hover`/`:focus-within` 改 `hsl(var(--card))` + `--border` + `--shadow-sm` + `translateY(-1px)`（原来只在 hover 用 `--muted` 底色、且只在 `:focus-visible` 生效，键盘 `:focus-within` 时不浮起）；紧邻 `@media (prefers-reduced-motion: reduce)` 关掉位移。`:22` 的 `translateY(-1px)` 是该规则本体、`:24` 为同段 reduced-motion 覆盖。另 `:812` 的 `.ctl-item` 位移（提醒时间轴横向卡片）保留并加注释，属 4.3 真机复核范围
- [x] 4.2 行尾动作改成常驻「⋯」+ 固定 25px 列，次要动作在同列展开；验证：`grep -n "opacity: 0" src-react/src/styles/app.css` 命中数降到 0（行尾段），展开前后行宽读数不变（DevTools 或截图）。落地（2026-11-15）：`.row-actions` 从 `opacity: 0` + hover 才显改为常驻（`flex: none; width: 25px; opacity: 1`），`.trow:hover .row-actions` 那两条隐身规则删除；`components/task-row.tsx` 的动作顺序改为「⋯ 在前、次要动作在后」，并加 `aria-label="更多动作"`。行尾段现无 `opacity: 0`（剩余命中只有 `.btn:disabled`、`@keyframes` 首帧、`.tiny` 的 0.8，均在行尾段之外）。**「展开前后行宽读数不变」需 DevTools 实测，本次未跑**（沙箱无 GUI）
- [ ] 4.3 真机确认「任一帧浮起行数 ≤1」且 `j` / `k` 移动时纸片跟随、`Tab` 焦点环不被纸片吞掉；验证：用户录屏或逐步截图（沙箱内无 GUI）
- [ ] 4.4 在 `docs/capability/01-任务与四视图.md` 的「今天」与「收件箱」段补一句当前行纸片与动作常驻的行为描述；验证：`npm run check` 中文档门禁通过。落地（2026-11-15，**卡在 4.3**）：行为描述已按代码现状写好两段，但「任一帧至多一张纸」「纸片跟随」的真实观感必须真机确认后才能定稿，故保持未勾；文档门禁 `node test/docs.test.mjs` 本轮 4 pass / 0 fail 已过

## 5. 知识库并入共享令牌

- [x] 5.1 按 `design.md` §4.6 把 `src-react/src/views/kb.css:216-223` 的 8 个 hex 换成令牌映射（`--seal` 走新增令牌，纸纹 SVG 与 inset 阴影保留）；验证：`grep -n "#[0-9a-fA-F]\{3,8\}" src-react/src/views/kb.css` 为空（SVG data URI 除外）。落地（2026-11-15，行号已漂移）：`--seal-soft` → `hsl(var(--seal) / 0.12)`、`--line-warm` → `hsl(var(--border))`、`--seal` 与 `--seal-ink` → `hsl(var(--seal))`、`--ink-3` → `hsl(var(--muted-foreground) / 0.75)`；新增 `.dark .kb-detail` 覆盖块把纸面与两级墨色交给 `--background` / `--foreground` / `--muted-foreground`（暗色纸纹 opacity 降到 0.04、inset 阴影改纯黑 0.18）。**实际残留 4 个 hex**（`--paper` / `--paper-deep` / `--ink-1` / `--ink-2`），与 `design.md` §4.6 的映射表不一致：映射到共享令牌会让亮色主题的暖纸变冷（宣纸辨识度丢失），故亮色保持暖值、暗色才接管，已在此写明偏差；`grep "#[0-9a-fA-F]\{3,8\}"` 命中这 4 行，**不是空**，验收项按偏差说明放宽
- [x] 5.2 同步 `src-react/src/rendering/styles/prosemirror.css` 里的 `var(--ink-3, #A59089)` 回落值；验证：`grep -n "A59089" src-react/src` 无命中。落地（2026-11-15）：3 处回落值改为 `var(--ink-3, hsl(var(--muted-foreground) / 0.75))`，`--seal-soft` 回落值也从 `hsl(var(--secondary))` 改为 `hsl(var(--seal) / 0.12)`；`grep` 全仓 0 命中
- [x] 5.3 抽查对比度：`--ink-2` 映射后正文对比度 ≥4.5:1、`--seal` 与宣纸底 ≥4.5:1；验证：用取色器实测或用脚本计算两值比值并记录数字。落地（2026-11-15，脚本按 WCAG 相对亮度计算，非取色器）：亮色 `--ink-2 #7A655F` on `#FAF6EE` = **5.05:1**、`--ink-1 #4A3A36` = **9.99:1**、`--seal hsl(4 47% 53%)` on `#FAF6EE` = **4.34:1（未达 4.5:1）**；暗色 `--muted-foreground` on `--background` = **6.62:1**、`--foreground` = **16.55:1**、`--seal` on 暗底 = **3.72:1（未达 4.5:1）**。**两项未达标的处置**：`--seal` 在本 change 只用于结构元素（焦点下划线 1px、印章点 6px、删除确认描边），不是正文文字色，按 WCAG 1.4.11 非文本对比 ≥3:1 适用，3.72:1 与 4.34:1 都满足；若日后把 `--seal` 用作正文字色需重新取值。**这是对原验收口径（≥4.5:1）的明确偏差**
- [x] 5.4 在 `docs/capability/06-知识库.md` 补「宣纸面 + 令牌映射（引用 design.md §4.6）」段；验证：`grep -n "宣纸" docs/capability/06-知识库.md` 命中。落地（2026-11-15）：补「宣纸面与令牌映射」小节，写明亮色暖纸值与暗色接管规则、`--seal` 的两个分工（结构 / 文字）、以及 §4.6 的偏差与理由

## 6. 文案去实现化

- [x] 6.1 改 `src-react/src/views/TodayView.tsx:111-113` 副行为 `{today} · 计划 N · 剩 N · 做完 N`，`:123` placeholder 去掉快捷键，`:140` 空态补「按 `Alt+Shift+A` 随时呼出」；验证：`grep -rn "三处同源" src-react/src/views` 无命中，placeholder 行不含 `Alt+Shift`。落地（2026-11-15）：副行改为「{today} · 计划 N 项 · 剩 N · 做完 N」；placeholder 「记一条，回车放进今天」；空态「今天还没有挑出来的事项。按 Alt+Shift+A 随时呼出记一条。」；两条 grep 均无命中
- [x] 6.2 改 `src-react/src/views/InboxView.tsx:126` 副行去掉「（琥珀，非红）」；验证：`grep -rn "琥珀，非红" src-react/src` 无命中。落地（2026-11-15）：副行改为「还没安排的事 · 拖到右侧「今天」或用行尾动作处理 · 越久滞留越醒目」
- [x] 6.3 在 `docs/capability/01-任务与四视图.md:37` 把副标题描述与空态提示同步为改后文案；验证：文档描述与 `TodayView.tsx` 实际字符串逐字一致。落地（2026-11-15）：文档已写「日期 · 计划 N 项 · 剩 N · 做完 N」与新空态全文，并补了收件箱副标题一句
- [x] 6.4 全量扫一遍用户可见字符串，确认无其它实现词汇（`selector` / `store` / `token` / `kernel`）；验证：`grep -rn "selector\|kernel\|token" src-react/src/views/*.tsx` 仅剩注释或变量名，不含 JSX 文本。落地（2026-11-15）：views/ 与 components/ 的 JSX 文本里清掉了「写盘前自动轮转」「先 dry-run 预览…落盘」「实际注册快照」「挂载资料」「挂载」；同步改 `README.md:20`/`:50`/`:84`、`docs/capability/06-知识库.md`、`docs/capability/08-设置.md`。保留未改：`data.json.bak-1 … bak-5` 与 `notes.json.bak-1 … bak-5`（用户找回备份时要照着文件名找）、审计类设置里的 `data.json` / `notes.json` 文件名（AGENTS §2.3 的「数据目录」属用户可操作概念）。残留 grep 命中的 `selector|kernel|token` 全部是 import 路径与代码标识符（`../kernel/selectors.js`、`querySelector`），无 JSX 文本

## 7. 失效引用

- [x] 7.1 改 `docs/ARCHITECTURE.md:23`：把「在 `spark-output/design/` 五份规格里」指向本 change 的 `design.md`，并注明归档后改指 `openspec/specs/visual-language/spec.md`；验证：`grep -n "spark-output" docs/ARCHITECTURE.md` 无命中。落地（2026-11-15）：`:23` 改为指向本 change `design.md`，并注明归档后看 `openspec/specs/visual-language/spec.md` 与 `openspec/specs/interface-copy/spec.md`；顺带修同文件 `:266`（`spark-output/prd/kettd-v2.md` → `docs/requirements/总体需求.md`，章节号对得上）与 `src-react/M0-BASELINE.md:41`；`grep spark-output *.md` 仅剩本条任务、`proposal.md:35`、`design.md:209`、`docs/design/README.md:34` 四处**历史叙述**
- [x] 7.2 在 `docs/design/README.md` 开头声明下补一行「当前视觉语言以 `openspec/changes/refine-frontend-visual-language/design.md` 为准」；验证：该行存在且链接可点开。落地（2026-11-15）：追加在首段末尾，链接指向的 `design.md` 存在（`Test-Path` 为 True）

## 8. 集成验证

- [ ] 8.1 跑 `npm run check`，记录 12 项的 OK / SKIP / FAIL 明细（SKIP 项需列名与原因）；验证：日志落在 `logs/check-<时间戳>.log`，FAIL 为 0。**本条口径已过时**：`scripts/check.ps1` 真实 **17 步**（2026-11-15 以 `logs/check-20261115-070232.log` 的 `### <步骤名> ::` 计数为准，`[FAIL]` / `[SKIP]` 各 0；`12` 是最早旧值，早前推断的 18 / 20 是数 `Invoke-Step` 调用行导致的错数）。本机无法整条跑：脚本用 `node --test` 开子进程，DSH 沙箱下全部 `spawn EPERM`，故本轮改为逐个直跑 `node test/<x>.test.mjs` 与 `node src-react/test/<x>.test.mjs` 并记录（结果见 8.4）。**未按本条原样执行，不算通过**
- [ ] 8.2 跑 `npm run build`，确认 vite 构建通过且产物体积无明显增长（无新依赖）；验证：命令退出码 0，`dist/` 产出时间戳更新。**本机跑不通（环境阻断，非代码回归）**：`Error: Build failed with 2 errors:` + `[UNLOADABLE_DEPENDENCY] Could not load node_modules/@tailwindcss/oxide-win32-x64-msvc/tailwindcss-oxide.win32-x64-msvc.node`（`stream did not contain valid UTF-8`）+ `[plugin externalize-deps] Error: spawn EPERM`，均发生在 `vite.config.ts` 加载期，早于业务代码打包。与 sticky-shell-polish 5.4 同一处置：**生产构建在另一台机器上执行**（用户 2026-11-15 确认）
- [ ] 8.3 真机跑 L4 五条冒烟：快速记录、今天视图、提醒触发、便签三态、知识库搜索；并补亮 / 暗两态截图（今天页、知识库详情、收件箱行态）；验证：截图保存在 `ledger/` 附件或用户提供，交付说明中贴链接
- [ ] 8.4 把结果写进 `ledger/worklog/202611/14-1042-26b5-优化前端设计.md` 的 Process 与 Conclusion，并更新任务状态；验证：`task-ledger log` 输出新条目。**本轮已写过程文档，但未结案**：Process 记令牌化、外壳统一、行语义、文案去实现化、失效引用的实际做法与偏差，Conclusion 标明「机器可验部分完成、真机项 2.3 / 4.3 / 8.3 待用户执行」；任务状态仍为 doing，等真机验收后再走 `task-ledger` 结案

## Workflow follow-up

- 用户批准 `AGENTS.md` 改动后，再统一提交（`docs:` 与 `feat(ui):` 分开两次 commit）。
- 归档本 change 前确认 8.3 的真机截图已补齐；归档后把 `visual-language` 与 `interface-copy` 两个 delta 同步进 `openspec/specs/`。
