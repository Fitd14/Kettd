# Metric Blueprint — Kettd v2/v3

- 生成时间：2026-09-08T19:17:20+08:00
- 数据工具：自建本地埋点（events.jsonl，绝不出网）
- 现有覆盖：partial（本轮补齐 4 个 must 缺口后 → 事件 11 个，核心流全覆盖）
- 复盘节奏：weekly（H1 验证门 4 周窗口）
- Owner：owner 本人（单人项目）

## 🌟 North Star：4 周留存
formula = 连续第 4 周仍为有效使用周（周内 capture_commit ≥5 或完成 ≥3）｜目标 = 达成（PRD ≥45% 单人口径）｜这是 roadmap H1 验证门的闸门：不达成，H2 知识网不投。

## 🚀 Drivers（5）
1. 捕获频次 ≥25 条/周（capture_open/commit ✅）
2. 捕获时延 P50 ≤3s（telemetry p50 快照 ✅）
3. 提醒触达率 100%（reminder_shown/missed ✅，切片 reason）
4. 周汇总采用 ≥40%（weekly_export ✅ + weekly_open 🆕 本轮补）
5. 钉动作发生率（sticky_pin 🆕 本轮补）——H1 门专项

## ⚠️ Counters（2+1 声明）
通知轰炸（单日 ≤12 条）｜便签壁纸化（钉住>7 的周占比 ≤20%）｜功能广度（声明式红线）

## 💊 Health
落库失败率 ≤1%（capture_commit_fail 🆕）｜自校验失败 <0.1% ✅｜未运行错过占比（记录）｜备份 ≥1 份

## 🔧 埋点缺口
must 4 项本轮全部补齐：weekly_open（含 React 回顾页周汇总入口恢复——PRD 承诺 3 的 UI 断点就此接通）、sticky_pin、capture_commit_fail、track_event 通用通道（命令 #49）。should：reminder_snoozed、pattern_usage。

## 📊 看板
首屏：NSM + 捕获频次 + 触达率 + 周汇总采用 + 落库失败率。节奏：每周五导出周汇总时看一眼。

## 下一步（路线图推进）
1. H0 尾款：提交 + push 回滚点（当前大量未提交改动）
2. M4：退役 vanilla src/（双奇偶测试护航）
3. H1：多张便签设计稿——等钉动作数据连续 2 周成立再投
4. 第 4 周：H1 验证门裁决会
