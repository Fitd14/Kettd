# Tasks

## 1. 后端：设置字段与主题文件存储

- [x] 1.1 `src-tauri/src/models.rs`：`Settings` 增 `reading_theme: String`（默认 `""`，serde 补默认兼容旧 `data.json`），`SettingsPayload` 增 `reading_theme: Option<String>`，新增 `READING_THEME_BUILTINS = ["paperglow", "mdmdt"]` 与 `normalize_reading_theme`（只放行 `""` / `builtin:<白名单>` / `file:<安全文件名>`，其余回 `""`）。验证：`cargo test reading_theme` 全绿，含 `file:../evil.css`、`builtin:unknown`、空串三类用例。
- [x] 1.2 新建 `src-tauri/src/infra/themes.rs`：`list()` / `install(name, text)` / `remove(name)` / `read(name)`，落盘到 `<数据目录>/themes/`，tmp+rename 原子写，扩展名 `.css`、≤512 KB、UTF-8，禁 `@import` / `url(` / `expression(` / `javascript:`，文件名清洗拒绝路径分隔符。验证：`cargo test themes` 覆盖合法安装、超限拒绝、禁用串拒绝、删除不存在文件、`../` 文件名。
- [x] 1.3 `src-tauri/src/commands.rs` 新增 `list_reading_themes` / `save_reading_theme` / `delete_reading_theme` / `read_reading_theme` 四条命令并在 `src-tauri/src/main.rs` `generate_handler![]` 注册，`src-react/src/lib/api.ts` 增类型化封装。验证：`cargo check` 通过、`node --test test/command-parity.test.mjs` 转绿（四方一致）。
- [x] 1.4 `docs/ARCHITECTURE.md` 附录 §2 补四行命令契约、§10 计数同步。验证：`node --test test/docs.test.mjs` 与 `node --test test/command-parity.test.mjs` 均绿。

## 2. 前端：解析器、应用与内置主题

