# 组件规格 · 复古便签（StickyNote · 悬浮窗重设计）

- 状态：**定稿（2026-09-04）** · 供 M3 实现 · 取代现有「悬浮窗 float」
- 关联：`spark-output/context/brief.json`、`review-redesign-spec.md`、ADR-001（React 迁移）
- 边界：本文件只做设计规格，不含实现代码。

---

## 1. 决策摘要（本轮定稿）

1. 淡化触发 = **鼠标移出/移入窗口（mouseover / mouseout）**。
2. 移出目标不透明度 = **≈38%**（够淡、融入桌面但仍可扫读）；移入恢复 100%。
3. **固定 = 仅切换「置顶」**（always-on-top on/off），**位置始终可拖拽**，不锁位。
4. **彻底删除三形态**（topmost / desktop 嵌入桌面 / mini 迷你条）→ 只保留**唯一便签**。
5. 便签内**不再有内联输入框**；录入**完全走 `Alt+Shift+A` 捕获条**（已实现）。
6. 视觉 = **现代便签**（暖白偏米纸面 + 无衬线(雅黑) + 圆角 + 柔和阴影 + 小图钉 + 折角轻隐喻，保留便签代入感但**去掉重黄/宋体/红边/横格等强复古元素**）。

## 2. 形态收敛（对现有代码的影响，M3/或 v2.1 先做）

现状 `float_form ∈ {topmost, desktop, mini}`，本次收敛为单一便签：
- 删除 `models.rs::FLOAT_FORMS` 的三态语义、`set_float_form` 分支、`park_mini`、`float_should_hide_on_blur`（desktop 专属）、`ministrip`（float.html）、托盘「置于顶层/嵌入桌面/迷你条」子菜单。
- `Settings.float_form` 字段：迁移为 `sticky_pinned: bool`（默认 true=置顶）+ `sticky_pos: Option<[i32;2]>`（记位）。旧值一次性归一（topmost/desktop→pinned true；mini→pinned true）。
- 保留的只有：一个无边框便签窗（`FLOAT_LABEL`）。

## 3. 窗口规格

| 属性 | 值 | 说明 |
|---|---|---|
| `decorations` | false | 无边框，自绘胶条标题 |
| `always_on_top` | 由「固定」按钮切换（默认 true） | 固定=置顶；解除=沉到普通层（可被盖，更融入） |
| `resizable` | false | 便签不缩放 |
| `skip_taskbar` | true | 不占任务栏 |
| `transparent` | 视复古纸而定：纸面不透明，**四角/边缘可留透明**做卷边/阴影 | 若要真·便签异形边缘再评估 |
| 尺寸 | ~380×456（自适应内容高度） | 与骨架图一致 |

## 4. 交互

- **拖拽记位**：按住顶部胶条空白区移动（`data-tauri-drag-region` 或 `start_dragging`）；`mouseout`/关闭时持久化 `sticky_pos`（复用 capture 的记位 + 离屏守卫 `available_monitors`）。
- **移出变淡**：`document.mouseleave` → 加 `.faded`（`opacity:.38`，`transition:opacity 180ms ease-out`）；`mouseenter` → 去 `.faded` 恢复。移出后仍置顶、仍可扫读，不拦截桌面操作。
- **固定按钮**：切换 `always_on_top` + 更新图钉视觉（钉住/未钉）。
- **关闭按钮**：`hide` 到托盘（进程常驻，沿用现 `hide_float`），非退出。
- **勾选完成**：点击圆角方框 → `toggle_task`，标题置灰 + 删除线。
- **空态**：今天无待办 → 便签显示「今天没有待办 · `Alt+Shift+A` 记一条」。

## 5. 现代便签视觉 token（新增，进 styles.css :root/.dark）

```
--sticky-paper   亮:44 40% 96%（暖白偏米）  暗:40 12% 16%（暖灰纸，不跟随系统变冷）
--sticky-strip   顶部条，比 paper 深一档（极淡）
--sticky-accent  琥珀点缀（图钉/进度/选中）亮:38 62% 55%  暗:38 45% 45%
--sticky-ink     正文（近黑暖）  --sticky-muted 次要灰
--sticky-line    细描边（卡片/ghost 按钮）
```
- 字体：**无衬线（雅黑 / system-ui）**，标题常规字重 + 少量字距；不用宋体。
- 形态：圆角（`--r-lg`）+ 柔和阴影（`--shadow-md`）+ 小图钉 + 右下折角，作轻隐喻，不铺满复古质感。
- **去掉**红左边距线、横格线、手写下划线；勾选用现代圆角方框（选中填 `--sticky-accent` + 白色勾）。
- 分类色仍复用 `--cat-*`（信息一致）；暗色保持暖纸调，不转冷灰。

## 6. 数据契约（复用现有命令，无新增后端）

- 拉取：`get_bootstrap` / `get_tasks` / `get_reminders`（今天 pending + 下一条提醒 + 完成数/进度）。
- 写：`toggle_task`；录入不在便签内（走捕获条 `add_task source=capture`）。
- 埋点：`capture_open/commit` 已有；建议加 `sticky_toggle_done`（可选，本地）。

