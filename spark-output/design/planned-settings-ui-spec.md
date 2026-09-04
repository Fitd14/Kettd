# 设计增量 · 计划 + 设置 UI 优化（理念不变 · 与便签/埋点对齐）

- 状态：定稿待实现 · 2026-09-04 · 供 M3
- 关联：`audit/planned-settings.md`、`today-list-ui-spec.md`（§A 统一行组件）、`sticky-note-component-spec.md`（便签/纸色/淡化）、`PRD-本地度量埋点`（telemetry）
- 一句话：计划页拆主次 + 提醒行精简；设置页**删三形态改便签控制**、补埋点隐私开关、时段用 Date Picker、加版本号——顺带把便签/埋点的新字段在设置页落位。

---

## A. 计划视图（对应 audit F1–F5）

### A.1 拆「安排 / 提醒」两段（F1）
- 顶部 Tabs：`安排`（按日期分组的未来任务）/ `提醒`（提醒列表）。不再一页两大块、深滚动。
- 默认进「安排」。

### A.2 提醒行精简（F2，复用 `today-list-ui-spec §A` 统一行组件）
- 默认行：`[启停开关] 标题 · 时间 · 状态badge`。
- hover/聚焦才浮出：`编辑时间 / 稍后10分 / ⋯(删除)`。
- 分类 chip 保留；`legacy`「旧版迁移」badge 弱化到 ⋯ 里或 tooltip。

### A.3 时间编辑统一 Date Picker（F5，对齐回顾页旗舰）
- `remTimeEditor` 由原生 `type=time` 堆多行 → shadcn **Date Picker（Popover+Calendar）+ 时刻选择**；多时刻用可增删 Chips。复用回顾页 `timeline-component-spec` 同款时间控件。

### A.4 手动加提醒降为次要（F3）
- 底部「＋ 手动加提醒」默认**折叠**；主路径 = 给任务设截止/提醒自动进计划。

### A.5 命名统一（F4）
- 全页统一叫「安排 / 提醒」，去掉"计划里又叫安排又叫提醒管理"的混用。计数只出现一处。

---

## B. 设置视图（对应 audit F6–F12）

### B.1 ★ 删三形态 → 便签控制（F6/F7，修与便签定稿的冲突）
- **移除**「悬浮面板形态：置顶/嵌桌面/迷你条」三选 seg 与独立「显示/收起」重复。
- 换成「便签」卡：
  - `固定(置顶)` 开关 → `Settings.sticky_pinned: bool`（默认 true）
  - `移出淡化` 开关（+ 可选强度）→ `Settings.sticky_fade: bool`（默认 true）
  - `显示 / 收起` 便签（保留一处即可）
- 迁移：旧 `float_form`（topmost/desktop/mini）一次性归一为 `sticky_pinned=true`（见 `sticky-note-component-spec §2`）。

### B.2 ★ 便签纸色分段控件（F11）
- 外观卡加「便签纸色」四选：`暖白 / 牛皮 / 淡青 / 暗墨` → `Settings.sticky_paper: enum`（默认 暖白）。对应 `sticky-note-component-spec §12.1`。

### B.3 快捷键（F8，保留 + 小增）
- 现有 绑定/试呼出/解绑/"未生效"红字 保留（做得好）。
- 新增「**恢复默认快捷键**」（Alt+Shift+A / Alt+Shift+O）。

### B.4 免打扰时段用 Date Picker（F5 同源）
- `dndFrom/dndTo` 原生 `type=time` → 时刻选择器（与 A.3 同组件）。

### B.5 ★ 埋点隐私控制（F9，补 PRD §5）
- 数据卡加「本地统计(埋点)」：
  - 开关 → `Settings.telemetry_enabled`（已存在，默认 true）
  - 「**清空本地统计**」按钮 → 新命令 `clear_events()`（删/重置 `events.jsonl`，二次确认）
  - 一句说明：「仅本机统计，绝不出网」。

### B.6 数据卡瘦身（F10）
- 备份列表默认折叠（▸ 展开），避免长列表撑高。

### B.7 ★ 版本号（F12）
- 底部隐私行加「版本 v2.1.0」。

---

## C. 新增/涉及的设置字段（落 `Settings`）
| 字段 | 类型 | 默认 | 来源 |
|---|---|---|---|
| `sticky_pinned` | bool | true | 取代 float_form 置顶语义 |
| `sticky_fade` | bool | true | 移出淡化开关 |
| `sticky_paper` | enum(暖白/牛皮/淡青/暗墨) | 暖白 | §12.1 预设纸色 |
| `telemetry_enabled` | bool | true | 已实现 |
| （删）`float_form` | — | — | 迁移后移除 |
- 新命令：`clear_events()`（埋点清空，带二次确认）。
- 迁移：`float_form` → `sticky_pinned`（一次性，写进 `migrate`/`coerce`，不破坏零丢失）。

## D. 可访问性
- Tabs/分段控件键盘可达（←/→ 切换）；开关有 `aria-label`；Date Picker 键盘可选 + 焦点管理；"清空本地统计"二次确认（error-prevention）。

## E. 验收（Given/When/Then）
- AC1 计划分「安排/提醒」两段，提醒行默认精简、动作 hover 才出。
- AC2 提醒/免打扰时间编辑用 Date Picker，不再原生 type=time 堆叠。
- AC3 设置无「置顶/嵌桌面/迷你条」三选，改为便签固定/淡化/纸色控制；旧设置迁移正确。
- AC4 设置含埋点开关 + 清空本地统计（带确认），并显示版本号。
- AC5 全页命名统一、计数不重复。

## F. 工时（M3 估）
- 计划：Tabs 拆分 + 提醒行套统一组件 + Date Picker + 折叠加提醒 ~3h
- 设置：删三形态→便签控制 + 纸色 + 埋点开关/清空 + 免打扰 Date Picker + 版本 + 字段迁移 ~3.5h
- `clear_events` 命令 ~0.5h
合计 ~7h。

## G. 落地顺序
先做 B.1（修冲突，字段迁移）→ B.5 埋点开关 → A.1/A.2 计划分段与行组件（复用 today-list §A）→ A.3/B.4 Date Picker → 其余。
