# Design

## Context

知识库详情页当前是三层：`.kb-detail-strip`（一行，含文件名 / 保存时间 / 五个动作 / 格式工具栏）→ `.kb-detail-scroll`（滚动区）→ `.kb-detail-inner`（`with-outline` 时 `grid: minmax(0,1fr) 176px`）→ `.kb-detail-main`（`width: min(100%, var(--kb-measure-cap))`）。

知识库外壳是：`.kb-layout` 两列（树列 + 详情），树列上方有一条横跨整个知识库区的搜索带。树头 `.kb-tree-head` 只有「目录」与一个 `FolderPlus` 图标，顶栏另有「打开数据目录」文字按钮与「+ 新建」主按钮。

三次实测暴露的结构问题见 proposal 的 Why。本变更只处理版面与入口，不改编辑器行为、不改数据、不加命令。

## Goals / Non-Goals

**Goals**

- 让正文尽量靠近纸面顶端（起点 ≤ 120px），顶部不再有大段空白。
- 让大开关件（大纲）不参与正文的布局计算，消除「开大纲正文横跳」。
- 让同一动作只有一个入口，且入口在树头。

**Non-Goals**

- 不做工具按钮的精简或重排（按钮集合不动）。
- 不把新写的原生控件迁到 `components/ui/`（那是 `radix-component-layer` 的事）。
- 不改字号 / 字距 / 列宽三个档位的取值与语义。
- 不改 `kb-item-detail.tsx` 之外的数据流，不动 Tauri 命令。

## Decisions

### D1 搜索框移入左列顶部

`KbView.tsx` 删除横跨整栏的搜索带容器，搜索输入移到树列顶部（`.kb-tree-pane` 内第一行），宽度 `100%`、高 28px。左列原有的「知识库 / 全部条目」两行标题一并删除：左列只剩「搜索 → 树头 → 树」。

为什么不是「把搜索带压窄」：压窄后仍占一整行高度，正文起点最多降 20px；整条移进左列才拿回 44px。

### D2 纸面顶行拆两行，中间加分割线

`.kb-detail-strip` 已是 `grid-template-areas: 'head actions' / 'tools tools'`（7.2 的成果），本变更只补三件事：

1. 第一行下缘加 `border-bottom: 1px solid var(--line-warm)`（现在只有第二行的 `border-bottom`，两行之间只有 `gap: 2px`）。
2. `.kb-strip-head` 显式 `align-items: baseline`，保存时间用 `line-height: 1` 且不再单独 `margin-top`。
3. 第一行高度固定 50px、第二行 38px，朱线留在第二行下缘（`box-shadow: 0 1px 0 hsl(var(--seal) / .16)` 已存在，保留）。

### D3 大纲轨道常驻，不参与布局

`.kb-detail-inner` 去掉 `with-outline` 的条件分支，恒为 `grid-template-columns: minmax(0, 1fr) 188px`；`.kb-outline` 恒在第二列，关闭时渲染空占位（`visibility: hidden` 保留宽度，避免重排）。`railFits` 的判据改为「扣除 188px 后正文列仍 ≥ 700px」——与是否开启无关。

这直接取代 `kb-doc-shell` 7.1 的「从正文右侧留白里让」：让位会造成 94px 平移，常驻才是零位移。

### D4 正文起点压到 112px

纸面顶端到正文首行 = 顶行 50 + 分割线 1 + 第二行 38 + 间距 11 + 正文上内边距 12 ≈ 112px。实现上只需要 `.kb-detail-strip` 的 `padding-top: 6px` 与 `.kb-detail-scroll` 的 `padding-top: 12px`，不再靠 `--kb-bottom` 之类的变量反推。

### D5 入口收敛到树头

树头右侧两个图标：打开数据目录（`FolderOpen`）、额外操作（`MoreHorizontal`）。额外操作点开复用 `KbContextMenu`——它现在收 `x`/`y` 坐标，调用处改为传按钮的 `getBoundingClientRect()`。菜单两项分别调既有的 `handleCreateFolder` / `handleCreate`，不新增命令。

`KbFileTree` 需要新收一个 `onCreateItem` 回调（现在只有 `onCreateFolder`），由 `KbView` 传入 `handleCreate`。

### D6 折叠态保留两个入口

`.kb-layout.is-collapsed .kb-tree-head { display: none }` 改为窄轨纵向图标条（两个图标上下排列，宽 96px 可容纳）。理由是 D5 把新建的唯一入口放进了树头，若折叠时整块消失，用户就没有任何新建入口——这是功能回退，不只是观感问题。

### D7 删除顶栏两个按钮

`.kb-open-dir-btn` 与 `.kb-new-btn` 从 `KbView.tsx` 移除；`kb-new-btn` 的快捷键与主按钮样式一并不再使用。顶栏为空后，`.kb-topbar` 容器一并删除，样式表里的 `.kb-topbar` 规则同步清理。

### D8 与 radix 线的边界

本变更新增的两个图标按钮与菜单都沿用现有实现（原生 `button` + 现有 `KbContextMenu`），不引入 `radix-ui`。这部分欠账记在 `radix-component-layer` 的 Impact 里，等它执行时统一收。

## Risks / Trade-offs

- **常驻 188px 在窄面板下浪费空间**：面板 < 888px 时切抽屉，正文不再受挤压；但抽屉形态下正文同样扣 188px 是否合理需要真机确认（若不合适，抽屉形态下可退回不扣）。
- **搜索移入左列后，空查询态的左列高度变短**：搜索框占 42px，树可视高度减少，换来正文上移 44px。
- **折叠态图标条**：窄轨只有 96px，两个图标上下排列会挤压树节点；若真机上显得拥挤，退回「保留一个合并的额外操作图标」。
- **规格顺序**：本变更 MODIFIED `knowledge-base/doc-shell` 与 `knowledge-base/outline`，这两个能力要等 `kb-doc-shell` 归档后才会进主规格，因此本变更必须在它之后归档（校验时会出现 `target spec does not exist` 的 INFO，属预期）。

## Migration Plan

1. 先改 CSS 骨架（D2 / D3 / D4），JSX 不动，确认正文位置与轨道常驻生效。
2. 再改 `KbView.tsx` 的搜索位置与顶栏删除（D1 / D7）。
3. 再改树头（D5 / D6），复用 `KbContextMenu`。
4. 同步 `docs/capability/06-知识库.md`。
5. 回归：`npx tsc -b`、`npm run lint`、`node test/docs.test.mjs`、`node src-react/test/md-roundtrip.test.mjs`；真机看四条：正文起点、开合大纲无位移、树头两入口、折叠态有新入口。

## Open Questions

1. 抽屉形态下是否也扣掉 188px？当前设计答案是「扣」（保持正文位置不变），真机再定。
2. 左列删掉「知识库 / 全部条目」标题后，空态是否需要一句说明？当前不加。