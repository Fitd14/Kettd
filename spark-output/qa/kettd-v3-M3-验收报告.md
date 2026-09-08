# 设计验收报告 · Kettd v3/M3 React UI

**验收目标**：2026-09-04 六份定稿规格（today-list / inbox / planned-settings / review-redesign / sticky-note / timeline）vs src-react 实现 + Rust 侧 sticky 字段
**验收时间**：2026-09-07T08:14+08:00
**验收模式**：自动比对（代码级）+ 截图像素取证（OCR=rapidocr + cv2 取色，test/20260907 五张真机截图）
**对照方法**：规格逐条 → 实现代码逐文件核对 → 截图 OCR/取色交叉验证；上游 check findings 属 v2 原型阶段产物，已被 2026-09-04 规格集取代，本轮不重复计数（0/0）

## 总览

| 严重度 | 数量 |
| --- | --- |
| 🔴 Blocker | 0 |
| 🟠 Major | 11 |
| 🟡 Minor | 10 |

**通过维度**（无 deviation）：间距、圆角阴影、响应式
**9 维度覆盖**：间距 ✓通过 · 颜色 3 项 · 字体/文案 1 项 · 圆角阴影 ✓通过 · 图标资产 1 项 · 交互态 2 项 · 状态完整性（含功能缺位）12 项 · 响应式 ✓通过 · 可访问性 1 项

**一句话结论**：便签窗与 TaskRow「行语言」是本次落地的亮点——规格里的视觉 token、交互结构在真机截图像素级命中；偏差集中在**五份规格的第二批功能**（计划页 Tabs、时间轴组件、设置页补项、拖拽排序、今天驾驶舱），与提交记录「五份规格落地 2/3」的进度声明一致，属已知的未落地部分而非实现走样。

## 截图取证摘要（test/20260907）

| 截图 | 视图 | 结论 |
| --- | --- | --- |
| 0c41f751…png | 便签窗（空态） | ✅ 纸色 `#2e2a24` 与暗墨 `hsl(40 12% 16%)` **精确匹配**；胶条 `#3b3730`≈`hsl(40 10% 21%)` ✓；仅固定/关闭两键 ✓；空态文案与规格一致 ✓；~380×450 ✓ |
| 8f9d8e99…png | 捕获条 | ✅ 日期行/提示语在位（M1 试点，不在六规格范围，仅记录） |
| c5b17237…png | 回顾页 | 热力图图例 5 档 ✓、倒序 ✓、chip ✓；❌ 时间轴竖排（qa2-10）、无周期 Tabs（qa2-11） |
| da24f59c…png | 设置页 | 纸色四选/置顶/热键注册快照/免打扰/上限/数据区 ✓；❌ 缺移出淡化、清空统计、版本号（qa2-06/07/15） |
| fde6dc2b…png | 今天页（空态） | 空态+三处同源口径 ✓；❌ 「就地记一条」走 prompt（qa2-04）、无内联输入框（qa2-03） |

> 便签截图上下的浅色带经取色确认为**桌面背景**（窗口圆角外），非样式错误。

## Deviations（按维度分组，严重度排序）

### 状态完整性 (State Coverage) — 12 项

1. **🟠 major** 今天列表未合并（B.2/AC2）
   - 设计源：单一列表 + carry 行内琥珀 badge，无独立「已拖到今天」段，术语改「顺延」
   - 实现：仍拆「今日到期」「已拖到今天」两段；todaySections 内核亦按三段式
   - 修复：合并 due+carried（TaskRow badge 已就绪，删段头即可）
   - 位置：TodayView.tsx:121-161；src/kernel/selectors.js:125-146
2. **🟠 major** §12.2 拖拽排序未实现
   - 设计源：Task.sort_order + reorder_tasks + 今天/便签拖拽排序 + Alt+↑/↓（标注「已确认要加」）
   - 实现：全库无 sort_order/reorder_tasks（收件箱 drag-to-plan 已实现 ✓）
   - 位置：models.rs / commands.rs / StickyWindow.tsx:86-108
3. **🟠 major** 今天页缺「下一条提醒 · HH:MM 标题」驾驶舱行（B.4/AC4）；口径句缺「今天计划 N 项」
   - 修复：nextReminderTime 已在 PlannedView 使用，直接复用
   - 位置：TodayView.tsx:93-98
4. **🟠 major** 今天页无内联捕获输入框（B.1：旧按钮已删 ✓，新入口未落 ✗）
   - 位置：TodayView.tsx:90-119
5. **🟠 major** 设置缺「移出淡化」开关（B.1★）：sticky_fade 全库无字段，淡化不可关；缺「显示/收起便签」入口
   - 位置：SettingsView.tsx:53-85；sticky.css:27-29
6. **🟠 major** 缺「清空本地统计」+ clear_events 命令（B.5★ 半实现：开关 ✓ 清空 ✗）
   - 位置：SettingsView.tsx:148-155；commands.rs
7. **🟠 major** 免打扰时段/提醒时间无编辑器（B.4/A.3/AC2）：from/to 静态文本、原生 type=time 未换 Date Picker、提醒行缺「编辑时间」
   - 位置：SettingsView.tsx:112-119；PlannedView.tsx:75-131
8. **🟠 major** 计划页缺「安排/提醒」Tabs（A.1/AC1）；提醒行按钮常驻（A.2）；加提醒未折叠（A.4）；段名未统一「安排」（A.5）
   - 位置：PlannedView.tsx:62-163
