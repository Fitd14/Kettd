# 验证记录：radix-component-layer

> 本文件只记「跑了什么命令、得到什么结果、还差什么」。任务勾选与实现细节在 [tasks.md](./tasks.md)，
> 决策与例外在 [design.md](./design.md)。

## 1. 静态与单测（可复现）

复现命令均在仓库根执行（`tsc` 在 `src-react/` 下执行）。

| 层 | 命令 | 结果 | 说明 |
| --- | --- | --- | --- |
| 类型 | `node node_modules/typescript/bin/tsc -b`（cwd=`src-react`） | exit 0 | 2026-11-17 最后一次改动后复跑 |
| Lint | `src-react\node_modules\.bin\oxlint.cmd .` | 31 warnings / 0 errors | 全部为既有告警（react-compiler、exhaustive-deps），本轮未新增 |
| CSS | `node scripts/css-parse.mjs` | 13 张样式表，0 张解析失败 | 门禁同一脚本 |
| 规格 | `openspec validate radix-component-layer --strict` | valid | 9 条 Requirement |
| 单测 | 8 个 `test/*.test.mjs` 逐个 `node` 执行 | 44 tests / 44 pass / 0 fail | 见下表 |
| 构建 | `npx vite build` | ✓ built，4 入口 + 16 assets | 需沙箱绕行，见 §3 |

单测明细：

| 文件 | 结果 |
| --- | --- |
| `command-parity.test.mjs` | 6 / 6 |
| `docs.test.mjs` | 4 / 4 |
| `heading-select-wiring.test.mjs` | 6 / 6 |
| `main-window-close.test.mjs` | 5 / 5 |
| `packaging.test.mjs` | 9 / 9 |
| `sticky-kbref-seam.test.mjs` | 4 / 4 |
| `tray-entry-parity.test.mjs` | 5 / 5 |
| `tray-menu.test.mjs` | 5 / 5 |

## 2. 静态审计（对应 5.1）

| 项 | 命令 | 结果 |
| --- | --- | --- |
| 原生按钮 | 正则扫 `src-react/src/**/*.tsx` 的 `<button` | 22，全在 D8 / D12 白名单 |
| 原生输入 | 同上扫 `<input` | 4：`ui/input.tsx` 自身 1 + `SettingsView` 白名单 3 |
| select / textarea | 同上 | 0 / 0 |
| 手写弹层语义 | `role="dialog"` / `menu` / `tab` / `listbox` | dialog 3（D14 抽屉 + 注释 + 选择器字符串）、menu/tab 0、listbox 3（D8 + 选择器字符串） |
| radix 依赖纪律 | 扫 `@radix-ui/react-` | 0 命中，只用统一包 `radix-ui` |
| 硬编码色值 | 扫 `components/ui/*.tsx` 与 `styles/primitives.css` | 0 命中 |

## 3. 沙箱限制与绕行（不是代码问题）

- `npm run check`：第一条 `command-parity` 即 `spawn EPERM`——`node --test` 需要管道 spawn，沙箱禁止。替代：逐个 `node test/x.test.mjs`。
- `npm run build` / `vite build`：vite 的 `optimizeSafeRealPathSync()` 在 Windows 上 `exec("net use")` 探测网络盘，同步抛 `spawn EPERM`。二进制本身完好；临时给该调用包 try/catch 可构建成功，构建后从 `.bak` 还原（SHA-256 核对 `F64038F0…4D80B0`）。
- 截图 / 真机：headless Edge 在沙箱内报 `platform_channel.cc:187 Check failed: 拒绝访问 (0x5)`，无法产出 UI 截图。
- 结论：L4 冒烟必须在正常桌面会话里做，见 §4。

## 4. L4 冒烟清单（待真机，5.3）

按 AGENTS §4.2 的五条，逐条记录「实际表现 / 是否通过」。请在真机上把结果贴回本文件或 issues。

| # | 路径 | 关注点（本轮改动影响） |
| --- | --- | --- |
| 1 | 快速记录：热键呼出 → 输入 → 回车入库 | 捕获窗首次拿到 `.btn` 底座；错误态走 `aria-invalid`；输入框 `size` 档位 |
| 2 | 今天视图：列表与侧栏计数一致 | `TodayView` / `task-row` 按钮换原语后布局未挤位；复选框三态 |
| 3 | 提醒触发：到点弹通知 | `PlannedView` 分段钮 `aria-pressed` 与 21→26px 尺寸上移 |
| 4 | 便签三态：展开 ⇄ 缩小 ⇄ 关闭 | `StickyWindow` / `TodoFloatWindow` 首次加载 `primitives.css`，确认 `sticky-btn` 盒子未被 `.btn` 改掉 |
| 5 | 知识库搜索：键入即出结果 | `kb-search-bar` 输入框、`kb-tree` 行容器未套按钮、右键菜单子菜单悬停 |

本轮迁移引入的**已知视觉风险点**（真机时重点看）：

1. 分段钮 21 → 26px：设置页两处三连段选、`typo-popover` 列宽段选是否换行。
2. 便签/待办窗：`.btn` 底座与 `sticky.css` 局部皮肤的叠放；`sticky-btn` 保留类名期间 hover 由 primitives 覆盖（颜色值相同，风险低）。
3. 弹层：`task-detail` 顶部对齐、`ImportPreviewDialog` 84vh 与 `.dialog-content` `100vh-32px` 的 max-height 竞争、Esc 与焦点归还。
4. 链接编辑条：右侧空间不足时的翻转、3 秒宽限后点外关闭、选区是否保持。
5. 编辑器浮条：深色气泡 24px 高、`aria-pressed` 底色、表格浮条五枚的间距。

## 5. 官方脚本复跑（待正常环境）

- `npm run check`：预期 17 项（沙箱内无法跑）。
- `npm run build`：预期不靠 §3 的绕行即可通过。