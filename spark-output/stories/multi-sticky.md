# Stories — 多便签（H1 增量）

- 生成时间：2026-09-08 ｜ Persona：小柯（沿用 brief/PRD）｜ 数据源：roadmap H1 + ADR-0007 + brief

## Story 索引
| # | 标题 | 颗粒度 | 优先级 | 关键假设 |
| --- | --- | --- | --- | --- |
| sticky-m1 | 再贴一张：第二张便签与各自记位 | Story | P0 | ⭐ |
| sticky-m2 | 随手写一张自由便签：不是待办，就是一张纸 | Story | P0 | ⭐ |
| sticky-m3 | 便签都管理起来：一张清单看清贴了什么 | Story | P1 | — |
| sticky-m4 | 防贴满废纸：上限与清理提示 | Story | P1 | — |
| sticky-m5 | 外观随全局：换纸色所有便签一起换 | Story | P2 | — |

完整 AC 与设计触点见 `spark-output/context/stories.json`（sticky-m1..m5，已追加至既有 8 故事之后）与规格增量 `spark-output/design/multi-sticky-spec.md`。
关键假设：多张便签是否真被用、自由便签是否真被写——埋点 sticky_create/close/count/free_edit 已在规格 §7 定义，发布即验证。
