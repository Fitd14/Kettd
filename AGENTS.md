# AGENTS.md — Kettd 代理协作规范

本文件写给参与 Kettd 开发的 AI 代理（Cursor / Claude Code / Qoder / DSH 等），人类开发者同样适用。

**只写两件事：必须遵守什么（红线），以及怎么自证做对了（验证协议）。**
项目介绍、技术架构、命令清单、测试与门禁、目录结构一律在 [README.md](README.md)，本文件不再复述。

---

## 1. 阅读顺序与效力

1. 先读 [README.md](README.md)：项目是什么、怎么构建、怎么发版（事实层）。
2. 再读本文件：哪些事不许做、交付前要跑什么（规则层）。
3. 改命令前读 `docs/ARCHITECTURE.md` 附录 §2（契约层）。

**效力顺序**：用户当前指令 > 本文件 > README。若本文件与代码 / 门禁冲突，以门禁实际结果为准，并在回复里指出冲突，不要自行脑补。

---

## 2. 硬约束（违反即返工）

### 2.1 时间

- 全链路本地语义字符串：`YYYY-MM-DDTHH:mm` / `YYYY-MM-DD` / `HH:MM`。
- **禁止 UTC 换算**：后端只用 `models.rs` 的 `DateTimeLocal`；`toISOString` / `Utc` / `naive_utc` 混入即 bug。
- 带秒自动归零走 `normalize_datetime`；`plannedDate` 只取前 10 位日期。

### 2.2 前端

- **内核单源**：三处「今天」语义必须同一 selector（`src-react/src/kernel/selectors.js`），禁止第二份口径。
- **事件总线**：跨窗通信只走 `kernel/event-bus.js`，不得引入全局状态库或第二套事件实现。
- **Token 类**：颜色一律 `hsl(var(--xxx))`，禁止硬编码色值。
- **桥接惰性取**：`__TAURI__` 注入可能晚于模块求值，`src-react/src/lib/api.ts` 的 `tauriBridge()` 每次惰性取，不缓存全局引用。

### 2.3 后端

- **锁纪律**：网络 / 通知 / 热键等外部调用必须在锁外，避免阻塞主线程。
- **原子写**：一切持久化走 `StoreBackend`（`infra/fs_store.rs` `write_data_at`，tmp + rename）；新增持久文件必须实现该 trait。
- **写前轮转**：写 `data.json` 前必须轮转 5 份备份，**不可跳过**。
- **损坏通道**：主数据损坏进恢复页；`runtime.json` 静默重建；知识库静默空库——**绝不渲染假空态**。
- **错误文案**：用户可见消息一律中文短句，不含路径与堆栈。
- **数据目录红线**：默认 `%APPDATA%\todo-list`，`KETTD_DATA_DIR` 可覆盖；测试 / 脚本 / 冒烟一律指临时目录，禁止写生产数据。

### 2.4 命令契约四方同步

新增或删除命令时，以下四处必须同时改，缺一处 parity 门禁当场失败：

1. `src-tauri/src/main.rs` `generate_handler![]` 注册
2. `src-tauri/src/commands.rs` `#[tauri::command]` 定义
3. `src-react/src/lib/api.ts` 类型化封装
4. `docs/ARCHITECTURE.md` 附录 §2 表格 + §10 计数断言

### 2.5 便签与窗口

- **kbRef 顶层参数**：`create_sticky` 的 `kbRef` 必须是 invoke 顶层参数；嵌进 `args` 里 Tauri 匹配不到，恒为 `None`。
- **content 始终序列化**：含空串；`skip_serializing_if` 吞空串会让前端 `.split` 抛 TypeError，整树卸载黑屏。
- **建窗必须 async**：WebView2 同步建窗偶发无限挂起，窗口创建命令走 async + helper 线程。

### 2.6 知识库

- **AI 混合检索已废弃**（2026-11-07）：`ai/` 模块整体删除；不要重新引入 embedding / hybrid / secrets，搜索保持纯本地。
- **打分只改一处**：标题 +2 / 标签 +1 / 正文 +1，大小写不敏感，只改 `kb.rs`。
- **知识便签同步**：`store-changed` 事件触发重拉；焦点在本窗时跳过，防覆盖。

### 2.7 版本与依赖

