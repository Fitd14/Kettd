# Design

## Context

阅读面现状（行号以 2026-11-17 的 master 为准）：

- 纸面与墨色落在 `.kb-detail` 子树：亮色 `src-react/src/views/kb.css:243-251`，暗色 `.dark .kb-detail` `:293-301`。正文靠 `.kb-detail { color: var(--ink-1) }`（`:274`）继承。**全量口径下这些默认声明要上移到 `:root` / `.dark`**，否则元素上的默认值会遮住根上的主题值。
- 外壳令牌在 `src-react/src/index.css:10-107`（`:root`）与 `:109` 起（`.dark`）：`--background` `--foreground` `--card` `--card-foreground` `--popover` `--popover-foreground` `--primary` `--primary-foreground` `--secondary` `--secondary-foreground` `--muted` `--muted-foreground` `--accent` `--accent-foreground` `--border` `--input` `--ring`，另有 `--link`、`--seal` 与语义色 `--cat-*` / `--pri-*` / `--destructive`。颜色值是**不带 `hsl()` 的 HSL 三元组**（如 `40 33% 98%`），消费处写 `hsl(var(--background))`。
- 字号与字距由 `:root[data-reading-size]` / `[data-reading-tracking]` 驱动（`kb.css:354-371`），列宽由 `--kb-measure-cap` 决定（`:380`）；三者属用户档位。
- 标题倍率硬编码：编辑器 `prosemirror.css:124-133`（1.35 / 1.2 / 1.05 / 1 / .93 / .88），只读渲染 `:572-577`（1.5 / 1.3 / 1.15 / 1.05 / .98 / .92）。行高有两个基线：编辑器 `.kb-editor-content` 1.85（`:119`）、只读 `.kb-md-render` 1.65（`:557`）；标题行高 1.3（`:569`）。
- 正文元素目前多数不吃纸面令牌：引用块 `prosemirror.css:199-204`（编辑器）与 `:620-625`（只读）用 `var(--text-muted, #ccc)` / `var(--text-secondary, #666)`，这两个变量全仓未声明；代码块 `:176/187/588` 用 `var(--code-bg, rgba(0,0,0,.04))`；编辑器表格 `:212/217` 用 `hsl(var(--foreground)/.38|.2)`；只读表格 `:603/609` 用 `var(--border)` / `var(--hover-bg)`；链接 `:138-147`、`:627-629` 用 `hsl(var(--link))`；图片圆角 `:639`、`:844` 写死 4px。这些是本次要接的线。
- `--paper-deep`（`kb.css:244`）全仓无消费者，属死令牌；本次不纳入白名单，避免「写了不生效」的假能力。
- 字体三态先例：`src-react/src/lib/reading-font.ts` 写 `--font-reading`；上传字体命令 `src-tauri/src/commands.rs:499-532`；落盘模式 `src-tauri/src/infra/fonts.rs`（校验扩展名与文件头、tmp+rename）。
- 明暗切换：`src-react/src/lib/api.ts:619-621` 只切 `<html class="dark">`；`set_settings` 在主题变化时 `emit_all("theme-changed")`（`commands.rs:474`）。当前监听方只有 `CaptureWindow.tsx:142`、`StickyWindow.tsx:271`、`TodoFloatWindow.tsx:45`，主窗仅在启动时 `applyTheme`（`App.tsx:99`）。
- 硬约束：AGENTS.md 要求颜色走令牌、命令四方同步（`main.rs` / `commands.rs` / `api.ts` / `docs/ARCHITECTURE.md` §2+§10）、持久文件原子写、错误文案中文短句。

## Goals / Non-Goals

**Goals:**

- 一个主题文件携带「元数据 + 亮/暗两套变量值」，可导入、切换、删除，当帧生效、重启保留。
- 主题覆盖阅读区**整套观感**：纸面与墨色、纸面几何（圆角/投影/纸纹）、字族、行高、标题（倍率/字距/底线）、引用、代码（块与行内）、表格（线/表头/斑马纹/圆角）、链接、图片圆角。目标是内置主题换上后与上游 Typora 主题的观感差距只来自字体缺失。
- 效果覆盖**全应用**：外壳配色 + 阅读区观感；字号/字距/列宽档位与页面布局不变。四个窗口（主窗、快速捕获、便签、待办悬浮）一致生效。
- 导入路径安全：拒外部资源、脚本类声明、超大文件；损坏时静默回退。
- 内置 `paperglow` 与 `mdmdt`（各含亮暗两套）作为完整样板。

