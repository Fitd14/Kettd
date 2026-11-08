# 📋 看板视图功能完整文档 v1.0

## 🎯 一、产品概述

### 1.1 目标用户场景

|用户类型 |看板使用方式 |价值主张 |
|----------|--------------|----------|
|**管理者** |按项目/部门分组列 → 监控全员进展 | ✅ 跨团队协作 + 进度可视化 |  
|**执行者** |个人任务拖拽排序 → 快速调整优先级 | ⚡ GTD-style "2 分钟规则"：低挂果任务立即处理 |
|**学生党** |学习周期管理（每日自习 → 每周复习） | 📖 知识卡片 + 复习间隔法 |

### 1.2 核心功能清单

|功能点 |MVP |P2|P3 |竞品对标 |
|--------|-----|-- |-- -----|
|✅拖拽排序 (列内/跨列) |✅ | ✅ |Todoist |  
|🔁重复任务智能循环 |❌ | ⚠️简单规则引擎 | GTD 实践书 |  
|⏰优先级聚合显示 | ⚠️颜色提示 | ✅排序算法 |滴答清单 |
|@关联笔记/任务 | ❌ | ✅ 轻量级提及功能 |Notion |  
|🎨自定义列配置 | ⚠️仅日期过滤 | ✅多条件组合 |Trello |  

---

## 📊 二、看板视图数据模型 (SQL Schema)

### 2.1 Tasks Table（任务主表）

```sql
-- === Tasks Table ===== --> 
CREATE TABLE tasks (
    id                 TEXT PRIMARY KEY,          /* UUID v4 */
    title              TEXT NOT NULL DEFAULT '' CACHE,           /* 标题 + emoji (最多 60 个字符) */
    description        TEXT,                      /* Markdown 描述内容 */
    category           TEXT CHECK(category IN ('work','personal','study','meet')),         /* 工作/个人/学习/开会 */  
    priority           TEXT DEFAULT 'low' CHECK(priority IN ('high','medium','low')),     /* low/medium/high */
    due_date           TEXT,                      /* UTC ISO-8601 format (YYYY-MM-DDTHH:mm:ss) */
    completed_at       TEXT,                      /* 最后完成时间戳 →用于判断"周期任务"已完成状态 */  
    repeat_frequency   INTEGER DEFAULT 1,         /* 1=每日/7=每周/30=每月/自定义 (天) */
    repeat_rule        TEXT DEFAULT 'daily',      /* daily/weekly/monthly/custom: N days */
    status             TEXT DEFAULT 'PENDING' CHECK(status IN ('PENDING','COMPLETED','ARCHIVED')), -- PENDING:待办完成 → COMPLETED 已归档 | ARCHIVED:已彻底完成，用户主动删除 |  
    is_cycle_task      BOOLEAN DEFAULT FALSE,          /* TRUE→勾选后次日继续显示」FALSE→单次任务 */
    
    created_at         TEXT NOT NULL DEFAULT (datetime('now','utc')),
    updated_at         TEXT NOT NULL DEFAULT (datetime('now','utc'))

-- 索引优化（性能）  
CREATE INDEX idx_completed_tasks ON tasks(status,due_date DESC);  
CREATE INDEX idx_due_date_priority ON tasks(completed, due_date DESC,priority);  
```

---

## 🔄 三、重复任务循环逻辑（周期管理引擎）

### 3.1 智能循环判断规则

|规则类型 |触发条件 |处理结果 |示例 |
|----------|----------|---------|------|
|**每日/每周任务** | `is_cycle_task=true`+`repeat_frequency<=7`→勾选完成 →次日自动恢复显示 | ✅ 重复任务循环引擎：PENDING→COMPLETED→ARCHIVED（用户主动） | "每天早晨跑步 30 分钟"任务勾选后，次日继续显示在看板列表中 |  
|**月度里程碑** | `repeat_rule='monthly'→每月 15 日自动创建新实例` | 📅 自动循环：PENDING→COMPLETED (不立即恢复) →ARCHIVED(月末) | "每月复盘会议"任务完成归档到历史记录，无需手动重复创建 |  
|**自定义周期** | `repeat_frequency=N days→每 N 天重置一次` | ⏰ 智能计算下次到期日期（考虑节假日/周末） | "每 7 天提交代码报告"N=7 天后自动恢复显示 PENDING状态 |  

### 3.2 循环引擎算法实现

```typescript
// === Cycle Logic Engine ===== -->  
class CycleEngine {
  private db = new SQLiteConnection('todos.db');   /* === Task State Filter ===== */   
  async handleCycle(task: Task): Promise<void> {
    const isToday = task.due_date.split('T')[0] === new Date().toISOString().split('T')[0];
    const isOverdue = task.is_cycle_task && !isToday && isNaN(safeTime(task.due_date));     
    
    if (task.repeat_frequency <= 3 && task.priority === 'high' && isOverdue) {   
      const tomorrow = new Date(Date.now() +86400000).toISOString().split('T')[0];
      await db.prepare(`UPDATE tasks SET due_date=?, priority='high' WHERE id=?`).run(tomorrow, task.id);
    } else if (task.category !== 'meet') {   
      // ⚠️ 开会类别 → 不自动循环，需手动确认 (manually confirm)  
      return; 
    }
    
    console.log(`Periodic loop triggered: ID=${task.id} → next due_date=${tomorrow}`); // 📊 Analytics tracking
  }

  skipToday(taskId: string): void {   /* === Skip Button Handler ===== -->  
    const task = this.db.findTaskById(taskId);   
    if (!task || !task.is_cycle_task) return; 
    
    const now = new Date().toISOString();
    await db.prepare(`UPDATE tasks SET status='ARCHIVED',completed_at=datetime('now','utc') WHERE id=?`).run(taskId);
    console.log(`User skipped task ${taskId}: Status=ARCHIVED → next cycle reset after 24h`);
  }
}
```

