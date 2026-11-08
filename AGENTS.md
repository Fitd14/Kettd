# AGENTS.md — Kettd AI 辅助开发规范

本文件为 AI 代理（Cursor / Claude Code / Qoder 等）参与 Kettd 开发时的必读规范。人类开发者同样适用。

---

## 1. 项目概览

**Kettd** 是一款离线优先的 Windows 桌面个人待办工具，核心循环：**记 → 排 → 醒 → 汇**。

- **技术栈**：Tauri v1（Rust 后端）+ React 19（前端）+ TypeScript + Tailwind v4 + shadcn + Tiptap 3
- **架构**：六边形分层（ports/infra/app）；JSON 文件持久化；无数据库
- **命令契约**：55 个 Tauri 命令，四方同步（`generate_handler!` ↔ `#[tauri::command]` ↔ `api.ts` ↔ `V2-API.md §2`）
- **分支策略**：master 与 v2 镜像同步；提交后双推并同步本地引用
- **当前版本**：2.2.0

---

## 2. 常用命令

### 构建与开发

```bash
# 安装依赖
npm install                          # 根目录：Tauri CLI
cd src-react && npm install && cd .. # 前端依赖

# 开发模式（前端需先构建）
npm run build:front                  # 构建前端到 src-react/dist
npm run tauri dev                    # 启动桌面应用（devPath 指向 dist）

# 发布打包
npm run tauri build                  # 自动先构建前端，产出 MSI/NSIS
```

**注意**：`beforeDevCommand` 为空，dev 模式需手动 `build:front`；前端改动后需重跑构建。

### 测试与门禁

```bash
# Rust 单测（时间契约 / 迁移规则 / 存储编排）
cd src-tauri && cargo test && cd ..

# 命令四方对账（55 命令）
node --test test/command-parity.test.mjs

# Markdown 渲染保真
node --test src-react/test/md-fidelity.test.mjs

# 前端类型检查
cd src-react && npx tsc -b && cd ..
```

**门禁纪律**：新增/删除命令必须四方同步，parity 门禁当场拦截不一致。

---

## 3. 目录与接口

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
│   └── docs/               # 契约文档（V2-API / 架构 / PRD / ADR）
├── test/                   # 命令四方对账门禁
└── docs/功能文档/           # 功能文档（设计/开发指南/现状）
```

**关键接口**：
- **命令契约真相源**：`src-tauri/docs/V2-API.md`
- **前端桥接**：`src-react/src/lib/api.ts`（55 命令类型化封装）
- **内核单源**：`src-react/src/kernel/`（时间/选择器/事件总线/捕获语法，禁止第二份实现）

---

## 4. 开发规范

### 4.1 时间处理

- **全链路本地语义字符串**：`YYYY-MM-DDTHH:mm` / `YYYY-MM-DD` / `HH:MM`
- **禁止 UTC 换算**：`models.rs` `DateTimeLocal` 类型；`toISOString` / `Utc` / `naive_utc` 混入视为 bug
- **带秒归零**：`normalize_datetime` 自动处理
- **plannedDate 仅取前 10 位日期**

### 4.2 前端规范

- **Token 类**：颜色一律用 `hsl(var(--xxx))`，禁止硬编码
- **内核单源**：三处「今天」语义必须同一 selector（`kernel/selectors.js`），禁止第二份口径
- **事件总线**：`kernel/event-bus.js`（E14 唯一实现），跨窗通信走事件不走全局状态
- **桥接惰性取**：`__TAURI__` 注入可能晚于模块求值，`lib/api.ts` `tauriBridge()` 每次惰性取

### 4.3 后端规范

- **锁纪律**：网络/通知调用**必须在锁外**，避免阻塞主线程
- **错误消息**：用户可见文案一律中文短句，不含路径与堆栈
- **原子写**：`fs_store.rs` `write_data_at`（tmp+rename）；新增持久文件必须走 `StoreBackend` trait
- **写前轮转**：`data.json` 写前轮转 5 份备份，**不可跳过**
- **损坏通道**：主数据进恢复页；runtime 静默重建；KB 静默空库——**绝不渲染假空态**

### 4.4 命令契约四方同步

新增/删除命令时，以下四处必须同步：
1. `src-tauri/src/main.rs` `generate_handler![]` 注册
2. `src-tauri/src/commands.rs` `#[tauri::command]` 定义
3. `src-react/src/lib/api.ts` 类型化封装
4. `src-tauri/docs/V2-API.md` §2 表格 + §10 计数断言

**parity 门禁**：`test/command-parity.test.mjs` 当场拦截不一致。

### 4.5 便签与窗口

