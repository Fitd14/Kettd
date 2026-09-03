# Kettd v2 · 设计验收报告（8 项发版前调整）

**验收目标**：悬浮面板精简×4 / 快速记录窗口 / 打开主界面全局热键 / 计划页时间组件 / 主窗口文案精简
**验收时间**：2026-09-03T21:56:03+08:00
**验收模式**：自动比对（模式 A · 静态代码核查）
**设计源基线**：需求方 8 项调整要求 + brief rev.2 交付基线 + styles.css token 唯一色源 + V2-API.md 契约
**实现产物**：`src/`（float/capture/index/styles/api）+ `src-tauri/`（models/store/runtime/commands/main/tauri.conf.json）

## 总览

| 严重度 | 数量 |
| --- | --- |
| 🔴 Blocker | 0 |
| 🟠 Major | 0 |
| 🟡 Minor | 4 |

**通过维度**（无 deviation）：间距 · 颜色 · 字体 · 圆角阴影 · 图标资源 · 响应式 · 可访问性
**待运行时确认**（静态无法证伪，需重启原生应用人工核对）：快速记录条透明观感 · Alt+Shift+O 实际呼出
**Check finding 解决率**：4 已解决 / 5 总（1 项维持 open，见下表）

## 8 项需求逐项核对

| # | 需求 | 结论 | 证据（file:line） |
| --- | --- | --- | --- |
| 1 | 悬浮面板标题去掉「悬浮面板」 | ✅ | tauri.conf.json float 窗 `"title": "待办列表"`；float.html:5 `<title>待办列表</title>` |
| 2 | 快速填写只留「记下…」 | ✅ | float.html:19 `placeholder="记下…"` |
| 3 | 记下按钮改图标（原竖排） | ✅ | float.html:20 `.btn.icon` 内联加号 SVG（15px、currentColor、aria-label）；styles.css `.btn.icon{flex:none;30×30}` 修复挤压竖排 |
| 4 | 去掉形态描述与打开主界面 | ✅ | float.html footer 已删；`formHint`×0、`float-foot`×0（html+css 均无残留） |
| 5 | 快速记录窗口去滚动条、透明 | ⚠️ 静态就位 | conf:100 `"height":120` + runtime.rs:157/206 同步 520×120；styles.css:265 `html.cap…{background:transparent;overflow:hidden}`；`"transparent":true` 配置链路完整。**实际透明观感需重启确认** |
| 6 | 打开主界面快捷键 | ✅（代码链路） | models.rs:16 `DEFAULT_MAIN_HOTKEY="Alt+Shift+O"`；Settings.mainHotkey→store patch→runtime 双键注册/互斥/回滚→commands `register_main_hotkey`→main.rs 启动注册→api.js:53→设置页换绑/解绑 UI；cargo check EXIT=0。**实呼待运行时确认** |
| 7 | 计划页时间改选择组件 | ✅（语义见 qa-4） | index.html:601/602/613/614 行内编辑与添加行均为 `type="date"`+`type="time"`；空时刻拦截「选一个提醒时刻」 |
| 8 | 主窗口提示精简 | ✅ | `kbdhint`×0；侧栏收敛 1 行（index.html:14）；「同源/悬浮面板里也能记/睡前想记/快捷键 N」等文案已删（行 215 命中为代码注释非 UI） |

## Deviations（4 项，均 Minor）

### 交互态 / 状态完整性 — 4 项

1. **[minor] 启动热键注册失败静默，「当前绑定」可能失真（qa-1）**
   - 设计源：brief「控件皆有实行为」
   - 实现：Alt+Shift+O 被占用时启动注册静默放弃，设置页仍显示已绑定
   - 修复建议：失败时事件通知前端标注「上次绑定未生效」或托盘提示一次
   - 位置：src-tauri/src/main.rs setup 热键线程
   - 关联 Check finding：—（沿用捕获热键既有模式）
