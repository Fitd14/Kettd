# PRD — Kettd v2 · 个人待办记录汇总（direction: 按旅程断点的可信重构）

- **生成时间**：2026-09-02T13:20 首签 · **rev.2** 2026-09-02T18:05 升版
- **数据源**：brief rev.2 ✅ / stories ✅8 / sitemap ✅18 / journey rev.2 ✅ / audit ✅24 / bench ✅ / **flow-web ✅（F0-F7 八文件交付：tsc 0 错 · vite build ✓）** / check ✅（5 全销账）/ edge ✅（must 16/16 CLEARED）/ extract ✅ tokens
- **完整度**：strong（thin#1『资产待生成』rev.2 已闭环，见文末声明）
- **交付对象**：coding agent / 工程师。v2 分支提交 `f470314`；离线交付物 `output/kettd-v2.bundle`（本地执行 `git fetch kettd-v2.bundle v2:refs/heads/v2`）；master 按 owner 批复保持不动；v1 `src/`+`src-tauri/`=改造基线，`prototype/`=验证原型

---

## 1. Summary

Kettd v2 是一款**离线优先的 Windows 桌面个人待办工具**，为「小柯」这样的学生/上班族而做：他们怕忘事、反感云账号与订阅费，需要一台机器上最快的记录入口和一份能救回周报的自动汇总。触发重构的原因是 v1 体验触底——搜索失效、数据可被静默覆盖、整条提醒链路不可达（Audit 三项 blocker），而竞品阵营同步走向重型化与订阅化（滴答 139-168 元/年、功能过载），留下「**3 秒记 · 到点真提醒 · 数据在你手里 · 永不收费**」的空白定位。成功形态是四个可量化承诺：捕获 3 秒内落库、运行期提醒触达率 100%、完成记录 100% 进日志并可一键导出周汇总、存量数据丢失率为零。

---

## 2. Background & Problem

### 用户视角的问题（来自 journey × audit，非产品视角）

小柯的问题不是「没有待办软件」，而是**不敢把重要的事情托付给任何一个**：

- 念头冒出来时，最快的路子往往是摸手机开备忘录——桌面工具从呼出到存下一条要开窗口、填表单、选日期（v1 实测 4+ 步，audit-16）；
- 记了怕丢：数据文件读不出来时 v1 直接把任务列表渲染成「空」（audit-1/2 双 blocker），删除没有撤销（audit-7），编辑一次还会让截止时间漂移 8 小时（audit-6）；
- 定了提醒也不会响：整条提醒系统在可达界面里不存在、后端没有通知代码（audit-4）；
- 一周做完了什么无人可查：完成即蒸发，没有日志、没有汇总、没有导出（audit-7 + Things 区用户原话「重规划轻总结，做完就完了不能导出」）。

### 当前 workaround（用户今天怎么办）

手机自带备忘录 / 微信群文件传输助手 self-mention / 纸笔贴显示器边框 / 退回浏览器里开 To Do 网页版。不够好的原因：记录点分散（手机记的电脑看不见）、没有到点触达、事后全部无法汇总。⚠️ 本小节来自 journey 假设推导（未跑 Probe），列为待验证假设之一。

### 为什么是现在

- 竞品格局：滴答清单把桌面常驻+真提醒做成了决定性卖点但捆绑云与订阅；Things 3 交互天花板但不进 Windows；To Do 免费却功能单薄且中文支持残缺（微软 NLP 仅英文）——「Windows 桌面 + 离线 + 有复盘出口」三合一位置空置（bench 全部 8 条 takeaways）；
- 用户侧：2025-2026 评测社区反复出现订阅疲劳与「回归本地/买断」的声音（知乎/横评多条原话）；
- 工程侧：Tauri v1→v2 迁移窗口期，本地重构成本最低。

### 本 PRD 不解决什么

1. **跨设备同步与移动端**（brief.out_of_scope；用「便携目录+导出」兜底，不建云）；
2. **团队/协作/多人共享**（个人单机定位，To Do/滴答的协作线不跟）。

---

## 3. Personas & User Segments

### Primary — 小柯

```
姓名：小柯
身份：上班族 / 备考学生；Windows 笔电（含 1366×768 小屏）为主战场
情境：写作/网课/办公时任务随机冒出；早晨需要 1 分钟定当日计划；
      周五要交周复盘或周报；夜里常切暗色模式
JTBD：当脑子里冒出一件事时，小柯想用比打开手机备忘录更快的速度把它记下来，
      到点被真正提醒，周末一眼看到一周做完了什么——而不是被工具反过来管理。
当前 workaround：手机便签随手记（电脑看不到）、纸贴、微信发给自己
痛点（journey 证据）：首启被悬浮面板+假控件劝回（S2）；记录 4+ 步、时间漂移、
      记完搜不回（S3 双断点）；提醒不响→工具价值崩塌（S4）；一周成果无法复盘（S5）
```

### Secondary — 无（单角色产品）

本产品买家=使用者，无 B2B 式双 persona。**团队 leader「消费小柯产出」的场景被刻意排除**（云共享不在版图中，周报由导出文件承担）。