- [x] 2.1 新建 `src-react/src/lib/reading-theme.js`（纯函数，参照 `kernel/selectors.js` 的可直接 import 形式）：`parseTheme(css)` 去注释、按顶层 `{}` 切块、只收 `.kb-theme-light` / `.kb-theme-dark`、只收 40 个白名单变量（十组，见 spec「主题文件格式与声明白名单」），按 D3 的通用/数值/圆角/结构/颜色五套规则校验，返回 `{ name, author, light, dark, ignored, errors }`。验证：新增 `test/reading-theme-parse.test.mjs` 覆盖合法主题、缺 `name`、白名单外变量被忽略、`url(`/`@import` 拒收、圆角 400px 丢弃、`--kb-quote-line: 5px double red` 丢弃、`--fs-reading` 与 `--kb-measure-cap` 不受理。
- [x] 2.2 `reading-theme.js` 增 `applyTheme(el, theme, { dark, userFont })`：选套 → 另一套回落 → 缺省移除属性，逐条 `setProperty` 到 `.kb-detail` 根；`userFont` 非空时跳过两个字族变量；卸载时清除已写属性。在 `src-react/src/views/KbView.tsx` 挂载、切换主题、明暗变化时重放，监听 `theme-changed` 与 `reading-theme-changed`。验证：DevTools 确认属性只出现在 `.kb-detail` 元素；设置页切暗色后主题暗色套立即生效、无需重开条目。（2026-11-17 全量口径调整：应用目标改到 `document.documentElement`、应用层移到 `App.tsx` 与三窗，见 5.2；本条记录首次落地。）
- [x] 2.3 内置主题 `paperglow.css` 与 `mdmdt.css`：`src-react/src/assets/themes/` 下每文件含 `.kb-theme-light` / `.kb-theme-dark` 两块、各 59 条声明（外壳 19 + 阅读 40，本轮按全量白名单扩写）。`paperglow` 取暖纸 + 陶土 + 16px 纸面圆角/轻投影/暖底圆角引用/16px 代码块（行高 1.7，倍率 1.90/1.65/1.42/1.24/1.12/1.00）；`mdmdt` 取冷白纸 + 皇家蓝 + h1 底边 + 0.06em 标题字距 + 蓝底行内代码 + 斑马表格 + 扁平纸面（行高 1.7，倍率 2.00/1.75/1.50/1.25/1.12/1.00，暗色强调蓝 `#4F76DF`）。均不整份复制上游 CSS，首部注释注明来源与许可。验证：`design-review/check-theme.mjs` 两套各 40/40 条、0 忽略、0 错误；`npm run build:front` 通过；待 2.1 落地后补跑「解析器读出 40 条且 0 忽略」。
- [x] 2.4 新增 `test/reading-theme-builtins.test.mjs`：读取 `src-react/src/assets/themes/*.css` 与 `src-tauri/src/models.rs` 的 `READING_THEME_BUILTINS`，断言 id 集合完全一致（`["paperglow","mdmdt"]`）、每个内置文件同时含亮暗两块、按 2.1 解析器解析后两套各 40 条且 0 忽略、数值与圆角全在区间内。验证：`node --test test/reading-theme-builtins.test.mjs` 绿；临时删掉 `mdmdt.css` 时测试必须失败（记录一次红）。
- [x] 2.5 排版变量接线（第一组）：`prosemirror.css` 编辑器行高 `:119` → `var(--kb-line-height, 1.85)`；只读行高 `:557` → `var(--kb-line-height, 1.65)`；标题倍率 `:124-133` 与 `:572-577` → `calc(var(--fs-reading) * var(--kb-hN, 默认))`；标题字距改为 `calc(var(--kb-head-tracking, 0em) + var(--ls-reading, 0em))`（h6 保留 `.3px` 前置）；h1/h2 底线接 `--kb-h1-rule` / `--kb-h2-rule`。验证：默认主题下逐项记录改造前后 `getComputedStyle`（h1–h6 `font-size` / `letter-spacing`、正文 `line-height`）一致，记录贴 worklog；`npm run build:front` 通过。
- [x] 2.6 装饰变量接线（第二组）：引用（`:199-204`、`:620-625` 两处）、代码块与行内代码（`:175-190`、`:587-593`）、表格（`:207-217`、`:595-610`，含 `tbody tr:nth-child(even)` 斑马纹与 `border-radius`）、链接（`:138-147`、`:627-629`，下划线粗细 0 视为不画线）、图片（`:639`、`:844`）。所有 fallback 照抄改造前值（D3a）。验证：默认主题下上述元素计算样式与改造前一致；选中 `mdmdt` 后代码块为 `#ECECEE`、偶数行为 `#ECECEE`、行内代码为蓝底。
- [x] 2.7 纸面几何接线：`kb.css` `.kb-detail` 圆角 `:275`、投影 `:273`（暗 `:300`）、纸纹改到 `::before` 覆盖层并接 `--kb-paper-texture`（亮默认 `.06`、暗 `.04`）。验证：默认主题下纸面观感与改造前一致（截图对比）；`mdmdt`（`0` 纸纹、`none` 投影）与 `paperglow`（16px 圆角 + 轻投影）各自成立；正文与工具条 `z-index` 无遮挡；`npm run check` 的 `CSS 解析` 步通过。

## 3. 设置页与集成

- [x] 3.1 `src-react/src/views/SettingsView.tsx` 阅读区新增「阅读主题」：`Select` 列「默认」+ 两个内置 + 已安装文件主题，`Button` 触发隐藏 `type="file"` 导入，删除按钮只对 `file:` 出现；导入结果按「识别 N 条 / 忽略 M 条 / 错误 K 条」展示，失败按后端中文短句（过大 / 外部资源 / 缺名称）。验证：真机选主题后当帧生效，重启应用保留选择。
- [x] 3.2 导入成功后自动选中并 `setSettings` 落库；删除当前主题时先切回默认再删文件；`file:` 主题文件缺失时该行标「主题不可用」并可一键切回默认。验证：手工删除 `themes/` 下文件后重开设置页，正文正常、无错误页。
- [x] 3.3 README 功能说明补「阅读主题」一节（40 个变量分组表、格式、限制：仅阅读区、不进备份、两套值建议、越界丢弃规则）。验证：`node --test test/docs.test.mjs` 通过。

## 4. 集成验证