**Non-Goals:**

- 不做在线主题商店、评分、分享。
- 不接受任意选择器与整份 Typora CSS（`h1 { … }`、`@media`、`@import` 一律不生效）；主题只能给变量赋值。
- 不改列宽、留白、页面边距、工具条布局（`knowledge-base/writing-layout` 的领地）；不提供主题编辑器。
- 不覆盖阅读字号、正文字距、列宽档位（用户档位优先）。
- 不把主题内容写进 `data.json`，v1 不进备份包。
- 不改语义色（`--cat-*` / `--pri-*` / `--destructive`）与快速捕获气泡的固定深底（`--bubble-*`）：危险、优先级、分类必须保持系统语义。

## Decisions

### D1 主题 = 变量声明子集，而非可注入的整份 CSS

主题文件是 `.css`，但只识别两个块：

```css
/* kettd-theme
name: 我的主题
author: 你
description: 暖纸 + 陶土；来源 cloud Liu/Paperglow（Apache-2.0），本地改写
*/
.kb-theme-light { --paper: #FCFAF6; --ink-1: #2B2621; --kb-code-radius: 16px; }
.kb-theme-dark  { --paper: #2B2926; --ink-1: #FAF8F3; --kb-code-radius: 16px; }
```

解析：去注释 → 按顶层 `{}` 切块 → 只接受选择器**恰好**为 `.kb-theme-light` / `.kb-theme-dark`（允许缺一）→ 块内只接受 `--name: value;`。不做选择器求值，因此没有样式泄漏路径，也无需引入 CSS 解析依赖。

- 备选 A：注入 `<style>`。否决——需重写全部选择器才能限定作用域，重写器本身是注入面，且 `@media` / `:root` 会污染全局。
- 备选 B：JSON 主题。否决——用户素材与心智都是 CSS，JSON 还得自定义单位与嵌套。
- 备选 C：引入 postcss / lightningcss 运行时。否决——为几十行声明引入解析器不划算，解析器只解决语法、不解决作用域。

### D2 应用方式：解析结果写到 `document.documentElement` 的内联自定义属性

`reading-theme.js` 暴露 `parseTheme(css)` 与 `applyTheme(el, theme, { dark, userFont })`：解析得 `{ name, author, light, dark }`，按当前明暗选套（缺省回落另一套，再缺则移除该属性回默认），逐条 `el.style.setProperty(name, value)`。切换主题、明暗变化、窗口挂载时重放；卸载时移除已写属性。

全量口径下 `el` 固定为 `document.documentElement`：`<html>` 上的内联自定义属性被整棵树继承，外壳与阅读区、弹层（portal 到 `body`）一并生效，也天然压过 `index.css` / `kb.css` 的同名默认声明，无需提高权重。`--font-reading` / `--font-reading-title` 仅在 `settings.readingFont` 为空串时写入，保证用户显式字体优先。

`--paper-font` 目前被 `prosemirror.css:118/185` 引用却全仓未声明，优先级高于 `--font-reading`；实现时一并删除该中间变量，避免日后有人声明它导致主题字族静默失效。

### D3 白名单：59 个变量，十一组

