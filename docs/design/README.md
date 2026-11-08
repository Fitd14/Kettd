# 设计文档

历史设计参考与调研归档。来源：v1 时代未合并分支（2026-07），**非当前实现规格**——当前架构见 [ARCHITECTURE.md](../ARCHITECTURE.md)，当前功能行为见 [capability/](../capability/)。

## 分类

| 目录 | 内容 | 来源分支 |
| --- | --- | --- |
| [research/](research/) | 市场调研、业务工作流、产品设计报告 | feature/optimize-float-window |
| [product/](product/) | 功能架构、看板视图、Markdown 引擎设计 | feature/optimize-float-window |
| [interaction/](interaction/) | 动画设计与动画系统 | feature/optimize-float-window |

## 文档清单

### research/
- `market_research_report.md` — 竞品与市场调研
- `business_workflow_report.md` — 业务工作流分析
- `product_design_report.md` — 产品设计报告

### product/
- `product_feature_architecture.md` — 产品功能架构
- `kanban_view_design.md` — 看板视图设计
- `kanban_views_kanban_complete.md` — 看板完整方案
- `markdown_engine_design.md` — Markdown 引擎设计（v2 知识库渲染的前身思路）

### interaction/
- `animation_design.md` — 动画设计
- `animation_system_v1.md` — 动画系统 v1

## 其他未合并分支文档说明

| 分支 | 文档性质 | 处置 |
| --- | --- | --- |
| feat/react-ui | M0 脚手架说明（prototype/README、spark-output） | 已并入 v2 历史，清理时删除，无新增价值 |
| v1 | openspec 执行规格 + .qoder/skills + .superpowers 会话产物 | 工具 artifacts，非项目文档，不迁移 |
| bugfix/ui-enhancement-fixes | 同上 + UI 修复计划/深度设计 | 执行产物，不迁移 |
| feature/optimize-float-window | 9 份设计/调研文档 + openspec 执行规格 | 设计文档已提取至本目录；执行规格不迁移 |

> 这些文档基于 v1 技术栈（与当前 Tauri+React 不同），引用其中的组件/路径可能已失效。价值在于设计思路与调研结论，而非实现细节。