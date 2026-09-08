# 设计验收报告 · 多便签（Multi-Sticky）

- **验收目标**：多便签全量（`multi-sticky-spec.md` 定稿规格 + 单张便签规格回归）——实现端 `src-react/src/sticky/*`、`SettingsView` 便签卡、Rust `models/store/runtime/commands` 便签面
- **验收时间**：2026-09-08 21:05
- **验收模式**：模式 A 自动比对（代码级逐条核对）+ **对比度计算实测** + **真机使用痕迹取证**
- **上游**：check.json 5 条 findings 反向核对 · brief 定性标准 · sticky-note-component-spec / multi-sticky-spec / sticky-background-pattern / sticky-row-ui-polish 四份规格

---

## 总览

| 严重度 | 数量 |
| --- | --- |
| 🔴 Blocker | **0** |
| 🟠 Major | **6** |
| 🟡 Minor | **4** |

**通过维度**：typography（雅黑/system-ui · strip 字距 0.02em · 常规字重）· assets（lucide SVG 图标 + currentColor + aria 齐全）· responsive（N/A：桌面固定便签窗，实测 bounds 380×456 与规格 §3 一致）

**实测结论（先说总判）**：多便签**符合定稿设计的主体语义**——分页镜像、两类便签、生命周期（收起/删除/上限）、ADR-0007 注册表、四套纸色、38% 淡化、6 项埋点全部落地，且**有真实使用实弹**（见下）。不达标项集中在：**牛皮纸/淡青的次级文字对比度**（可量化修复）、暗色下用户纸色被吞、14 天清理建议未实现、以及阴影/间距未走 token。

## 真实使用痕迹（实际效果取证，2026-09-08 20:03–20:44）

| 证据 | 值 | 说明 |
| --- | --- | --- |
| `runtime.json notePos` | 4 键 | `sticky`（1 号）+ 3 张动态 `note:*` |
| 窗口 bounds 实测 | `[2029,647,380,456]` | 与 `notePos.sticky` 记忆**完全一致**（重启回位 ✓）且尺寸正中规格 §3 |
| stickies 数据 | 3 张 todo 便签 `pinned:true, hidden:true` | 创建→钉住→收起 全生命周期真实发生（AC1/AC3 实弹） |
| 埋点 | `sticky_create/close/count` 均已接线 | H1 验证门仪表就位 |

---

## Deviations（按维度分组，严重度排序）

### 可访问性 (Accessibility) — 2 项（本轮对比度实测数据）

1. **[major] qa3-01** 牛皮纸 muted 对比不足
   - 设计源：便签规格 §8「正文常态对比 ≥4.5:1」（check findings[4] 要求实测）
   - 实现：`.paper-kraft` 未覆盖 `--sticky-muted`，继承基础 `hsl(30 8% 40%)`
   - 差异：**muted/纸 3.55:1、muted/胶条 3.03:1**（胶条 12px 文字 + 完成划线 + 空态全命中）
   - 修复建议：`.paper-kraft` 增加 `--sticky-muted: hsl(28 22% 26%)`（实测对纸 6.14 / 对胶条 5.23，双达标）
   - 位置：`src-react/src/sticky/sticky.css:39-43` · 关联 Check finding：findings[4]

2. **[major] qa3-02** 淡青 muted 对胶条不足
   - 差异：**muted/胶条 3.89:1**（<4.5）；muted/纸 4.50 恰好踩线
   - 修复建议：`.paper-cyan` 独立覆盖 `--sticky-muted`（约 hsl(195 22% 30%)，按同法复算至双达标）
   - 位置：`sticky.css:44-48` · 关联 Check finding：findings[4]

> 正文（ink）四套纸色 **14.01 / 8.33 / 10.59 / 11.06 全部 ≥4.5 ✓**；琥珀文字问题已被上轮自修解决（墨字 + 琥珀浅底徽章 + 琥珀底图钉 chip，sticky.css:151-155/211-220 有明确对比度注释）。

### 颜色 (Color) — 1 项

3. **[major] qa3-03** 暗色下用户纸色选择被吞
   - 设计源：规格 §12.1「dark 下默认落到暗墨，**但用户可覆盖**」
   - 实现：`.dark .sticky-note`（sticky.css:57）恒定覆盖 paper/strip/ink
   - 差异：暗色下显式选择 kraft/cyan 无效（恒为暗墨）——规格「可覆盖」半句未实现
   - 修复建议：dark 且 `stickyPaper==='warm'`（默认未选择）才落暗墨；显式选择则尊重用户
   - 位置：`sticky.css:57-63`

### 状态完整性 (State Coverage) — 2 项

4. **[major] qa3-04** 14 天清理建议未实现
   - 设计源：multi-sticky-spec §3 + §6 AC4「14 天未动自由便签出现清理建议」
   - 实现：清单无任何 14 天/清理建议逻辑（全仓 grep 无命中）
   - 修复建议：清单渲染时 `updatedAt` 超 14 天且 `!pinned && kind==='free'` → 行内「建议清理」标记
   - 位置：`SettingsView.tsx` 便签清单段

5. **[minor] qa3-07** 首帧无加载区分
   - 实现：boot 未返回时 `total=0` → 渲染空态文案（闪空态，与真空空态不可分）
   - 修复建议：`boot===null` 渲染骨架行而非空态
   - 位置：`StickyWindow.tsx:222-226`

