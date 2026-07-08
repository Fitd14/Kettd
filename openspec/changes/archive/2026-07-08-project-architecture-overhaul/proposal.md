## Why

当前 kettd 项目作为一个单机版待办工具，后端所有逻辑集中在 `main.rs` 单文件中（约850行），前端状态管理分散，数据存储使用JSON文件且缺乏并发控制，代码可维护性和可扩展性较差。随着功能需求增长（历史记录、搜索、统计等），当前架构已难以支撑后续开发。本次重构旨在建立清晰的分层架构，解决并发安全问题，引入SQLite持久化，完善测试覆盖和日志系统，为项目长期维护和功能扩展奠定坚实基础。

## What Changes

1. **后端模块化重构** - 将 `main.rs` 拆分为 models、persistence、validation、service 四个模块
2. **并发安全控制** - 使用 `RwLock` 保护共享数据，防止多窗口同时操作导致数据损坏
3. **前端状态管理** - 使用 Vue Composition API 封装集中状态管理，消除组件间数据请求重复
4. **数据存储迁移** - 从 JSON 文件迁移到 SQLite 数据库，支持更复杂的查询和数据管理
5. **测试框架搭建** - 添加单元测试、集成测试和 E2E 测试，覆盖核心业务逻辑
6. **日志系统** - 引入结构化日志，便于问题追踪和调试
7. **版本管理** - 建立数据迁移系统，支持平滑升级

## Capabilities

### New Capabilities

- `backend-modularization`: 后端模块化架构，包含 models、persistence、validation、service 模块划分
- `concurrency-control`: 并发安全控制，使用 RwLock 保护共享状态
- `frontend-state-management`: 前端集中状态管理，使用 Vue Composition API
- `sqlite-persistence`: SQLite 数据持久化，替代 JSON 文件存储
- `testing-framework`: 测试框架搭建，包含单元测试、集成测试、E2E测试
- `logging-system`: 日志系统，支持结构化日志输出
- `version-migration`: 数据版本迁移系统，支持平滑升级

### Modified Capabilities

- None

## Impact

- **Backend**: `src-tauri/src/main.rs` 重构为多模块结构
- **Frontend**: `src/api.js` 重构为状态管理模式，组件逻辑调整
- **Data**: 数据存储从 `data.json` 迁移到 `kettd.db`（SQLite）
- **Dependencies**: 添加 `rusqlite`、`log`、`env_logger`、`pinia` 等新依赖
- **Build**: 测试命令和构建流程调整
