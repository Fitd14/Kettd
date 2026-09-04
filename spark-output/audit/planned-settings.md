# Audit — Kettd「计划」+「设置」视图 UI 走查

- 生成：2026-09-04 · 走查对象：`src/index.html` `viewPlanned` / `viewSettings`（代码模式）
- 理念不变：计划=未来安排+提醒控制台；设置=偏好/快捷键/免打扰/数据/隐私。

## ⚠️ 必须先处理的冲突
设置页仍保留「悬浮面板形态：置顶/嵌桌面/迷你条」三选 seg，但**便签重设计已定稿删除三形态**（见 `sticky-note-component-spec.md` §2）。便签落地时该 seg 必须同步替换为「便签控制：固定(置顶) + 预设纸色 + 移出淡化」。

## 总览
🔴 Blocker 0（除上述冲突按 Major 计）｜ 🟠 Major 7 ｜ 🟡 Minor 5

## 计划视图 findings
- **F1** [aesthetic] 未来安排 + 提醒管理两大块挤一页，焦点多、滚动深。→ Tabs「安排 / 提醒」分段。major
- **F2** [aesthetic 8.1] 提醒行控件过载（开关+标题+分类+时间编辑器+状态+稍后+删除）。→ 默认精简，编辑/稍后/删除收进 hover/⋯（统一行组件）。major
- **F3** [consistency] 提醒两来源并存（任务截止派生 + 底部手动加表单）。→ 手动加降级为次要/折叠。major
- **F4** [real-world-match] 术语混用：计划/安排/提醒管理。→ 统一命名。minor
- **F5** [flexibility] `remTimeEditor` 原生 `type=time` 堆多行。→ 上 shadcn Date Picker（对齐回顾页）。major
- 亮点：两处空态引导到位，保留。

## 设置视图 findings
- **F6** [consistency] 悬浮形态三选与便签定稿冲突（见上）。→ 换便签控制。**blocker 级一致性**。major
- **F7** [aesthetic] "显示/收起"与形态 seg 冗余。→ 收敛。minor
- **F8** [help-docs] 快捷键区做得好（绑定/试呼出/解绑/未生效红字）。→ 保留，加"恢复默认"。minor
- **F9** [help-docs] 缺「埋点开关 + 清空本地统计」入口（埋点 PRD §5）。→ 补。major
- **F10** [aesthetic] 数据卡偏长（备份列表）。→ 折叠/滚动。minor
- **F11** [consistency] 缺"便签纸色"分段控件（§12 定了 4 套）。→ 补。major
- **F12** [visibility] 底部无版本号。→ 加 v2.1.0。minor

## 改版机会点
- 🥇 **O1 设置页与便签/埋点对齐**（F6/F7/F9/F11/F12）：删三形态→便签控制(固定/纸色/淡化)；补埋点开关+清空；去显示/收起冗余；加版本号。**最高优先**（消除与已定设计的矛盾）。
- 🥈 **O2 计划页主次拆分 + 提醒行精简**（F1/F2/F3/F4）：安排/提醒分段、提醒行收进 hover、命名统一、手动加提醒降级。
- 🥉 **O3 统一时间/日期编辑组件**（F5 + 免打扰时段）：计划页 remTimeEditor 与设置页免打扰 type=time 都上 shadcn Date Picker。

## 下一步
- O1 优先（它是"矛盾修复"不是"优化"，不做会导致便签落地时设置页自相矛盾）。
- 关键项建议用真实使用验证（Audit 为专家启发式）。
