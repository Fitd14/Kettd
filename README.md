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
- **定时提醒**：单次 / 每日 / 工作日 / 每周，支持多时刻；系统通知、免打扰时段、稍后提醒
- **快速记录**：全局热键呼出置顶记录条，回车即记
- **多便签**：自由便签（≤6 张）+ 知识便签（≤4 张，知识库条目贴桌面）；四套纸色与花纹、缩小态悬浮文本条、关闭即销毁
- **待办悬浮窗**：今天列表镜像窗，可置顶
- **知识库**：Markdown 即时渲染编辑（WYSIWYG）、标签筛选、本地全文搜索、知识便签贴到桌面、任务挂载资料、条目一键建待办
- **本周汇总导出**：Markdown / CSV
- **本地度量**：仅本机统计，可随时关闭或清空

## 使用说明

### 窗口与全局热键

| 窗口 | 用途 | 默认热键 |
| --- | --- | --- |
| 主窗口 | 今天 / 收件箱 / 计划 / 回顾 / 知识库 / 设置 | `Alt+Shift+O` |
| 快速记录 | 置顶记录条，回车记下、Esc 收起、按住 ✎ 拖动 | `Alt+Shift+A` |
| 新建便签 | 创建一张自由便签 | `Alt+Shift+S` |
| 待办悬浮窗 | 呼出 / 隐藏今天列表镜像 | `Alt+Shift+T` |

热键均可在「设置」页改绑或解绑；解绑后仍可从托盘呼出。

### 托盘菜单

打开主界面 · 快速记录 · 待办悬浮窗（勾选态）· 新建便签 · 显示/隐藏全部便签 · 管理提醒 · 本周汇总导出 · 打开数据文件夹 · 退出。

关闭主窗口不退出应用——托盘常驻，提醒照常触发。

### 便签三态

展开（纸片编辑）⇄ 缩小（置顶悬浮文本条，显示正文第一行，点击展开）⇄ 关闭（即销毁，无确认）。自由便签支持 Markdown 渲染开关；知识便签指向知识库条目、随编辑同步，关闭不删数据。今天列表镜像由待办悬浮窗承担。

### 知识库

左侧搜索 + 标签筛选，右侧阅读/编辑即时切换；条目可「贴到桌面」成为知识便签（视口指向条目，随编辑同步），也可一键建待办或挂载到任务资料区。搜索为纯本地全文扫描，键入即渲染。

## 操作文档

### 数据目录

所有数据位于 `%APPDATA%\todo-list`（托盘 → 打开数据文件夹）：

| 文件 | 内容 | 写策略 |
| --- | --- | --- |
| `data.json` | 任务 / 提醒 / 设置 | 写前轮转 5 份备份，原子替换 |
| `notes.json` | 知识库条目 | 原子写，与主数据物理隔离 |
| `runtime.json` | 窗口位置 / 提醒触发态 / 迁移报告 | 高频写，不轮转 |
| `events.jsonl` | 本地度量事件 | 设置页可清空 |
| `data.json.bak-1~5` | 轮转备份（bak-1 最新） | 设置页可恢复 |
| `data.v1.json` | v1 旧版迁移留档 | 永不覆盖 |

### 备份与恢复

每次写入 `data.json` 前自动轮转备份（最多 5 份）。设置页「数据与备份」可查看各备份的时间 / 任务数 / 可读性并一键恢复。知识库损坏时静默降级为空库，不传染主数据。

### 设置项

主题（亮 / 暗）、四个全局热键、便签纸色 / 花纹 / 移出淡化、免打扰时段、每小时提醒上限、本地度量开关、数据与备份管理。

## 技术架构

### 技术栈

- **前端** `src-react/`：Vite 8 · React 19 · TypeScript 6 · Tailwind v4 · shadcn · Tiptap 3；hash 路由，无状态库；`lib/api.ts` 为 64 个 Tauri 命令的类型化桥接
- **后端** `src-tauri/`：Tauri v1 · Rust（ports / infra / app 六边形分层）；JSON 文件持久化，无数据库
- **契约文档** `docs/`：`ARCHITECTURE.md`（架构 + 命令契约附录）、`requirements/总体需求.md`、ADR 0001–0007（历史档案）

### 分层与数据流

```
UI 层（React）
  views/ components/          六视图与共享组件
  capture/ sticky/ todo/      快速记录窗 / 便签窗 / 待办悬浮窗
  rendering/                  Markdown 与 Tiptap 渲染
  kernel/                     时间、选择器、事件总线、捕获语法（唯一实现）
  lib/api.ts                  64 命令类型化 invoke
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
- **持久化**：一切写入走 `StoreBackend`（tmp + rename 原子替换）；`data.json` 写前轮转备份，`notes.json` 与主数据物理隔离。
- **提醒**：`scheduler.rs` 按分钟检查到点提醒；通知调用在锁外，免打扰时段内不发。
- **窗口**：`runtime.rs` 管理主窗 / 快速记录 / 便签 / 悬浮窗与托盘、热键；建窗一律 async + helper 线程，避开 WebView2 同步建窗挂起。

### 命令契约

前端与后端的接口是 **64 个 Tauri 命令**，四方必须同步：

1. `src-tauri/src/main.rs` `generate_handler![]` 注册
2. `src-tauri/src/commands.rs` `#[tauri::command]` 定义
3. `src-react/src/lib/api.ts` 类型化封装
4. `docs/ARCHITECTURE.md` 附录 §2 表格 + §10 计数断言

