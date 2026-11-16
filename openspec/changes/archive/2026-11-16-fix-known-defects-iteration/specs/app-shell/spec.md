# Spec Delta

## Purpose

定义应用外壳的交互行为与进程生命周期：侧栏与知识库左栏的折叠方式、WebView2 右键菜单策略，以及退出应用时对 WebView2 子进程的清理，保证界面不被浏览器默认能力干扰、进程不留残骸。

## ADDED Requirements

### Requirement: 主侧栏可完全折叠
应用主侧栏 SHALL 支持折叠为仅图标窄轨并支持再次展开，折叠状态 SHALL 在重启后保持。

#### Scenario: 折叠与展开
- **WHEN** 用户点击侧栏折叠按钮
- **THEN** 侧栏收窄为图标窄轨，导航项仅显示图标并有悬浮提示；再次点击恢复完整侧栏

#### Scenario: 状态持久化
- **WHEN** 侧栏处于折叠状态时重启应用
- **THEN** 侧栏保持折叠

### Requirement: 知识库左栏折叠保留标题
知识库左栏折叠 SHALL NOT 完全隐藏标题；折叠后 SHALL 以窄轨形式保留条目标题，并按可用宽度截断显示（省略号）。

#### Scenario: 折叠知识库左栏
- **WHEN** 用户折叠知识库左栏
- **THEN** 左栏变为窄轨，条目仍显示标题（超宽时省略号截断），点击标题可展开回完整列表

### Requirement: 右键菜单不暴露 WebView2 默认项
发布构建中，应用内容区右键 SHALL NOT 出现「检查」「重新加载」等 WebView2 默认菜单项；文本输入区域的复制/粘贴能力 MUST 仍可用（键盘快捷键或自绘菜单）。

#### Scenario: 发布构建右键
- **WHEN** 用户在发布版应用的非文本区域右键
- **THEN** 不出现 WebView2 默认菜单（含「检查」）

#### Scenario: 文本框内复制粘贴仍可用
- **WHEN** 在禁用默认右键菜单后，用户在文本输入框使用 Ctrl+C / Ctrl+V
- **THEN** 复制粘贴正常工作；若提供自绘菜单，菜单包含复制/粘贴/全选

### Requirement: 退出时清理 WebView2 子进程
应用退出 SHALL 确保 `msedgewebview2.exe` 等 WebView2 子进程随之退出，不残留占用用户数据目录的进程。

#### Scenario: 主窗口退出
- **WHEN** 用户通过托盘退出或关闭主窗口退出应用
- **THEN** 退出完成后系统中不再有该应用启动的 msedgewebview2.exe 残留进程

#### Scenario: 多窗口场景退出
- **WHEN** 同时存在便签窗、待办窗、捕获窗时退出应用
- **THEN** 所有窗口关闭且 WebView2 子进程全部退出