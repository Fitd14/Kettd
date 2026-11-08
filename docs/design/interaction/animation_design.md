# Animation & Motion Design System

## 🎨 概述

本动画系统设计遵循以下原则:
- **目的性**: 每个动画都有明确的用户反馈或功能意义
- **克制性**: 避免过度装饰，保持专注体验
- **一致性**: 所有微交互共享相同的时序与缓动曲线

---

## 🎬 一、核心动画库

### 1.1 基础过渡 (Transitions)

```css
/* ============ GLOBAL TRANSITIONS ============ */

:root {
  /* 时间轴定义 */
  --t-very-short: 0.05s;   /* 微交互：按钮 hover 状态切换 */
  --t-short: 0.12s;        /* 常规操作：勾选完成、图标点击 */
  --t-medium: 0.22s;       /* 页面切换、内容展开 */
  --t-long: 0.35s;         /* 数据加载、复杂效果 */
  --t-slow: 0.6s;          /* 背景淡入/出等低频动画 */
  
  /* 缓动函数 */
  --ease-out: cubic-bezier(0.16, 1, 0.3, 1);    /* iOS 风格：回弹轻微 */
  --ease-in: cubic-bezier(0.55, 0.055, 0.675, 0.19);   /* Android 启动效果 */
  --ease-in-out: cubic-bezier(0.4, 0, 0.2, 1);     /* 自然回弹（首选）*/
  --ease-bounce: cubic-bezier(0.68, -0.55, 0.265, 1.55);
}

/* ============ COMPONENT TRANSITIONS ============ */

.fade-in { opacity: 0; }
.fade-in-enter-active { opacity: 1; transition: var(--t-very-short) var(--ease-out); }

.slide-up { transform: translateY(-8px); }
.slide-up-enter-active { transform: translateY(0); transition: var(--t-medium) var(--ease-in-out); }

.scale-pop { transform: scale(0.94); }
.scale-pop-enter-active { transform: scale(1); transition: var(--t-short) var(--ease-out); }
```

### 1.2 任务完成动画 (Task Complete)

#### CSS 实现

```css
/* === Checkbox Stroke-Draw Animation === */

.checkmark-path {
  /* 路径：9×9 viewBox，虚线长度=stroke-dasharray: "10 10" = 20px */
  stroke-dasharray: 10 10;
  stroke-dashoffset: 10;  /* 初始状态：隐藏一半 */
  transition: none;        /* 动画触发前禁用过渡，防止闪烁 */
}

/* 完成瞬间：一次性跳变显示 */
.checkbox.checked .checkmark-path {
  stroke-dashoffset: 0;
}

/* 延迟应用过渡（等待 DOM 更新后生效） */
.init-done .checkmark-path {
  transition: stroke-dashoffset var(--t-short) var(--ease-out);
}

/* === Priority Dot Spin === */

.priority-dot.pulse-high {
  animation: pulse-high 1.5s ease-out;
}

@keyframes pulse-high {
  0%   { transform: scale(1); opacity: 1; }
  33%  { transform: scale(1.3); opacity: 0.8; }
  50%  { transform: scale(1.6); opacity: 0.6; }
  67%  { transform: scale(1.2); opacity: 0.9; }
  100% { transform: scale(1); opacity: 1; }
}
```

#### JavaScript Hook (Vue 组合式)

```typescript
// 动画类管理
const animationClasses = createObject({
  // ✅ 任务完成：绿色填充 + checkmark
  taskComplete: () => ['checked'],
  
  // 🔴 高优先级闪烁警告（即将过期）
  priorityHighExpiringSoon: () => [
    'pulse-high'
  ],
  
  // ⏰ 日历事件渐入渐出
  calendarEventFade: () => ['fade-event']
});

// 使用示例
function handleTaskComplete(taskId) {
  const checkedEl = document.querySelector(`[data-task-id="${taskId}"] .checkbox`);
  
  if (checkedEl && !checkedEl.classList.contains('init-done')) {
    checkedEl.classList.add('init-done'); // ✨ 延迟启用 transition
  }
  
  checkedEl.classList.add('checked');   // ✅ 立即切换 class，触发动画
}
```