| 组 | 变量 | 消费者（实现落点） |
| --- | --- | --- |
| 外壳配色（19） | `--background` `--foreground` `--card` `--card-foreground` `--popover` `--popover-foreground` `--primary` `--primary-foreground` `--secondary` `--secondary-foreground` `--muted` `--muted-foreground` `--accent` `--accent-foreground` `--border` `--input` `--ring` `--link` `--seal` | `index.css` `:root` 与全站组件（取值是 HSL 三元组，消费处 `hsl(var(--x))`） |
| 纸面与墨色 | `--paper` `--ink-1` `--ink-2` `--ink-3` `--line-warm` `--seal-track` `--seal-ink` `--seal-soft` | `kb.css` `.kb-detail` 及其子树、工具条、slash 菜单 |
| 纸面形态 | `--kb-paper-radius` `--kb-paper-shadow` `--kb-paper-texture` | `.kb-detail` 本体 |
| 字族 | `--font-reading` `--font-reading-title` | 正文、标题 |
| 段落 | `--kb-line-height` | 编辑器与只读正文行高 |
| 标题 | `--kb-h1`…`--kb-h6` `--kb-head-tracking` `--kb-h1-rule` `--kb-h2-rule` | h1–h6 字号、字距、底线 |
| 引用 | `--kb-quote-bg` `--kb-quote-line` `--kb-quote-ink` `--kb-quote-radius` | blockquote 两处（编辑器 + 只读） |
| 代码 | `--kb-code-bg` `--kb-code-ink` `--kb-code-radius` `--kb-inline-code-bg` `--kb-inline-code-ink` `--kb-inline-code-radius` | pre / code / `.kb-code-block` |
| 表格 | `--kb-table-line` `--kb-table-head-bg` `--kb-table-zebra` `--kb-table-radius` | 编辑器表格与 `.kb-table` |
| 链接 | `--kb-link-ink` `--kb-link-underline` | 正文链接 |
| 图片 | `--kb-img-radius` | 正文图片 |

`--paper-deep` 不进白名单：它在 `kb.css:244` 有声明但全仓零消费，写进主题只会制造「设置了没反应」的假能力。保留其声明供后续布局变更使用，不在本变更加消费者。

取值校验（解析器与 `design-review/check-theme.mjs` 同一套规则）：

- 通用：值长度 ≤ 200 字符；禁 `url(`、`@`、`{`、`}`、`;`、`expression(`、`javascript:`、`<`、`>`、`!important`；禁 `var(--fs-`、`var(--ls-`、`var(--kb-measure`、`var(--kb-gutter`（防借变量名夹带档位）。
- 数值组：
  - `--kb-line-height` 1.3–2.4；`--kb-h1..h6` 0.8–2.0；`--kb-paper-texture` 0–0.12；`--kb-head-tracking` -0.02em–0.2em；`--kb-link-underline` 0–2px。
  - 圆角组：`--kb-paper-radius` 0–24px、`--kb-quote-radius` 0–16px、`--kb-code-radius` 0–16px、`--kb-inline-code-radius` 0–8px、`--kb-table-radius` 0–16px、`--kb-img-radius` 0–16px；允许 1–4 个 px 值（如 `0 8px 8px 0`），每值在区间内。
- 结构组：`--kb-h1-rule` / `--kb-h2-rule` 取 `none` 或 `<0–4px> <solid|dashed|dotted> <颜色>`；`--kb-quote-line` 取 `<0–6px> <solid|dashed|dotted> <颜色>`；`--kb-paper-shadow` 取 `none` 或 `box-shadow` 形态的值（以 `inset` 或 px 偏移起头，最多 4 组偏移 + 颜色）。
- 颜色组：任何 CSS 颜色（hex / `rgb()` / `hsl()` / `hsl(var(--seal))` 这类 token 引用均可）。
- 越界或不合语法：丢弃该条、计入「已忽略」计数，不整份拒收。

白名单由前端单点定义（`reading-theme.js` 的 `THEME_VARS`），与 `kb.css` / `prosemirror.css` 的消费点比对；新增可主题化变量必须同时改声明、消费点与校验表。

### D3a 无主题时像素不变

所有消费点一律写成「主题变量 → 现值」的 fallback 链，例如：

```css
/* 改造前 */ background: var(--code-bg, rgba(0, 0, 0, 0.04));
/* 改造后 */ background: var(--kb-code-bg, rgba(0, 0, 0, 0.04));
/* 引用线  */ border-left: var(--kb-quote-line, 3px solid var(--text-muted, #ccc));
```

fallback 一律照抄改造前的计算值（含 `var(--text-muted, …)` 这类既有回退），使「未选主题」与变更前逐像素相等。这条规则排除「顺手把硬编码换成共享令牌」——那会改变默认观感，属另一个变更。

验收方式：真机在默认主题下截图对比，或 DevTools 逐项记录改造前后 `getComputedStyle`（D7 表格里的默认值列即基线）。

### D4 文件约定、校验与存储

