## Context

kettd 是一个基于 Tauri + Rust + Vue 的单机版待办工具。当前后端所有逻辑集中在 `main.rs` 单文件（约850行），包含数据模型定义、持久化逻辑、业务验证和 API 命令。前端状态管理分散，每个组件独立发起 API 请求。数据存储使用 JSON 文件，缺乏并发控制。随着功能需求增长，当前架构已难以支撑后续开发和维护。

## Goals / Non-Goals

**Goals:**
1. 后端模块化重构，建立清晰的分层架构（models、persistence、validation、service）
2. 解决并发安全问题，防止多窗口同时操作导致数据损坏
3. 前端使用 Composition API 建立集中状态管理
4. 数据存储从 JSON 文件迁移到 SQLite 数据库
5. 搭建测试框架，覆盖单元测试、集成测试和 E2E 测试
6. 引入日志系统，支持结构化日志输出
7. 建立数据版本迁移系统，支持平滑升级

**Non-Goals:**
1. 添加新业务功能（如团队协作、云端同步等）
2. 修改 UI 设计风格
3. 引入复杂的依赖注入框架
4. 支持多用户

## Decisions

### 1. 后端架构设计

**Decision**: 采用分层模块化架构，将代码分为 models、persistence、validation、service 四个模块

**Rationale**: 
- 单文件代码膨胀难以维护，模块化后职责清晰
- 便于单元测试，每个模块可独立测试
- 方便后续扩展，如替换存储实现、添加新服务

**Architecture:**
```
src-tauri/src/
├── main.rs              # 入口、命令注册、依赖注入
├── lib.rs               # 公共类型定义
├── models/              # 数据模型（Task、Reminder、HistoryItem等）
├── persistence/         # 持久化层（抽象接口 + SQLite实现）
├── validation/          # 验证层（任务验证、提醒验证）
└── service/             # 业务逻辑（TaskService、ReminderService等）
```

**Alternatives Considered:**
- 保持单文件：简单但难以维护，排除
- 使用完整的依赖注入框架：过度设计，当前项目规模不需要

### 2. 并发控制

**Decision**: 使用 `std::sync::RwLock` 保护共享状态

**Rationale**:
- Tauri 支持多窗口，多个窗口同时操作数据会导致竞争条件
- RwLock 允许多个读操作同时进行，写操作独占，适合读多写少的场景
- 实现简单，无需引入复杂的异步运行时

**Implementation:**
```rust
struct AppState {
    db: RwLock<Database>,
}

#[tauri::command]
fn get_tasks(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<Task>, String> {
    let db = state.db.read().map_err(|e| format!("{}", e))?;
    db.get_tasks()
}
```

**Alternatives Considered:**
- 使用 Mutex：简单但性能较差（写锁时阻塞所有读）
- 使用 async runtime：增加复杂度，当前不需要

### 3. 前端状态管理

**Decision**: 使用 Vue Composition API 封装状态管理（不引入 Pinia）

**Rationale**:
- 当前项目规模较小，不需要完整的状态管理库
- Composition API 的 `reactive` 和 `provide/inject` 足以满足需求
- 保持轻量级，减少依赖

**Implementation:**
```javascript
// src/stores/taskStore.js
import { reactive, computed } from 'vue';

const state = reactive({
  tasks: [],
  reminders: [],
  loading: false,
});

export function useTaskStore() {
  const pendingTasks = computed(() => state.tasks.filter(t => !t.completed));
  // ...
  return { state, pendingTasks, loadTasks, addTask, ... };
}
```

**Alternatives Considered:**
- Pinia：功能完整但对于当前项目过度
- Vuex：已废弃，官方推荐 Pinia

### 4. 数据存储

**Decision**: 使用 rusqlite 库将数据存储从 JSON 迁移到 SQLite

**Rationale**:
- SQLite 支持复杂查询（历史记录按日期、类型过滤等）
- 事务支持，保证数据一致性
- 性能更好，适合数据量增长
- 广泛支持，社区成熟