### 这个产品不为谁做

需要跨设备（手机↔电脑）同步提醒的人、项目管理/团队协作场景、习惯打卡与番茄钟全家桶用户。---

## 4. Goals & Success Metrics

**产品目标**（outcome-framed）：让小柯在**不打断手头工作**的前提下完成「记 → 排 → 醒 → 汇」全循环，且对数据的信任度达到「敢把重要的事托付给它」。

### 成功度量表

| 指标 | 度量什么用户行为 | 目标值 | 时间窗 |
| --- | --- | --- | --- |
| 🥇 捕获时延（主） | 热键唤起到落库的实际耗时（P50） | ≤ 3s，按键 ≤ 2 次 | 上线 30 天 |
| 🥈 捕获频次 | 周人均快录条数（用了才说明信任在建立） | ≥ 25 条/周 | 60 天 |
| 提醒触达 | 运行/托盘状态下到点必达率 | 100%（未运行态除外，见 S8 监控口径） | 每次版本回归必测 |
| 周汇总采用 | 打开/导出过周汇总的周活跃用户占比 | ≥ 40% | 90 天 |
| 数据可信 | 数据丢失工单数 / 崩溃后自校验失败率 | 0 / <0.1% | 持续 |
| 4 周留存（健康度） | 周活跃中第 4 周仍使用者（单人工具核心健康值） | ≥ 45% | cohort 90 天 |

**Anti-metrics（明确不优化）**：
- ❌ **提醒通知条数/打开率**——为凑活跃轰炸提醒会立刻毁掉「安静文具」的信任（bench avoid A-1 的教训）；
- ❌ **功能广度指标**（启用的模块数、清单层级深度）——滴答式"没它也行"的根源就是拿广度当卖点。

**关键假设（错了就做错产品）**：「每周自动汇总+导出」是真实高频刚需而非设计师自嗨——上游证据全部来自竞品用户差评外推（bench 评分、Things 区原话），**自身用户零验证**（未跑 Probe/Test）。对应 Story-6 打了 ⭐。

---

## 5. Value Proposition

**对小柯**：
> 小柯终于可以「在任何软件里一拍就记、到点必被提醒、周五两分钟交出周报底稿」而不必把数据交给云、学一套 GTD 软件或为会员墙付费——因为 v2 把**本地+免费+可追溯**当成产品主张本身，而不是功能附录。

**竞争差异化（具体版）**：
- 滴答清单赢在 7 端同步与提醒生态，留下「不愿登录/不想付费/只需要本机」的空白——v2 用**零账号、便携目录导出**正面填；
- To Do 赢在免费与我的一天仪式，留下「中文弱提醒（重复逻辑坑）+ 无日志无汇总」——v2 用**显式重复规则 + 自动周汇总**填；
- Things 3 定义了 When/Where 双轴与交互天花板，但不进 Windows、日志不可导（自家差评）——v2 在 **Windows 桌面做Things式粘性今天 + Things没做的导出**。
---

## 6. Solution & Feature Scope

> 按优先级降序（P0×6 → P1×2）。设计资产现状：`prototype/src/flows/shared/`（types.ts + mock-data.ts，含受限语法解析器 `parseCapture`）与 23 个 shadcn 组件已生成；6 个流程屏与 App Shell **待生成**（flow-web 中断于 Phase B 前）。实现宿主为 `src/`（vanilla）+ `src-tauri/`，本 PRD 以能力规格描述，不绑定原型技术栈。

### 6.1 三秒捕获：一键一行就记下 ⭐关键假设

**Persona**：小柯 ｜ **Job**：干活时冒出念头 → 3 秒落库 ｜ **优先级**：**P0**

**功能描述**：全局快捷键在任何应用中唤起一条极简输入浮条，打一行字回车即完成记录；行内的日期/时间/`#分类`/`!优先级`被识别为可点修正的解析标签；识别失败也照收，纯文本直接进收件箱。

**In Scope**：
- 全局热键（默认 `Alt+Shift+A`，可改，注册 Tauri global shortcut）→ 置顶输入浮条，回车落库、Esc 关闭
- 受限语法解析：仅 日期词 / 时间 / `#(工作|学习|生活)` / `!(高|中|低)` 四类；解析结果以 chips 呈现，点击可改，**1 秒后自动按 chip 值落库**
- 落库成功轻 toast（「已记下」+ 查看），失败明示「没有保存」+重试
- 「明天15:00」类输入落库后的截止时间必须等于字面值（本地时区，见 6.6 存储层统一）
- 捕获条常驻于悬浮面板底部（win:capture 与 float quick-add 复用同一实现）

**Out of Scope（本版不做）**：语音录入、多任务批量拆分、URL/选中文本捕获（浏览器扩展）、英文长句 NLP。