- UTF-8（去 BOM）、扩展名 `.css`、≤ 512 KB；文件名清洗为「去路径 + 去 `<>:"/\|?*` + 保留原名」，拒绝空名与超 64 字符。
- 后端 `save_reading_theme(name, data)` 收明文（CSS 是文本，不经 base64），校验大小、UTF-8、禁用串（`@import`、`url(`、`expression(`、`javascript:`），写临时文件后 rename 到 `<数据目录>/themes/`，与 `infra/fonts.rs` 同一模式。
- `list_reading_themes() -> Vec<ReadingThemeInfo { name, sizeKb }>`；`read_reading_theme(name) -> Result<String, String>`；`delete_reading_theme(name)`。
- 内置主题作为前端资源 `src-react/src/assets/themes/paperglow.css`、`mdmdt.css`，用 Vite `?raw` 引入，走同一 `parseTheme`；id 与后端 `READING_THEME_BUILTINS = ["paperglow", "mdmdt"]` 对齐（照 `READING_FONT_SYSTEMS` 的双份白名单先例，`models.rs:513`）。两套均为「按上游风格的令牌重写为 Kettd 白名单变量」，不整份复制，文件头注释写明来源、许可与改动。

### D4a 两套内置主题的基准值与换算规则

已写出（本轮按全量白名单扩写）。纸面与排版部分：

| 令牌 | Paperglow 亮 / 暗 | Mdmdt 亮 / 暗 |
| --- | --- | --- |
| `--paper` | `#FCFAF6` / `#2B2926` | `#FAFAFC` / `#1B1B1F` |
| `--ink-1` | `#2B2621` / `#FAF8F3` | `#070909` / `#D0D0D0` |
| `--ink-2` | `#61574F` / `#D5CDC0` | `#666666` / `#A6A6A6` |
| `--ink-3` | `#8A7D70` / `#A99D90` | `#8C959B` / `#7E8A92` |
| `--line-warm` | `#DDD5CA` / `#3D3936` | `#D2D2D2` / `#464B50` |
| `--seal-track` | `#BC6A3A` / `#D59567` | `#3E69D7` / `#4F76DF` |
| `--seal-ink` | `#9C5127` / `#E3A57A` | `#2F479F` / `#BBC7FD` |
| `--seal-soft` | `rgba(188,106,58,.12)` / `rgba(213,149,103,.16)` | `rgba(62,105,215,.12)` / `rgba(62,105,215,.20)` |
| `--kb-line-height` | `1.7` | `1.7` |
| `--kb-h1..h6` | `1.90 / 1.65 / 1.42 / 1.24 / 1.12 / 1.00` | `2.00 / 1.75 / 1.50 / 1.25 / 1.12 / 1.00` |

装饰与几何部分（上游特征的直接映射）：

| 令牌 | Paperglow 亮 / 暗 | Mdmdt 亮 / 暗 |
| --- | --- | --- |
| `--kb-paper-radius` | `16px` | `2px` |
| `--kb-paper-shadow` | `0 0 0 1px rgba(96,78,63,.08), 0 8px 20px rgba(70,52,38,.06)` / 同形换 `rgba(0,0,0,.28)` | `none` |
| `--kb-paper-texture` | `0.06` / `0.05` | `0` |
| `--kb-head-tracking` | `0em` | `0.06em` |
| `--kb-h1-rule` / `--kb-h2-rule` | `none` / `2px solid #DDD5CA` | `1px solid #D2D2D2` / `none`（暗 `1px solid #464B50`） |
| `--kb-quote-bg` | `#F3EDE5` / `#2C2926` | `rgba(62,105,215,.06)` / `rgba(62,105,215,.12)` |
| `--kb-quote-line` | `3px solid #C58C64` / `3px solid #6B5D50` | `4px solid #3E69D7` / `4px solid #4F76DF` |
| `--kb-quote-ink` | `#4A4039` / `#DDD2C6` | `#3A3A3A` / `#CFCFCF` |
| `--kb-quote-radius` | `0 8px 8px 0` | `8px` |
| `--kb-code-bg` | `#FCFBF8` / `#2A2725` | `#ECECEE` / `#282A32` |
| `--kb-code-ink` | `#1C1815` / `#F5EFE6` | `#1B1B1F` / `#E8E8EA` |
| `--kb-code-radius` | `16px` | `8px` |
| `--kb-inline-code-bg` | `#F2EDE7` / `#332E2B` | `rgba(62,105,215,.15)` / `rgba(62,105,215,.30)` |
| `--kb-inline-code-ink` | `#A3493F` / `#E1A092` | `#2F479F` / `#BBC7FD` |
| `--kb-inline-code-radius` | `6px` | `4px` |
| `--kb-table-line` | `rgba(188,106,58,.30)` / `rgba(213,149,103,.30)` | `#D2D2D2` / `#464B50` |
| `--kb-table-head-bg` | `rgba(188,106,58,.06)` / `rgba(213,149,103,.08)` | `#ECECEE` / `#282A32` |
| `--kb-table-zebra` | `transparent` | `#ECECEE` / `#282A32` |
| `--kb-table-radius` | `8px` | `8px` |
| `--kb-link-ink` | `#8F4B28` / `#E3A57A` | `#3E69D7` / `#6D8FF0` |
| `--kb-link-underline` | `1px` | `0px` |
| `--kb-img-radius` | `8px` | `8px` |

