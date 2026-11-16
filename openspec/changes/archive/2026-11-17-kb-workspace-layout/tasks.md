# Tasks

## 1. 纸面骨架（CSS，JSX 不动）

- [x] 1.1 `.kb-detail-strip` 第一行下缘加 1px 分割线，两行高度固定为 50px / 38px
  验证：`views/kb.css` 中 `.kb-detail-strip` 与 `.kb-strip-head` 的 `border-bottom` / `height` 生效，`npx tsc -b` 通过
- [x] 1.2 保存时间与文件名基线对齐（`align-items: baseline`，保存时间 `line-height: 1`）（后被 kb-doc-title 7.2 收敛：外层居中 + 内层 head 保基线）
  验证：真机对照原型图，保存时间不再靠下 2–3px
- [x] 1.3 正文起点压到 112px（strip `padding-top: 6px`、`.kb-detail-scroll` `padding-top: 12px`）
  验证：真机截图量出纸面顶端到首行正文 ≤ 120px

## 2. 大纲轨道常驻

- [x] 2.1 `.kb-detail-inner` 去掉 `with-outline` 条件，恒为 `minmax(0,1fr) 188px`
  验证：`views/kb.css` 中 `.kb-detail-inner` 恒有两列，`with-outline` 只控制 `.kb-outline` 的 `visibility`
- [x] 2.2 `railFits` 判据改为「扣除 188px 后 ≥ 700px」，与开关无关
  验证：`components/kb/kb-item-detail.tsx` 中判据不再读 `outlineOpen`；开合大纲时 `mainRef` 宽度不变
- [x] 2.3 大纲关闭时渲染空占位而非不渲染
  验证：真机开合大纲，正文首行横向位置不变（截图对比像素一致）

## 3. 入口收敛到树头

- [x] 3.1 删除知识库顶栏容器与两个按钮（`.kb-open-dir-btn`、`.kb-new-btn`），清理 `.kb-topbar` 样式
  验证：`KbView.tsx` 中两处按钮与容器已移除；`views/kb.css` 无 `.kb-topbar` 残留
- [x] 3.2 搜索框移入左列顶部，宽度同树列，高 28px；删除左列「知识库 / 全部条目」标题行
  验证：真机左列顶部即搜索框，正文起点上移 ≥ 40px
- [x] 3.3 树头右侧两个图标：打开数据目录、额外操作；`aria-label` 与 `title` 齐备
  验证：真机悬停有提示，Tab 可达，读取 `aria-label`
- [x] 3.4 额外操作菜单复用 `KbContextMenu`（传按钮 rect），两项分别接既有新建文件夹 / 新建文件
  验证：点开菜单两项可见可用；点外或 Esc 关闭；不新增 Tauri 命令（`node test/command-parity.test.mjs` 通过）
- [x] 3.5 `KbFileTree` 新增 `onCreateItem` 回调并由 `KbView` 传入 `handleCreate`
  验证：`components/kb/kb-tree.tsx` 类型定义含 `onCreateItem`，`npx tsc -b` 通过
- [x] 3.6 折叠态（窄轨 96px）保留两个入口，不再 `display: none`
  验证：真机折叠后仍能打开数据目录与新建

## 4. 文档与规格

- [x] 4.1 `docs/capability/06-知识库.md`：结构段改「搜索在左列 / 纸面两行 / 轨道常驻」；入口段改「树头两个图标 + 额外操作菜单」；量算口径补 112px 正文起点
  验证：`node test/docs.test.mjs` 通过
- [x] 4.2 修订 `kb-doc-shell` 的 7.1 记录（被本变更取代），避免实现说明与规格打架
  验证：`kb-doc-shell/tasks.md` 7.1 条目注明「已被 kb-workspace-layout 取代」

## 5. 验证

- [x] 5.1 `npx tsc -b`（在 `src-react/` 下）
  验证：exit 0
- [x] 5.2 `npm run lint`（在 `src-react/` 下）
  验证：0 errors（warnings 数量不高于当前 34）
- [x] 5.3 `node test/docs.test.mjs`、`node test/command-parity.test.mjs`、`node src-react/test/md-roundtrip.test.mjs`
  验证：三者 exit 0，round-trip 的 `FAILURES=0`
- [x] 5.4 真机四条：正文起点 ≤120px、开合大纲无位移、树头两入口可用、折叠态仍有入口（用户 2026-11-17 真机核验通过）
  验证：用户确认截图；未确认前本条不打勾
- [x] 5.5 `openspec validate kb-workspace-layout --strict`
  验证：`Change 'kb-workspace-layout' is valid`；`target spec does not exist` 的 INFO 属预期（需先归档 `kb-doc-shell`）

## Workflow follow-up

- [x] 归档顺序：`kb-writing-experience` → `kb-doc-shell` → `kb-workspace-layout`；顺序颠倒会覆盖前者的规格条款（已按序归档）
- [x] 归档后回归：`openspec list --specs` 中 `knowledge-base/{workspace,doc-shell,outline}` 的条款与本文一致（归档后核对完成）
