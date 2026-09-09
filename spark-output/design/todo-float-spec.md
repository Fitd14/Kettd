# 待办悬浮窗规格（todo-float）

> 2026-09-09 拍板定稿 · owner 三问拍板：① 能力档=纯轻量镜像 ② 呼出=托盘开关+独立热键（Alt+Shift+T）③ 视觉=便签纸感

## 1. 定位

待办的桌面悬浮形态：今天列表的镜像窗，看、勾两件事不开主窗。
与便签（sticky-separation 后=纯自由文本）彻底分家；写操作（增/删/改）归主窗与快速记录条。

## 2. 行为

| 项 | 规格 |
|---|---|
| 内容 | 今天列表镜像（todayList 全量，无分页）：勾选框 + 标题；完成即勾、置灰划线 |
| 标题行 | `今天要做的 · N · 做完 M`；胶条拖动（data-tauri-drag-region），位置记忆 note_pos["todo"] |
| 置顶 | 窗内 Pin 钮 ⇄ `settings.todoFloatPinned`（默认开），set_settings 即时同步 always_on_top |
| 呼出/隐藏 | ① 托盘「待办悬浮窗」勾选项 ② 全局热键 Alt+Shift+T（第四槽，可换绑/解绑）③ 窗内 ✕ / Alt+F4 |
| 状态记忆 | `runtime.json todoFloatVisible`：重启按上次显隐恢复（损坏重建默认显示） |
| 点击标题 | 打开主窗并跳今天页 |
| 空/记一条 | 空态提示 Alt+Shift+A（快速记录热键） |

## 3. 实现锚点

- 窗口：动态建（label `todo`，`runtime::todo_builder` 380×456 纸片），不进 conf；
  CloseRequested = 藏窗 + 落盘（**非销毁**，与便签的关闭即销毁相反——待办数据在 tasks，常驻）
- 建窗超时：复用 `runtime::build_with_timeout`（便签分离时已泛化，埋点 `todo_window_error`）
- 热键：四槽位（Capture/Main/Sticky/TodoFloat），`HotkeyAction::ToggleTodoFloat` 回调 spawn 后台线程
  （首次呼出建窗可能挂起，不上事件循环线程）
- 视觉：复用 sticky.css 全套纸感令牌与类（.sticky-note/.sticky-strip/.sticky-row/.tcheck/折角/花纹），
  仅补 `.sticky-title.is-done` 完成态样式（todo.css）；暗色/纸色/花纹随设置即时跟随
- 命令：`register_todo_hotkey` / `hide_todo_float` 新增（对账 53→55）；Pin 走 set_settings 事务外普通补丁

## 4. 真机复验清单

① Alt+Shift+T 呼出/隐藏；② 托盘勾选同步；③ Pin 切换置顶即时生效且重启记住；
④ 勾选完成 → 主窗今天页同步；⑤ 主窗改任务 → 悬浮窗同步；⑥ ✕ 后重启不自动出现，
托盘勾回后恢复；⑦ 拖动记位重启回位；⑧ 四热键互斥（把 Alt+Shift+T 改成别的槽的键要报占用）。