### 1.3 窗口透明度变换 (Opacity Animation)

```css
.opacity-container {
  /* 默认：鼠标悬停恢复不透明 */
  transition: opacity var(--t-medium) var(--ease-in-out);
  
  pointer-events: none;  /* 低透明度时禁用点击 */
}

.opacity-container.init-done:hover {
  pointer-events: all;   /* 恢复交互 */
}

/* === Pinned (置顶) vs Floating (悬浮) 状态 === */

.float-window:not(.init-done) .opacity-container {
  opacity: 0;            /* 初始加载：透明 */
}

.float-window.init-done {
  /* ✅ 鼠标悬停 → 1,离开→降至默认透明度 */
  .opacity-container:not(:hover) { opacity: 0.5; }
}

.float-window.is-pinned {
  /* 📌 固定状态：始终完全不透明，禁用 hover 效果 */
  .opacity-container { opacity: 1 !important; pointer-events: all; }
}
```

---

## 🎬 二、功能模块动画清单

### 2.1 碎片化笔记输入 (Flomo-Lite)

```css
/* ============ Quick Add Input Animation ============ */

.quick-add-input {
  transform-origin: top center;
  background: transparent;
}

/* 首次按下 Ctrl+N / Cmd+N：弹出 */
.quick-add-input.show {
  animation: pop-in var(--t-short) var(--ease-out);
}

@keyframes pop-in {
  0%   { transform: scaleY(0.8); opacity: 0; }
  50%  { transform: scaleY(1.1); opacity: 1; }
  100% { transform: scaleY(1); }
}

/* 输入焦点高亮动画 */
.quick-add-input:focus-within {
  animation ring-pulse var(--t-medium) var(--ease-in-out);
}

@keyframes ring-pulse {
  0%   { box-shadow: 0 0 0 0 rgba(219, 39, 119, 0.4); }
  70%, 100% { box-shadow: 0 0 0 6px transparent; }
}
```

### 2.2 任务创建/删除反馈

**创建成功**:
```typescript
async function showTaskCreated(taskId) {
  // Vue toast + Tauri 通知
  app.use('show-toasty', {
    message: '✅ 任务已添加',
    theme: '#10b981'
  });
  
  // 可选：Tauri Shell 打开最近添加任务的链接（复制到剪贴板）
  const taskUrl = `tauri://tasks/${taskId}`;
  window.__TAURI__.clipboard.writeText(taskUrl);
}
```

**删除确认**:
```css
/* 删除前预显示警告卡片 */
.delete-confirmation-card {
  opacity: 0;
  transform: translateY(-10px);
  transition: opacity var(--t-short) var(--ease-out), 
        transform var(--t-short) var(--ease-in);
}

.delete-confirmation-card.show {
  opacity: 1;
  transform: translateY(0);
}
```

### 2.3 日历视图动画

**日期翻页**:
```css
.calendar-nav button {
  animation: slide-in-right var(--t-medium) var(--ease-in-out) both;
}

@keyframes slide-in-right {
  0%   { transform: translateX(12px); opacity: 0; }
  45%, 100% { transform: translateX(0); opacity: 1; }
}
```

**事件渐显**:
```css
.event-dot.fade-in {
  animation: event-fade var(--t-short);
}

@keyframes event-fade {
  from { opacity: 0; }
  to   { opacity: 1; }
}
```

### 2.4 优先级循环切换动画

```css
/* === Priority Cycle Spin Effect === */

.priority-dot.level-low.level-high-just-upgraded {
  animation: priority-spin 3s infinite linear;
}

@keyframes priority-spin {
  0%   { transform: rotate(0deg); }
  100% { transform: rotate(-270deg); } /* 高→中→低，逆时针旋转 */
}

.priority-dot.level-high.pulse-urgent {
  animation: pulsate 0.4s ease-in-out alternate infinite;
}