**验收标准（G/W/T）**：
- Given 任意前台应用，When 按下热键，Then ≤1s 输入条居中显示且有光标
- Given 输入「明天下午3点找导师 #学习 !中」回车，Then ≤2s 任务出现在收件箱，dueAt=明日 15:00（±0 分钟）、分类=学习、优先级=中，且解析 chips 与输入语义一致
- Given 输入「买牛奶」，Then 同样落库（无截止、分类=默认生活）且**无任何强制弹窗**
- Given 保存失败（磁盘满/IO 错），Then toast 变红「没有保存 · 重试」，输入内容不丢
- 边缘：热键冲突→设置页提示重设；空输入→直接关闭（静默丢弃）
- 空状态：n/a（捕获条天然无空态）

**设计触点**：屏 win://capture、panel:task（chip 修正态）、toast；状态 输入中·解析中·落库成功·解析兜底·保存失败；模式 quick-capture / form-flow
**关联 Sitemap 页面**：`win-capture` 快捕获条 · `page-inbox` /inbox ·（深链 `?open=:id`）
**已生成设计资产**：`prototype/src/flows/flow-1-capture/flow1-quick-capture.tsx`（3 屏：浮条 chips 实时/落库回执/收件箱高亮；error:save-failed、纯文本兜底、Esc 弃、maxLength200+truncate 内藏）+ `shared/mock-data.ts#parseCapture` 结构化契约版
**待澄清**：chip 自动应用的 1s 超时值是否符合「3 秒总时长」口径（Edge 阶段用真实输入延迟复测）

### 6.2 首次打开，三课就够 ⭐关键假设

**优先级**：**P0** ｜ **Job**：安装完成后 1 分钟得到一次完整成功体验

**功能描述**：首启以三步气泡带用户完成「按热键记一条 → 在今天看到它 → 把面板收进托盘」；每步可跳过且永不复现；预置演示任务（含假作者「李经理」备注）全部清除，收件箱从空白开始。

**In Scope**：三步引导浮层（步 1 强制实操：不记一条不放行下一步）；窗口控制实装（删自绘假按钮：`decorations:false` + 真 IPC 最小化/最大化/关闭，消除双标题条）；1366×768 无裁切（内容自适应 + 侧栏折叠 <1280）；引导完成态「以后随时在托盘找回我」。首启落地页 = /today。

**Out of Scope**：视频教程、功能导览手册、FAQ 页（用一次性三气泡替代长期文档）；主窗默认可见策略保持（float 为可选接待，默认首启后隐藏主窗、面板驻桌面）。

**验收标准**：
- Given 全新安装首次启动，When 完成三步，Then 全程 ≤90s、收件箱出现用户刚记的真任务、「李经理」类假数据 0 残留
- Given 引导任意步骤，When 点跳过，Then 该步与后续步不再出现，功能仍可用
- Given 窗口缩小至 1366×768（含），Then 无横向滚动条、主操作全部可达；When 原生窗口按钮点击，Then 真实响应
- 边缘：热键被占用无法实操步 1 → 引导提供「点这里代替按键」的按钮路径

**设计触点**：overlay://onboarding、win://capture、win://float、page-today；状态 步1-3·已跳过·首次成功·热键失败降级；模式 onboarding/progressive disclosure
**关联 Sitemap 页面**：`overlay-onboarding` · `win-capture` · `win-float` · `page-today`
**已生成设计资产**：`prototype/src/flows/flow-2-first-run/flow2-onboarding.tsx`（5 屏：欢迎→实操→看今天→收托盘→Done；skip-persistence=localStorage dismissed、breakpoint-resume=KEY_STEP、热键冲突免快捷键主按钮三态齐；生产迁 Tauri store）
**待澄清**：「主窗隐藏+悬浮面板接待」是价值还是困惑——假设 v1 已证伪（audit-15/3），但**引导后默认形态**（面板 or 主窗）建议 Beta 灰度二测

### 6.3 每天的「今天」是挑出来的 ⭐关键假设

**优先级**：**P0** ｜ **Job**：早晨 1 分钟形成可信的当日清单

**功能描述**：「今天」不再等于"到期日=今天"：任意列表的一键「加入今天」写入 plannedDate；昨天未完成自动粘留并带「拖了 N 天」标记；逾期收进一条折叠行；三处（侧栏/悬浮面板/统计卡）今日进度数字同源。

**In Scope**：Task 模型新增 `planned_date`、`carried_from`（v1 数据一次性迁移：老数据按 due_date 生成初始计划，见 6.6）；今天视图三段式（今日到期/已拖到今天/逾期折叠行，默认折叠显示计数）；加入今天动作（行内按钮 + 键盘 `t`）；数据同源化（单一 selector，消灭 v1 三套口径 audit-11/12）；今日完成庆祝态（当日最后一条完成时轻量动效，1 秒内）。

**Out of Scope**：四象限视图、日程时间块（calendar scheduling）、GTD 完整「将来也许」。

