# Kettd

一款简洁高效的本地待办应用：待办、多便签、定时提醒、知识库。离线优先，数据只在本机，绝不出网。

Windows 桌面应用，基于 Tauri v1（Rust 后端）+ React 19（前端）。当前版本 0.0.2-rc.1。

| 想做什么 | 看哪里 |
| --- | --- |
| 用起来 | [功能一览](#功能一览) · [使用说明](#使用说明) · [操作文档](#操作文档) |
| 改代码 / 构建 / 发版 | [技术架构](#技术架构) · [开发](#开发) · [测试与门禁](#测试与门禁) · [发布打包](#发布打包) |
| 了解开发纪律 | [AGENTS.md](AGENTS.md)（硬约束 / 历史陷阱 / 验证协议） |

## 功能一览

- **待办管理**：今天 / 收件箱 / 计划 / 回顾四视图；分类（工作 / 学习 / 生活）、优先级、子任务、备注；软删回收站（保留 30 天）、撤销删除、手动排序
- **定时提醒**：单次 / 每日 / 工作日 / 每周，支持多时刻；系统通知、稍后提醒
- **快速记录**：全局热键呼出置顶记录条，回车即记
- **多便签**：自由便签与知识便签同池，可见合计 ≤5 张（隐藏不计）；四套纸色与花纹、缩小态悬浮文本条、**关闭 = 隐藏**（数据保留，可在便签页重开）；侧栏「便签」页以卡片墙集中管理（搜索 / 打开 / 两步删除）
- **待办悬浮窗**：今天列表镜像窗，可置顶
- **知识库**：Markdown 即时渲染编辑（WYSIWYG）、本地全文搜索、知识便签贴到桌面、任务关联知识条目、条目一键建待办、当前文档一键导出 Word（`.docx`）与 PDF（`.pdf`）
- **本周汇总导出**：Markdown / CSV
- **本地度量**：仅本机统计，可随时关闭或清空

## 使用说明

### 窗口与全局热键

| 窗口 | 用途 | 默认热键 |
| --- | --- | --- |
| 主窗口 | 今天 / 收件箱 / 计划 / 回顾 / 知识库 / 便签 / 设置 | `Alt+Shift+O` |
| 快速记录 | 置顶记录条，回车记下、Esc 收起、按住 ✎ 拖动 | `Alt+Shift+A` |
| 新建便签 | 创建一张自由便签 | `Alt+Shift+S` |
| 待办悬浮窗 | 呼出 / 隐藏今天列表镜像 | `Alt+Shift+T` |
| 直达知识库 | 唤起主窗口并跳到知识库 | `Alt+Shift+K` |

热键均可在「设置」页改绑或解绑；解绑后仍可从托盘呼出。

### 托盘菜单

打开主界面 · 待办悬浮窗（勾选态）· 新建便签 · 打开数据文件夹 · 退出。

关闭主窗口不退出应用——托盘常驻，提醒照常触发。

### 便签三态

展开（纸片编辑）⇄ 缩小（置顶悬浮文本条，显示正文第一行，点击展开）⇄ 关闭（**隐藏**：数据与窗口位置保留，在侧栏「便签」页可重开）。自由便签为 WYSIWYG 编辑（正文 ≤500 字）；知识便签指向知识库条目、随编辑同步，删除便签不影响条目。侧栏「便签」页集中管理：卡片墙（摘要 / 更新时间 / 可见状态 / 知识来源）、搜索、打开与两步删除。今天列表镜像由待办悬浮窗承担。

### 知识库

左侧搜索，右侧阅读/编辑即时切换；条目可「贴到桌面」成为知识便签（视口指向条目，随编辑同步），也可一键建待办或关联到任务资料区。搜索为纯本地全文扫描，键入即渲染。存储上是「索引 + 正文分文件」：`notes.json` 只存元数据，正文落 `notes/<id>.md`，图片落 `assets/<sha256>.<ext>`；旧格式（正文内联）首次启动自动迁移，原件存 `notes.v1.json`。

### 导出为 Word 与 PDF

知识库详情页工具条上有两个导出按钮：Word（`.docx`）与 PDF（`.pdf`），把当前打开的文档导出到本机。点击后弹系统保存对话框：默认落在系统「下载」文件夹，默认名取条目标题（已清洗 Windows 不允许的字符），可在对话框里换目录或改名；取消不会产生文件。

Word 保真范围：标题 1–3（进 Word 导航窗格）、加粗 / 斜体 / 行内代码 / 超链接、有序与无序列表（保留缩进层级）、引用（左缩进 + 左侧竖线，最多两层）、代码块（等宽字体 + 浅灰底，不保高亮）、表格（单线边框 + 首行表头）、正文图片（内容寻址的 `assets/` 图片等比缩到 15 cm 以内，单图上限 5 MB）。

PDF 走系统 WebView2 的打印接口生成 A4 页面，用的是**中性文档基线**：白纸黑字，只表达结构（标题层级、列表缩进、引用左侧竖线、代码块等宽 + 浅灰底、表格单线边框、图片等比缩放），不套用应用的外观主题（纸色、圆角、斑马纹都不带），也不带页眉页脚与页码；图片同样来自 `assets/`。

已知不保真：任务列表 `- [ ]` 落成字面方括号，不做勾选框；代码块不内嵌语法颜色；WebP 图片在较老版本 Word 里可能显示空白。PDF 打印依赖系统 WebView2 运行时，缺失时按钮会提示「缺少导出 PDF 所需的运行组件」。

导出是只读操作：不改正文、图片、索引或备份；写入失败返回中文短句，且不会留下半截文件。

## 操作文档

### 数据目录

所有数据位于 `%APPDATA%\todo-list`（托盘 → 打开数据文件夹）：

| 文件 | 内容 | 写策略 |
| --- | --- | --- |
| `data.json` | 任务 / 提醒 / 设置 | 写前轮转 5 份备份，原子替换 |
| `data.json.bak-1~5` | 主数据轮转备份（bak-1 最新） | 设置页可恢复 |
| `notes.json` | 知识库**索引**（元数据数组，不含正文） | 原子写；覆盖前轮转 5 份；与主数据物理隔离 |
| `notes/<id>.md` | 知识库**正文**（每条一个 Markdown 文件） | 先写正文、后写索引；单条原子写 |
| `assets/<sha256>.<ext>` | 知识库图片素材（按内容哈希命名去重） | 原子写；删除条目不清理资源 |
| `folders.json` | 知识库文件夹树 | 原子写；不存在视为空表 |
| `notes.json.bak-1~5` | 知识库索引轮转备份（bak-1 最新） | 设置页可回滚 |
| `notes.v1.json` | 旧格式 `notes.json`（正文内联）迁移前存档 | 只写一次，永不覆盖 |
| `runtime.json` | 窗口位置 / 提醒触发态 / 迁移报告 | 高频写，不轮转 |
| `events.jsonl` | 本地度量事件 | 设置页可清空 |
| `backups/` | 手动备份包（`kettd-backup-<时间戳>.kettdbackup`，单文件 JSON） | 导出命令写入，永不自动轮转 |
| `data.v1.json` | v1 旧版迁移留档 | 永不覆盖 |

知识库是「索引 + 正文分文件」：`notes.json` 只存元数据（`id` / `title` / `tags` / `folderId` / `order` / `pinned` / 时间戳 / `contentHash`），正文在 `notes/<id>.md`，图片在 `assets/<sha256>.<ext>`。写入顺序固定为「先正文、后索引」，崩溃最多留下孤儿正文，不会出现索引指向错误正文；索引里的 `contentHash` 是 `sha256(正文)`，加载时重算比对，不一致以正文为准（索引自动修复，不判损坏），正文缺失按空串加载（条目不丢）。旧版正文内联的 `notes.json` 首次加载自动迁移，原件存 `notes.v1.json`。

正文里的图片只存相对引用 `assets/<sha256>.<ext>`（绝对路径一挪就坏）。渲染时前端经 `rendering/asset-url.ts` 用 `Bootstrap.assetsDir` + Tauri `convertFileSrc` 转成可加载地址，打包配置在 `tauri.conf.json` 用 `protocol.asset` + 窄 `assetScope`（`$APPDATA/todo-list/assets/**`）放行。已知限制：若用环境变量 `KETTD_DATA_DIR` 把数据目录指到别处，该 scope 不覆盖自定义目录，图片会显示不出来（数据本身不丢）。

### 备份与恢复

每次写入 `data.json` 前自动轮转备份（最多 5 份）；`notes.json` 索引同样轮转 5 份。设置页只保留「手动备份」（导出 / 导入 `.kettdbackup`），不再列出自动备份列表——自动轮转照旧在后台发生，主数据损坏时启动流程会进恢复页，按页内提示选用 `get_backups` / `restore_backup` 通道恢复；知识库索引可回滚 `notes.json.bak-1~5`。知识库索引 / 文件夹损坏时进可恢复错误态（引导到备份恢复），不降级为假空库，也不传染主数据；回滚旧版本时把 `notes.v1.json` 复制回 `notes.json` 再装旧版即可（详见 [docs/ARCHITECTURE.md §6](docs/ARCHITECTURE.md)）。

**手动导出 / 导入**（`.kettdbackup` 单文件）：

- **导出**：设置页「数据与备份」手动导出，写 `<数据目录>/backups/kettd-backup-<YYYYMMDD-HHmmss>.kettdbackup`。包内含任务 / 提醒 / 设置 / 便签快照 + 知识条目正文 + 文件夹 + 正文引用到的图片资源（缺失或读失败只计跳过数）；导出不轮转、不改本地数据。
- **导入预览**：选择 `.kettdbackup` 后先出预览，展示「新增 / 更新 / 冲突 / 跳过 / 资源新增与复用 / 冲突标题」计数，**预览阶段不动任何文件**；确认后才真正应用。
- **合并规则**：按 id 合并，`updatedAt` 较新的版本覆盖较旧的；本地较新则跳过；同刻但内容不同时保留本地并生成标题带「（导入冲突）」的副本；提醒 / 文件夹冲突降级为跳过；设置与便签不覆盖、本地独有绝不删。同一备份重复导入全部跳过，不会产生二次副作用。

### 外观主题

主题作用于整个应用（主窗、快速记录条、便签、待办悬浮四个窗口一起换装），内置两套并可直接导入 CSS。

- **内置两套**：`Paperglow`（暖纸、陶土强调、16px 圆角纸面与轻投影，行高 1.7）与 `Mdmdt`（冷白纸、皇家蓝强调、h1 底边与 0.06em 标题字距、斑马表格、扁平纸面）。两套都自带亮 / 暗值，随应用明暗自动切换；灵感来源见各主题文件头部注释（Apache-2.0）。
- **导入格式**：一个 UTF-8 `.css` 文件，≤ 512 KB，文件头写 `kettd-theme` 声明（name 必填，author / description 可选），正文只认 `.kb-theme-light` 与 `.kb-theme-dark` 两个块里的变量声明——其余选择器与 at-rule 一律不生效。导入时整份扫描：出现 `@import`、`url(`、`expression(`、`javascript:` 直接拒收。
- **可改 59 个变量、十一组**：外壳配色 19（`--background`、`--foreground`、`--card`、`--popover`、`--primary`、`--secondary`、`--muted`、`--accent`、`--border`、`--input`、`--ring`、`--link`、`--seal` 及各自的 `-foreground`；值是**不带 `hsl()` 的 HSL 三元组**，与 `index.css` 同形），纸面墨色 8（`--paper`、`--ink-1~3`、`--line-warm`、`--seal-track`、`--seal-ink`、`--seal-soft`）、纸面形态 3（`--kb-paper-radius` / `-shadow` / `-texture`）、字族 2、行高 1、标题 9（`--kb-h1~h6`、`--kb-head-tracking`、`--kb-h1-rule`、`--kb-h2-rule`）、引用 4、代码 6、表格 4、链接 2、图片圆角 1。字号档位、正文字距、列宽、语义色（分类 / 优先级 / 破坏性动作）与气泡底不在白名单内。
- **取值有区间**：行高 1.3–2.4、标题倍率 0.8–2.0、纸纹 0–0.12、标题字距 -0.02–0.2em、下划线 0–2px、纸面圆角 0–24px（其余圆角 0–16px / 行内 0–8px）。越界的单条声明会被丢弃并计数——设置页「主题体检」显示「识别 / 忽略 / 错误」。
- **兜底**：读不到文件、没有有效声明或文件被删时回到默认外观，设置值保留并在设置页标「主题不可用」，修好后自动恢复；没选主题时不写任何内联变量，观感与升级前逐像素一致。

### 设置项

主题（亮 / 暗）、五个全局热键、便签纸色 / 花纹 / 移出淡化、知识库阅读字体（下拉：跟随正文 / 宋体·楷体·仿宋·微软雅黑 / 自己上传的字体文件）、外观主题（默认 / 内置两套 / 导入 CSS，作用于四个窗口）、每小时提醒上限、本地度量开关、手动导出与导入备份。

## 技术架构

### 技术栈

- **前端** `src-react/`：Vite 8 · React 19 · TypeScript 6 · Tailwind v4 · shadcn · Tiptap 3；hash 路由，无状态库；`lib/api.ts` 为 75 个 Tauri 命令的类型化桥接
- **后端** `src-tauri/`：Tauri v1 · Rust（ports / infra / app 六边形分层）；JSON 文件持久化，无数据库
- **契约文档** `docs/`：`ARCHITECTURE.md`（架构 + 命令契约附录）、`requirements/总体需求.md`、ADR 0001–0007（历史档案）

### 分层与数据流

```
UI 层（React）
  views/ components/          六视图与共享组件
  capture/ sticky/ todo/      快速记录窗 / 便签窗 / 待办悬浮窗
  rendering/                  Markdown 与 Tiptap 渲染
  kernel/                     时间、选择器、事件总线、捕获语法（唯一实现）
  lib/api.ts                  77 命令类型化 invoke
        │  invoke
        ▼
接口层  commands.rs + main.rs  generate_handler! 注册（薄壳，只做参数校验与转调）
应用层  app/                   tasks / reminders / settings 用例编排
端口层  ports/                 StoreBackend · Clock · Notifier · Hotkeys（trait 定义）
基础设施 infra/                fs_store 原子写与轮转 · tauri_notifier · tauri_hotkeys · env
        │
        ▼
本地 JSON 文件（%APPDATA%\todo-list，KETTD_DATA_DIR 可覆盖）
```

- **单向数据流**：UI 只通过 `api.ts` 调命令，不直接碰文件；跨窗口通信用 `store-changed` 事件，不用全局状态。
- **时间语义**：全链路本地字符串（`YYYY-MM-DDTHH:mm`），后端 `DateTimeLocal` 保证不混入 UTC。
- **持久化**：一切写入走 `StoreBackend`（tmp + rename 原子替换）；`data.json` 写前轮转备份，`notes.json` 与主数据物理隔离；知识库 = `notes.json` 纯索引 + `notes/<id>.md` 正文，写序固定「先正文后索引」，索引 `contentHash = sha256(正文)` 加载时校验、不一致以正文为准。
- **提醒**：`scheduler.rs` 按分钟检查到点提醒；通知调用在锁外，所有时段照常发出。
- **窗口**：`runtime.rs` 管理主窗 / 快速记录 / 便签 / 悬浮窗与托盘、热键；建窗一律 async + helper 线程，避开 WebView2 同步建窗挂起。

### 命令契约

前端与后端的接口是 **77 个 Tauri 命令**，四方必须同步：

1. `src-tauri/src/main.rs` `generate_handler![]` 注册
2. `src-tauri/src/commands.rs` `#[tauri::command]` 定义
3. `src-react/src/lib/api.ts` 类型化封装
4. `docs/ARCHITECTURE.md` 附录 §2 表格 + §10 计数断言

真相源是 `docs/ARCHITECTURE.md` 附录；`test/command-parity.test.mjs` 会当场拦下任何一处漏改。新增命令的写法与约束见 [AGENTS.md §2.4](AGENTS.md)。

### 视觉语言

- **字号四档 + 阅读档位**：chrome 按角色取 `--fs-meta` 11px（时间 / 标签 / kbd）、`--fs-sub` 12.5px（副行 / 说明 / 按钮）、`--fs-body` 14px（行标题 / 输入框 / 正文）、`--fs-masthead` 22px（页面报头，KPI 数字同档）；编辑器正文只用 `--fs-reading`，它由设置项 `readingSize` 在 12–24px 逐像素 13 档间切换（默认 15px，行高 1.85），字距由 `readingTracking` 在常规 0 / 宽松 0.03em / 较宽 0.06em 三档间切换（默认常规），列宽上限由 `readingMeasure` 在 96ch / 112ch 两档间切换（默认 96ch）。只在 `.kb-detail` 子树内生效，便签窗不受影响。
- **颜色只从令牌取**：一律 `hsl(var(--xxx))`；新增 `--seal`（朱砂 `4 47% 53%`）用于知识库印章与危险动作。
- **两种纸，看得越久纸越实**：描图纸（`--vellum-*`，捕获浮条，停留 <2s）→ 白页（`--card` + `--shadow-sm`，当前行）→ 宣纸（`--background` + `--seal` + 纸纹，知识库详情 / 编辑器，停留 >10s）；一个面只用一种纸。
- **唯一用力处**：同一帧只有当前行浮起（`.trow:hover` / `:focus-within`），其余行平贴账本。
- 令牌定义在 `src-react/src/index.css`，视图骨架与行样式在 `src-react/src/styles/app.css`；完整规格见 `openspec/changes/refine-frontend-visual-language/design.md`。

### 关键文件索引

| 文件 | 职责 |
| --- | --- |
| `src-tauri/src/main.rs` | 启动、初始化、命令注册 |
| `src-tauri/src/commands.rs` | 77 个 Tauri 命令（参数校验 + 转调 app） |
| `src-tauri/src/app/` | 用例编排：tasks / reminders / settings |
| `src-tauri/src/ports/` | 端口 trait：StoreBackend / Clock / Notifier / Hotkeys |
| `src-tauri/src/infra/fs_store.rs` | 原子写、备份轮转、数据目录解析 |
| `src-tauri/src/store.rs` | 持久化编排、v1 迁移、损坏通道分流 |
| `src-tauri/src/models.rs` | 领域模型、校验、`DateTimeLocal` |
| `src-tauri/src/scheduler.rs` | 提醒调度 |
| `src-tauri/src/runtime.rs` | 窗口 / 托盘 / 热键 |
| `src-tauri/src/kb.rs` | 知识库搜索收窄（标题 100/50/20 + 祖先文件夹名 8/4/2；不读正文、不匹配标签） |
| `src-tauri/src/telemetry.rs` | 本地度量事件 |
| `src-react/src/lib/api.ts` | 命令桥接（`__TAURI__` 惰性取） |
| `src-react/src/index.css` | 设计令牌：字号 / 颜色 / 材质 / 字体 |
| `src-react/src/styles/app.css` | 视图骨架、行与行尾动作样式 |
| `src-react/src/styles/primitives.css` | 原语皮肤：按钮 / 输入 / 勾选 / 弹层 / 浮层 / 菜单 / 标签页（四个窗口入口共用） |
| `src-react/src/kernel/` | 时间 / 选择器 / 事件总线 / 捕获语法单源 |
| `scripts/` | `version.mjs` 版本同步 · `check.ps1` 门禁 · `css-parse.mjs` 样式表解析 · `release.ps1` 一键发版 |
| `test/` | 门禁：命令四方对账 / 主窗口关闭契约 / 打包配置契约 / 文档结构契约 |

## 开发

### 环境要求

- Windows 10+（WebView2 Runtime 随系统或手动安装）
- Node.js ≥ 22.12（根 `package.json` 与 `src-react/package.json` 的 `engines` 已锁）
- Rust stable ≥ 1.70（`src-tauri/Cargo.toml` 的 `rust-version`，Tauri 1.8 / dirs 6 的底线）

### 本地开发

```bash
npm install                     # 根目录：Tauri CLI
npm --prefix src-react install  # 前端依赖
npm run build:front             # 构建前端（devPath 指向 dist，先构建再跑 dev）
npm run tauri dev               # 启动桌面应用
```

前端改动后需重跑 `npm run build:front`（`beforeDevCommand` 为空，dev 直接读 `src-react/dist`）；也可以直接用 `npm run dev`（= `build:front` + `tauri dev`）。

### 环境变量与存储目录

复制根目录 `.env.example` 为 `.env`，可把存储根从默认 `%APPDATA%\todo-list` 挪走（`.env` 不提交）。典型用途：dev / 手点回归指到临时目录，不碰正式数据。

- 开发态读项目根 `.env`，打包态读 exe 同目录 `.env`；优先级：系统环境变量 > exe 同目录 > 项目根
- `KETTD_DATA_DIR` 设绝对路径时原样作为数据根（不再拼 `todo-list`），留空 = 默认
- **Windows 路径写正斜杠 `F:/kettd-data` 或单引号 `'F:\kettd-data'`**：dotenv 把裸 `\` 当转义符，整行解析会失败；失败时 dev 控制台报错，打包态无控制台则静默沿用默认目录

### 目录结构

```
Kettd/
├── src-react/              # React 前端
│   ├── src/
│   │   ├── App.tsx         # 路由与全局壳
│   │   ├── main.tsx        # 入口
│   │   ├── views/          # 六视图（今天/收件箱/计划/回顾/知识库/设置）
│   │   ├── components/     # 组件：ui/ 原语（radix+token）· <domain>/ 业务域 · 根为跨视图壳
│   │   ├── capture/        # 快速记录窗
│   │   ├── sticky/         # 便签窗
│   │   ├── todo/           # 待办悬浮窗
│   │   ├── rendering/      # Markdown / Tiptap 渲染层
│   │   ├── kernel/         # 共享 JS 内核（时间/选择器/事件总线/捕获语法）
│   │   ├── lib/            # api.ts 类型化桥接
│   │   └── styles/         # primitives.css 原语皮肤（四窗共用）· app.css 壳与页面样式
│   └── test/               # 前端门禁（渲染保真 / round-trip）
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── commands.rs     # 77 个 Tauri 命令
│   │   ├── store.rs        # JSON 持久化 + 备份轮转
│   │   ├── models.rs       # 领域模型 + 校验
│   │   ├── scheduler.rs    # 提醒调度
│   │   ├── runtime.rs      # 窗口 / 托盘 / 热键
│   │   ├── kb.rs           # 知识库搜索收窄
│   │   ├── ports/ infra/ app/  # 六边形分层
│   │   └── telemetry.rs    # 本地度量
│   └── tauri.conf.json     # 打包配置（bundle / wix / 窗口）
├── scripts/                # version.mjs / check.ps1 / release.ps1
├── test/                   # command-parity / main-window-close / packaging / docs
└── docs/                   # requirements / capability / design / adr / ARCHITECTURE（含命令契约）
```

### 测试与门禁

```bash
npm run check                                                     # 一把跑完 21 项
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check.ps1 -SkipRust    # 只查前端
```

门禁顺序：版本一致性 → 命令四方对账 → 主窗口关闭契约 → … → Markdown round-trip → 前端类型检查 → 前端 lint → **CSS 解析** → 前端构建 → `cargo check` → `cargo test`。任一步非零即退出 1；日志在 `logs/check-<时间戳>.log`。依赖缺失导致跳过的项会标 `SKIP` 并单独提示（当前 `md-roundtrip` 需要 `jsdom`，缺失时跳过）——**SKIP 不算通过**。

「CSS 解析」用 `scripts/css-parse.mjs`，走与 `vite:css-post` 相同的 lightningcss 压缩链路：`tsc` 与 `eslint` 都不解析 CSS，样式表语法错过去要到构建阶段才炸（2026-11-16 一次 `Invalid empty selector` 就是这样把整轮发版打回的），这一步把它提前到秒级门禁里。

单独跑某一项：

```bash
cd src-tauri && cargo test && cd ..                    # Rust 单测（210 项）
node --test test/command-parity.test.mjs               # 命令四方对账：注册 ↔ 定义 ↔ api.ts ↔ ARCHITECTURE 附录
node --test test/main-window-close.test.mjs            # 主窗口关闭契约
node --test test/packaging.test.mjs                    # 打包配置契约（8 条）
node --test test/docs.test.mjs                         # 文档结构契约（README / AGENTS 分工）
node --test src-react/test/md-fidelity.test.mjs        # Markdown 渲染保真
node scripts/css-parse.mjs                             # 样式表解析（lightningcss，同构建链路）
cd src-react && npx tsc -b                             # 前端类型检查
```

新增 / 删除命令时，四方（`generate_handler!`、`#[tauri::command]`、`api.ts`、`ARCHITECTURE.md 附录 §2`）必须同步，parity 门禁会当场拦截不一致。

### 发布打包

```bash
npm run check        # 先过门禁（可跳过，见下）
npm run release      # 门禁 → 构建前端 → tauri build → 校验产物 → 归档
```

一条链路做完四件事：

1. **版本单一来源**：`scripts/version.mjs`（`get` / `check` / `set`）维护四处版本——根 `package.json`、`src-react/package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`；不一致时门禁直接拦下。
2. **门禁**：默认先跑 `scripts/check.ps1`（21 项），`-SkipChecks` 可跳过。
3. **构建**：调用 `tauri build --bundles msi,nsis`；bundle 阶段偶发 `os error 32`（Defender 瞬时锁）会自动退避重试，默认 2 次。
   - **预发布版本只出 NSIS**：WiX/MSI 要求版本里的预发布标识是纯数字（`0.0.2-1` 可以，`0.0.2-rc.1` 不行，会报 `pre-release identifier ... must be numeric-only ... for msi target`）。脚本检测到非数字标识时自动剔除 `msi`，只打 NSIS，并在 `build-manifest.json` 的 `droppedTargets` 记下原因。
4. **归档**：产物拷到 `artifacts/v<版本>/`，同目录生成 `SHA256SUMS.txt` 与 `build-manifest.json`（含 git commit / 分支 / 是否脏工作区、node·npm·cargo·rustc 版本、各安装包大小与 sha256）。

常用参数：

```bash
npm run release -- -Version 2.2.1              # 改版本后再打包（自动同步四处）
npm run release -- -Bundles msi                # 只出 MSI（默认 msi+nsis）
npm run release -- -SkipChecks                 # 急用：跳过门禁
npm run release -- -Clean -NoArchive           # cargo clean 后全量构建、不归档
```

原始产物仍在 `src-tauri/target/release/bundle/`（MSI / NSIS）；WiX 已配置 `zh-CN` 码页。`artifacts/` 与 `logs/` 均不进版本库。

### 依赖维护

```bash
npm --prefix src-react outdated     # 前端可升版本
npm outdated                        # 根（Tauri CLI）
cd src-tauri && cargo update --dry-run && cd ..   # Rust 可升版本
```

升级原则（写在 `test/packaging.test.mjs` 里，回退会被门禁拦住）：

- **Tauri 停在 v1 线**（`@tauri-apps/cli` 1.6.x、`tauri` 1.8.x）；迁 v2 是大版本工程，另开任务。
- **TypeScript 停在 6.x**，不追 7.x。
- **`window-vibrancy` 锁 0.4**：0.5 起 `apply_blur` 入参改为 `Color` 结构体，升级要同步改 `src-tauri/src/runtime.rs`。
- **不留无用声明**：`@types/dompurify`（dompurify 3.x 自带类型）、`markdown-it-highlightjs`、`@radix-ui/react-slot` 已移除，禁止回加。
- 升级后必须 `npm run check` 全绿再提交；`src-react/package-lock.json`、`src-tauri/Cargo.lock` 随升级一起提交。

### 常见构建故障

| 现象 | 原因 | 处理 |
| --- | --- | --- |
| `error: failed to bundle project: os error 32` | Defender 正扫 `target/`，文件被占 | 直接重跑；`release.ps1` 已自动退避重试 2 次 |
| `LGHT0311` / 中文界面乱码 | WiX 未指定 `zh-CN` | `tauri.conf.json` 的 `bundle.windows.wix.language` 必须为 `zh-CN` |
| dev 里改了前端不生效 | `beforeDevCommand` 为空，dev 读 `src-react/dist` | 重跑 `npm run build:front`，或直接用 `npm run dev` |
| MSI 报 `pre-release identifier ... must be numeric-only` | 预发布标识含字母（如 `rc.1`） | 改用纯数字（`0.0.2-1`），或让脚本只出 NSIS |
| `.ps1` 中文乱码 / 日志半截 | PS 5.1 读无 BOM 文件、`Tee-Object` 写 UTF-16 | 脚本保持 UTF-8 BOM；日志用 `[System.IO.File]::AppendAllText` |
| `artifacts/` 与上次 sha256 不同 | 时间戳进包，构建不可复现 | 正常现象；只核对同一批产物的 `SHA256SUMS.txt` |
