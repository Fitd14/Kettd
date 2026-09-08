# 设计走查报告 · Kettd v3（六份定稿规格 vs 需求）

**走查目标**：2026-09-04 六份定稿规格（today-list / inbox / planned-settings / review-redesign / sticky-note / timeline）是否符合需求——需求源 = PRD-v2 四承诺 + brief rev.2（业务目标 / 设计标准 / 策略维度 / out_of_scope）+ stories 8 故事 26 条 AC + journey key_moments
**走查时间**：2026-09-07T19:40+08:00
**走查模式**：Mode C 定向验证（链路完整：brief + stories + journey）+ 10 类清单补充；实现层仅交叉引用 qa.json（qa2 台账，2026-09-07 验收报告），不做还原度复核（QA 职责）

## 总览

| 严重度 | 数量 |
| --- | --- |
| 🔴 Blocker | 0 |
| 🟠 Major | 2 |
| 🟡 Minor | 3 |
| ✅ 通过类别 | 7/10（flow-continuity / ia / components / visual-hierarchy / edge-states / responsive / feedback 无发现） |

**一句话结论**：**设计本身符合要求**——六份规格与 brief 三大策略维度逐条对得上，26 条 AC 在规格层全部有落点，out_of_scope 零越界，异常态/键盘等价/token 纪律覆盖完整；发现的 2 个 Major 都是「上游文档未回写」和「实现层违反设计标准（待真机确认）」，不是设计缺陷。**当前状态（设计+实现）尚未完全达标**：qa2 台账 11 个 Major 未实现项是实现与规格之间的主要差距。

## 优先确认（Mode C 定向核对）

### C.1 Brief Strategy Dimensions 逐项验证

| 维度 | thesis（Brief） | 落地位置（规格） | 判定 |
| --- | --- | --- | --- |
| 信息架构 IA | 「今日计划」单源视图，收件箱-计划-回顾主轴 | today §B.2 合并单列表；inbox 清空工作台；planned §A.1 安排/提醒 Tabs；review 单页总览+下钻 | ✅ 规格通过（实现今天列表仍拆两段，qa2-01） |
| 交互设计 | 记-看-改-删可信闭环，到点真的提醒 | today §B.1 单捕获入口；TaskRow §A.3 键盘全等价；§12.2 拖拽排序；planned §A.2 提醒行+§B.4 免打扰；review 下钻 | ✅ 规格通过（实现缺拖拽排序 qa2-05、j/k qa2-21、prompt 风险 qa2-04） |
| 视觉设计 | 桌面文具感，常驻面积极小 | sticky 现代便签+38% 淡化+~380×456；TaskRow 默认只读 hover 出动作；老化琥珀非红 | ✅ 规格通过（实现便签像素级命中，TaskRow 行语言落地） |

### C.2 Stories AC 三态核对（8 故事）

| Story | AC 要点 | 规格落点 | 判定 |
| --- | --- | --- | --- |
| story-1 快速记录 | ≤1s 唤出、纯文本不弹窗 | capture 已实现（M1）；今天页内联输入框 = today §B.1 | ✅ 规格 ✅ / ⚠️ 实现走 prompt（qa2-04） |
| story-2 搜索 | 打字即搜、分组结果 | 不在六规格范围（KB/M2 已落内存扫搜索） | ➖ 范围外 |
| story-3 首启引导 | 三步引导、1366 无裁切 | 不在六规格范围（v2 已落，React 迁移未重验） | ➖ 范围外（回归项挂账） |
| story-4 今天 | 一键加入今天、粘留标记、逾期折叠、数字同源 | today §B.2/§B.3/§B.4 + TaskRow badge | ✅ 规格 ✅ / 实现列表未合并（qa2-01）、逾期未琥珀（qa2-16） |
| story-5 提醒 | 系统通知、**三态切换**、装饰计时清零 | planned §A.2/§B.4；**三态被 sticky 定稿删除** | ❌ **AC 与定稿冲突 → Major-1**（有意推翻未回写） |
| story-6 周汇总 | 周五草稿、导出 md/csv | review §1 明确「周汇总保留为页内次级入口，不改」 | ✅ 保留 |
| story-7 暗色/缩放 | 主题同步、1024 起回流、禁裸色值 | review §6 响应式；sticky §5 纸色独立明暗；全规格 token 纪律 | ✅ |
| story-8 撤销/备份 | ≥10s 撤销、5 份留档、零丢失 | TaskRow savefail 重试保留；sticky §2 迁移「不破坏零丢失」 | ✅ |

### C.3 Journey Key Moments 状态确认（journey 无 repair_strategies 字段，按 stage_landed/key_moments 核对）