外壳配色（全量口径新增，取值是 HSL 三元组）：

| 令牌 | Paperglow 亮 / 暗 | Mdmdt 亮 / 暗 |
| --- | --- | --- |
| `--background` | `34 37% 93%` / `40 5% 11%` | `240 25% 98%` / `240 7% 11%` |
| `--foreground` | `30 13% 15%` / `43 41% 97%` | `180 13% 3%` / `0 0% 82%` |
| `--card` | `40 50% 98%` / `36 6% 16%` | `0 0% 100%` / `228 11% 18%` |
| `--card-foreground` | 同 `--foreground` | 同 `--foreground` |
| `--popover` | 同 `--card` | 同 `--card` |
| `--popover-foreground` | 同 `--foreground` | 同 `--foreground` |
| `--primary` | `22 53% 48%` / `25 57% 62%` | `223 66% 54%` / `224 69% 59%` |
| `--primary-foreground` | `40 50% 98%` / `30 3% 13%` | `0 0% 100%` / `240 25% 98%` |
| `--secondary` | `34 37% 93%` / `22 9% 18%` | `240 6% 93%` / `228 11% 18%` |
| `--secondary-foreground` | `30 13% 15%` / `43 41% 97%` | `180 13% 3%` / `0 0% 82%` |
| `--muted` | `33 30% 93%` / `22 9% 18%` | `240 6% 93%` / `228 11% 18%` |
| `--muted-foreground` | `30 11% 41%` / `31 16% 69%` | `0 0% 40%` / `0 0% 65%` |
| `--accent` | `31 47% 89%` / `26 14% 20%` | `217 72% 94%` / `222 15% 22%` |
| `--accent-foreground` | `22 60% 38%` / `25 65% 68%` | `227 54% 40%` / `229 94% 86%` |
| `--border` | `35 22% 83%` / `26 6% 23%` | `0 0% 82%` / `210 7% 29%` |
| `--input` | `30 12% 61%` / `31 11% 58%` | `204 7% 58%` / `204 8% 53%` |
| `--ring` | 同 `--primary` | 同 `--primary` |
| `--link` | `20 56% 36%` / `25 65% 68%` | `223 66% 54%` / `224 81% 68%` |
| `--seal` | `22 53% 48%` / `25 57% 62%` | `223 66% 54%` / `224 69% 59%` |

换算与改写规则（新增内置主题同用）：

1. **行高按行盒等价折算**：Paperglow 原 1.5（17px × 1.5 ≈ 25.5px）、Mdmdt 原 1.6（16px × 1.6 ≈ 25.6px），在 Kettd 15px 正文下都落到 1.7。
2. **标题倍率 = 原主题 hN 字号 ÷ 原正文基准字号**，上限 2.0；Paperglow 1.90 起，Mdmdt 2.00。
3. **装饰值直取上游语义量**：圆角、投影、斑马纹、引用线宽、代码圆角、链接下划线粗细都按上游原值写入；单位统一为 px / em，颜色统一转小写 hex 或 `rgba()`。
4. **对比度可偏离原值一档并留痕**：Mdmdt 暗色强调蓝 `#3E69D7` 在 `#1B1B1F` 上不足，提亮到 `#4F76DF`；Paperglow 暗色保留 `#D59567`。
5. **字族不做字体嵌入**：Montserrat 未随包分发，栈里保留名字并回落 Noto Sans SC / PingFang SC / 微软雅黑。
6. **文件头固定四行元数据**：`/* kettd-theme` + `name:` + `author:` + `description:`，说明来源仓库、许可（Apache-2.0）与本地改动。
7. **一致性门禁**：测试读取资源目录与 `READING_THEME_BUILTINS`，断言 id 集合一致、每个内置文件两套块齐全、按解析器 0 条忽略。
8. **评审件不进安装包**：`design-review/*-preview.html` 只用于人工确认观感。
9. **外壳取上游的界面底色**：`--background` 取窗口底、`--card` / `--popover` 取卡片底、`--border` 取上游线条色；上游 `--bg-color` 与 `--card-bg` 是「窗底 vs 浮纸」两层，Kettd 同样有两层（`.view` 底与卡片），不能合并成一个值。
10. **三元组化**：外壳颜色必须是 `H S% L%`（不带 `hsl()`，与 `index.css` 同形），十六进制只在阅读区令牌里出现；生成值用脚本从上游 hex 换算（`hsl()` 取整到 1% 即可，渲染色差 ≤1/255）。

