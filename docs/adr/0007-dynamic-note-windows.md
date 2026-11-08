# ADR-0007: 动态便签窗口的注册表与位置持久化

**状态**：Accepted（2026-09-05 拍板；实施时机 Deferred 到 H0 便签 v2.1 定稿后）
**日期**：2026-09-05
**关联**：`ARCHITECTURE-v3.md` §3/§5/§8-R6 · ADR-005（`note_pos`）· ADR-0001（React 基座）· `spark-output/design/sticky-note-component-spec.md`

## Context

产品内核已从「待办 app + 可选便签」重定为「待办 + 知识双主体，便签是统一界面层」（roadmap §0）。落到工程上意味着：**窗口数量从固定 3 个变成运行时可变**。

当前架构对"固定窗口"的假设硬编码在四处：

| 证据 | 假设 |
|---|---|
| `runtime.rs:12-14` `MAIN_LABEL`/`FLOAT_LABEL`/`CAPTURE_LABEL` 三个常量 | 窗口 label 是编译期已知的 |
| `main.rs:178/183` `on_window_event` 里按 label 字符串匹配 | 事件处理只认这三扇窗 |
| `scheduler.rs:387` `blur_float_on_desktop_form` 轮询特定 label | 调度器知道具体窗口名 |
| `models.rs:292` `capture_pos: Option<[i32;2]>` | **位置是单值**，N 张便签放不下 |

新增一个窗口类别要动 **5 个文件、8–10 处**（`runtime.rs` label 常量与 builder、`main.rs` 事件匹配与 invoke_handler、`scheduler.rs` 轮询、`tauri.conf.json` windows 数组、`models.rs` 位置字段、`commands.rs` 新命令）。

好消息是有现成模板：**capture 窗已经是运行时用 `WindowBuilder` 创建的**（`runtime.rs:154-170`），不是从 `tauri.conf.json` 静态声明来的 —— 动态窗口只需把这套模式参数化。

## Decision Drivers

- 必须：新增第 N 张便签时**不改** label 常量、事件匹配、调度轮询三处。
- 必须：位置持久化支持 map，且**不能**把 N 个位置写进用户数据文件（ADR-005 已定：运行态归 `runtime.json`）。
- 必须：便签关闭/删除时回收窗口与位置记录，否则 `runtime.json` 无限增长。
- 应该：H0 的"单张便签"与 H1 的"多张便签"用同一套代码，不做两次。
- 红线：不得为便签引入云端或第三方组件。

## Considered Options

### A. `WindowRegistry` + 前缀化 label + 位置 map（推荐）

```rust
// ui/notes.rs
pub const NOTE_PREFIX: &str = "note:";
pub struct NoteRegistry { by_label: HashMap<String, NoteId> }   // "note:ab12" -> 便签 id
// 位置：runtime.json 里 note_pos: { "<noteId>": [x, y, w, h] }
```

- 事件路由从「按 label 常量匹配」改为「按前缀分派」：`main.rs` 的 `on_window_event` 变成 `if label.starts_with(NOTE_PREFIX) { notes::handle(...) }`。
- 调度器不再轮询具体 label，改为订阅"注册表里的可见便签集合"。
- **优**：新增便签零改常量；位置与生命周期有唯一归属；沿用 capture 已验证的 WindowBuilder 模式。
- **劣**：需要一次集中改造（正是本 ADR 的内容）；`runtime.json` 结构变复杂一点。

### B. 每张便签复用 float 窗（一窗多便签，前端自己画）

- **优**：后端零改动。
- **劣**：与产品语义冲突 —— 便签的价值是"可以单独拖到屏幕角落、单独淡化、单独钉住"，塞进一个窗等于没有便签网。否掉。

### C. 用 WebView 内嵌多窗口（单窗 + 绝对定位子层）

- **劣**：跨不出真实桌面窗口，无法"贴在不同显示器角落"，且失去 OS 级置顶控制。否掉。

### D. 现在就实现，不等 H0 定稿

- **劣**：便签 v2.1 的交互（固定/拖拽/移出淡化/预设纸色）还在 `spark-output/design/sticky-note-component-spec.md` 定稿阶段，注册表设计会被具体交互牵着改。已拍板 Deferred。

## Decision

采用 **A**，但**实施时机 Deferred 到 H0 便签 v2.1 定稿之后**（roadmap H1 的 M1 之前或同期）。

方向现在就锁定，避免 H0 阶段做出与它冲突的实现：

1. **H0 便签 v2.1 期间**：允许沿用 capture 的"单窗 + 单值位置"模式，但**必须**把位置写进 `runtime.json`（ADR-005）而不是 `settings`，且 label 用 `note:` 前缀而不是新常量 —— 这样 H1 扩到多张时只改集合，不改归属。
2. **H1**：引入 `NoteRegistry`，事件按前缀分派，位置改 `note_pos: map`，便签关闭即从 map 删除。
3. **回收**：启动时清理 `note_pos` 里已不存在于任务/便签集合的孤儿键（与 `purge_expired` 回收 fired 键同一模式 —— 注意 ADR-003 记录过那个函数的键解析缺陷，这里要写成纯函数 + 表驱动测试）。

## Consequences

**正面**
- 第 N 张便签不再触碰 label 常量、事件匹配、调度轮询。
- 位置与埋点这类运行态有了不污染用户数据的归属（依赖 ADR-005）。
- 事件路由从"字符串等值匹配"变成"前缀分派"，为后续窗口类别（知识便签、KB 预览窗）留出同一扩展点。

**负面**
- 动态窗口数量上升会放大 ADR-004 的锁竞争（每窗都可能触发重拉与位置写入）—— **因此 ADR-004 必须先落地**，否则便签越多卡顿越明显。这条是本 ADR 的硬前置。
- `runtime.json` 从"几个键"变成"含 map"，损坏重建的语义要更小心（重建 = 所有便签回默认位置，可接受）。
- 多窗口的焦点/失焦交互（移出淡化 38%、移入恢复）需要每窗独立计时，前端复杂度上升 —— 属 UI 层，不在本 ADR 范围。

**风险与缓解**
- 风险：孤儿位置永不清理 → `runtime.json` 单调增长。缓解：启动时按便签集合剪枝 + 一条表驱动测试。
- 风险：前缀分派写错，某个 label 恰好以 `note:` 开头被误吞。缓解：便签 id 用现有 `new_id("n")` 生成（`n` 前缀 + 时间戳），并在注册表里以「label 是否存在于 registry」为准，前缀只做快速路由、不做权威判定。
- 风险：H0 为了赶发布把位置写进 `settings`，H1 再迁一次。缓解：本 ADR 明确 H0 的两条约束（`runtime.json` + `note:` 前缀），写进 roadmap H0 出口判据。

## Implementation Notes

前置：ADR-004（锁边界）、ADR-005（`runtime.json`）。
落点：roadmap H1，M1 同期或之后。
出口判据：① 创建/关闭 5 张便签后 `runtime.json` 的 `note_pos` 键数同步变化且无孤儿；② 新增一张便签不需要改任何 label 常量或事件匹配代码（以 diff 为准）；③ 便签数量 5 张时，到点提醒的 UI 写命令无可感知卡顿（ADR-004 的复验）。
