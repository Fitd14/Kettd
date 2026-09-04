# Audit — Kettd「今天」视图 UI 走查

- 生成：2026-09-04 · 走查对象：`src/index.html` `viewToday`（代码模式）· 模式：自动（代码走查）
- 理念不变：今天 = 每日执行主视图（"记下来就安全"之后的"今天做哪些"）。
- 亮点（保留）：空态 / 全完成态（"✓ 今天清空了" / "今天还是空的，从下面挑一条"）设计到位。

## 总览
🔴 Blocker 0 ｜ 🟠 Major 6 ｜ 🟡 Minor 4

## 改版机会点
- 🥇 **O1 统一"列表行组件 + 单一捕获入口"**（F1/F2/F3/F10）：今天/收件箱/便签共用同一行组件（hover 处理、老化色、拖拽），合并捕获入口，carry 内联化。最高优先——与已定的收件箱/便签改造天然对齐。
- 🥈 **O2 逾期/老化语气与位置统一**（F4/F5/F6）：红色→琥珀、逾期不隐身、规划主/辅路径分清。
- 🥉 **O3 今天=执行驾驶舱**（F7/F8/F9）：就地显示下一条提醒 + 键盘流 + 进度口径。

核心张力：今天想当"驾驶舱"（信息全）又想守"安静文具"（少即是多）——O1 的收敛即为取回平衡。

## Findings

### Major
- **F1** [consistency/aesthetic] 双捕获入口冗余：顶部「快速记录（Alt+Shift+A）」按钮 + 紧接 `newBar('today')` 内联输入，抢同一动作。→ 合并为一个输入框，placeholder 带"或按 Alt+Shift+A"，删重复按钮。证据：`viewToday` 内 `open-capture` 按钮 + `newBar('today')`。effort: quick-win。
- **F2** [aesthetic 8.1] 一屏四区（今天要做的/已拖到今天/已逾期/可拉进今天），且 fresh/carried 拆两段割裂主列表。→ 合成一个"今天要做的"列表，carry 用行内 badge。effort: medium。
- **F3** [real-world-match 2.1] 「已拖到今天」的"拖"（拖延）与即将引入的"拖拽"撞词。→ 改「顺延到今天」或只留"拖了 N 天"badge。effort: quick-win。
- **F4** [aesthetic/real-world-match] 逾期用红色 destructive，与"安静文具"调性冲突。→ 与收件箱老化统一为琥珀，非红。effort: quick-win。
- **F5** [visibility] 逾期折叠在底部、主列表不含，易"隐身"。→ 顶部/折叠条给计数说明。effort: quick-win。
- **F6** [consistency] 「可拉进今天」与收件箱→今天两条规划路径并存。→ 保留为快捷方式，明确主路径在收件箱处理。effort: medium。

### Minor
- **F7** [recognition/flexibility] 今天无"下一条提醒"就地可见（float 有）。→ 进度下加"下一条 15:00 …"。effort: quick-win。
- **F8** [flexibility 7.1] 无键盘流，与"快"定位不符。→ j/k、Enter、T、X。effort: medium。
- **F9** [visibility] 进度条 `total=今天done+今天pending` 口径未标注。→ 旁注"今天计划 N 项"。effort: quick-win。
- **F10** [consistency 4.x] 今天行组件与刚定的收件箱 hover/老化不一致。→ 三视图共用同一行组件。effort: medium（并入 O1）。

## 下一步
- 建议 O1 优先（统一列表组件 + 单一入口），与收件箱/便签改造合并为"统一列表语言"一个改版主题，进 Brief 收敛。
- 本走查为专家启发式，关键项（如逾期语气、carry 是否单列）宜用 3–5 人真实使用验证。