- **版本单一来源**：只走 `scripts/version.mjs`（`get` / `check` / `set`），维护四处清单；`src-tauri/Cargo.lock` 里 `name = "kettd"` 的版本不归脚本管，改完手工同步。
- **依赖冻结线**：Tauri 停 v1（不升 v2）；TypeScript 停 6.x（不升 7）；`window-vibrancy` 锁 `0.4`；`@types/dompurify` / `markdown-it-highlightjs` / `@radix-ui/react-slot` 已移除，禁止回加。细节见 [README §依赖维护](README.md#依赖维护)。
- **预发布版本不许手打 MSI**：WiX 只收纯数字预发布标识；`release.ps1` 会自动剔除 msi 并记进 `droppedTargets`，不要绕过脚本手工 `tauri build`。
- **安装包不可复现**：同一源码两次构建 sha256 不同，`build-manifest.json` 只对当次产物负责，不得跨次比对。

---

## 3. 历史陷阱

| 陷阱 | 结论 | 落点 |
| --- | --- | --- |
| WebView2 建窗挂起 | 必须 async + helper 线程 | `runtime.rs` 注释 |
| kbRef 嵌在 args | Tauri 匹配不到，恒 None | `commands.rs` 注释 |
| content 空串被吞 | 前端 `.split` 抛 TypeError，整树卸载 | `store.rs` 迁移注释 |
| 桥接缓存全局 | `__TAURI__` 注入晚，引用会 undefined | `api.ts` `tauriBridge()` |
| parity 门禁 | 命令必须四方同步，漏改当场红 | `test/command-parity.test.mjs` |
| 假空态 | 数据损坏进恢复页，不显示「你还没有任务」 | `store.rs` 加载逻辑 |
| 三处「今天」分叉 | 侧栏 / 悬浮窗 / 统计必须同一 selector | `kernel/selectors.js` |
| AI 栈已废弃 | 不回引 embedding / hybrid / secrets | 2026-11-07 提交 |
| 时间混入 UTC | 全链路本地语义字符串 | `models.rs` 注释 |
| 跳过写前轮转 | `data.json` 写前必须轮转 5 份 | `store.rs` `save()` |
| 损坏通道混用 | 主数据恢复页 / runtime 重建 / KB 空库 | `store.rs` `load()` |
| 预发布打 MSI | `0.0.2-rc.1` 必失败，脚本自动只出 NSIS | `scripts/release.ps1` |
| `.ps1` 编码 | PS 5.1 读无 BOM 中文乱码；`Tee-Object` 写 UTF-16 毁日志 | `scripts/*.ps1` |
| 文档职责回流 | 命令 / 目录树 / 架构写回 AGENTS 会被 docs 门禁拦下 | `test/docs.test.mjs` |

构建类故障（`os error 32`、`LGHT0311`、前端不重建导致 dev 看旧界面、安装包哈希变化）属操作问题，处理办法统一见 [README §常见构建故障](README.md#常见构建故障)。

---

## 4. 验证协议：对话结果如何验证

### 4.1 断言纪律

- **没跑过就不能说「已修复 / 已完成 / 全绿」**。结论必须附命令与真实输出，不接受「应该没问题」。
- 失败照贴：贴报错原文与退出码，不美化、不省略。
- 部分完成就写清楚边界：`已完成 X / 未验证 Y（原因）/ 阻塞 Z`。
- 只验证了子集时，明确说「本次只跑了 X，未跑 Y」，不要用局部证据冒充整体通过。

### 4.2 分层证据

按改动面选**最小充分集**，逐层往上加：

| 层 | 手段 | 证明什么 | 何时必需 |
| --- | --- | --- | --- |
| L0 静态 | `cargo check` · `npx tsc -b` · `npm run lint` | 能编译、无类型/语法错误 | 任何代码改动 |
| L1 单测 | `cargo test`（103 项）· parity · packaging · md-fidelity | 契约与行为没被破坏 | 改 Rust / 命令 / 渲染 / 配置 |
| L2 构建 | `npm run build`（vite）· `cargo build --release` | 可打包 | 改依赖、构建配置、入口 |
| L3 产物 | `npm run release` + 核对 `artifacts/v<版本>/` | 安装包可交付 | 改打包脚本、版本、tauri.conf |
| L4 真机 | 启动应用冒烟 5 条关键路径 | 用户路径可用 | 改 UI / 交互 / 窗口 / 提醒 |

**L4 冒烟五条**：快速记录（热键呼出 → 回车入库）· 今天视图（列表与侧栏计数一致）· 提醒触发（到点弹通知）· 便签三态（展开 ⇄ 缩小 ⇄ 关闭）· 知识库搜索（键入即出结果）。

### 4.3 一键门禁与 SKIP

默认跑 `npm run check`（12 项，日志 `logs/check-<时间戳>.log`）。汇报证据时写清条数与结果，例如：

```
npm run check → 12 项：11 OK + 1 SKIP（Markdown round-trip：缺 jsdom），0 FAIL
```

- **SKIP ≠ 通过**：必须在交付说明里列名并说明原因，不得算进「全绿」。
- 门禁失败先修再谈交付；确实无法本地复现的（如真机类），写明未验证原因与风险。

### 4.4 UI / 交互改动

必须有真机操作或截图证据；「代码看着对」「构建通过」都不算验证过界面。

### 4.5 交付报告模板

固定五段，缺段视为没交付完：

1. **改动摘要**：改了哪些文件、为什么改（一句话一条）。
2. **验证证据**：跑了什么命令、真实结果（条数 / 通过 / 失败原文）。
3. **未验证项与原因**：SKIP 项、未跑的真机路径、环境限制。
4. **影响面与回滚**：谁会受影响、出问题怎么退（改回哪个文件 / 重跑哪条命令）。
5. **遗留 TODO**：本次没做但该做的事，写清下一步入口。

### 4.6 台账与文档门禁

- 跨轮任务在 `ledger/` 登记，worklog 记录「目的 / 过程 / 结论」，结案前更新状态。
- **文档类交付**（README / AGENTS / 功能文档）先给内容待人工审核，**批准前不 commit、不 push**；代码任务不受此门禁。

---

## 5. 分支与提交

- **分支策略**：仅 master 分支（v2 已于 2026-11-08 归档删除）。
- **推送链路**：`git push origin master`（E 盘 Git + GCM 凭据；Clash 全局模式）。
- **提交信息**：中文 conventional-commit，`feat(kb):` / `fix(test):` / `chore:` / `refactor(kb):` 这一族。
- 提交前确认 `npm run check` 的真实结果，不提交带 FAIL 的树；只提交本次相关文件，不夹带无关改动。

---

## 6. 文档索引

| 文档 | 内容 |
| --- | --- |
| [README.md](README.md) | 功能、使用、操作、技术架构、开发、测试与门禁、发布打包、依赖维护 |
| `docs/ARCHITECTURE.md` | 架构设计与命令契约（附录 §2 为真相源） |
| `docs/capability/` | 10 个功能模块的设计 / 开发指南 / 现状 |
| `docs/adr/` | 历史决策档案 0001–0007 |
| `docs/requirements/总体需求.md` | 产品需求（理想态，部分未实现） |

---

**最后更新**：2026-11-14 · 版本 0.0.2-rc.1