**验收标准**：
- Given 任务未计划，When 在其所在视图点「加入今天」，Then ≤1 次点击即出现在 /today 且刷新后仍在
- Given 昨天有 2 条未完成，When 今日首次打开，Then 此 2 条自动出现在「已拖到今天」并显示「拖了 1 天」
- Given 存在 5 条逾期，When 打开 /today，Then 5 条不可见但折叠行显示「已逾期 5 项」，展开后每条可选：改截止/移今天/丢弃（丢弃进软删，可撤销见 6.6）
- Given 任意完成/勾选操作，Then 侧栏角标、面板计数、统计卡三处数字相等且等于"今日口径"
- 边缘：全部完成后今天视图显示完成庆祝空态而非空白

**设计触点**：page-today（3 段）、page-inbox/page-planned（动作复用）、win-float（今日剩余摘要）；状态 晨间确认·空今天·全部完成·逾期折叠；模式 daily planning
**关联 Sitemap 页面**：`page-today` /today · `page-inbox` /inbox · `page-planned` /planned
**已生成设计资产**：`prototype/src/flows/flow-3-today/flow3-today.tsx`（三段式+逾期折叠逐条处置+空今天+全完成+慢盘骨架+写失败行内重试；字段同 `types.ts` 定稿）
**待澄清**：「自动粘留」上限 N 天需产品确认（建议 7 天后静默移入「回顾·逾期」防列表僵尸化）

### 6.4 到点了，真的会响 ⭐关键假设

**优先级**：**P0** ｜ **Job**：离开应用也被提醒，敢把重要的事托付出去

**功能描述**：任务 remindAt 到点弹 Windows 系统通知，点击经深链直达该任务详情，通知带「稍后 10 分钟 / 今天晚些」；悬浮面板支持三形态切换（置于顶层 / 嵌入桌面 / 迷你条）；v1 面板里的装饰性计时卡要么真实运行（倒计时+专注时长入库）要么删除——不允许第三种。

**In Scope**：Tauri notification 集成 + 到点调度（**应用未运行时必达**：Windows 计划任务注册 CLI 触发 vs 常驻托盘二选一，工程决策见 S8 前置）；提醒语义显式化（每天/每工作日/每周X/每月X日 + 「逾期不重复」开关，规避 To Do「日期漂移」反例）；一条任务 ≤3 个提醒点；免打扰时段（默认 23:00-07:30 可关）；win:float 三态 + 迷你条「今日剩 N · 下一条 HH:MM」；snooze；「管理提醒」入口从面板直通 `/planned/reminders`（v1 孤岛页归位，audit-4）；v1 假控件清零（audit-3/9/10 在本 story 内销账）。

**Out of Scope**：应用完全未启动且未注册计划任务时的必达承诺（明示限制）、邮件/短信渠道、位置提醒。

**验收标准（G/W/T）**：
- Given 设了 16:30 提醒且应用在托盘，When 到点，Then ≤5s 弹系统通知；点击后主窗打开且该任务详情选中（深链 ?open=）
- Given 在通知上点「稍后 10 分钟」，Then 16:40 再次触达；同任务 snooze 重触发 ≤3 次/小时防骚扰
- Given 面板切到迷你态，Then 高度 ≤56px、仅两行信息、无滚动（极小面积承诺）
- Given 完全未运行 + 未注册计划任务，When 下次启动，Then 立即补发「错过 1 条提醒」（承诺边界收口）
- 边缘：系统通知权限未开 → 设置/通知页一键跳 Windows 通知设置（ms-settings:）而非静默失败

**设计触点**：系统通知（外部）、win://float 三态、page-reminders、page-notify；状态 临期·到点·稍后·权限缺失·未运行补发；模式 notifications/ambient
**关联 Sitemap 页面**：`panel-task` · `page-reminders` /planned/reminders · `page-notify` /settings/notify · `win-float`
**已生成设计资产**：`prototype/src/flows/flow-4-reminders/flow4-reminders.tsx`（通知卡+snooze×2 档/三形态 Tabs/权限缺失卡/未运行补发/提醒管理卡含 invalid-date 双选与 99+0 边界演示）；`types.ts#ReminderEvent`（pending/fired/snoozed 已定义）· **A3 批复：默认托盘常驻=生产机制唯一确定项**
**待澄清**：常驻 vs 计划任务的功耗取舍需工程拍板（建议默认托盘常驻 +「节能模式」降级计划任务）

### 6.5 一周的完成不蒸发：自动周汇总 ⭐差异化主轴

**优先级**：**P0** ｜ **Job**：周五两分钟拿到周报底稿

**功能描述**：完成的任务不再消失——写入带完成时刻的日志簿（按日分组可回看），每周五自动生成可编辑「本周汇总」草稿（完成/顺延/逾期处置 + 分类分布 + 全明细），一键导出本地 md/csv。

**In Scope**：完成事件回写 `done_at`（v1 迁移期缺省 date-only + legacy 标记）；`/review/log` 按日时间线（完成时刻精确到分）；草稿规则：周五 07:30 生成（可关），条目默认全勾选、取消勾选不入导出；导出 md 模板 = 本周完成 / 顺延未完成 / 下周候选（拖留≥3 天 Top5），csv = 全明细，UTF-8 明文、路径可选；周/月翻页。**首版不做 AI 生成文本**（明文聚合即竞争力）。

