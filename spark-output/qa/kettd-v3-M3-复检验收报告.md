# 设计验收报告 · Kettd v3/M3 复检自修（qa2 台账销账）

**验收目标**：qa2 台账（2026-09-07T08:14，21 项）全量复核与自修；目标 = 销账全部可修项，达到 v2 第一版可上线
**验收时间**：2026-09-07T20:30+08:00
**验收模式**：自动比对（代码级）自检自修 + 全量回归
**回归结果**：cargo test **78/78 通过** · node --test **26/26 通过** · tsc -b **0 错** · vite build **✓** · oxlint **0 错误**（5 条 React Compiler 提示，既有模式）· 全库 `window.prompt/confirm/alert` **0 残留**

## 总览

| 严重度 | 数量 |
| --- | --- |
| 🔴 Blocker | 0 |
| 🟠 Major | 0 |
| 🟡 Minor | 2（均为实现方案备注级，不阻断上线） |

**qa2 销账率：21/21（100%）**
**Check finding 解决率：4/5**（余 1 项 = 四套纸色对比度实测，需真机 QA，转 access 专项）

## 新增修复（自修过程中发现）

1. **【隐藏假控件】设置页「本地度量」开关后端无效**：`SettingsPayload` 缺 `telemetry_enabled` 字段，`set_settings` 静默丢弃该补丁，`telemetry::set_enabled` 仅在启动时读一次——开关拨了等于没拨。已补：payload 字段 + `apply_settings_patch` + 命令内即时 `set_enabled`。此为上轮 QA 误判「开关 ✓」的漏网项，本次以补丁链路核查确认修复。
2. 命令总数 46 → **48**（`reorder_tasks` / `clear_events`），奇偶测试断言与 V2-API.md §2 表/§10 计数三处同步（奇偶测试当场抓住未同步，机制有效）。
3. 文档回写：stories.json story-4 术语（顺延 N 天）与 story-5 便签形态 AC（按 H1 拍板，标注三形态 AC 被取代）；便签规格 §7「宋体标题」残留改为无衬线。

## qa2 逐项销账（21/21）

| # | 修复摘要 |
| --- | --- |
| qa2-01 | kernel 新增 `todayList()`（due+carried 合并+排序），今天页单列表「今天要做的」，顺延行内 badge |
| qa2-02 | 「下一条提醒 · HH:MM」驾驶舱行（复用 `nextReminderTime`）+「今天计划 N 项」口径句 |
| qa2-03 | 今天页顶部内联输入框，placeholder 按规格原文，回车放进今天 |
| qa2-04 | `window.prompt` 全部移除：批量排期改内联日期面板（今天页由 qa2-03 内联输入承接） |
| qa2-05 | `Task.sort_order` + `reorder_tasks` 命令 + 今天/便签拖拽排序 + Alt+↑/↓ 键盘排序 |
| qa2-06 | `Settings.sticky_fade` 全链贯通（coerce/patch/开关/便签条件淡化）+「显示/收起便签」按钮 |
| qa2-07 | `clear_events` 命令（重置 events.jsonl）+ 两步内联确认按钮（3s 超时复位） |
| qa2-08 | 提醒编辑 = 日期(可选)+时刻弹层；免打扰 from/to = 自绘 HH:MM 时刻输入（校验/非法红边） |
| qa2-09 | 「安排/提醒」Tabs 默认安排；提醒行 hover 才出动作；手动加提醒折叠；段名统一 |
| qa2-10 | 自研横向 `CompletionTimeline`：轨道+分类色圆点+`<time datetime>`+横向滚动+更早分页+节点下钻+role=list |
| qa2-11 | 周期 Tabs 年(默认)/近90天/本月（52/13/5 列） |
| qa2-12 | 逾期折叠条琥珀化（描边+文字 `--pri-med` 系） |
| qa2-13 | 热力图按规格 §7 落 5 档绿（`--heat-1..4` 令牌，亮暗双栈，vanilla styles.css 奇偶对齐） |
| qa2-14 | 便签固定/关闭换 lucide `Pin`/`PinOff`/`X`（currentColor） |
| qa2-15 | `Bootstrap.version` + 设置页「Kettd vN」行 +「恢复默认快捷键」按钮 |
| qa2-16 | 「⋯」= 弹出菜单（详情/删除）；Enter 与菜单「详情」接只读 `TaskDetail` 层（今天/收件箱/回顾通用） |
| qa2-17 | j/k 行间焦点导航（`useRowNav`，输入态不劫持；今天/收件箱/便签通用） |
| qa2-18 | 收件箱标题「收件箱 · N 条未整理」（与分组总数同源） |
| qa2-19 | paper-cyan 色点对齐 `hsl(190 45% 88%)` |
| qa2-20 | 便签 badge「顺延 N 天」；stories story-4 回写；收件箱空态「都安排好了 ✓」 |
| qa2-21 | KPI 四项：本周完成 · 连续打卡 · 本年总计（按年过滤）· 无日期完成（恒显） |

## 余留 Deviations（2 项 minor）

1. **[typography] 今天空态文案改稿未回写规格**（qa3-01）：实现「今天还没有挑出来的事项。」vs 规格 B.6「今天还是空的…」——设计侧拍板二选一后回写，无需改码。
2. **[state-coverage] 时刻编辑器为自研轻量方案**（qa3-02）：日期(可选)+HH:MM 受控输入替代 shadcn Calendar（避免引入 react-day-picker 依赖）；已满足 AC「不再原生 type=time 堆叠」，视觉方案差异留待设计侧确认。

## 上线前仍需真机验证（agent 不可替代）

1. 提醒到点：仍响且不重复（journey 阶段 4 遗留判据）
2. 捕获热键时延 P50 ≤3s（journey moment-of-truth 判据，qa 从未实测）
3. 便签拖动记位重启回位 + 托盘「便签置顶」勾选
4. 新设置字段 `sticky_fade` 首启自动迁移（预期默认 true，无需手工干预）
5. 四套纸色对比度抽测（转 access 专项核查）

<!-- spark-context:qa ref="spark-output/context/qa.json" -->
