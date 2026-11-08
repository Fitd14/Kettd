# Kettd

一款简洁高效的本地待办应用：待办、多便签、定时提醒、知识库。离线优先，数据只在本机，绝不出网。

Windows 桌面应用，基于 Tauri v1（Rust 后端）+ React 19（前端）。当前版本 2.2.0。

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

## 开发说明

### 技术栈

- **前端** `src-react/`：Vite 8 · React 19 · TypeScript · Tailwind v4 · shadcn · Tiptap 3；hash 路由，无状态库；`lib/api.ts` 为 55 个 Tauri 命令的类型化桥接
- **后端** `src-tauri/`：Tauri v1 · Rust（ports / infra / app 六边形分层）；JSON 文件持久化，无数据库
- **契约文档** `docs/`：`ARCHITECTURE.md`（架构 + 命令契约附录）、`requirements/总体需求.md`、ADR 0001–0007（历史档案）

### 环境要求

- Windows 10+（WebView2 Runtime 随系统或手动安装）
- Node.js ≥ 22.12
- Rust stable（MSVC 工具链）

### 本地开发

```bash
npm install                 # 根目录：Tauri CLI
cd src-react && npm install && cd ..
npm run build:front         # 构建前端（devPath 指向 dist，先构建再跑 dev）
npm run tauri dev           # 启动桌面应用
```

前端改动后需重跑 `npm run build:front`（`beforeDevCommand` 为空，dev 直接读 `src-react/dist`）。

### 发布打包

```bash
npm run tauri build         # 自动先构建前端，产出安装包
```

产物在 `src-tauri/target/release/bundle/`（MSI / NSIS）。WiX 已配置 `zh-CN` 码页；bundle 阶段若遇 `os error 32` 为 Defender 瞬时锁，重试即可。

### 测试与门禁

```bash
cd src-tauri && cargo test && cd ..                    # Rust 单测（时间契约 / 迁移规则 / 存储编排）
node --test test/command-parity.test.mjs               # 命令四方对账：注册 ↔ 定义 ↔ api.ts ↔ ARCHITECTURE 附录
node --test src-react/test/md-fidelity.test.mjs        # Markdown 渲染保真
cd src-react && npx tsc -b                             # 前端类型检查
```

新增 / 删除命令时，四方（`generate_handler!`、`#[tauri::command]`、`api.ts`、`ARCHITECTURE.md 附录 §2`）必须同步，parity 门禁会当场拦截不一致。

### 目录结构

```
Kettd/
├── src-react/              # React 前端
│   ├── src/
│   │   ├── views/          # 六视图（今天/收件箱/计划/回顾/知识库/设置）
│   │   ├── components/     # 共享组件 + kb 组件
│   │   ├── capture/        # 快速记录窗
│   │   ├── sticky/         # 便签窗
│   │   ├── todo/           # 待办悬浮窗
│   │   ├── rendering/      # Markdown / Tiptap 渲染层
│   │   ├── kernel/         # 共享 JS 内核（时间/选择器/事件总线/捕获语法）
│   │   └── lib/            # api.ts 类型化桥接
│   └── test/               # 前端门禁
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── commands.rs     # 55 个 Tauri 命令
│   │   ├── store.rs        # JSON 持久化 + 备份轮转
│   │   ├── models.rs       # 领域模型 + 校验
│   │   ├── scheduler.rs    # 提醒调度
│   │   ├── runtime.rs      # 窗口 / 托盘 / 热键
│   │   ├── kb.rs           # 知识库搜索打分
│   │   ├── ports/ infra/ app/  # 六边形分层
│   │   └── telemetry.rs    # 本地度量
├── test/                   # 命令四方对账门禁
└── docs/                   # requirements / capability / design / adr / ARCHITECTURE（含命令契约）
```