**Out of Scope**：年报、OKR 对齐、团队汇总、AI 润色周报（Phase 3 再议）。

**验收标准（G/W/T）**：
- Given 本周完成 23 条，When 打开 /review/log，Then 23 条全部可查、每条含完成时刻（到分钟）
- Given 到周五（或空态点「立即生成」），Then 草稿三桶数字满足：完成数=实际数、分类分布合计=总数、顺延数=carried≥1 未完成数
- Given 取消勾选 1 条后导出，Then md/csv 均不含该条；二次导出不覆盖首版文件
- Given 导出成功，Then toast 显示完整路径 +「在文件夹中显示」；文件可被任意文本编辑器打开
- 边缘：空周（0 完成）→ 温和空态文案，不出现除 0 崩溃

**设计触点**：page-log、page-log-date、page-weekly、page-weekly-detail；状态 空周·首版草稿·编辑中·导出成功·跨月长列表；模式 review-loop
**关联 Sitemap 页面**：/review 命名空间全 5 节点（page-review/log/log-date/weekly/weekly-detail）
**已生成设计资产**：`prototype/src/flows/flow-5-review/flow5-weekly-review.tsx`（三桶统计+条目勾选导出+只读盘失败横幅/换个位置重试+空周预览+日志簿分组）+ `mock-data.ts#seedWeeklyDraft/lastWeekLogs` 基线
**待澄清**：「下周候选」排序权重（拖留天数 vs 优先级）；导出是否需要贴图版（Beta 收集）

### 6.6 点错的能回来，装坏了也不丢（信任底座 + 存储安全层）

**优先级**：**P0**（两条 Audit blocker 的对应项） ｜ **Job**：误删可撤销、永远有退路

**功能描述**：删除＝软删 + 10s 撤销 toast；每次写盘前自动轮转备份（最近 5 份可查可回滚）；数据损坏显示显式恢复页而非空应用；写入失败明示重试。承载存储层技术统一：**本地时区 ISO 8601 + 原子写（tmp+rename）+ v1 数据一次性迁移**（根除 audit-6 时间漂移与 audit-2 静默覆盖）。

**In Scope**：`deleted_at` 软删 + 撤销窗口（子任务/备注完整还原）；写前备份 `data.json.bak-N` 轮转 5 份；损坏恢复页列备份（时间+任务数）一键回滚，回滚前损坏文件另存 `data.corrupt.<ts>`；**JSON 解析失败禁止渲染为空态**（audit-2 病灶）；datetime 全链路本地语义（`YYYY-MM-DDTHH:mm`，杜绝 toISOString 混用）；导入导出=同一备份机制的用户出口；回收站入口（/settings/data 内，30 天自动清理软删）。

**Out of Scope**：逐条版本历史 diff、多库管理、云备份。

**验收标准（G/W/T）**：
- Given 任意视图删任务，When 10s 内点「撤销」，Then 原视图原位原状态恢复（含子任务/备注/计划标记）
- Given data.json 被外部截断，When 启动，Then 出现恢复页（≥1 份可读备份及其时间），绝不出现「你还没有任务」
- Given 编辑任务保存 15:00，Then 重开仍为 15:00（v1 漂移 8h 回归用例入 CI）
- Given 模拟写失败（只读目录），When 勾选任务，Then 显式「没有保存 · 重试」且勾选态回滚
- 边缘：全部备份不可读 → 恢复页提供「损坏文件位置 + 从导出重建」指引，留存日志

**设计触点**：toast-undo、modal-recovery、page-data；状态 undo 窗口·损坏恢复·写失败重试·备份轮转；模式 error-prevention & recovery
**关联 Sitemap 页面**：`page-data` /settings/data · `panel-task`（横切三视图）
**已生成设计资产**：`prototype/src/flows/flow-6-recovery/flow6-recovery.tsx`（撤销 10s 倒计时/备份五档列表/损坏恢复/写失败重试/备份全坏升格兜底屏含复制路径）+ **新增 `flow-boot/flow-boot-migration.tsx`＝F0 迁移失败三选屏**（owner Gate 最高优先 must，PRD v1 未立、rev.2 补录：批准口径=旧文件永不删+回 v1 护资产+空库需明示）+ `types.ts#BackupSnapshot/DataHealth`
**待澄清**：备份轮转 5 份 → 数据量增大后改「10 份 / 总 5MB」双上限；v1 日期兼容矩阵进 QA 用例池

### 6.7 打字就能找回来（搜索，P1）

**功能描述**：主窗列表获焦时直接开始打字即触发全局搜索浮层，分组返回进行中/日志簿命中，关键词高亮、回车直达；先把 v1「监听存在但过滤未接」的断线修通。