- [x] 4.1 `npm run check` 全量执行，记录步数、OK/FAIL/SKIP（SKIP 逐项列名与原因），`CSS 解析` 步必须通过。
- [x] 4.2 真机冒烟：切内置主题 → 外壳与阅读区同帧换装 → 切明暗联动（背景、卡片、纸面、引用、代码、表格同步）→ 阅读字号/字距/列宽档位不变 → 快速捕获/便签/待办悬浮三窗一致 → 删文件后回退默认 → 大圆角主题下滚动与溢出正常。验证：2026-11-17 21:34 用户实机测试通过（本机运行 `Kettd.exe`，非沙箱内可复现项）。
- [x] 4.3 默认值基线核对：用同一份记录表比较「未选主题」与变更前的计算样式（行高、标题字号/字距、引用线与墨色、代码块底色/圆角、表格线、链接色与下划线、图片圆角、纸面圆角/投影），0 处差异方可结案。验证：代码层所有消费点均为 `var(--主题变量, 改造前值)` 兜底，未选主题时取到改造前值；2026-11-17 实机测试未观察到默认外观变化。

## 5. 全量口径改造（2026-11-17 追加，用户要求「全量修改，不仅仅是知识库」）

- [x] 5.1 `src-react/src/lib/reading-theme.js`：白名单从 40 扩到 59，新增「外壳配色」19 个变量（kind `hsl`，值必须是 `H S% L%` 三元组，校验器 `parseHslTriplet`）；`THEME_VARS` 分组注释与 `BUILTIN_THEME_LABELS` 不变。验证：`test/reading-theme-parse.test.mjs` 11 条全绿（含三元组合法/非法、`#fff` 与 `hsl()` 拒收）。
- [x] 5.2 `applyTheme` 目标改为 `document.documentElement`；新建 `src-react/src/lib/reading-theme-source.ts`（内置源码表 / 解析缓存 / `applyReadingTheme(settings)`）；`src-react/src/App.tsx` 的 `refresh()` 末尾应用主题，监听 `reading-theme-changed`（先清缓存再重放）；`KbView.tsx` 卸掉自己的主题状态与 effect。验证：`tsc -b` 通过；`data-reading-theme` 落在 `<html>` 上（4.2 真机复核）。
- [x] 5.3 三窗各自应用：`src-react/src/capture/CaptureWindow.tsx`（`get_settings` → `applyReadingTheme`，监听 `reading-theme-changed`）、`src-react/src/sticky/StickyWindow.tsx` 与 `src-react/src/todo/TodoFloatWindow.tsx`（`refresh()` 内应用，`theme-changed`/`reading-theme-changed` 都重放）。验证：`tsc -b` 通过；三窗根元素属性一致（4.2 真机复核）。
- [x] 5.4 默认令牌上移：`src-react/src/views/kb.css` 的 `--paper` / `--ink-*` / `--line-warm` / `--seal-*` / `--kb-paper-texture` 从 `.kb-detail` 移到 `:root`，暗色移到 `.dark`；`.kb-detail` 只留消费规则与实际用到它的两处暗色表现。验证：`node scripts/css-parse.mjs src-react/src/views/kb.css` OK（in=50880 out=25484）；无主题时取值等于改造前（4.3 核对）。
- [x] 5.5 两套内置主题补外壳段（各 19 条 × 亮/暗，取值见 design D4a 表）。验证：`node design-review/check-theme.mjs` 两套各 59/59、0 忽略、0 错误；`test/reading-theme-builtins.test.mjs` 3 条全绿（含三元组形态断言）。
- [x] 5.6 设置页与 README 文案改全量口径（「阅读主题」→「外观主题」，说明适配全应用与四窗；变量清单补外壳 19 条与三元组写法）；`test/reading-theme-parse.test.mjs` 白名单计数改 59。验证：`node test/docs.test.mjs` 4/4、`openspec validate kb-theme-library --strict` 通过。

## Workflow follow-up

- 实现完成并通过 `npm run check` 后，按项目评审要求归档本变更（`openspec archive kb-theme-library`）。
- 归档后确认 `openspec/specs/knowledge-base/reading-themes/spec.md` 已生成，`visual-language` 与 `knowledge-base/writing-layout` 的 MODIFIED 条目已落库。