9. **🟠 major** 回顾时间轴未按规格实现：竖排行列表 vs 横向 CompletionTimeline（轨道+分类色圆点+分页 loading+节点下钻+`<time>`+role=list）
   - 位置：ReviewView.tsx:131-141
10. **🟠 major** 回顾页缺周期 Tabs 年/近90天/本月（review §3）
    - 位置：ReviewView.tsx:96-113
11. **🟡 minor** 设置缺版本号（B.7）与「恢复默认快捷键」（B.3）
    - 位置：SettingsView.tsx:87-108,137-156
12. **🟡 minor** 收件箱标题缺「N 条未整理」计数（⑤；侧栏徽标部分代偿）
    - 位置：InboxView.tsx:108-110
13. **🟡 minor** 回顾 KPI 3/4：「全部完成」为全期口径（规格「本年总计」）；「无日期完成」降为条件脚注
    - 位置：ReviewView.tsx:71-75,83

### 交互态 (Interaction States) — 2 项

14. **🟠 major** `window.prompt` 死控件风险：「就地记一条」（TodayView:80）与批量「排期…」（InboxView:38）依赖原生 prompt，Tauri v1（wry）WebView 不提供，可能静默无效——违背「控件皆有实行为」。**待真机确认**
15. **🟡 minor** 「⋯」直接删除而非「详情/删除」；Enter 打开详情未接线（onOpen 未传）
    - 位置：task-row.tsx:64,137-139

### 颜色 (Color) — 3 项

16. **🟡 minor** 逾期折叠条非琥珀（B.3）：中性 border+灰字，达成「非红、可见」，失去琥珀暖压语义 — app.css:165-181
17. **🟡 minor** 热力图灰→琥珀阶 vs 规格「5 档绿」——更贴「安静文具」调性，疑为有意，**请拍板后回写规格 §7** — app.css:406-410
18. **🟡 minor** paper-cyan 色点 86% vs 纸面 token 88%，2 点亮度漂移 — app.css:371 vs sticky.css:38

### 图标与资产 (Assets) — 1 项

19. **🟡 minor** 便签固定/关闭用 emoji 📌/📍/✕ 非 lucide Pin/PinOff/X；且两枚图钉 emoji 形态相近，钉/未钉辨识弱 — StickyWindow.tsx:129,134-136

### 字体/文案 (Typography) — 1 项

20. **🟡 minor** 术语漂移：便签 badge「拖了 N 天」（规格：顺延/滞留，且与主界面 TaskRow「顺延 N 天」跨视图不一致）；今天/收件箱空态文案已改写（B.6 为「保留项」，如改稿定稿请回写规格） — StickyWindow.tsx:99 等

### 可访问性 (Accessibility) — 1 项

21. **🟡 minor** 键盘 j/k 行间导航缺失（A.3；Space/Enter/T/E/X 已实现 ✓） — task-row.tsx:61-68

## 已验证符合（亮点，抽样列主）

- **便签窗**（sticky-spec AC1-AC6 视觉/结构层全过）：四套纸色 token、胶条比纸深一档、折角、38% 淡化 180ms、仅两键、拖拽记位+离屏守卫、无输入框、空态文案、~380×456
- **TaskRow 统一行语言**（§A）：默认只读、hover/focus-within 浮出、琥珀竖条+badge 非红（`--pri-med` 32 85% 38%）、键盘五键、focus-visible 环
- **收件箱**：老化三组（今天进的/本周/更早≥5 天，阈值与规格一致）、批量栏、整行拖拽+右侧「今天」drop、空态正反馈
- **Rust 侧**：sticky_pinned/sticky_paper 字段+coerce、set_sticky_pinned、float_form 三形态删净（全库 0 匹配）、旧值迁移归一
- **设置页**：纸色四选 key 与后端 `STICKY_PAPERS = ["warm","kraft","cyan","ink"]` 契约一致、热键「实际注册快照」（绑定失败明示——上轮 qa-1 已修 ✓）
- **回顾页**：热力图 5 档（0/1-2/3-5/6-9/10+）、列=周横滚、格子下钻（0 完成不可点）、legacy/回收站排除、无日期完成脚注
- **上轮 QA 销账**：qa-1（热键注册失败明示）✓ 已解决；qa-2/qa-3/qa-4 涉及的 vanilla 页面已被 React 重写取代（口径迁移到新规格核查）

## 修复优先级建议

- **必须修复**（0 blocker；影响主流程/契约的 Major）：qa2-04（prompt 死控件，先真机确认）→ qa2-01/02/03（今天页三件套）→ qa2-06/07（设置便签卡补全）
- **建议修复**（其余 Major）：qa2-05 拖拽排序（规格估 ~3h）→ qa2-08/09 计划页 → qa2-10/11 回顾页
- **可延后**（Minor 10 项）：其中 qa2-13（热力图色系）与 qa2-20（文案）只需**设计侧拍板回写规格**即可销账，无需改码

## 验证局限

1. 截图均為深色主题：便签显示暗墨可能来自 `.dark` 回退而非 `stickyPaper` 设置——四套纸色切换需亮色主题真机复核
2. prompt 在 Tauri v1 的实际行为未真机验证（qa2-04）
3. OCR 对小字号/低对比文本有漏检（如设置页「本地度量」行），相关结论以代码为准
4. 本验收只看实现与设计源的还原度，不替代功能测试与用户验收
