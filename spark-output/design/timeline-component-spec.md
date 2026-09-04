# 组件规格 · CompletionTimeline（完成时间轴 · 横向 · Ant Timeline 风格）

- 状态：**定稿（2026-09-04）** · 供 M3 实现 · 父页面：回顾页（`review-redesign-spec.md`）
- 一句话：一条**默认内嵌、横向可滚动、按完成时间倒序**的完成事件时间轴；视觉与 API 对齐 **Ant Design Timeline v6**，但用 shadcn token 自研，不引入 antd 整包。

---

## 1. 设计决策（先定这条）

- **为什么自研而非 `import { Timeline } from 'antd'`**：ADR-001 已把全站统一到 **shadcn/ui**（单一设计系统、token 一致）。仅为一个时间轴引入 antd 会破坏「组件统一」并增加包体/主题双轨成本。→ **用 shadcn token 手写一个 Ant-Timeline 兼容组件**，视觉与 props 语义对齐 Ant v6，未来若整体迁 antd 可无缝替换。
- 若你更看重"直接用官方组件、接受引入 antd"，此项可推翻——但默认按自研走。

## 2. 对齐 Ant Design Timeline v6 的解剖

| Ant v6 概念 | 本组件对应 |
|---|---|
| 容器 `orientation="horizontal"` | 固定横向 |
| 容器 `mode="start"` | 时间标签统一在轨道一侧（上方） |
| `itemRail`（轨道） | 水平连接线（`--border`） |
| `itemIcon`（节点标记） | 分类色圆点（工作红/学习绿/生活蓝；`legacy` 灰空心） |
| `item.title`（单独展示的标签） | **完成时刻 `MM-DD HH:mm`**，置于轨道上方 |
| `item.content` | 任务标题 + 分类 chip，置于轨道下方 |
| `item.loading` / `pending` | 底部/右端「加载更早」节点 |
| `item.icon`（自定义） | 预留：逾期项可换图标（当前不展示逾期，留扩展位） |

## 3. 组件 API（建议签名）

```ts
type TimelineItem = {
  id: string
  doneAt: string        // RFC3339，排序与展示锚点
  title: string         // 任务标题
  category: '工作'|'学习'|'生活'
  legacy?: boolean      // 无 doneAt 的迁移项：本组件不渲染（由页面过滤）
}
type CompletionTimelineProps = {
  items: TimelineItem[]                 // 已按 doneAt 倒序（最近在前）
  onItemSelect?: (id: string) => void   // 点节点 → 打开任务详情
  onLoadMore?: () => void               // 滚到右端 → 加载更早
  loading?: boolean                     // 右端 loading 节点
  emptyLabel?: ReactNode                // 无完成时的引导
  errorLabel?: ReactNode                // 读取失败
}
```

## 4. 视觉与密度 token（对齐高信息密度偏好）

- 轨道线：`--border`，1–2px；节点圆点直径 ~12px，间距**紧凑**（节点步进 ~90–110px，随视口）。
- 时间标签：`--muted-foreground`，11–12px，`MM-DD HH:mm`（同一天多条仍带时分，保证可区分）。
- 任务标题：`--foreground`，13–14px，**不加粗过度**；分类 chip = 小色块 + 11px 文字（非颜色唯一载体）。
- 无优先级、无准点/逾期标签（定稿）；节点保持单行信息密度。
- 暗色：全部走 `--*` 变量，`.dark` 自动切换。

## 5. 行为

- **默认排序**：`doneAt` 倒序，**最近在最左**；首屏自动定位到最左（最新）。
- **横向滚动**：外层 `overflow-x: auto`；右端**渐隐遮罩 + 「…更早 →」**提示可继续；滚到接近右端触发 `onLoadMore`，右端追加 `loading` 空心节点。
- **今天锚点（可选增强）**：若最新一条 = 今天，最左节点上方标「今天」小徽，帮助定位。
- **点击**：`onItemSelect(id)` → 复用既有任务详情弹层/路由。
- **hover**：Tooltip 显示完整标题 + 完整时间戳（`YYYY-MM-DD HH:mm`），应对标题被 ellipsis 截断。

## 6. 状态机

| 状态 | 表现 |
|---|---|
| default | 横向节点流 |
| loading（分页中） | 右端空心脉冲节点 + 「加载更早…」 |
| empty | 居中引导「完成第一件事后，这里会按时间长出你的执行轨迹」+ 去「今天」按钮 |
| error | 「读取完成记录失败」+ 重试按钮 |

## 7. 响应式

- 组件本身**始终横向滚动**（不受窗口宽限制节点数）；窗口 `minWidth:1024` 下轨道区可用宽 ≥ ~800px。
- 窗口拖宽 → 视口内可见节点增多（步进不变，露出更多，右侧渐隐相应右移）。
- 与父页热力图共用「横向滚动容器」心智，避免整页横向溢出。

## 8. 可访问性（WCAG 2.1 AA）

- 容器 `role="list"`，节点 `role="listitem"`；每个节点是可聚焦 `button`（`onItemSelect`）。
- 键盘：Tab 进入、`←/→` 或 `Home/End` 横向滚动与跳节点、`Enter` 打开详情；`focus-visible` 环（`--ring`）。
- 时间用 `<time datetime>` 语义；分类信息除颜色外有文字 chip（色盲冗余）。
- 圆点/文字与背景对比 ≥3:1（非文本）/ ≥4.5:1（文本）。

## 9. 验收标准

- AC1 排序：Given 乱序完成数据，When 渲染，Then 按 `doneAt` 倒序、最近在最左。
- AC2 横向滚动：Given 节点超出视口，When 向右滚，Then 触发 `onLoadMore` 并在右端出现 loading 节点，无整页横向破版。
- AC3 下钻：Given 点某节点，Then 打开对应任务详情。
- AC4 空/错：Given 无完成 / 读取失败，Then 分别显示 empty 引导 / error + 重试。
- AC5 a11y：键盘可遍历与滚动，颜色非唯一信息载体，对比达标。
- AC6 一致性：全部颜色/圆角/间距取自 shadcn token，与回顾页其余部分无视觉分叉。

## 10. 依赖与工时（M3 估）

- 依赖：shadcn `Tooltip`（含 Provider）、`ScrollArea`（或原生 overflow-x）、`Button`、`Skeleton`；lucide 图标（`Clock`/`Inbox`）。
- 自研 HeatmapGrid 另计。CompletionTimeline 本体估 **~3–4h**（含横向滚动、分页 loading、a11y、空/错态）。