---

## 🎨 四、前端组件实现（Vue + Draggable）

### 4.1 KanbanColumn 组件（任务列表容器）

```vue
<!-- src/components/KanbanColumn.vue -->
<script setup>
import { ref, computed, onMounted } from 'vue';
// Vue 3 Composition API: reactive state management  
interface KanbanTask {
  title: string;              /* 任务标题 + emoji (最多 60 个字符) */
  priority: 'high'|'medium'|'low';   /* low/medium/high */
  category: 'work'|'personal'|'study'|'meet';
  due_date?: string;          /* UTC ISO format:YYYY-MM-DDTHH:mm:ssZ*/
}

interface KanbanColumn {
  id: string,                 /* todo/inprogress/review/done */
  label: string;              /* Column header text */  
  filterByPriority?: boolean; /* Show high-priority tasks only? */
}

const props = defineProps({columnId: String, filterByPriority: Boolean}) as KanbanColumn;
const taskList = ref<KanbanTask[]>([]);   /* === Task Filtering Logic ===== -->  

/* Computed Properties (Lazy-load) ===== */  
const filteredTasks = computed(() => {  return taskList.value.filter(t=>{      
  if (t.category==='meet') return false; // ⚠️ Meeting tasks → manually confirm only  
  const todayStr = new Date().toISOString().split('T')[0];
  return t.is_cycle_task && t.status==='PENDING' && date.now() < new Date(todayStr).getTime(); 
}});

/* === Drag Handle (Drag-to-reorder) ===== -->  
const draggableItem = ref({class:'drag-handle', onClick: ()=>{ moveTaskToIndex(currentIndex, newIndex); }}),
</script>

<template>
  <div class="kanban-task-column">
    <!-- Task Items Rendering ===== -->
    <draggable-item item-as="div" v-for="(task,index) in filteredTasks">
      <!-- Card Header ===== -->
      <span style="font-size:12px;font-weight:600;color:#9ca3af">{{ task.title }}:</span> 
      
      <!-- Priority Indicator & Category Badge ===== -->  
      <div :class="[getPriorityColor(task.priority), 'priority-dot']"></div>

      <!-- Time Stamp (Today/Overdue highlighting) ===== -->    
      <span v-html="formatItemTime(task)" :style="getTimeBadgeStyle(task),">
        {{ formatDate(task.due_date).replace('今天','📅 今天').replace(' overdue','❌ 已逾期') }}
      </span>

      <!-- Category Badge (Color-Coded) ===== -->  
      <span class="task-badge {{ task.category.toLowerCase() }}-badge" :style="getCategoryBadgeStyle(task)">
        {{ getCategoryColor(task.category).text }}
      </span>
      
      <!-- Repeat Task Icon (Cycle Indicator) ===== -->  
      <template v-if="task.is_cycle_task">
        <span class="cycle-badge" style="background:#dbeafe;border-radius:4px;font-size:9px;color:#1e40af;padding:1px 6px;">
          🔁 重复任务 - {{ task.repeat_frequency ===1?'每日':task.repeat_frequency===7?'每周':'每 N 天' }}  
        </span>
      </template>

    </draggable-item></draggable-item> <!-- Empty State (No Tasks) ===== --> <div v-if="filteredTasks.length===0" class="kanban-empty-state">暂无任务，快速添加 '+'</div></section></div> <!-- Footer: Quick Add Button ===== --> <button @click="showQuickAddModal()" class="quick-add-btn">➕ 快速添加</button></template>
  
<script setup>
/* === Quick Add Modal (Typora-style input) ===== */  
const quickAddContent = ref('');    // Vue reactive string state  
const showQuickAddModal = (): void => { document.querySelector('.modal-overlay')?.classList.add('show'); },
```

---

## 🌐 五、浏览器兼容性检查

|特性 |Chrome |Firefox |Safari (iOS) |兼容性等级 |
|------|-------|--------|----------|----------|
|Vue Composition API | ✅ ES6+ |⚠️ polyfill | iOS13+ | MVP |  
|`transform:translateY()` | ⚡ GPU合成 | CSS3 | CoreGraphics | P2/P3 |  
|@提及功能 (Cross-column linking) | ⚡ Naive UI 内置支持 |⚠️ 需手动实现 | P3 |  

---

## 📊 六、验收标准

|验收项 |测试方法 |通过条件 |
|--------|---------|---------|
|TC-KAN-001:✅拖拽排序无视觉跳变 | UI 回放 +逐帧检查 DOM | <10ms (GPU合成) |  
|TC-KAN-002:🔁周期任务正确循环 | Chrome DevTools Console logs | ✅自动恢复显示在 PENDING 列表 |
|TC-KAN-003:⏰优先级聚合排序 | 高优先级任务置顶 + 逾期标记 | <50ms (Vue computed) |  

---

**撰写**: AI Assistant  
**评审日期**: 2026 年 8 月  
