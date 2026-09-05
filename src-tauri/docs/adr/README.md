# 架构决策记录（ADR）

Kettd（待办列表 / 桌面知识·待办网）的技术决策存档。

- **总览与现状审计**：[`../ARCHITECTURE-v3.md`](../ARCHITECTURE-v3.md)
- **接口契约**：[`../V2-API.md`](../V2-API.md)（40 命令 + 事件表，有对账脚本）
- **产品规格**：[`../PRD-v2.md`](../PRD-v2.md)

## 索引

| # | 标题 | 状态 | 日期 | 一句话 |
|---|---|---|---|---|
| [0001](0001-frontend-react-migration.md) | 前端基座迁移到 React + shadcn（双轨） | Accepted | 2026-09-04 | 换栈但双轨不接管发布；OS 能力留 Rust 层；M1 用最小窗先试 |
| [0002](0002-backend-ports-and-testability.md) | 引入四个自有端口（Clock/StoreBackend/Notifier/Hotkeys） | Proposed | 2026-09-05 | 全 crate 0 trait、`Local::now()` 9 处直调 → 核心逻辑离线不可测 |
| [0003](0003-store-ownership-and-rule-sinking.md) | Store 所有权收敛与写规则下沉 | Proposed（分两期） | 2026-09-05 | `Store.data` 被三模块跨层直改、命令层内嵌规则 → 一期下沉规则、二期私有化 |
| [0004](0004-scheduler-lock-boundary.md) | 调度器 tick 的锁边界重排 | Proposed（优先级最高） | 2026-09-05 | 持锁跨 OS 通知 + 磁盘写 → 三段式 + 写回幂等重校验 |
| [0005](0005-storage-schema-layering.md) | 存储 schema 分层（data.json / runtime.json） | Proposed（已拍板要做） | 2026-09-05 | 运行态混在用户数据里 → 备份被顶掉、位置无处安放；含懒迁移与回滚方案 |
| [0006](0006-shared-frontend-kernel.md) | 双轨期共享内核 | Proposed（**执行第一步**） | 2026-09-05 | 快录组参三份复制、测试绑源码文本 → 纯 ESM 内核两栈共用 |
| [0007](0007-dynamic-note-windows.md) | 动态便签窗口的注册表与位置持久化 | Proposed（Deferred） | 2026-09-05 | 窗口数从 3 变 N → `WindowRegistry` + `note:` 前缀 + 位置 map |

## 状态语义

| 状态 | 含义 |
|---|---|
| **Proposed** | 已提出，等待逐项拍板。不可作为实现依据 |
| **Accepted** | 已拍板，正在或已经执行 |
| **Deferred** | 方向锁定，实施时机明确延后（见各 ADR 的落点） |
| **Superseded** | 被后续 ADR 取代，**不删除文件**，在索引标注取代者 |
| **Deprecated** | 前提已消失，不再适用 |

## 约定

1. **不改已 Accepted 的 ADR**。要变就写新的，并在旧文件顶部写 `Superseded by ADR-00NN`。
2. 编号 `NNNN` 四位递增，文件名 `NNNN-kebab-case-title.md`。
3. 每条决策必须带**可证伪的出口判据**（写在各 ADR 的 Implementation Notes 或 Consequences 里）。没有判据的决策不算决策。
4. 证据一律用 `文件:行号`。行号以该 ADR 日期当天的工作区为准 —— **代码在会话之间会被改动，引用前先复核**（本项目已多次踩到文档说的与代码不一致）。
5. 涉及用户数据文件的 ADR（当前只有 0005）**必须**在正文里含迁移与回滚方案，且要求"原件永不删除"。

## 新增流程

1. 复制任一现有 ADR 作模板（推荐 0004 的轻量结构 + 0005 的迁移章节）。
2. 填 Context（带证据）→ Decision Drivers → Considered Options（**含被否掉的，失败的决策同样有价值**）→ Decision → Consequences（正/负/风险三块都要写，负面不许省略）。
3. 更新本索引表。
4. 状态先 `Proposed`，逐项拍板后改 `Accepted`。
