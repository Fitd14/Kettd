# Spec Delta

## Purpose

定义界面文案的口径：只描述用户看到的结果，不解释实现；快捷键只出现在用户学得到的地方，同一概念只用一个词。

## ADDED Requirements

### Requirement: 不出现实现词汇

用户可见字符串 MUST NOT 含内部实现术语（如「三处同源」「selector」「store」「token」）；SHALL 改写成用户可感知的结果，或直接删除。

#### Scenario: 今天页副行

- **WHEN** 打开 `#/today`
- **THEN** 副行只含日期与计数（计划 / 剩 / 做完），不含「三处同源」

### Requirement: 不在文案里解释配色

用户可见字符串 MUST NOT 解释颜色含义（如「（琥珀，非红）」）；颜色语义 SHALL 由视觉本身与 `aria-label` / `title` 承载。

#### Scenario: 收件箱副行

- **WHEN** 打开 `#/inbox`
- **THEN** 副行不含括号颜色注释，而「更早 · 滞留 ≥5 天」分组仍以琥珀呈现

### Requirement: 快捷键只出现在可学到的地方

输入框 placeholder MUST NOT 承载快捷键；快捷键 SHALL 只出现在空态提示、设置页或帮助文档。

#### Scenario: 今天页捕获框

- **WHEN** 今天页为空
- **THEN** placeholder 只说明「记一条，回车放进今天」，`Alt+Shift+A` 出现在空态提示中

#### Scenario: 有任务时不打扰

- **WHEN** 今天页已有任务
- **THEN** 输入框附近不再出现快捷键说明，行内也没有多余提示文案

### Requirement: 文案口径统一

用户可见字符串 SHALL 为中文短句，MUST NOT 含文件路径或堆栈；同一概念 SHALL 只用一个词（今天 / 收件箱 / 计划 / 回顾 / 知识库 / 便签）。

#### Scenario: 保存失败

- **WHEN** 保存失败
- **THEN** 提示为一句中文短句，不含路径、堆栈或内部错误码