**In Scope**：标题+备注+子任务全文匹配（本地）；输入即结果（debounce 150ms）；`/` 显式聚焦搜索框（与打字即搜并存，兼容鼠标用户习惯）；无结果空态含建议词。
**Out of Scope**：模糊纠错、拼音首字母、运算符语法（`due:week` 类）——P2。
**验收标准**：输入 2 字符 ≤1s 出结果并高亮；日志簿条目可命中并深链过去；Esc 关闭恢复原焦点。
**设计触点**：overlay-search、panel-task ｜ **关联页面**：`overlay-search` ｜ **资产（rev.2 补）**：`prototype/src/flows/flow-search/flow-search.tsx`＝F7（debounce/慢检索骨架/两型空态区分/高亮直达）

### 6.8 暗色真的能暗，窗口缩了还能用（P1）

**功能描述**：建立 CSS 变量 token 层，三窗（主/面板/汇总）+ 系统通知跟随明暗；最小窗宽 1024 内布局自适应、侧栏 <1280 折叠为图标栏；杜绝组件新增内联颜色。

**In Scope**：token 语义层（对照原型 `prototype/src/index.css` 的 slate/diverge 色板：纸白底 40 33% 98% + 石墨主色 240 6% 10%）；主题切换即时跨窗同步并持久化；对比度自查（正文 ≥4.5:1，交付前由 Access/Check 复审）；v1 `audit-13`（切 dark 不可读）作为回归用例。
**验收标准**：切暗色后三窗无任何白底黑字残留面板；1024 宽主流程可完成；重启保持主题。
**设计触点**：三窗肤后暗态、page-appearance ｜ **资产**：`prototype/src/index.css` ✅ 已生成（token 基线，可直接移植）

### 6.9 页面级状态覆盖矩阵（scenario 强制 9 态 × 关键页）

> 依据 Productivity Tool scenario 的「状态覆盖要求」+ story design_touchpoints 汇总。**R=已在本 PRD 给定规格 / E=Edge 阶段待细画 / 空=不适用**。此矩阵是 `/异常态（Edge）` 的输入清单。

| 页面 | Initial/空 | Loading | 提交中 | 成功 | 错误 | 草稿/输入中 | 折叠/溢出 | 权限缺失 |
| --- | --- | — | — | --- | --- | --- | --- | --- |
| win://capture | — | — | ✅ 回车禁用重复落库 | ✅ toast「已记下」 | ✅「没有保存·重试」(6.6) | ✅ chips 实时解析(6.1) | 超长单行省略 | 热键被占(6.2) |
| page-today | ✅ 完成庆祝/空清单(6.3) | 本地即开 | 勾选乐观+回滚(6.6) | 划掉+计数-1 | ✅ 写盘失败 toast(6.6) | — | ✅ 逾期折叠行 | — |
| page-inbox | 引导文案「记第一条就在按 Alt+Shift+A」 | 同上 | 同上 | 同上 | 同上 | — | 长列表分段渲染 | — |
| page-reminders | 空态+「从任务设置提醒」CTA(6.4) | 同上 | 同上 | 同上 | 同上 | — | 重复规则展开预览 | — |
| /review/log | ✅ 空周温和文案(6.5 AC5) | 同上 | — | — | 读损坏 → 6.6 恢复页 | — | 跨月虚拟列表(6.5 E) | — |
| page-weekly-detail | 「立即生成」空态 | 生成中 skeleton | 导出中按钮 loading | ✅ 导出路径 toast(6.5 AC4) | 导出目标不可写明示 | ✅ 勾选即编辑态 | 31 天分组折叠 | — |
| overlay-search | 打字即显结果 | debounce 150ms | — | ✅ 高亮直达 | 本地检索异常兜底列表 | ✅ 结果随击键刷新 | 分组截断「查看全部」 | — |
| overlay-onboarding | 步1 强制实操 | — | — | 完成撒花(轻量 1s) | 热键失败→按钮降级(6.2 E) | — | 1024 宽气泡避让 | — |
| modal-recovery / page-data | 无备份极端态(6.6 E) | 备份扫描 | 回滚中禁用双击 | ✅ 回滚完成+损坏件留存路径 | 5 备份全坏→指引(6.6 AC5) | — | 备份多行滚动 | 目录只读(6.6) |
| 通知（系统） | — | — | — | 到点卡片 | 权限未开→设置深链(6.4 E) | snooze 计数 | Windows 通知分组折叠 | ✅ ms-settings 跳转 |
| win://float 三态 | 迷你条下限 56px | — | — | 勾选即时同步 | 30s 轮询→事件总线单源(6.1 触点) | quick-add 同 capture | 溢出滚轴 no-scrollbar | — |

（✅=规格已定 · E=Edge 待细画——与 thin_sections 3 呼应）

### 6.10 既有设计资产清单（截至本 PRD 签发）

