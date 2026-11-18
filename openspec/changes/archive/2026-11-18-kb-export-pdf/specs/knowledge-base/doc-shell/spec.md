# Spec Delta

## MODIFIED Requirements

### Requirement: 工具条上的文档动作

文档工具条 SHALL 提供以下文档级动作：排版入口、复制 Markdown、导出为 Word、导出为 PDF、大纲开合；这些动作 MUST 位于工具条右侧、与格式工具之间以分隔线分开，且 MUST 有可读的中文名称或 `aria-label`。复制动作的图标 MUST 取「文件 + 代码」语义的图形，MUST NOT 使用通用剪贴板图形。两个导出动作 MUST 在同一组内相邻排列，导出进行中时仅该动作变为进行中状态，其余动作仍可点击。

#### Scenario: 窄窗下三个动作仍可用

- **WHEN** 详情面板宽度不足以并排大纲
- **THEN** 排版、复制与两个导出按钮仍然可点击，大纲按钮改为打开右侧抽屉（见 `knowledge-base/outline`），任何动作都不因窄窗被隐藏

#### Scenario: 导出动作的状态

- **WHEN** 当前条目正文为空
- **THEN** 两个导出动作 MUST 处于禁用状态
- **AND** 有正文时点击其中一个导出动作，MUST 只让该动作显示进行中，MUST NOT 同时禁用或改写另一个