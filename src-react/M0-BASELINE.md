# src-react · M0 前端基座（双轨 · 未接管发布）

本目录是 Kettd v3 React 迁移的**新前端工程**（ADR-001 的 M0 产物）。M0 阶段是**双轨**：
vanilla 发布路径（`../src` + `tauri.conf` 的 `devPath/distDir`）**完全未改**，本 React 工程暂不接管 App 产物。

## 技术栈（M0 实测锁定）
- Vite 8.2 / React 19.2 / TypeScript 6（`tsc -b` 已去 `baseUrl`，paths 相对配置文件解析）
- Tailwind CSS v4（`@tailwindcss/vite` 插件 + `@theme inline` 令牌映射）
- shadcn/ui（`components.json` new-york / cssVariables / `@/` 别名；`add button` 已验 registry 取件链路通）
- 依赖：cva · clsx · tailwind-merge · lucide-react · tw-animate-css · @radix-ui/react-slot

## 设计令牌
`src/index.css` 的 `:root` / `.dark` 与 vanilla `../src/styles.css` **逐字对齐**（含 `--cat-*` / `--pri-*` 语义色、`--radius`），
故两栈共用同一份色板 → 视觉单一来源。`--r-pill` 等以 `@theme inline` 暴露为 Tailwind 工具类。

## 本地命令
```
cd src-react
npm install        # 装依赖
npm run dev        # Vite 预览（含我们写的 Button + 暗色切换冒烟页），http://localhost:5173
npm run build      # tsc -b && vite build → dist/
```
M0 验收：`npm run build` 通过（实测 1932 modules，dist 生成，CSS 内含 cat-life/pri-high/rounded-pill 等令牌）。

## Tauri × Vite 接线（**准备好，M1 才激活**，勿在 M0 改发布路径）
M1 起，`src-tauri/tauri.conf.json` 的 `build` 段拟改为：
```
"beforeDevCommand":   "npm --prefix ../src-react run dev",
"beforeBuildCommand": "npm --prefix ../src-react run build",
"devPath":  "http://localhost:5173",
"distDir":  "../src-react/dist",
```
注意事项（留待 M1 验证）：
1. Tauri v1 多窗（index/float/capture）现按 `WindowUrl::App("xxx.html")` 从 `../src` 取；SPA 化后需路由或多入口 vite build 对齐这些 URL —— 这正是 **M1 用最小的 capture 窗先试**要打通的点。
2. `base: './'` 已设，保证 dist 资源在自定义协议下相对加载。
3. 激活前 `cargo build` 会锁 Windows 上的运行中 exe，需先停实例。
4. 回滚：任一阶段可把 `tauri.conf` 的 build 段还原为 `../src`、`dist` 即退回 vanilla 发布（`git` 双轨）。

## 下一步
M1：把 capture 窗 React 化（仅重写 capture.html 约 120 行卡片；OS acrylic/拖拽/记位属 Rust 层，与 vanilla 版共用）。
M2：api.ts / Zustand / Toast / Undo 底座。详见 `spark-output/pitch/` 的 ADR-001。