真相源是 `docs/ARCHITECTURE.md` 附录；`test/command-parity.test.mjs` 会当场拦下任何一处漏改。新增命令的写法与约束见 [AGENTS.md §2.4](AGENTS.md)。

### 关键文件索引

| 文件 | 职责 |
| --- | --- |
| `src-tauri/src/main.rs` | 启动、初始化、命令注册 |
| `src-tauri/src/commands.rs` | 64 个 Tauri 命令（参数校验 + 转调 app） |
| `src-tauri/src/app/` | 用例编排：tasks / reminders / settings |
| `src-tauri/src/ports/` | 端口 trait：StoreBackend / Clock / Notifier / Hotkeys |
| `src-tauri/src/infra/fs_store.rs` | 原子写、备份轮转、数据目录解析 |
| `src-tauri/src/store.rs` | 持久化编排、v1 迁移、损坏通道分流 |
| `src-tauri/src/models.rs` | 领域模型、校验、`DateTimeLocal` |
| `src-tauri/src/scheduler.rs` | 提醒调度 |
| `src-tauri/src/runtime.rs` | 窗口 / 托盘 / 热键 |
| `src-tauri/src/kb.rs` | 知识库搜索打分（标题 +2 / 标签 +1 / 正文 +1） |
| `src-tauri/src/telemetry.rs` | 本地度量事件 |
| `src-react/src/lib/api.ts` | 命令桥接（`__TAURI__` 惰性取） |
| `src-react/src/kernel/` | 时间 / 选择器 / 事件总线 / 捕获语法单源 |
| `scripts/` | `version.mjs` 版本同步 · `check.ps1` 门禁 · `release.ps1` 一键发版 |
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
│   │   ├── components/     # 共享组件 + kb 组件
│   │   ├── capture/        # 快速记录窗
│   │   ├── sticky/         # 便签窗
│   │   ├── todo/           # 待办悬浮窗
│   │   ├── rendering/      # Markdown / Tiptap 渲染层
│   │   ├── kernel/         # 共享 JS 内核（时间/选择器/事件总线/捕获语法）
│   │   └── lib/            # api.ts 类型化桥接
│   └── test/               # 前端门禁（渲染保真 / round-trip）
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── commands.rs     # 64 个 Tauri 命令
│   │   ├── store.rs        # JSON 持久化 + 备份轮转
│   │   ├── models.rs       # 领域模型 + 校验
│   │   ├── scheduler.rs    # 提醒调度
│   │   ├── runtime.rs      # 窗口 / 托盘 / 热键
│   │   ├── kb.rs           # 知识库搜索打分
│   │   ├── ports/ infra/ app/  # 六边形分层
│   │   └── telemetry.rs    # 本地度量
│   └── tauri.conf.json     # 打包配置（bundle / wix / 窗口）
├── scripts/                # version.mjs / check.ps1 / release.ps1
├── test/                   # command-parity / main-window-close / packaging / docs
└── docs/                   # requirements / capability / design / adr / ARCHITECTURE（含命令契约）
```

### 测试与门禁

```bash
npm run check                                                     # 一把跑完 12 项
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check.ps1 -SkipRust    # 只查前端
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check.ps1 -Only Markdown  # 按名字过滤
```

门禁顺序：版本一致性 → 命令四方对账 → 主窗口关闭契约 → 打包配置契约 → 文档结构契约 → Markdown 渲染保真 → Markdown round-trip → 前端类型检查 → 前端 lint → 前端构建 → `cargo check` → `cargo test`。任一步非零即退出 1；日志在 `logs/check-<时间戳>.log`。依赖缺失导致跳过的项会标 `SKIP` 并单独提示（当前 `md-roundtrip` 需要 `jsdom`，缺失时跳过）——**SKIP 不算通过**。

单独跑某一项：

```bash
cd src-tauri && cargo test && cd ..                    # Rust 单测（103 项）
node --test test/command-parity.test.mjs               # 命令四方对账：注册 ↔ 定义 ↔ api.ts ↔ ARCHITECTURE 附录
node --test test/main-window-close.test.mjs            # 主窗口关闭契约
node --test test/packaging.test.mjs                    # 打包配置契约（8 条）
node --test test/docs.test.mjs                         # 文档结构契约（README / AGENTS 分工）
node --test src-react/test/md-fidelity.test.mjs        # Markdown 渲染保真
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
2. **门禁**：默认先跑 `scripts/check.ps1`（12 项），`-SkipChecks` 可跳过。
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