### D5 设置字段与三态

`Settings.reading_theme: String`（默认 `""`，serde 补默认以兼容旧 `data.json`），`SettingsPayload.reading_theme: Option<String>`；`normalize_reading_theme` 只放行 `""`、`builtin:<白名单 id>`、`file:<清洗后的名>`，其余回 `""`。JSON 侧因 `rename_all = "camelCase"` 落为 `readingTheme`。

设置页「阅读主题」区：`Select` 列「默认」+ 两个内置 + 已安装文件主题，`Button` 触发隐藏 `type="file"`（AGENTS 允许的页面层例外），删除按钮只对 `file:` 出现；导入成功自动选中并落库。

### D6 命令四方同步

四条新命令加入 `src-tauri/src/main.rs` `generate_handler![]`、`commands.rs`、`src-react/src/lib/api.ts`、`docs/ARCHITECTURE.md` 附录 §2 与 §10；`test/command-parity.test.mjs` 必须绿。

### D7 消费点接线表

每行都要保留「改造前计算值」作为 fallback（D3a）：

| 元素 | 文件:行 | 改造前 | 改造后 |
| --- | --- | --- | --- |
| 编辑器行高 | `prosemirror.css:119` | `1.85` | `var(--kb-line-height, 1.85)` |
| 只读行高 | `:557` | `1.65` | `var(--kb-line-height, 1.65)` |
| 编辑器标题 | `:124-133` | `calc(var(--fs-reading) * 1.35)` … | `calc(var(--fs-reading) * var(--kb-hN, 默认))` |
| 只读标题 | `:572-577` | `1.5 / 1.3 / 1.15 / 1.05 / .98 / .92` | 同一组 `--kb-hN`，默认保持原值 |
| 标题字距 | `:124-133`、`:566-570` | h1–h5 无、h6 `calc(.3px + var(--ls-reading,0em))` | 统一 `calc(var(--kb-head-tracking, 0em) + var(--ls-reading, 0em))`，h6 再前置 `calc(.3px + …)` |
| h1/h2 底线 | 编辑器与只读 h1/h2 规则 | 无 | `border-bottom: var(--kb-h1-rule, 0)` / `var(--kb-h2-rule, 0)` + `padding-bottom: var(--kb-head-rule-gap, 0)` |
| 引用（编辑器） | `:199-204` | `border-left: 3px solid var(--text-muted,#ccc)`、`color: var(--text-secondary,#666)` | `var(--kb-quote-line, 3px solid var(--text-muted,#ccc))`、`var(--kb-quote-ink, var(--text-secondary,#666))`，补 `background: var(--kb-quote-bg, transparent)`、`border-radius: var(--kb-quote-radius, 0)` |
| 引用（只读） | `:620-625` | 同上 | 同上 |
| 代码块 | `:175-182`、`:587-593` | `var(--code-bg, rgba(0,0,0,.04))` | `var(--kb-code-bg, rgba(0,0,0,.04))`，补 `color: var(--kb-code-ink, inherit)`、`border-radius: var(--kb-code-radius, 6px)` |
| 行内代码 | `:184-190` | `var(--code-bg, …)`、`border-radius: 3px` | `var(--kb-inline-code-bg, rgba(0,0,0,.04))`、`var(--kb-inline-code-ink, inherit)`、`var(--kb-inline-code-radius, 3px)` |
| 编辑器表格 | `:207-217` | 外框 `hsl(var(--foreground)/.38)`、单元格 `/.2` | `var(--kb-table-line, hsl(var(--foreground)/.38))`、单元格 `var(--kb-table-line, hsl(var(--foreground)/.2))` |
| 只读表格 | `:595-610` | `var(--border)`、`var(--hover-bg)` | `var(--kb-table-line, var(--border))`、`var(--kb-table-head-bg, var(--hover-bg))`，新增 `tbody tr:nth-child(even){background:var(--kb-table-zebra,transparent)}` 与 `border-radius: var(--kb-table-radius, 0)` |
| 链接（编辑器） | `:138-147` | `hsl(var(--link))`、下划线 1px | `var(--kb-link-ink, hsl(var(--link)))`、`text-decoration-thickness: var(--kb-link-underline, 1px)`；0px 视为不画线 |
| 链接（只读） | `:627-629` | 同上 | 同上 |
| 图片 | `:637-640`、`:844` | `border-radius: 4px` | `var(--kb-img-radius, 4px)` |
| 纸面圆角 | `kb.css:275` | `2px` | `var(--kb-paper-radius, 2px)` |
| 纸面投影 | `kb.css:273`、暗 `:300` | `inset 0 0 90px rgba(90,80,60,.07)` / `rgba(0,0,0,.18)` | `var(--kb-paper-shadow, 各自的默认值)` |
| 纸纹强度 | `kb.css:270-272`、暗 `:298-299` | SVG 内 `opacity=.06` / `.04` | 纸纹搬到 `.kb-detail::before` 覆盖层：`background-image` 去色噪点、`opacity: var(--kb-paper-texture, .06)`，暗色默认 `.04`；图 `pointer-events:none`、`z-index:0`，正文 `position: relative` |
| 默认令牌落点 | `kb.css:243-251`、暗 `:293-301` | 声明在 `.kb-detail` 上 | 上移到 `:root` / `.dark`（与 `index.css` 同层），元素上不再有同名声明，根上的主题值才能被继承 |
| 主题应用（主窗） | `App.tsx:89-104` `refresh()` | 只重放亮暗、字体、阅读档位 | 增加读 `readingTheme` → `parseTheme` → `applyTheme(document.documentElement, …)`，并监听 `reading-theme-changed` |
| 主题应用（三窗） | `CaptureWindow.tsx:142`、`StickyWindow.tsx:271`、`TodoFloatWindow.tsx:45` | 只监听 `theme-changed` 切 `<html class>` | 各自读 `getSettings()` 并应用同一套令牌，监听 `theme-changed` 与 `reading-theme-changed` |