```
prototype（设计验证原型，Fitd14/Kettd@v2 分支 prototype/）
├── src/index.css                      ✅ 明暗双态 token（diverge 纸白+石墨色板，story-7 直接移植）
├── tailwind.config.js / components.json ✅ slate 主题基建
├── src/components/ui/*.tsx            ✅ shadcn 23 件（button/dialog/tabs/select/sheet/toast…）
├── src/hooks/use-toast.ts + toaster  ✅ 撤销 toast 行为基线（story-8 交互原型同源）
├── src/flows/shared/types.ts          ✅ Task/ParseChip/ReminderEvent/WeeklyDraft/BackupSnapshot/DataHealth
├── src/flows/shared/mock-data.ts      ✅ 种子任务/上周日志/周汇总草稿/备份列表 + parseCapture 语法样例
├── src/flows/flow-1..6/*.tsx          ✅ rev.2 全部交付
├── src/flows/flow-boot/ · flow-search/    ✅ 新增（edge Gate 补课 F0/F7）
└── src/flows/kettd-app/kettd-app.tsx      ✅ Shell（8 路由+侧栏+主题切换，入口 src/main.tsx BrowserRouter）
```

v1 生产基线（同仓库 master）：`src-tauri/src/main.rs`（持久化与 17 命令）为 6.6 改造对象；`src/index.html|float.html|scheduled.html` 为被替代视图。机读链路：`spark-output/context/{brief,stories,sitemap,journey,audit,bench}.json`。

### 未列入本版的 P2 / Backlog

嵌桌面形态功耗调优（透明+事件穿透细节打磨）· 周报分享图自动生成 · 便携版（AppImage/exe portable）· 全键盘流扩展（j/k）· 多语言。

---

## 7. Constraints & Risks

**技术约束**（brief.constraints）：沿用 Tauri(v1→建议升 v2 迁移另立项) + 原生前端，不引入重框架；单人项目节奏，范围克制；Windows 优先且 1366×768 必须可用；v1 存量 data.json 迁移**零丢失**为硬门槛。
**设计约束**（brief.design_criteria.qualitative）：三处「今天」语义一致；**所有数字可追溯、所有控件皆有实行为**（v1 装饰性 UI 全面清零，audit-3/9/10/11）；术语单表执行（sitemap.naming_table：提醒/计划/悬浮面板），代码内命名走英文 key 禁止中文值硬映射。
**数据与隐私**：全量数据本機 `%APPDATA%/todo-list/data.json` 及其备份，不出网、无遥测（**反指标承诺的技术形态：不埋任何用户行为遥测**，度量全部本地统计）；合规风险极低，仅需在导出模板中不内嵌第三方品牌字样。

### 非功能要求（设计主张的量化面）

| 维度 | 要求 | 验收来源 |
| --- | --- | --- |
| 冷启动 | 双击图标 → 悬浮面板可交互（中配笔电/机械盘保守值；基准测 SSD） | 对治 Things「2-3s 白屏」差评（audit 对照） |
| Tauri v1 基线 | 保持 v1 运行时交付（升级 v2 另立项，见 S8）；若实测冷启 >2.5s 或 200 任务渲染 >100ms，触发 v2 升级议题提前 | Wave-1 性能基线 spike（待测） |
| 捕获热路径 | 热键→浮条 ≤1s；勾选→落库+三窗同步 ≤300ms | brief B1 |
| 常驻内存 | 托盘+面板常态 ≤300MB | bench A-3（体积重量是差评源） |
| 键盘兜底 | 5 键可用核心循环（Esc/Enter/t/↑↓/⌫）；无纯快捷键强依赖 | story-1/2/3 AC |
| 崩溃面 | 任意写失败/损坏均不得出现「空应用」假象，必达恢复页 | 6.6 AC2 |
| 可访问性底线 | 明暗双态对比度 ≥4.5:1；焦点可见；迷你态可点击穿透/锁定开关 | 目标值待 Access 阶段实测（Check/Access 未跑，无实测数据） |

### v1 → v2 数据迁移规格（6.6 的落地附件）

| v1 字段/格式 | v2 目标 | 规则 |
| --- | --- | --- |
| `due_date: "2026-08-29 18:00"`（空格）/ `"…T18:00"` | `dueAt: "2026-08-29T18:00"`（本地语义） | 两种输入均接受；禁止任何 UTC 换算路径 |
| date-only（无时分） | 保留 date-only 语义 | 提醒默认 09:00 可选延后 |
| `completed: true` 无完成时刻 | `doneAt: date-only + legacy:true` | legacy 在日志簿灰点+悬浮说明 |
| category「工作/个人/学习」（v1：工作/个人/学习） | 「工作/学习/生活」 | 「个人→生活」重映射；旧分类筛选行归并 |
| 侧栏视图枚举（inbox/today/upcoming/completed） | plannedDate 空 | 迁移时一次性生成初始计划：today 集内且未完成→补 plannedDate=today，附 `migrated:true` |
| 全量 data.json | 同路 `data.v1.json` 原件 | 永不删除；异常时 6.6 恢复页引用 |

### 关键风险表