@keyframes pulsate {
  from   { box-shadow: 0 0 0 0 rgba(239, 68, 68, 0.4); }
  to     { box-shadow: 0 0 0 10px rgba(239, 68, 68, 0); }
}
```

---

## 🎬 三、完成成就反馈系统

### 3.1 徽章解锁动画

**序列帧展示流程**:
```css
/* 触发徽章获取 */
.achievement-badge {
  opacity: 0;
  transform: scale(0);
}

.achievement-badge.animate-earned {
  animation: badge-unlock-var-2s forwards;
}

@keyframes badge-unlock-var {
  0%   { opacity: 0; filter: blur(8px) grayscale(1); }
  5%, 28% { filter: none; }          /* 淡入后短暂停顿，等待用户观察 */
  30%  { transform: scale(0.5); opacity: 0.5; }   /* 放大+渐显 */
  60%  { transform: scale(1.2); }    /* 达到最终尺寸 */
  70%  { transform: scale(1); }      
  100% { opacity: 1; }
}

/* 徽章图标跳动（庆祝效果） */
.achievement-badge .badge-icon.animate-bounce {
  animation: bounce-up var(--t-long) cubic-bezier(0.175, 0.885, 0.32, 1.275) both;
}

@keyframes bounce-up {
  from, 20%, 50%, 80%, 100% { transform: translateY(-0.8); } 
  40% { transform: translateY(0); } 
  60% { transform: translateY(-0.6); }
}

/* 徽章标题渐入 */
.achievement-badge .badge-title.animate-fade-in {
  opacity: 0;
  animation: text-fade-up-2s var(--t-medium) both;
}

@keyframes text-fade-up-2s {
  from   { opacity: 0; } 
  to     { opacity: 1; }
}
```

### 3.2 Toast 通知样式系统

**基础模板**:
```scss
$toast-duration: 4s;

.mt-toast {
  position: fixed;
  top: 20px;
  right: 20px;
  width: 360px;
  padding: 12px 18px;
  background: rgba(255, 255, 255, 0.99);
  
  backdrop-filter: blur(12px) saturate(130%);
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.08), 0 1px 4px rgba(0,0,0,0.02);
  
  transition: opacity var(--t-short) ease-in-out, 
              transform var(--t-short) ease-in-out;
}

/* 成功消息 */
.mt-toast--success {
  background: rgba(68, 178, 111, 0.95);
  color: #ffffff;
}

.mt-toast--warning {
  background: rgba(247, 147, 26, 0.95);
}

.mt-toast--error {
  background: rgba(239, 68, 68, 0.95);
}

/* 显示时：向上滑入 + fade-in */
.mt-toast.open {
  opacity: 1;
  transform: translateY(0);
}

/* ===== 关闭前动画 ============= */
$transition-duration: $toast-duration - 0.2s; 