- **kbRef 顶层参数**：`create_sticky` 的 `kbRef` 必须是 invoke 顶层参数（嵌在 `args` 里 Tauri 匹配不到 → 恒 None）
- **content 始终序列化**：含空串；曾因 `skip_serializing_if` 吞空串导致前端 `.split` 抛 TypeError 整树卸载黑屏
- **建窗必须 async**：WebView2 同步建窗偶发无限挂起；窗口创建命令走 async + helper 线程

### 4.6 知识库

- **AI 混合检索已废弃**（2026-11-07）：ai/ 模块整体删除，命令 59→55；**不要重新引入 AI 栈**；搜索保持纯本地
- **搜索打分**：标题 +2 / 标签 +1 / 正文 +1，大小写不敏感；只改 `kb.rs` 一处
- **知识便签同步**：`store-changed` 事件触发重拉；焦点在本窗时跳过防覆盖

---

## 5. 重点关注事项（历史陷阱）

| 陷阱 | 说明 | 出处 |
| --- | --- | --- |
| WebView2 建窗挂起 | 同步建窗偶发无限挂起；必须 async + helper 线程 | `runtime.rs` 注释 |
| kbRef 嵌套参数 | 嵌在 `args` 里 Tauri 匹配不到 → 恒 None | `commands.rs` 注释 |
| content 空串吞掉 | `skip_serializing_if` 导致前端 `.split` 抛 TypeError 整树卸载 | `store.rs` 迁移注释 |
| 桥接惰性取 | `__TAURI__` 注入晚于模块求值；缓存全局引用会 undefined | `api.ts` `tauriBridge()` |
| beforeDevCommand 空 | dev 模式需手动 `build:front`；前端改动后需重跑构建 | `tauri.conf.json` |
| WiX zh-CN | 打包必须 `wix.language=zh-CN`，否则 LGHT0311 | `tauri.conf.json` |
| os error 32 | bundle 阶段 Defender 瞬时锁；重试即可 | 构建日志 |
| parity 门禁 | 新增命令必须四方同步；门禁当场拦截 | `test/command-parity.test.mjs` |
| 假空态 | 数据损坏时绝不渲染「你还没有任务」；进恢复页 | `store.rs` 加载逻辑 |
| 三处「今天」同源 | 侧栏/悬浮窗/统计必须同一 selector | `kernel/selectors.js` 注释 |
| AI 栈已废弃 | 不要重新引入 embedding/hybrid/secrets | 2026-11-07 提交 |
| 用户数据目录 | `%APPDATA%\todo-list`；测试/脚本禁止写入生产目录 | `fs_store.rs` `default_dir()` |
| 时间零 UTC | 全链路本地语义字符串；`toISOString` / `Utc` 混入视为 bug | `models.rs` 注释 |
| 写前轮转不可跳过 | `data.json` 写前必须轮转 5 份备份 | `store.rs` `save()` |
| 损坏通道分流 | 主数据恢复页；runtime 静默重建；KB 静默空库 | `store.rs` `load()` |

---

## 6. 分支与提交

- **分支策略**：master 与 v2 镜像同步；提交后双推并同步本地引用
- **推送链路**：`git push origin master master:v2`（E 盘 Git + GCM 凭据；Clash 全局模式）
- **文档审核门禁**：文档类交付（功能文档 / AGENTS.md / README 等）**先提供内容待人工审核，批准前不 commit/push**；代码任务不受此门禁
- **提交信息风格**：中文，conventional-commit 格式（`feat(kb):` / `fix(test):` / `chore:` / `refactor(kb):`）

---

## 7. 验证清单

提交前必须通过：

- [ ] `cargo check`（0 error；warning 为存量死代码，不新增）
- [ ] `cargo test`（89 项全绿）
- [ ] `cd src-react && npx tsc -b`（0 error）
- [ ] `cd src-react && npm run build`（vite build 成功）
- [ ] `node --test test/command-parity.test.mjs`（55 命令四方对账通过）
- [ ] `node --test src-react/test/md-fidelity.test.mjs`（Markdown 渲染保真）
- [ ] 真机冒烟（启动应用，核对关键路径：快速记录 / 今天视图 / 提醒触发 / 便签三态 / 知识库搜索）

**文档类交付**：额外通过人工审核（内容准确性 / 表述清晰度）。

---

## 8. 相关文档

- `README.md`：快速上手、使用说明、开发环境
- `docs/功能文档/`：10 个功能模块的设计/开发指南/现状
- `src-tauri/docs/V2-API.md`：55 个命令的契约真相源
- `src-tauri/docs/ARCHITECTURE-v3.md`：架构设计
- `src-tauri/docs/adr/`：架构决策记录（0001–0007）
- `src-tauri/docs/PRD-v2.md`：产品需求文档（理想态，部分未实现）

---

**最后更新**：2026-11-08 · 基于 v2 分支 01819eb