标题默认值按「编辑器/只读各留自己基线」：同一变量、不同 fallback（`--kb-line-height` 编辑器 1.85、只读 1.65），主题一旦给值两边统一。

### D8 失败回退

读取失败、0 条有效声明、或文件已删除：按默认外观渲染，正文不动；设置值**保留**（不静默清空），设置页该项标「主题不可用」并提供「切回默认」。理由：清空会让用户修好文件后还得重选，也与「损坏不吞正文」的约定冲突。

### D9 默认令牌上移到 `:root`

`--paper` / `--ink-*` / `--line-warm` / `--seal-*` / `--kb-paper-texture` 与纸面圆角投影目前声明在 `.kb-detail` 上。全量口径下主题值写在 `<html>` 的内联样式里，`.kb-detail` 上的默认声明会优先于继承值，主题对阅读区就失效。因此这些默认声明 MUST 上移到 `:root`（暗色移到 `.dark`），`.kb-detail` 只保留与元素本体有关的样式（`background`、`color: var(--ink-1)` 等消费规则）。

验收：无主题时逐项比对 `getComputedStyle(.kb-detail)` 的 `--paper` / `--ink-1` / `--kb-paper-texture` 与改造前一致。

### D10 四个窗口各自应用

主窗、快速捕获、便签、待办悬浮是四个独立 WebView，各有自己的 `document`。主题 MUST 由每个窗口自己读取同一份设置并写到自己的 `documentElement`；MUST NOT 依赖主窗广播的 DOM 状态。事件层面复用既有 `theme-changed`（明暗）与 `reading-theme-changed`（主题 id 变化），窗口挂载时各读一次 `getSettings()` 兜底。