| 风险 | 可能性 | 影响 | 缓解措施 |
| --- | --- | --- | --- |
| ⭐「周复盘是真需求」假设错（唯一无自证的主轴押注） | **H** | **H** | 最便宜测试：Beta 10 人 2 周，看草稿打开率/导出率（度量表周汇总采用 ≥40% 的先行版）；不达标则降级为纯日志簿 |
| 应用未运行提醒必达方案选错（计划任务 vs 常驻） | M | H | S8 Gate：先做 spike 各 0.5 天验证唤醒可靠性再定；产品承诺措辞按结论收口 |
| v1→v2 数据迁移丢时间/串格式（audit-6 病根变体） | M | **H** | 迁移脚本 + 兼容矩阵入 CI；迁移前强制留 `data.v1.json` 原件永不删 |
| 单人 8 Story 全面 P0 → 范围失控 | M | M | 按 S8 三波发版；每波有独立可回退分支（v2 分支纪律：master 未动，替换需二次确认） |
| Audit 已识别但未修复的设计问题（check 未跑）：搜索空态文案、迷你条溢出、撤销 toast 与通知叠层 | — | M | **Release Gate：先跑 `/设计走查（Check）` 对 6 个 flow 屏走查 + `/异常态（Edge）` 补状态矩阵**，再进 QA |
| 「永不收费」承诺后的维护压力（口碑反噬风险） | L | M | 明示本地开源可选路径（读 bench：社区对「跑路转订阅」高度敏感） |

**journey 流失点对照**：v2 若首启引导（S2）或捕获（S3）任一未达标，前 48h 双 dropout-risk 原样保留——本 PRD 的 6.1/6.2 即为这两点的工程化回收。

---

## 8. Release Approach

**推荐顺序（三波，每波含一个 ⭐假设测试）**：
1. **Wave-1 底座+首因**：6.6 存储安全 → 6.1 三秒捕获 → 6.2 首启三课。理由：6.6 是其余全部 story 的写盘前提；6.1/6.2 对应旅程 S2→S3 双断点，先赢进门前 48 小时（测试假设：捕获摩擦说 & 落地形态说）。
2. **Wave-2 日循环兑现**：6.3 今日计划 + 6.4 真提醒 +（6.8 暗色 token 并行，成本低）。测试假设：主动计划仪式说 & 常驻+必达留存说。
3. **Wave-3 差异化武器**：6.5 周汇总。放最后**但先做 10 人灰度**——它是全 PRD 唯一未自证的主轴押注；6.7 搜索随 Wave-3 捎带（修 v1 底线 bug）。

**MVP 定义**：6.1 + 6.2 + 6.3 + 6.4 + 6.6（=「记-排-醒-回」闭环 + 信任底座）。**MVP 可发布但没有 6.5，仍只是「更好的 To Do」——6.5 转正才构成定位差异**，故 Wave-3 前营销物料不承诺周报功能。

**Phase 2**：上表 P2 Backlog。延后理由：全部依赖 Wave-1/2 数据与形态稳定；嵌桌面形态打磨需要真实功耗数据。
**上线考虑**：
- 前置通知：GitHub watchers 与 v1 存量用户（README changelog 预告「不会丢数据」为第一承诺）；
- **（owner 批复 A1/A4）** 改为 **owner dogfood**：无社区 Beta、无问卷——Gate 指标由本机本地统计自测（4 周自用）；周汇总不达即降级纯日志簿，不对外承诺；
- 头两周盯（自用口径）：捕获时延 P50、提醒补发日志（**提醒默认=托盘常驻已定，A3**；计划任务仅节能降级）、撤销使用率（>15% 说明删除确认偏弱）、自迁移校验必须 0 丢失；
- 分发前红线：若未来走社区分发，Gate 数据缺失即不得上线周汇总承诺（Ask4 批复的延期条款）；
- 回退预案：master 保持 v1 可用直至 v2 迁移零事故确认后才执行替换（**任何对远端 master 的合并需用户二次授权**）。

**埋点规格（本地统计，非遥测）**：`capture_saved{latency_ms, chips_used}`、`undo_used{context}`、`reminder_fired{source:tray|schtasks}`、`weekly_draft_opened{auto|manual}`、`export_done{format}`——字段命名与 Metric 蓝图待 `/设计度量` 阶段对齐（当前 PRD 表 4 度量所需字段已全部覆盖，无缺）；实现为本地 events 表（7 天滚动，用户可在 /settings/data 查看与清空）。

---

> ⚠️ **薄弱章节提示（thin_sections）**
> 1. ~~Section 6 资产路径~~ ✅ rev.2 闭环：9 条路径全回填（含新增 F0/F7/F6′ 兜底件），coding agent 直接按文件开工即可。
> 2. **Section 3 workaround 小节**：基于 bench/知乎横评外推，未跑 Probe——上线灰度时加一题「你之前用什么记」（来源选择题）即可低成本补证。
> 3. **Section 7 check/edge 销账（rev.2 更新）**：Check 5 findings 全处理（B1/M1 修复、3 minor 随实现落）；edge must 16/16 已原型化。风险表#5 转为 QA 对照项。
> 4. **发布模式（v1 笔误修正）**：PRD 首签写『Beta 10-20 人 + Gate ≥40%』——owner 批复（Ask A1/A4）改定为**个人 dogfood**：无 Beta、无问卷，Gate 指标由本机 events 自测满 4 周判定；**周汇总不达→降级纯日志簿；分发决定前不得拿自用数据充当用户研究结论**（延期条款）。
