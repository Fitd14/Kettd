## 1. Backend Modularization

- [x] 1.1 Create module directories (models, persistence, validation, service)
- [x] 1.2 Move Task, Subtask, Note, Reminder, HistoryItem structs to models module
- [x] 1.3 Create lib.rs with re-exports for all modules
- [x] 1.4 Move validate_task and validate_reminder to validation module
- [x] 1.5 Create TaskService and ReminderService in service module
- [x] 1.6 Refactor main.rs to use new module structure

## 2. Concurrency Control

- [x] 2.1 Add RwLock-wrapped AppState to main.rs
- [x] 2.2 Update all Tauri commands to use State and RwLock
- [x] 2.3 Add proper error handling for lock poisoning
- [x] 2.4 Minimize lock scope by cloning data before processing

## 3. SQLite Persistence

- [x] 3.1 Add rusqlite dependency to Cargo.toml
- [x] 3.2 Create database schema with all tables
- [x] 3.3 Implement Database trait with SQLite implementation
- [x] 3.4 Implement JSON to SQLite migration logic
- [x] 3.5 Update service layer to use Database trait
- [x] 3.6 Add transaction support for write operations

## 4. Frontend State Management

- [x] 4.1 Create stores directory with taskStore.js using Composition API
- [x] 4.2 Add reactive state for tasks, reminders, loading, errors
- [x] 4.3 Implement computed properties (pendingTasks, completedTasks, todayTasks)
- [x] 4.4 Add methods for loadTasks, addTask, updateTask, deleteTask, toggleTask
- [x] 4.5 Refactor App.vue to use taskStore
- [x] 4.6 Refactor MainPage.vue to use taskStore
- [x] 4.7 Refactor ScheduledPage.vue to use taskStore
- [x] 4.8 Refactor FloatApp.vue to use taskStore

## 5. Logging System

- [x] 5.1 Add log and env_logger dependencies to Cargo.toml
- [x] 5.2 Initialize logging in main()
- [x] 5.3 Add log statements to key operations (create, update, delete)
- [x] 5.4 Add error logging with context
- [x] 5.5 Configure log level via environment variable

## 6. Version Migration

- [x] 6.1 Create Migration struct and migration system
- [x] 6.2 Store current version in app_config table
- [x] 6.3 Implement migration execution logic
- [x] 6.4 Add initial migration from JSON to SQLite
- [x] 6.5 Add rollback support for failed migrations

## 7. Testing Framework

- [ ] 7.1 Configure Rust unit tests for validation module
- [ ] 7.2 Write unit tests for validate_task function
- [ ] 7.3 Write unit tests for validate_reminder function
- [ ] 7.4 Configure integration tests for database operations
- [ ] 7.5 Write integration tests for Task CRUD operations
- [x] 7.6 Add Vitest to frontend dependencies
- [x] 7.7 Write unit tests for taskStore methods
- [ ] 7.8 Configure Playwright for E2E testing
- [ ] 7.9 Write E2E tests for core user flows

## 8. Build and Integration

- [x] 8.1 Update Cargo.toml with all new dependencies
- [x] 8.2 Update package.json with test dependencies
- [ ] 8.3 Test backend compilation (cargo check) - Requires Rust toolchain
- [x] 8.4 Test frontend build (npm run build) ✓
- [ ] 8.5 Run all tests to verify functionality
- [ ] 8.6 Verify data migration works correctly
