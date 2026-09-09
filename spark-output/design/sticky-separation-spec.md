# 便签功能分离改造规格（sticky-separation）

> 2026-09-09 拍板定稿 · owner 四问拍板：① 1号待办便签彻底退役 ② 钉任务功能一并退役
> ③ 快捷键=新建便签（默认 Alt+Shift+S，可换绑/解绑）④ 关闭直接销毁（字面一次性语义）

## 1. 定位

便签 = **纯自由便签**：一张 ≤500 字的纯文本纸，随手记、常驻置顶、多开（≤6 张）、
不进今天、不参与提醒、与待办零耦合。待办只归主窗（今天/收件箱/计划）。

三态生命周期：

```
展开（380×456 纸片） ⇄ 缩小（380×40 置顶悬浮文本条） ──✕──▶ 关闭（销毁）
```

- **缩小**：正文第一行截断显示；点文本展开；⠿ 拖动；常驻置顶
- **关闭**：点 ✕ 或 Alt+F4 → 数据、窗口、note_pos 一并回收，无确认无撤销；
  误关靠写前轮转备份（bak-1..5）兜底
- 形态持久化（`StickyNote.mini`），重启按原样恢复

## 2. 退役清单（分离的全部内容）

| 退役项 | 处置 |
|---|---|
| float 主窗（1号待办便签，conf 静态声明） | 窗口声明删除；全部便签走动态 `note:<id>` 窗（多便签规格 H1b 落地） |
| `kind` 字段（todo 镜像/自由双轨） | 便签只剩自由；迁移时 kind="todo" 的镜像便签**丢弃**（内容活在 tasks，零损失） |
| `StickyNote.hidden`（收起=藏进托盘） | 升格为 `mini`（缩小悬浮条，可见可展开） |
| `StickyNote.pinned` / `settings.stickyPinned` 双轨置顶 | 常驻置顶，无开关；窗内 Pin 钮、设置页置顶行、托盘置顶勾选全删 |
| `Task.stickyPinned`（钉单条待办到便签） | 整链路退役：Task 字段、TaskPayload patch、今天页钉钮、task-row 取下钮、`sticky_pin` 埋点 |
| `show_float` / `hide_float` / `set_sticky_pinned` 命令 | 删除（命令对账 55→53） |

## 3. 数据迁移（v2 便签 schema → v3，JSON 层，幂等）

`store::normalize_stickies_v3`：load 时在 `from_value::<AppData>` 前执行——
① kind="todo" 条目丢弃 ② hidden=true → mini=true ③ kind/pinned/hidden 字段清除；
变化记入 `runtime.migration` 留痕并立即落盘。旧文件多余字段（stickyPinned 等）
由 serde 忽略，不炸不脏。

## 4. 新增能力

| 能力 | 实现 |
|---|---|
| 全局热键「新建便签」 | 热键基建第三槽（HotkeySlot::Sticky / HotkeyAction::NewSticky），默认 Alt+Shift+S，设置页换绑/解绑；三槽互斥 + 任一失败整批回滚（事务扩自两槽版） |
| `runtime::create_note` | 新建唯一入口：设置页命令（async）与热键回调（spawn 后台线程——建窗挂起绝不上事件循环线程）共用；建窗失败回滚入库行；位置从最近便签窗级联 +32px |
| `update_sticky {content?, mini?}` | 内容 ≤500 字；mini 联动 `set_size`（resizable(false) 不拦编程改尺寸，capture_resize 同款），展开时聚焦 |
| note:* CloseRequested | Alt+F4 = 同「关闭即销毁」（堵住旧版「只关窗数据残留」漏洞） |
| 托盘 | `新建便签` / `显示全部便签` / `隐藏全部便签`（替换原 float 专属两项 + 置顶勾选） |

## 5. 保留不动

纸色四预设 / 纸面花纹 / 移出淡化+滑杆（缩小条不参与淡化）/ 折角装饰 / 拖动记位
（note_pos）/ `sticky_create` `sticky_delete` `sticky_free_edit` 埋点；
新增 `sticky_minimize` 语义记在 update_sticky mini=true 分支（后续可加）。

## 6. 已知取舍

- 关闭无撤销（owner 字面拍板）；清单管理（设置页）保留两步确认删除作为防误触管理面
- 旧「收起」便签升级后变成可见缩小条（hidden→mini 的直接后果），不喜欢可逐张 ✕
- float 的历史 note_pos 键 "sticky" 残留在 runtime.json，无害不清理
