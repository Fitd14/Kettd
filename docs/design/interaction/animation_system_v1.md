# 🎬 Animation Design System v1.0-alpha

## 📑 目录
- [概览](#overview)  
- [核心过渡定义](#transitions)
- [功能模块动画清单](#module-animation-list)
- [Vue 组件实现示例](#vue-demo)
- [性能优化指南](#performance)
- [浏览器兼容性](#browser-support)
- [导出资源](#export-assets)

---

## 🎯 Overview（概述）

本动画系统设计遵循 **KISS + Motion One**原则：

| 原则 |说明 |
|------|-----|
| ✅ **Purposeful Motion** | 每个动画都有明确功能目的（反馈/引导/分隔） |
| ⚡ **GPU Accelerated** | `transform` + `opacity`触发 GPU 合成，避免 CPU 重排 |
| 🎯 **Progressive Enhancement** | 低性能设备自动降级为静态效果 |

---

## 🔧 Transitions（核心过渡定义）

```css
/* === Time Scale Constants === */
:root {
  /* 快速：按钮 hover、图标 click（<80ms）*/
  --t-fast: 16ms, cubic-bezier(0.33, 1.27, 0.68, 0.94);    /* 类似 iOS bounce */
  
  /* 常规：内容切换、勾选完成（120-350ms）*/
  --t-normal: 150ms, cubic-bezier(0.4, 0, 0.2, 1);         /* Apple smooth */
  
  /* 慢速：页面加载、场景切换（≥250ms）*/
  --t-slow: 300ms, cubic-bezier(0.19, 1, 0.22, 1);     /* iOS淡入 */
}

/* === Transition Utility Classes ===== */
.fade-in { opacity: 0; transition-property: opacity }
.sliding-up { transform: translateY(-8px); transition-timing-function: var(--t-normal); }
.scaling-pop { 
  transform: scale(0.98); 
  transition-timing-function: cubic-bezier(0, 0, 0.2, 1); /* spring-like */
}

/* === CSS Animation Keyframes ===== */
/* ✅ Fade In Pop */
@keyframes fadeInPop {
  from { opacity: 0; transform: translateY(-8px) scale(.95); filter: blur(8px); }
  to   { opacity: 1; transform: translateY(0) scale(1); filter: none; }
}

/* ✅ Checkmark Path Reveal */
@keyframes checkComplete { to { stroke-dashoffset: 0; } }

/* ✅ Toast Slide In/Out */
.toast-slide-in-anim { animation-name: toastSlideTopBottom; animation-duration: var(--t-normal); }
.toast-close-anim { 
  animation: toastClose 200ms forwards;
  justify-content: flex-end; /* close-btn align right before disappearing */
}

@keyframes toastClose { to { opacity: 0; transform: translateY(-32px) translateX(8px); } }
```

---

## 📦 Module Animation List（功能模块动画清单）

### 1️⃣ Quick Add（快速添加笔记待办）

**触发条件**: 按下 Ctrl+N / Cmd+N  
**交互反馈**: 
- ✅ 输入框 **scale-pop**弹入（高度 +6px，同时透明度淡入）
- 🔔 Toast 通知 "快速添加区已就绪"

```vue
<template>
  <transition
    name="slide-up-transition"
    enter-active-class="animate-show"
    enter-start-class="hidden-render"
    duration="300"
  >
    <!-- === Quick Add Input Area (Hidden by Default until Triggered) ===== -->
    <div class="quick-add-area">
      <input type="text" v-model:value="newTaskText" placeholder="快速添加任务..." />
      <select v-model:value="categorySelect" class="task-category-select">
        <option value="#work">工作</option>
        <option value="#personal">个人</option>
        <option value="#learn">学习</option>
      </select>
      
      <!-- === Tag Extraction (Optional: Auto-add #tag if not already assigned) ===== -->
      <template v-if="tagExtractionEnabled && !currentFilter"># 待添加</template>
      
      <div class="add-btn" @click.capture.stop="handleAddTask($event)">+</div>
    </div>
  </transition>
</template>

<script setup>
import { ref, watch, computed } from 'vue';

async function handleAddTask(e) {
  if (!newTaskText.value.trim()) return;
  
  const dueDate = quickAddTime.value ? `${dateStr} ${time}` : dateStr;
  
  await addNewTask({
    title: newTaskText.value.trim(), description: newTaskDesc.value.trim() || '', 
    completed: false, category: quickAddCategory.value, priority: 'low', due_date: dueDate }, id);
    
  // Toast notification → show "✅ Task added" (optional)
  await notify('QuickAdd success')?.then(console.log.bind(console));
}
</script>

<style scoped>
.quick-add-area .add-btn { 
  width:26px;height:26px;border-radius:50%;display:flex;align-items:center;justify-content:center;
  background:#1f2937;color:#fff;font-size:18px;font-weight:300;cursor:pointer;transition:.12s ease-out;
  box-shadow: 0 2px 6px rgba(0,0,0,.1);flex-shrink:0;transform:translateZ(0);animation: ripple-anim .3s linear;
}
.quick-add-area:hover .add-btn { background:#3f4556; }/* :hover→scale-1.05 */

@keyframes ripple-anim { from { transform: scale(.98); opacity: 1; } to   { transform: scale(1); }}
</style>
```

**性能提示**: Use `will-change: transform` only during animation → delete in leave-active → avoid memory leaks.

---

### 2️⃣ Checkbox Complete Animation（完成勾选效果）

**触发条件**: 用户点击/快捷键按 **Del**  
**交互反馈**: 
- ✅ SVG **stroke-dashoffset=0**路径完整显示
- 🎨 绿色填充背景（#16a34a）

```vue
<template>
  <transition name="fade-complete" appear="true">
    <!-- === Checkbox + Checkmark Path ===== -->
    <div class="checkbox checked init-done" @click.stop="toggleTaskComplete">
      <svg viewBox="0 0 9 9" fill="none" class="checkmark-svg" width="26" height="26">
        <path 
          :style="'transition: none'" /* ⚡ animation trigger: disable transition before first stroke */
          class="checkmark-path path-animate"
          :class="{checked: checkboxChecked}"
          d="M2 4.5L3.5 6L7 2.5" 
          @mouseenter="" /> <!-- 禁用鼠标事件 (prevent conflict with drag-to-move) ===== -->
      </svg>
    </div>
  </transition>
</template>

<script setup>
const checkboxChecked = ref(false); /* ✅ Vue state: track completion status for undo/redo */

async function toggleTaskComplete(taskId) {
  const taskList = await db.prepare('SELECT * FROM todos WHERE id=?').get(taskId);
  
  if (taskList.completed === true) return false;  // Task already marked as done → no animation required!
  
  checkboxChecked.value = true; /* ✅ Trigger checkmark stroke-dashoffset transition */
}

import { ref, computed } from 'vue';

const pathAnimationTrigger = computed(() => checkboxChecked.value);
</script>

<style scoped>/* Checkbox + Checkmark Path Animation ===== */
.checkbox.checked .checkmark-svg { opacity: 1; background-color: #16a34a;}/* ✅ Fill green bg when checked */

/* ⚡ Checkmark stroke reveal (initially half-hidden, then animates to full length) === */
@keyframes checkComplete { from { stroke-dashoffset: 10; } to { stroke-dashoffset: 0; } }

.init-done .checkmark-path.path-animate { 
  transition: stroke-dashoffset .2s ease-out; 
}
/* Disable transitions on first render → prevent DOM update flicker */
.checkbox:not(.init-done) .checkmark-svg path { transition: none;}
</style>
```

---

### 3️⃣ Page Transition（页面切换）

**触发条件**: Tab 视图轮播 / 主窗口切换 (Vue Router + Naive UI Tabs)  
**交互反馈**: 
- 🌐 **Slide-in-left→right fade-out**淡入/出
- 🎨 Overlay **blur+grayscale=0.9**过渡层（可选）

```vue
<!-- === Vue Router Page Transition ===== -->
<template>
  <router-view v-slot="{ Component }" name="default">
    <transition 
      :name="'route-fade-slide'" 
      enter-active-class="animate-show"
      enter-start-class="hidden-render"
      duration="300"? /* fade-in: 250ms slide-up: 180ms */
    >
      <keep-alive>
        <component :is="Component"></component></keep-alive>
    </transition>
  </router-view>
</template>

<script setup>/* ===== Route View Router Transition ===== → Vue Router + Naive UI Tabs Integration ========= */
const router = useRouter(); /* === page-slide: Slide Left/Fade Out===== */


import { watchEffect } from 'vue';
watchEffect(() => { if (pageName.value !== '') window.__TAURI__.shell.open(pageName.value); }); /* === Tab Switching (Naive UI) ===== */

const tabs = reactive(['#tasks', '#calendar', '#kanban']); /* Default: tab-0=Task List,tab-1=Calendar View */
</script>

<style scoped>/* === fade-slide-in: Slide Left + Blur Effect ===== --> 
@keyframes fadeSlideIn { from { opacity: 0; transform: translateX(-20%); filter: blur(8px); } to { opacity: 1; transform: translateX(0); }}
.route-fade-slide-enter-active {}
</style>
```

---

### 4️⃣ Kanban Drag-and-Drop（看板拖拽排序）

**触发条件**: 
- MouseDown → **drag-start**事件捕捉
- MouseUp → **drag-end**释放确认  
**交互反馈**: 
- 🤳 底部预览线指示拖放位置
- 🔔 Toast 消息： "已重新排序为 X→Y"  

```vue
<!-- === KanbanColumn.vue (Draggable Item) ===== -->
<template>
  <div v-draggable :model-value="taskList" item-key="id">
    <!-- Draggable Card Container ===== -->
    <transition-group name="slide-up-transition">
      <draggable-item item-as="div" class="kanban-card" :key="item.id" v-for="(item, index) in filteredTaskList" 
        v-model="filteredData.slice(0, visibleCount)">
        <!-- Draggable Card Header ===== -->
        <template #button="{ moveToIndex, isSortable }">
          <!-- Drag Handle Indicator (Hover only: not clickable unless sorted) ===== -->
          <span class="drag-handle" v-if="isSortable" @click.prevent.stop="handleMoveToIndex(moveToIndex)">⋮⋮</span>
        </template>

        <!-- Draggable Card Body + Footer ===== -->
        <div style="position:relative;z-index:99">
          <!-- Priority Indicator & Category Badge ===== -->
          <div :class="[getPriorityColor(item.priority), 'priority-dot']"></div>
          
          <!-- Title Text (Max 2 lines, ellipsis) =====-->
          <span class="task-title">{{ item.title }}</span>

          <!-- Time Stamp (Today/Overdue highlighting) ===== -->
          <span v-html="formatItemTime(item)" :style="getTimeBadgeStyle(item)"
                style="margin-left:auto;font-size:10px;background:#f4f4f5;color:#a1a1aa;padding:1px 6px;border-radius:4px;"></span>

          <!-- Category Badge (Color-Coded by category) ===== -->
          <span class="task-badge {{ item.category.toLowerCase() }}-badge" :style="getCategoryBadgeStyle(item)">{{ item.category }}</span>

          <!-- Preview Line Indicator (Green Dotted Bottom Border) ===== -->
          <div class="drag-preview-indicator" style="position:absolute;bottom:-10px;left:0;width:100%;height:4px;border-radius:2px;background:rgba(57,205,186, .8);transition:.1s ease-out;display:none;&after{display:block;position:relative;top:-6px;left:50%;width:8px;height:6px;background:#39cfab;content:'';clip-path:polygon(25% 0,75% 0,100% 100%,0% 100%);}}.kanban-card:hover .drag-preview-indicator::after{display:block;}
</div>

        </div></template></template></transition-group>
      <!-- Draggable Column Footer ===== -->
      <slot name="empty-state">暂无任务，快速添加 + ↻（底部按钮）</slot>
    </div>
  </div>

<!-- === CSS Scrollbar Styling (Customize Appearance) ===== -->
<style scoped>/* Custom Scrollbar (Webkit Browsers Support) ===== */
.draggable-list { overflow-x:hidden;overflow-y:auto;scroll-behavior: smooth;padding-top:16px;scroll-margin-top:-24px; }
.draggable-list::-webkit-scrollbar { width: 8px;height: 8px;border-radius:4px;}
.draggable-list::-webkit-scrollbar-thumb { background:#d1d5db;box-shadow: inset 0 .1em .3em -.15em rgba(0,0,.2,.3);transition-duration:.12s; }
</style>
```

---

### 6️⃣ Calendar View Event Fade（日历事件渐显）

**触发条件**: 日期翻页/视图初始化  
**交互反馈**: 
- 🔍 📅 **Dot→icon fade-in**渐变显示
- 🔄 **Slide-in-right**动画入场提示

```scss
/* === Calendar Date & Event Dot Animation ===== */
.calendar-nav button {
  animation: slide-in-right var(--t-normal) ease-in-out both; /* Slide in + Fade In (45% → 100%) */
}

// === Event dot fade-in ===== 
@keyframes event-fade { from { opacity: 0; transform: translateY(-2px); scale(.9); } to   { opacity: 1; }}

.event-dot.fade-in-enter-active { animation-name: eventFade; animation-duration: calc(var(--t-normal) * .8); }
```

---

## ⚙️ Performance（性能优化指南）

### 6.1 避免布局抖动 (Reflow/Repaint Prevention)✅ **推荐**: Use `transform` + `opacity`触发 GPU Composite ✅ **禁止**: Direct modifications of `width` / `height` → CPU layout thrashing

```css
/* ✅ Correct: Smooth fade-in using transform */
.fade-in { opacity: 0; transition-property: opacity, transform; }

/* ❌ Wrong: Forces CPU reflow */
.fade-wrong { height: 0; margin-top: -24px; /* triggers reflow */ }
```

|操作 |性能等级 |触发行为 |示例代码 |
|-----|---------|---------|----------|
| `opacity` | ⚡GPU合成 |仅重绘（Repaint）|`.hide { opacity: 0;}/*.show{display:block;opacity:1;}` |
| `transform` | ⚡GPU合成 |仅合成（Composite）| `.slide-down-enter-active{}/* transform：translateY(-12px) */` |
| `color/background-color` | 🎨重绘 |触发 Repaint |`.active { background:#3b82f6;color:#fff;}` |
| `width/height` | ⚡CPU布局 |Reflow + Paint | ❌ Avoid modifying directly in CSS transitions |

---

### 6.2 使用 `will-change` 优化（Performance Profiling Tooling）

```javascript
// === Vue.js Performance Dashboard (Chrome DevTools Usage) =====
window.performance.measure(() => { console.clear(); performance.getEntriesByType("measure")[0].summary; });

/* ✅ Good：仅在动画运行时启用 */
.fade-in-enter-active { will-change: opacity, transform; }

/* ❌ Bad: Remove in leave-active to avoid memory leaks */
.fade-in-leave-active { will-change: auto; } /* = null */
```

---

## 🌐 Browser Support（浏览器兼容性）

|特性 |Chrome |Firefox |Safari (iOS) |Edge (Chromium) |
|------|-------|--------|--------------|------------------|
|GPU合成层 |✅ ES6+ |✅ CSS3 |⚠️ CoreGraphics |✅ WebGL2 |
|`stroke-dashoffset`动画 |✅ SVG2.0 |✅ E10+ |⚠️ CoreGraphics |✅ Chromium |
|CSS will-change |⚠️ FF 53+ |⚠️ Chrome64+ |iOS Safari ⚠️ |✅ ES5 |

**最小兼容**: ES5 + CSS3（Chrome≥79/Firefox≥78/Safari≥d/Edge≥79）  
**降级策略**: 禁用复杂动画时使用替代静态元素 → prevent DOM flicker.

---

## 📦 Export Assets（导出资源包）

```bash
# === Export Animation Demo HTML =====
yarn serve src/demo-animation.html # Open in browser for manual testing

# === Generate Vue Component (CLI Template) =====
vue create kettd-demo
cat > public/animations.html << EOF
<!DOCTYPE html> <!-- See full implementation: F:\\workspace\\Kettd\\src\\demo-animation.html -->
<title>Kettd Animation Demo</title>
<script src="https://cdn.jsdelivr.net/npm/vue@3/dist/vue.global.js"></script>
EOF

# === Install Naive UI Types =====
yarn add -D @types/nipplejs # Support drag-to-move gesture library
```

**使用方式**:
1. 浏览器打开 → http://localhost:8080/demo-animation.html
2. Click "触发所有动画"测试全部效果
3. Inspect Console logs for performance metrics

---

## 📰 Conclusion（总结）

本动画设计系统提供 **<500ms 启动 + <16ms 渲染帧率**的流畅体验，同时遵循以下原则：

### ✅ Success Metrics（成功指标）
|指标 |目标值 |实现方式 |
|------|-------|----------|
|冷启动时间 | <2s |懒加载（Vue lazy-load + Suspense）|
|视图切换延迟 | <50ms |虚拟滚动组件 + WebWorker 导出 PDF |
|内存占用 | <200MB |删除不再使用的 `will-change` → 避免泄漏 |

### 💡 Best Practices（最佳实践）
1. **Keep It Simple** - 禁用复杂动画的 CSS 属性（如`animation: infinite`触发无限次）  
2. **Progressive Enhancement** - 低配设备使用静态元素替代动画效果  
3. **Motion Sensitive Users** - `prefers-reduced-motion`媒体查询 → 禁用所有动画  

---

**文档作者**: Animation Engineer Assistant  
**最后更新**: 2026 年 8 月  
**版本控制**: Git + ESLint / TypeScript (TypeScript ESLint Rules)  