.mt-toast .close-btn {
  opacity: 0;      /* 默认隐藏，仅最后 200ms 可见 */
  transition-delay: calc(#{($transition-duration + var(--t-short))/2});
  transform: translateX(8px);
}

.mt-toast:before,
.mt-toast:after {
  content: ' ';         /* 三角切角视觉效果（可选）*/
  position: absolute; top: 20%; width: 6px; height: 6px;
  background: inherit;
  border-top-left-radius: 3px;
}
```

---

## 🎬 四、看板拖拽排序动画

### 4.1 Ghost 层视觉反馈 (Vue Drag-and-Drop)

**实现原理**: Naive UI `Draggable` 组件 + custom ghost DOM

```vue
<!-- KanbanColumn.vue -->
<template>
  <div class="kanban-column">
    <!-- 卡片列表（虚拟渲染优化） -->
    <div 
      v-draggable
      :model-value="taskList" 
      @start="onDragStart" 
      @end="onDragEnd"
    >
      <draggable-item item-key="id" class="kanban-card">
        <span :class="{ 'drag-grip': isDragging }">🤳</span> <!-- 拖拽手柄 -->
        {{ task.title }}
        <span :style="getPriorityColor(task.priority)">●</span>
      </draggable-item>
    </div>

    <!-- ============ Drag Handle ========= -->
    <template #button="{ moveToIndex, isSortable }">
      <span 
          v-if="isSortable"
          class="drag-handle"
          @click.prevent.stop="moveToColumn(moveToIndex)"
      >
        ⋮⋮            <!-- 垂直三格点（拖拽手柄） -->
      </span>
    </template>
  </div>
</template>

<style scoped>
/* === Visual Effect: Drag Handle Animation ===== */
.drag-handle {
  font-size: 14px; color: #6b7280; cursor: grab; margin-right: 8px;
  transition: color var(--t-very-short), transform var(--t-short);
}

.kanban-card:hover .drag-handle {
  color: #1e40af;            /* 悬停时蓝色提示可拖拽 */
  font-size: 18px;           /* 放大强调交互区域 */
}

.drag-handle:active {       /* 鼠标按下：显示抓握图标 */
  animation: drag-grip-move var(--t-short) infinite alternate;
}

@keyframes drag-grip-move {
  from   { content: '⋮‿⋮'; } <!-- 左移 -->
  to     { content: '⋮' }     <!-- 右移 -->
}
</style>
```

### 4.2 拖拽位置预览线 (Position Indicator)

```css
/* === 拖拽放置反馈：底部提示线 ===== */

.drag-preview-indicator {
  position: absolute; bottom: -10px; left: 0; width: 100%;
  height: 4px; border-radius: 2px;
  background: rgba(16, 185, 129, 0.8);        /* 绿色虚线 */
  transition: all var(--t-short) ease-in-out;
}

.kanban-card:hover .drag-preview-indicator::after {
  content: '';           /* 三角指示符（CSS pseudo 元素）*/
  display: block;
  position: absolute; bottom: -6px; left: 50%; width: 8px; height: 6px;
  background: inherit;
  clip-path: polygon(25% 0%, 75% 0%, 100% 100%, 0% 100%);
}

.kanban-card:hover:first-child .drag-preview-indicator::after {
  bottom: -4px;          /* 首卡片：三角形向上 */
}

/* 拖拽超出列边界 → 吸附到相邻列顶部/底部 */
.drag-over-kidney-zone {
  opacity: 0.3; transform: rotate(-1deg);      /* 🌐 虚线边框区域视觉渐变 */
  animation: pulse-opacity-4s var(--t-long) ease-in-out infinite alternate;
}

@keyframes pulse-opacity-4s {
  from   { opacity: 0.2; } 
  to     { opacity: 0.5; }
}
```

---

## 🎬 五、页面切换动画 (Page Transition)

### 5.1 Tab 视图轮播（主窗口/悬浮窗切换）

```css
/* === Naive UI Tabs 动画 ===== */

.n-page-tab-pane .page-content {
  opacity: 0;
  transform: translateX(-12px);                /* 从左侧滑入的起点偏移 */
  transition: opacity var(--t-medium) var(--ease-in-out), 
              transform var(--t-medium) var(--ease-in-out);
}

.n-page-tab-pane:not(.n-page-tab-pane--active) .page-content {
  display: none;     /* 隐藏未激活视图 */
}

/* ✅ 当前激活的标签页内容显示：opacity=1, translateX=0 */
.n-tabs-tabpane--active+.view-container.view-container--fade-in {
  opacity: 1;  
  transform: translateX(0);
}
```

### 5.2 Vue Transition Component (完整 Demo)

```vue
<!-- src/components/PageTransition.vue -->
<script setup>
import { nextTick } from 'vue';

defineProps(['name']) // name: string, e.g., 'fade-slide', 'spring-in'
</script>

<template>
  <transition 
    :name="`route-${name}-enter`"
    @after-enter="onEntered"
  >
    <slot />
  </transition>
</template>

<style scoped>
/* === fade-slide: 淡入 + 左滑 ===== */
.route-fade-slide-enter-active {
  animation-name: fade-slide-in-1;
}

@keyframes fade-slide-in-1 {
  from {
    opacity: 0; transform: translateX(-20%);
    filter: brightness(0.9) blur(8px);      /* 淡入叠加层叠模糊效果 */
  }
  to {
    opacity: 1; transform: translateX(0); 
    filter: none;
  }
}

/* === spring-in: 弹簧回弹 ===== */
.route-spring-in-enter-active {
  animation-name: spring-pop-4s;
}

@keyframes spring-pop-4s {
  from { opacity: 0; transform: scale(0.95); }
  to   { opacity: 1; transform: scale(1); }
}
</style>
```

---

## 🎬 六、性能优化指南 (Web Vitals Optimization)

### 6.1 避免布局抖动 (Reflow/Repaint Prevention)

**✅ 正确**: CSS `opacity` + `transform` 触发 GPU 加速层  
**❌ 错误**: 修改 `width/height/top/left` → 强制 CPU 重排

```css
/* ✅ 推荐：使用 translate3d/hardware-accelerated */
.slide-down-enter-active {
  animation: slide-up-anim-300ms;
}

@keyframes slide-up-anim-300ms {
  from { transform: translateY(-24px); }
  to   { transform: translateY(0); }
}

/* ⚠️ 避免：修改盒模型尺寸 */
.slide-up-enter-active.error-layout-thrash {
  animation: slide-up-error; 
}

@keyframes slide-up-error {
  from { opacity: 0; height: 0; margin-top: -24px; } /* ❌ */
  to   { opacity: 1; height: auto; }                  /* ✅ 仅用 transform + opacity */
}
```

### 6.2 `will-change` 属性使用规范

```css
/* === 何时添加 will-change ===== */
/* ✅ 仅在动画运行时启用，防止性能开销 */
.fade-in-enter-active {
  will-change: opacity;    /* 仅改变不透明度：GPU 合成 */
}

.slide-element-enter-active {
  will-change: transform;     /* 位移 + 缩放：GPU 合成 */
}

/* ⚠️ 删除不再需要的 will-change */
.fade-in-leave-active {
  will-change: auto;         /* 移除属性，避免内存泄漏 */
}
```

---

## 📦 七、动画效果 Demo（代码片段）

### 7.1 Vue 3 Animation Hook (完整实现)

创建：`src/composables/useAnimation.ts`

```vue
// <script setup>
import { ref, computed } from 'vue';

export function useTaskCompleteAnimation() {
  const checkmarkPath = ref({ strokeDashoffset: 10 });
  
  // ✅ 完成动画触发器（Vue 组合式 API）
  return {
    markAsComplete(taskId) {
      setTimeout(() => {
        document.querySelector(`[data-task-id="${taskId}"] .checkbox`)?.classList.add('checked');
        checkmarkPath.value.strokeDashoffset = 0;      // ✅ 触发 stroke-offset 过渡动画
      }, 50);    /* 延迟 50ms → 避免与输入框 focus/blur 竞争 */
                      
      if (window.__TAURI__?.notification.hasPermissions()) {       // 🎯 发送系统通知 + 徽章震动
        window.__TAURI__.notification.sendNotification({ title: '✅ 任务完成', icon: './icons/icon.png' });
      }
    }
  };
}
</script>
```

### 7.2 CSS Animation Library (导出使用)

创建：`src/assets/animations.css`

```css
/* ============ COMPONENT ANIMATIONS ============ */

/* === Fade in ===== */
@keyframes fade-anim {
  from { opacity: 0; filter: blur(8px); }
  to   { opacity: 1; }
}

.fade-in-enter-active { animation: fade-anim var(--t-medium) ease-out; }

/* === Scale Pop ===== */
@keyframes scale-pop-anim {
  from   { transform: scale(0.98); opacity: 0; }
  to     { transform: scale(1); opacity: 1; }
}

.scale-pop-enter-active { animation: scale-pop-anim var(--t-short) ease-out; }

/* === Checkbox Checkmark ===== */
@keyframes check-complete {
  from   { stroke-dashoffset: 10; }           /* 初始：隐藏一半长度 */
  to     { stroke-dashoffset: 0; }            /* ✅ 完成：显示整条路径 */
}

.check-marked { animation: check-complete var(--t-short) ease-out; }

/* === Toast notification ===== */
@keyframes toast-slide-top-bottom {
  from   { transform: translateY(-32px); opacity: 0; }
  to     { transform: translateY(0); opacity: 1; }
}

.toast-in-anim-enter-active { animation: toast-slide-top-bottom var(--t-medium) ease-out; }

@keyframes toast-close {
  from   { opacity: 1; }
  to     { opacity: 0; }
}

.toast-anim-leave-active { animation: toast-close var(--t-short) forwards; }

/* === Priority pulse ===== */
@keyframes priority-pulse {
  from, to { box-shadow: 0 0 0 0 rgba(239,68,68,0); }
  50%     { box-shadow: 0 0 0 12px rgba(239,68,68,0); }
}

.priority-high.pulse-anim::after { content: ''; position: absolute; inset: 0; animation: priority-pulse 1s infinite ease-in-out; }
```

使用：

```vue
<div class="fade-in-enter-active">Loading...</div>
```

---

## 🛠️ 八、开发指南 (Quickstart)

### 8.1 安装 Naive UI + Vue Transition

```bash
# npm / yarn / pnpm
yarn add naive-ui @vueuse/core

# TypeScript 类型定义（Vue 3）
yarn add -D @types/nipplejs # 可选：支持拖拽手势库
```

### 8.2 集成虚拟滚动

创建 `src/components/VirtualScroller.vue`:

```vue
<script setup>
// Virtual Scroller + Vuesax + Draggable
import { computed } from 'vue';
</script>

<template>
  <div class="viewport--no-scroll">
    <div class="scroller" ref="rootRef">
      <!-- ✅ Vue Lazy Load (每页显示前 30 项) -->
      <draggable-item v-for="(item, index) in filteredData.slice(0, visibleCount)" 
                       :key="index"
                       item-as="div" class="list-item">
        {{ item.title }}<span class="date">{{ formatDate(item.dueDate) }}</span>
      </draggable-item>

      <!-- 占位条（加载更多） -->
      <div v-if="loading" class="loader-bar">⋮</div>    <!-- 加载指示器 → 垂直省略符号 -->
    </div>
  </div>
</template>

<style scoped>
.viewport--no-scroll { overflow: hidden; }
.scroller { max-height: calc(100vh - 64px); position: relative; }

/* ✅ 虚拟滚动：隐藏未渲染元素 */
.list-item.hidden-render { display: none; min-height: 50px; }
.list-item.visible-render { display: block; }      <!-- 可见区域才渲染内容 -->
</style>
```

---

## 📐 九、可访问性 (Accessibility Checklist)

|检查项 |实现方法 |
|-------|---------|
| 焦点可见 | `:focus-visible` ring（不破坏键盘导航）|
| 动画触发器 | 避免频繁使用 `animation: infinite` → 使用 CSS `transition` + 动态 class|
| 减少运动病 | 设置浏览器媒体查询：`@media (prefers-reduced-motion: reduce)`，禁用所有动画 |

```css
/* ===== 对 Motion Sensitive Users ===== */
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important; /* ✅ 禁用全部动画：立即完成效果 */
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
  
  .fade-in-enter-active,
  .slide-up-enter-active { display: none; } /* 替换静态内容 → 避免闪烁 */
}
```

---

## 🌐 十、浏览器兼容性检查

**测试清单**:

|动画技巧 |Chrome |Firefox |Safari |Edge |
|---------|-------|--------|-------|-----|
| `opacity + transform` | ✅ ES6 | ✅ CSS3 | ✅ iOS | ✅ Chromium |
| `stroke-dashoffset` | ✅ SVG2.0 | ✅ E10+ | ⚠️ Safari 内核 | ✅ Chromium |
| GPU accelerated compositing | ✅ WebGL | ✅ Canvas2D | ✅ CoreGraphics | ✅ GPU2 |

**最小兼容性**: ES5 + CSS3（Chrome64+/Firefox78+/Safarid/Edge79）

---

## 📄 十一、导出与分享（Animation Demo）

创建：src/demo-animation.html (独立页面)

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <title>Ketidd Animation Demo</title>
  <style>
     :root { --t-very-short: 0.05s; --t-short: 0.12s; }
     
     .fade-pop-enter-active { animation-name: fade-pop-in-350ms; }
     @keyframes fade-pop-in-350ms { 
       from { opacity: 0; transform: translateY(-8px); }
       to   { opacity: 1; transform: translateY(0); }
     }
     
     .task-card.pulse-high-anim { animation-name: pulse-high-anim; }
     @keyframes pulse-high-anim { 
       from, to { box-shadow: 0 0 0 rgba(239, 68, 68, 0); }
       50% { box-shadow: 0 0 10px rgba(239, 68, 68, 0.3); }
     }
     
     .checkmark-path { stroke-dasharray: 10 10; stroke-dashoffset: 10; animation-name: check-complete-250ms; }
     @keyframes check-complete-250ms { to { stroke-dashoffset: 0; } }
     
     .draggable-item.draggable { cursor: grab; }
     .draggable-item:hover { transform: translateY(-2px); transition: var(--t-short); }
  </style>
</head>
<body style="margin:0;padding:32px;font-family:'Segoe UI',sans-serif;background:#f8fafc;color:#1e293b">
  
  <!-- === Task Create Animation === -->
  <div class="fade-pop-enter-active" style="position:relative;border-radius:12px;padding:18px;background:#ffffff;box-shadow:0 4px 12px rgba(0,0,0,0.08);">
    <h3>✅ 任务完成动画：<span>(绿色填充 + checkmark)</span></h3>
    
    <!-- Task Card -->
    <div style="border:2px solid #e4e4e7;border-radius:8px;padding:12px;display:flex;align-items:center;gap:10px;margin-top:8px">
      <!-- Checkbox Animation -->
      <input type="checkbox" class="check-box" data-animated="true"/>
      
      <!-- Task Description -->
      <p style="margin-right:auto;font-size:14px;line-height:1.6">完成这个 demo</p>
      
      <!-- Priority Indicator -->
      <span class="priority-high-anim priority-high-urgent dot"></span>
    </div>
  </div>
  
  <!-- ==== Drag Handle Demo ===== -->
  <div style="margin-top:24px;border-radius:8px;padding:12px;background:#fafafa">
    <strong style="display:block;font-size:13px;color:#6b7280;margin-bottom:8px">🤳 拖拽手柄：</strong>
    
    <!-- Draggable Items -->
    <div style="display:flex;flex-wrap:wrap;gap:8px;border-top:1px solid #e4e4e7;padding-top:8px;margin-top:8px"
         class="draggable-list">
      <div draggable class="draggable-item" data-id="item-1" style="width:120px;height:60px;background:#f1f5f9;border-radius:6px;display:flex;align-items:center;justify-content:center;font-size:13px;color:#475569">
        任务一
      </div>
      
      <div draggable class="draggable-item" data-id="item-2" style="width:120px;height:60px;background:#f1f5f9;border-radius:6px;display:flex;align-items:center;justify-content:center;font-size:13px;color:#475569">
        任务二
      </div>
      
      <div draggable class="draggable-item" data-id="item-3" style="width:120px;height:60px;background:#f1f5f9;border-radius:6px;display:flex;align-items:center;justify-content:center;font-size:13px;color:#475569">
        任务三
      </div>
    </div>
  </div>

</body>
</html>
```

---

## 🎬 十二、总结

**动画原则**:
1. **少即是多 (Less Motion)** - 每个动画都有功能目的，不为装饰而动
2. **渐进式复杂度** - 从简单 toggle→复杂序列触发器逐步增强体验
3. **性能优先** - GPU 合成 + will-change 合理使用避免 CPU 重排

**参考资源**:
- [Motion One](https://www.motion.one/) (Vue 动画库)  
- [Framer Motion](https://www.framer.com/motion/) (参考文档)  
- [Material Design Guidelines](https://material.io/design/motion/introduction.html)

---

**撰写**: Animation Engineer Assistant  
**更新日期**: 2026 年 8 月  