2. **[minor] 时间错误文案仍是手输格式教学（qa-2）**
   - 设计源：需求 7（选择而非手输）
   - 实现：「提醒时间格式不对，用 HH:MM 或 …」仅剩兜底路径触发
   - 修复建议：改「请选一个未来时刻」类短句
   - 位置：src/api.js validateReminderTime；index.html ACTS['rem-fix-raw']
3. **[minor] 旧版多时刻提醒编辑会降级为单时刻（qa-3）**
   - 设计源：V2-API.md 时间约定（HH:MM/HH:MM 多值合法）
   - 实现：多值进入行内编辑时选择器置空，保存即覆盖为单值
   - 修复建议：检测多值先提示，或该行回退文本输入
   - 位置：index.html ACTS['rem-edit'] / ['rem-edit-save']
4. **[minor] 需求 7 范围增益：日期+时刻双组件（qa-4）**
   - 设计源：需求 7 字面为「时间组件」
   - 实现：日期留空=每天循环，选日期=一次性（保留原双语义）
   - 修复建议：请需求方确认语义；只要纯时刻可移除日期组件
   - 位置：index.html viewPlanned

## 9 维度核查结论

| 维度 | 结论 | 要点 |
| --- | --- | --- |
| 间距 | ✓ 通过 | 新增仅控件宽度（150/140/100/96px），沿用仓内既有 inline 宽度模式；无新增裸 spacing |
| 颜色 | ✓ 通过 | 零新增 hex；图标用 currentColor，色源仍 styles.css 20 色对 |
| 字体 | ✓ 通过 | 全部复用既有类（.tiny/.sm/.mono），无新字号 |
| 圆角阴影 | ✓ 通过 | `.btn.icon` 用 var(--r-md) token |
| 图标资源 | ✓ 通过 | 内联 SVG 矢量 15px、currentColor 可控 |
| 交互态 | ✓ 通过* | 新按钮/输入继承 hover/active/focus-visible 三态（:active 全仓 8 处）；*见 qa-1 |
| 状态完整性 | ✓ 通过* | 空时刻拦截、行内错误、回执路径保留；*见 qa-2/qa-3/qa-4 |
| 响应式 | ✓ 通过 | 桌面固定窗：计划页添加行 flex-wrap 换行，1024 最小宽内可用（brief 约束 1366） |
| 可访问性 | ✓ 通过 | 新增 4 控件均有 aria-label；focus-visible 全局规则覆盖；对比度实测仍 open（见下） |

## Check Finding 核对（check.json 5 项）

| Check Finding | 状态 |
| --- | --- |
| B1: 6 flow 无实现 | ✅ 已解决（生产层 Wave-1+2 全量交付，38 命令接线 + cargo check 绿） |
| M1: parseCapture 自相矛盾 | ✅ 已解决（api.js 结构化返回 title/chips/dueAt/autoRemind/fellBack） |
| #3: 脚手架残留 | ✅ 已解决（src/ 仅 5 个产物文件，无模板残留） |
| #4: active 按压态全仓 0 次 | ✅ 生产层已解决（styles.css 8 处 :active） |
| #5: muted 对比度临界（4.4-4.6:1） | ⚠️ 维持 open（需 /无障碍检查 实测，超出本 QA 静态范围） |

## 待运行时确认清单（重启应用后人工核对）

1. Alt+Shift+O 全局呼出主窗口；设置页换绑/解绑即时生效
2. 快速记录条（520×120）无滚动条、四周透出桌面
3. 悬浮面板标题栏显示「待办列表」；快速添加按钮为图标不换行
4. 计划页时间选择器选取→回车/添加 落库，到点提醒时间正确

## 修复优先级建议

- 必须修复（Blocker + 影响主流程 Major）：**0 项**
- 建议修复（Minor）：4 项（qa-3 涉及旧数据覆盖，建议最先处理）
- 待需求确认：1 项（qa-4 语义确认）
