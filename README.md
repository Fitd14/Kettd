# kettd

> 记录任何要做的事情

基于 Tauri v1 + Rust + Vue 3 构建的现代化待办事项管理工具。

## 特性

- ✅ **任务管理** - 创建、编辑、删除、完成任务
- ✅ **子任务支持** - 任务可包含多个子任务
- ✅ **备注功能** - 为任务添加备注和评论
- ✅ **定时提醒** - 设置自定义提醒时间
- ✅ **历史记录** - 记录所有操作历史
- ✅ **搜索功能** - 快速搜索任务和提醒
- ✅ **任务统计** - 完成率、分类统计等
- ✅ **主题切换** - 支持深色/浅色主题
- ✅ **悬浮面板** - 快速添加任务
- ✅ **数据导入导出** - JSON 格式备份

## 技术架构

### 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                        kettd 架构                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐     IPC通信     ┌──────────────┐             │
│  │              │◄───────────────►│              │             │
│  │   Vue 3      │                 │    Rust      │             │
│  │   Frontend   │  Tauri Commands │   Backend    │             │
│  │              │                 │              │             │
│  │  ┌─────────┐ │                 │  ┌────────┐  │             │
│  │  │ App.vue │ │                 │  │main.rs │  │             │
│  │  ├─────────┤ │                 │  └────────┘  │             │
│  │  │Stores   │ │                 │  ┌────────┐  │             │
│  │  │  API    │ │                 │  │models  │  │             │
│  │  ├─────────┤ │                 │  ├────────┤  │             │
│  │  │Components│ │                 │  │service │  │             │
│  │  └─────────┘ │                 │  ├────────┤  │             │
│  │              │                 │  │persist │  │             │
│  │  ┌─────────┐ │                 │  │validate│  │             │
│  │  │ taskStore│ │                 │  └────────┘  │             │
│  │  └─────────┘ │                 │              │             │
│  └──────────────┘                 └──────────────┘             │
│         │                              │                       │
│         ▼                              ▼                       │
│    ┌──────────┐                 ┌──────────────┐               │
│    │  Vite    │                 │   SQLite     │               │
│    │  Build   │                 │   kettd.db   │               │
│    └──────────┘                 └──────────────┘               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 项目结构

```
kettd/
├── src/                        # 前端 Vue 3 源码
│   ├── components/             # 组件
│   │   ├── MainPage.vue        # 主页面
│   │   └── ScheduledPage.vue   # 定时任务页面
│   ├── stores/                 # 状态管理
│   │   ├── taskStore.js        # 集中状态管理
│   │   └── taskStore.test.js   # 单元测试
│   ├── api.js                  # 后端 API 封装
│   ├── App.vue                 # 主应用组件
│   ├── FloatApp.vue            # 悬浮面板
│   ├── main.js                 # 入口文件
│   └── style.css               # 全局样式
├── src-tauri/                  # 后端 Rust 源码
│   ├── src/
│   │   ├── main.rs             # 入口、命令注册
│   │   ├── lib.rs              # 模块导出
│   │   ├── models/             # 数据模型
│   │   │   └── mod.rs          # Task, Reminder, HistoryItem 等
│   │   ├── service/            # 业务服务
│   │   │   └── mod.rs          # TaskService, ReminderService 等
│   │   ├── persistence/         # 持久化层
│   │   │   ├── mod.rs          # Database trait + SQLite 实现
│   │   │   └── migration.rs    # 版本迁移系统
│   │   └── validation/         # 验证层
│   │       └── mod.rs          # validate_task, validate_reminder
│   ├── Cargo.toml              # Rust 依赖配置
│   └── tauri.conf.json         # Tauri 配置
├── openspec/                   # OpenSpec 变更管理
│   └── changes/                # 变更提案、设计文档
├── package.json                # Node.js 依赖配置
├── vite.config.js              # Vite 配置
├── vitest.config.js            # Vitest 配置
└── README.md                   # 项目说明文档
```

### 核心模块

| 模块 | 职责 | 技术 |
|------|------|------|
| **models** | 数据结构定义 | Rust struct + serde |
| **service** | 业务逻辑处理 | Rust service |
| **persistence** | 数据持久化 | SQLite (rusqlite) |
| **validation** | 输入验证 | Rust 函数 |
| **stores** | 前端状态管理 | Vue Composition API |

## 环境要求

### 前端

- Node.js >= 18
- npm >= 9

### 后端

- Rust >= 1.65
- Cargo >= 1.65
- Tauri CLI >= 1.0

## 开发指南

### 安装依赖

```bash
# 安装前端依赖
npm install

# Rust 依赖会在构建时自动安装
```

### 开发模式

```bash
# 启动前端开发服务器（浏览器预览）
npm run dev

# 启动 Tauri 开发模式（桌面应用）
npm run tauri:dev
```

### 代码结构

**前端规范**:
- 组件使用 Vue 3 Composition API
- 状态管理使用 `src/stores/taskStore.js`
- API 调用封装在 `src/api.js`
- 组件放在 `src/components/`

**后端规范**:
- 数据模型放在 `src-tauri/src/models/`
- 业务服务放在 `src-tauri/src/service/`
- 持久化逻辑放在 `src-tauri/src/persistence/`
- 验证逻辑放在 `src-tauri/src/validation/`

## 测试

### 前端单元测试

```bash
# 运行前端单元测试（带热重载）
npm test

# 运行前端单元测试（一次性）
npm run test:run
```

### 后端测试

```bash
# 运行后端单元测试
cd src-tauri
cargo test

# 运行后端集成测试
cargo test --features "integration"
```

### E2E 测试

```bash
# 安装 Playwright 浏览器
npx playwright install

# 运行 E2E 测试
npm run test:e2e
```

## 构建与打包

### 前端构建

```bash
# 构建前端生产版本
npm run build
```

### 桌面应用打包

```bash
# 开发模式构建（快速测试）
npm run tauri:dev

# 生产模式打包
npm run tauri:build
```

**打包产物**:
- Windows: `src-tauri/target/release/kettd.exe`
- macOS: `src-tauri/target/release/bundle/macos/kettd.app`
- Linux: `src-tauri/target/release/kettd`

### 打包选项

在 `src-tauri/tauri.conf.json` 中配置打包选项：

```json
{
  "build": {
    "beforeBuildCommand": "npm run build",
    "beforeDevCommand": "npm run dev",
    "devPath": "http://localhost:5173",
    "distDir": "../dist"
  }
}
```

## 数据存储

### 数据库位置

- Windows: `%APPDATA%\kettd\kettd.db`
- macOS: `~/Library/Application Support/kettd/kettd.db`
- Linux: `~/.local/share/kettd/kettd.db`

### 数据迁移

- 首次启动时自动从旧版 JSON 文件迁移
- 迁移后旧文件备份为 `.json.bak`
- 支持版本迁移系统，平滑升级

### 导入导出

```bash
# 通过 UI 界面导出数据
# 导出格式: JSON

# 通过 UI 界面导入数据
# 支持导入之前导出的 JSON 文件
```

## 配置

### 环境变量

```bash
# Rust 日志级别
RUST_LOG=info    # debug, info, warn, error
```

### 主题配置

应用支持浅色/深色主题切换，主题设置保存在数据库中。

## 贡献指南

1. Fork 项目
2. 创建特性分支 (`git checkout -b feature/xxx`)
3. 提交更改 (`git commit -m "feat: xxx"`)
4. 推送到分支 (`git push origin feature/xxx`)
5. 创建 Pull Request

## 许可证

MIT License

## 联系方式

如有问题或建议，欢迎提交 Issue 或 Pull Request。