**Schema Design:**
```sql
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    category TEXT NOT NULL,
    priority TEXT NOT NULL,
    due_date TEXT,
    completed BOOLEAN DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE subtasks (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    title TEXT NOT NULL,
    completed BOOLEAN DEFAULT 0,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

CREATE TABLE notes (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    author TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

CREATE TABLE reminders (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    time TEXT NOT NULL,
    category TEXT NOT NULL,
    completed BOOLEAN DEFAULT 0
);

CREATE TABLE history (
    id TEXT PRIMARY KEY,
    action TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    target_title TEXT NOT NULL,
    detail TEXT,
    timestamp TEXT NOT NULL
);

CREATE TABLE app_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

**Alternatives Considered:**
- JSON 文件：简单但查询能力有限，排除
- PostgreSQL：需要单独服务，不适合单机应用

### 5. 测试框架

**Decision**: 后端使用 Rust 内置测试，前端使用 Vitest，E2E 使用 Playwright

**Rationale**:
- Rust 内置测试框架足够强大，无需额外依赖
- Vitest 是 Vue 官方推荐的测试框架，与 Vite 无缝集成
- Playwright 支持多浏览器测试，功能强大

**Test Coverage:**
- 单元测试：验证逻辑、工具函数（后端和前端）
- 集成测试：API 命令、数据存储（后端）
- E2E 测试：核心用户流程（前端）

### 6. 日志系统

**Decision**: 使用 log + env_logger 作为日志框架

**Rationale**:
- Rust 生态标准日志库，接口统一
- env_logger 配置简单，适合开发和生产环境
- 支持日志级别过滤，便于调试

**Implementation:**
```rust
use log::{info, warn, error, debug};

env_logger::init();

fn save_task(task: &Task) -> Result<(), String> {
    info!("Saving task: {}", task.title);
    // ...
    match db.save_task(task) {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Failed to save task: {}", e);
            Err(format!("Failed to save task: {}", e))
        }
    }
}
```

### 7. 版本管理

**Decision**: 建立数据迁移系统，每次版本升级执行相应迁移脚本

**Rationale**:
- 当前只有一个版本标识，缺乏版本管理机制
- 迁移系统确保平滑升级，不丢失用户数据
- 支持增量迁移，便于维护

**Implementation:**
```rust
struct Migration {
    version: &'static str,
    apply: fn(&mut Database) -> Result<(), String>,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: "1.0.1",
        apply: |db| {
            db.execute("ALTER TABLE tasks ADD COLUMN sort_order INTEGER DEFAULT 0")
        },
    },
];

fn migrate_if_needed(db: &mut Database) -> Result<(), String> {
    // 检查当前版本，执行未应用的迁移
}
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| SQLite 迁移可能丢失用户数据 | 迁移前自动备份 JSON 文件，支持回滚 |
| 后端重构可能引入回归问题 | 完善测试覆盖，迁移后进行全面测试 |
| 并发控制可能引入死锁 | 使用 RwLock 而非 Mutex，保持锁持有时间短 |
| SQLite 在 Windows 上的兼容性 | 使用 rusqlite 的 bundled 特性，避免系统依赖 |
| 测试框架搭建耗时 | 优先覆盖核心逻辑，逐步扩展 |

## Migration Plan

1. **Phase 1**: 后端模块化重构，保持 JSON 存储不变
2. **Phase 2**: 引入 SQLite，支持从 JSON 自动迁移
3. **Phase 3**: 前端状态管理重构
4. **Phase 4**: 添加测试和日志系统
5. **Phase 5**: 版本迁移系统

**Rollback Strategy:**
- 每个阶段的代码都应向后兼容
- SQLite 迁移失败时自动回退到 JSON 存储
- 保留旧代码分支，便于快速回滚

## Open Questions

1. 是否需要支持从旧版本 JSON 文件自动迁移到 SQLite？
2. 测试框架的具体测试用例范围？
3. 日志是否需要输出到文件？