## 7. 组件映射（M3 · shadcn）

| 元素 | 组件 |
|---|---|
| 便签容器 | 自研 `StickyNote`（复古 CSS，非 shadcn Card） |
| 固定/关闭 | 图标按钮（shadcn Button ghost + lucide `Pin`/`PinOff`/`X`） |
| 待办行 | 自研（方框 checkbox + 宋体标题 + 分类 chip） |
| 进度 | shadcn Progress（复古描边覆盖） |
> 便签是独立 Tauri 窗，`float.html` → M3 重写为 React `StickyNote`；拖拽/记位/淡化/固定主要落在 **Rust 窗口层 + CSS**（与框架无关，可先在 v2.1 vanilla 做，React 化时继承，同 capture 便签套路）。

## 8. 可访问性

- 淡化态是"弱化可读"，交互（勾选/固定/关闭）要求移入恢复不透明后再操作；`focus-visible` 环保留。
- 勾选框可键盘操作（Tab + Space）；`aria-label` 齐全；分类信息除颜色外有文字（色盲冗余）。
- 38% 不透明为**装饰性弱化**，不作为常态可读文本；正文常态对比 ≥4.5:1。

## 9. 验收标准（Given/When/Then）

- AC1 形态收敛：Given 旧设置含 float_form，When 升级，Then 归一为便签（默认置顶 + 无 mini/嵌入桌面入口）。
- AC2 仅两键：Given 便签，Then 顶部只有 固定 / 关闭，无最大化/最小化/系统标题栏。
- AC3 固定=置顶可拖：切换固定只改置顶；任何时候都能拖动且记住位置，重启回原位。
- AC4 移出淡化：鼠标移出 → ≈38% 融入桌面；移入 → 恢复；不拦截桌面操作。
- AC5 录入：便签无输入框；`Alt+Shift+A` 捕获条可记一条并即时反映到便签清单。
- AC6 现代便签视觉：暖白偏米纸面 / 无衬线 / 圆角 / 柔和阴影 / 小图钉 + 折角，亮暗均保持纸质调。

## 10. 工时（M3 估）

- 形态收敛（Rust 删三态 + 设置迁移）~2h
- 无边框 + 固定/关闭 + 拖拽记位（Rust）~2h
- 移出淡化交互（CSS/JS）~0.5h
- 复古视觉 token + CSS ~2h
- 便签 React 化（M3）~2h
合计 ~8.5h（其中 Rust+CSS 部分可并入 v2.1 先落，React 化随 M3）。

## 12. 借鉴增量（来自 PaperTodo 拆解，已确认要加）

### 12.1 预设纸色（改造：预设而非取色器）
- 便签提供 **4 套预设纸色**（对齐"桌面文具感"，走 token，不做自由取色器，避免设置爆炸）：
  - `暖白纸`（默认，`--sticky-paper` 亮 44 40% 96%）
  - `牛皮纸`（更暖更深一档）
  - `淡青`（冷调护眼）
  - `暗墨`（低光/夜间，文字转浅）
- 每套只改 3 个变量：`--sticky-paper / --sticky-strip / --sticky-ink`；分类色 `--cat-*`、强调 `--sticky-accent` 不随纸色变（保证信息一致）。
- 选择入口：设置页一个「便签纸色」四选一分段控件（Tabs/RadioGroup），非取色器。**这是唯一新增的设置项**，符合"少即是多"。
- 与系统 theme 关系：纸色独立于 light/dark（dark 下默认落到"暗墨"，但用户可覆盖）。

### 12.2 待办拖拽排序（行业标配）
- **范围**：便签清单 + 主界面「今天」列表，支持鼠标拖拽调整顺序。
- **数据**：Task 增加 `sort_order: i32`（同视图内可比较）；拖拽结束调用新命令 `reorder_tasks(ids_in_order: Vec<String>)` 批量落库。默认序 = `sort_order`，未手动排过的按创建时间兜底。
- **交互**：拖动手柄 = 行左侧 grip（或整行可拖，输入/勾选区排除）；拖拽中半透明占位 + 落点指示线；键盘可达（选中 + Alt+↑/↓）。
- **不做**：跨视图拖拽（今天↔收件箱）本期不做，避免范围蔓延；仅同列表内排序。
- **副作用控制**：`sort_order` 是新增可选字段，v1 迁移缺省→按 created_at 兜底，不破坏"零丢失"。

### 12.3 明确不加（守住定位）
- ❌ 图状双向链接、自由取色器、鼠标穿透、一键置底、绑定第三方窗口、边缘胶囊折叠（先观察淡化态够不够）。

## 13. 工时增补
- 预设纸色（token + 设置分段控件）~1h
- 待办拖拽排序（`sort_order` + `reorder_tasks` 命令 + 前端拖拽 + 键盘）~3h（便签与今日视图共用）