### 圆角与阴影 (Radius & Shadow) — 1 项

6. **[major] qa3-05** 阴影未走规格点名 token
   - 设计源：规格 §5「圆角（--r-lg）+ 柔和阴影（**--shadow-md**）」
   - 实现：自定 `box-shadow: 0 6px 24px hsl(...)`；`--shadow-md` 在 index.css 已存在未用
   - 修复建议：改 `var(--shadow-md)`（亮暗随 token 翻转）；圆角 `--r-lg` 已 ✓
   - 位置：`sticky.css:21,55`

### 间距 (Spacing) — 1 项

7. **[major] qa3-06** 间距裸 px（8/10/12/6/2px），`--sp-*` token 覆盖率 0%
   - 修复建议：换算至 --sp 档位；便签窗已加载 index.css，token 可直接用
   - 位置：`sticky.css` 全文件

### 交互态 (Interaction States) — 1 项

8. **[minor] qa3-08** 自由便签 spec 为「contentEditable **单段**纯文本」，实现为 textarea（可多行）
   - 功能超集、实现更稳；建议保存时折叠换行，或回写规格
   - 位置：`StickyWindow.tsx:208-218`

### 存储与规格一致性 — 1 项

9. **[minor] qa3-09** stickies 存 `data.json`（`AppData.stickies`）而非 spec §2 的独立 `stickies.json`
   - **deviation 但推荐反向修规格**：并入 data.json 使便签获得轮转备份保护，优于 spec 原意（spec 的红线「不进 runtime.json」仍满足）
   - 位置：`models.rs:419`

### 组件挂载 (Assets) — 1 项

10. **[minor] qa3-10** 规格 §7 的进度条（shadcn Progress）未挂，进度以「做完 N」文字承载（信息无损、形式缺失）
    - 位置：`StickyWindow.tsx:194-200`

---

## 有意偏离（规格自身已留账，不算 deviation）

| 项 | 规格留账 |
| --- | --- |
| 托盘新建入口未做（设置页入口已覆盖） | multi-sticky-spec 实施落点「H1b 留账」 |
| blur 轮询统一 | §5「H1b：另开 ADR 记录」 |
| float 窗未并入全动态注册表（仍为 conf 静态窗 + id=sticky） | §1「迁移兼容」+ H1b 留账 |
| 数据门语义（设计先行，发布后验证） | §0 owner 拍板记录 |

## Check Finding 核对（5 条）

| Check Finding | 状态 |
| --- | --- |
| findings[0] story-5 AC2 三态切换上游未回写 | ❌ 未解决（**文档侧**；实现已按定稿执行） |
| findings[1] window.prompt 在 wry 静默无效（qa2-04） | ✅ 已解决（src-react 无 prompt；两步内联确认/行内排期替代） |
| findings[2] 规格 §7 宋体 vs §5 无衬线矛盾 | ⚠️ 部分（实现正确跟随 §5；规格文档未改） |
| findings[3] 老化术语三处不一（qa2-20） | ✅ 已解决（便签 badge 已统一「顺延 N 天」） |
| findings[4] 四纸色对比度无实测 | ⚠️ 部分（本轮实测：正文 4/4 过、琥珀已修；kraft/cyan muted 残留 → qa3-01/02） |

## 规格验收标准核对（multi-sticky-spec §6）

| AC | 结论 | 证据 |
| --- | --- | --- |
| AC1 N 张并存/独立记位/重启回位 | ✅ | notePos 4 键 + bounds 实测一致 + 3 张真实创建 |
| AC2 自由便签点击即写/失焦保存/不进今天 | ✅（代码级） | textarea + commitFree + ≤500 + 字数桶埋点（真机尚无 free 样本） |
| AC3 清单实时/删除确认/收起展开 | ✅ | 新建 ▾ + 清单 + confirmDel 两步确认 + hidden=true ×3 |
| AC4 第 7 张拦下 / 清理建议 | ⚠️ | 拦下 ✅（STICKY_CAP Err 就地提示）；清理建议 ❌（qa3-04） |
| AC5 外观全局生效 | ✅ | paper/pattern/fade 全部读全局 settings |
| AC6 旧单张升级零感知 | ✅ | id=sticky 迁移兼容、notePos 键不动、置顶/纸色保留 |

## 修复优先级建议

- **必须修复（影响可读性/规格 AC 的 Major）**：qa3-01、qa3-02（对比度，一行 token 各一处）、qa3-04（AC4 缺半）——共 3 项
- **建议修复（其余 Major）**：qa3-03（暗色纸色可覆盖）、qa3-05（阴影 token）、qa3-06（间距 token）——共 3 项
- **可延后（Minor）**：qa3-07/08/09/10 + 文档回写 2 项（story-5 AC2、规格 §7 宋体行）——共 6 项

> ⚠️ 局限声明：本环境无屏幕捕获且验收时桌面锁屏，**像素级目视**（淡化动效观感、花纹实贴效果、暗色实切）未覆盖；对比度结论为按实际 CSS 值的 WCAG 公式计算，非屏幕取样。运行中窗口的无障碍树与窗口 bounds 已实测。