| 阶段 | 类型 | 上轮状态 | 当前判定 |
| --- | --- | --- | --- |
| 2 首启 | dropout-risk | mitigated(原型) | ➖ 六规格范围外，React 迁移后未重验（挂账） |
| 3 捕获 | moment-of-truth | implemented(待生产热键测延迟) | ⏳ 仍待真机测延迟（qa 未覆盖时延指标） |
| 4 提醒 | dropout-risk | mitigated(UI 层)·生产通知待接 | ⚠️ 提醒到点不重复的真机验证仍挂账（上会话遗留②①） |
| 5 汇总 | recovery | implemented | ✅ review 规格保留周汇总+导出 |

## Findings（按严重度排序）

### 🟠 Major（2 项）

1. **[brief-consistency] story-5 AC2 与 sticky 定稿冲突，上游未回写**
   - 出现位置：`context/stories.json` story-5 AC2 vs `design/sticky-note-component-spec.md` §1.4/§2
   - 问题：stories 仍要求「面板三态一键切换（置顶/嵌桌面/迷你条）」，sticky 定稿已「彻底删除三形态 → 唯一便签 + 固定(置顶)」。这是 H1 拍板的有意演进，但 AC 未回写，后续实现者/验收者会按旧 AC 误判。
   - 修复建议：story-5 AC2 回写为「单一便签：固定开关 + 移出淡化 + 拖拽记位」；PRD 对应段落同步；或标注 superseded_by。
2. **[brief-consistency] 实现层违反「控件皆有实行为」设计标准（qa2-04）**
   - 出现位置：`src-react/src/views/TodayView.tsx:80`、`InboxView.tsx:38`
   - 问题：今天页「就地记一条」与批量「排期…」用 window.prompt，Tauri v1 WebView 不提供原生 prompt，可能静默无效——直接违反 brief 定性标准（待真机确认）。
   - 修复建议：今天页换内联输入框（规格 §B.1 本就要求）；收件箱复用 TaskRow 排期弹出。

### 🟡 Minor（3 项）

3. **[copy] sticky 规格内部矛盾**：§7 组件映射表残留「宋体标题」，与 §1.6/§5「无衬线、不用宋体」定稿冲突 → 改 §7 表格为「无衬线标题」。
4. **[copy] 老化术语三处不一**：stories「拖了N天」/ 规格「顺延/滞留」/ 便签实现仍「拖了 N 天」（qa2-20）→ 以规格口径回写 stories + 修便签 badge。
5. **[accessibility] 四套纸色对比度未实测**：规格 §8 要求正文 ≥4.5:1，4 套纸色 × 独立 ink 无实测数据（v2 曾测得 4.4-4.6:1 临界）→ 真机 QA 逐纸色抽测。

## 10 类清单结论（无发现类别）

链路通畅性（关闭=托盘、热键全局入口、Tabs 默认态均闭环）、信息架构（五视图导航命名一致）、组件使用（shadcn+自研边界清晰、DS Missing 有记录）、视觉层级（便签两键、琥珀非红）、异常态（六规格三终态全覆盖）、响应式（1024 minWidth + 1366 约束 + 热力图横滚降级）、反馈（拖拽指示线、批量栏、savefail 重试）——均通过。

## 当前状态结论（回应走查目标）

- **设计符合要求**：六份定稿规格可安全作为 M3 实现依据，无需重设计；上述 Major-1/Major-2 都不是设计缺陷。
- **实现尚未完全符合规格**：qa2 台账 11 Major + 10 Minor 未销（详见 `spark-output/qa/kettd-v3-M3-验收报告.md`），集中在五规格第二批功能（今天页三件套、设置便签卡、计划 Tabs、回顾时间轴、拖拽排序）。
- **两项真机验证仍挂账**：提醒到点触发/不重复、捕获热键时延（journey moment-of-truth 判据）。

## 修复优先级建议

- **必须修复**：Major-1（回写 story-5 AC，半小时文档活）→ Major-2（qa2-04 真机确认后改内联输入）
- **建议修复**：Minor-3/4（规格与文案随手改）→ 按 QA 优先序销实现台账（qa2-01/02/03 → 06/07 → 05 → 08-11）
- **可延后**：Minor-5（并入下次真机 QA 一并抽测）

## 验证局限

1. 本走查对象是**规格文档层**，未重跑实现还原度（以 2026-09-07 qa 报告为准，不重复计数）。
2. story-2/3、journey 阶段 2 不在六规格范围，仅标注「范围外/挂账」，未核 React 迁移后的现状。
3. Mode C 的 AC 核对以规格文本为据，未逐条对照实现代码（避免与 qa2 台账重复计数）。
