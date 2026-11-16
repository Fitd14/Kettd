# Spec Delta

## Purpose

定义窗口创建的稳健性要求：WebView2 用户数据目录的选择与可写校验、建窗失败时的错误处理与用户提示，使应用在受限目录、权限异常等环境下仍能启动窗口或给出可诊断、可恢复的失败信息。

## ADDED Requirements

### Requirement: 显式使用可写的 WebView2 用户数据目录
应用启动与创建任何窗口时 SHALL 显式指定位于应用数据目录下的 WebView2 用户数据目录，并在使用前校验该目录可创建、可写；默认 `%LOCALAPPDATA%` 目录不可写时 MUST 回退到可写位置而不是直接建窗失败。

#### Scenario: 正常环境使用应用数据目录
- **WHEN** 应用启动并创建主窗口与便签/待办/捕获窗
- **THEN** 各窗口的 WebView2 用户数据目录均指向应用数据目录下的固定子目录，不依赖默认的 `%LOCALAPPDATA%\com.todo.list`

#### Scenario: 默认目录不可写时回退
- **WHEN** 默认用户数据目录无法创建或不可写
- **THEN** 应用依次尝试备选可写目录（应用数据目录、临时目录），并至少成功创建主窗口

### Requirement: 建窗失败可诊断
窗口创建失败时系统 SHALL 捕获错误、写入可读日志（含错误原因与尝试的目录），并向用户展示中文提示；成功路径 MUST NOT 因超时保护被误判。

#### Scenario: 建窗失败提示
- **WHEN** 创建某窗口（便签/待办/捕获）失败
- **THEN** 界面提示「窗口创建失败」及简短原因，日志/telemetry 记录完整错误与所用目录

#### Scenario: 不无限挂起
- **WHEN** 底层建窗调用长时间无响应
- **THEN** 超时保护触发并返回失败提示，不阻塞主线程

### Requirement: 环境变量仅作兜底并说明局限
系统 SHALL 在文档或启动日志中说明 `WEBVIEW2_USER_DATA_FOLDER` 的兜底用途；由于 Tauri v1 会为窗口显式设置 data_directory 并覆盖该环境变量，实现 MUST 以显式目录为主、环境变量为辅。

#### Scenario: 启动日志说明
- **WHEN** 应用以调试模式启动
- **THEN** 日志中可见实际使用的 WebView2 用户数据目录及是否回退，便于用户/开发者排查

#### Scenario: 环境变量兜底
- **WHEN** 显式目录方案在某环境下不可用
- **THEN** 允许通过 `WEBVIEW2_USER_DATA_FOLDER` 指定目录作为兜底手段，文档中有对应说明