失败回退与 D8 一致：读不到设置或主题文件损坏时，窗口按默认外观渲染并在下次事件到达时再试。

## Risks / Trade-offs

- 用户粘贴整份 Typora 主题，只有少数声明生效 → 导入提示「识别 N 条，忽略 M 条」；README 写清子集格式。
- 只给亮色套的主题在暗色下可能对比不足 → 允许一键切回；文档建议两套都给，不强制。
- 值语法允许 `var()` 与任意颜色 → 极端值可让纸面透明或墨色不可读；只做长度/危险串/区间校验，不试图做视觉校验。
- 装饰几何（大圆角、重投影）可能与布局边界打架：`--kb-paper-radius` 会让纸面角落露出底层背景；表格圆角需要 `overflow: hidden` 才不会让单元格刺出；`--kb-paper-shadow` 可能溢出到侧栏。→ 取值区间钳制 + 预览页人工确认 + 默认 `0 / none`；一旦出现溢出即收紧区间。
- 纸纹搬到伪元素会改变 DOM 层叠：正文与工具条都是 `position: relative` 以上，需要一次性核对 `z-index`（工具条 8、slash 60）。→ 实现任务里单列一条「层叠回归」，默认值下逐像素比对。
- `--kb-quote-*` 同时作用于编辑器与只读渲染，两处 DOM 不同（ProseMirror blockquote vs markdown-it blockquote）→ 接线必须成对改，测试两处各一根引用。
- 主题字族依赖本机字体，换机会回落 → 与现有系统字族行为一致，静默回落。
- 文件主题不进备份 → v1 明确不备份，README 记入限制。
- 主题值仍只赋给自定义属性，组件不新增硬编码色值 → 与「颜色走令牌」不冲突；但主题可为 `--kb-code-bg` 给任意颜色，等于绕过令牌体系，这是主题能力的固有代价。全量口径下影响面从阅读区扩到全应用，兜底手段改为：外壳只开放 19 个语义令牌、语义色与气泡底不开放、主题可一键切回默认。
- 外壳对比度（尤其暗色 `--muted-foreground` 与 `--border`）不好校验 → 内置主题按上游原值取、只在明显不足时偏离一档并留痕；用户主题出现不可读组合时靠「切回默认」自救，不做自动对比度修正。
- 四个窗口各自应用，可能出现「主窗已换、便签还是旧主题」的瞬时不一致 → 各窗口挂载时主动读设置 + 监听两类事件；事件丢失时下次聚焦窗口再读一次。
- 内置主题按上游「风格」重写而非移植，字体缺失时观感有差距 → 文件头与 D4a 写明取舍；不复刻上游的 `@media` 打印样式与界面 chrome。
- 上游改版不再跟随 → 基准值固化在 `assets/themes/*.css` 与 D4a 表；更新时按同一规则重算并改测试基线。
- 内置 id 与资源文件可能漂移 → 一致性测试把 `READING_THEME_BUILTINS`、资源文件、设置页选项绑在一起。

## Migration Plan

0. 内置 id 定稿 `paperglow` / `mdmdt`；早期占位名「素笺 / 夜航」从未发布，直接废弃，发布后 id 不改名。
1. 先后端：字段 + 四条命令 + `themes/` 落盘；前端暂不读，行为不变。
2. 再前端：解析器 + 应用 + 设置页 UI + D7 接线（先排版、后装饰两组）+ 两套内置主题；无主题时逐项对齐改造前计算值。
3. 全量口径追加（2026-11-17，用户要求「全量修改，不仅仅是知识库」）：白名单从 40 扩到 59（外壳 19）+ 默认令牌上移 `:root` + 应用层从 KbView 改到 App 与三窗 + 两套主题补外壳段。旧用户主题只有阅读段时，外壳保持默认配色（合法且可预期），不需要迁移。
4. 无数据迁移：旧 `data.json` 反序列化得空串，等同现状。
5. 回滚：`settings.readingTheme` 置空并还原提交；`themes/` 下文件不参与启动流程，留着无害。

## Open Questions

- 是否提供「导出主题」与分享（可独立变更）。
- 主题是否进备份包（当前明确不进）。
- 是否再开 `--kb-block-gap`（段落节奏）：当前正文段落无显式 margin 令牌，开放它会先改动默认排版，暂列 Non-Goal。