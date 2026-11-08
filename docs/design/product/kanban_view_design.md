# 📋 看板视图（Kanban）功能深度设计

## 🎯 一、产品定位与目标

### 1.1 痛点分析

|用户场景 |当前竞品问题 |我们的解决方案 |
|----------|--------------|---------------|
|任务太多难以快速筛选 |Todoist/Notion: 只显示前 20 条 → 需手动展开 |✅ 拖拽排序重排 + 自定义过滤规则 |
|无法直观判断紧急程度 |仅用颜色提示（红黄绿）→ 易被忽视 | ✅ 优先级 + 截止日期双维度聚合展示 |
|重复任务需每次重新创建 |Things/Todoist: 手动勾选完成 → 次日重置待办列表 | ✅ "周期任务"智能循环：勾选后自动移至次日，用户主动删除才算结束 |
|跨项目协作需求弱 |单用户待办 + 简单@提及功能 | ✅ 支持自定义列（按项目/部门/客户分组）+ 共享看板模式 |

### 1.2 核心能力矩阵

|功能维度 | MVP 版本 | P2 版本 | P3 版本 |
|----------|---------|-------------|-------------|
|拖拽排序 (任务重排) | ✅ 基础版（列内排序） | ✅ 跨列拖放 | 🔄 自动吸附 + 智能分组 |
|重复任务周期管理 | ❌ 暂不支持 | ⚠️简单循环逻辑 | ✅ GTD-style 智能循环引擎 |
|优先级聚合显示 | ✅ 颜色提示 | ✅ 动态排序算法 | 📊优先级趋势图 |
|自定义过滤规则 | ⚠️仅日期过滤 | ✅ 多条件组合 | 🎯AI 自动推荐规则 |

---

## 🏗️ 二、看板数据模型设计 (SQL Schema)

### 2.1 Tasks Table（任务主表）

```sql
CREATE TABLE tasks (
    id                TEXT PRIMARY KEY,          /* UUID v4 → 唯一标识 */
    title             TEXT NOT NULL DEFAULT '' CACHE,           /* 标题 + emoji (最多 60 个字符) */
    description       TEXT,                      /* Markdown 描述内容 */
    category          TEXT NOT NULL CHECK(category IN ('work','personal','study','meet','other')),   /* ENUM: 工作/个人/学习/开会/其他 */
    priority           TEXT NOT NULL DEFAULT 'low' CHECK(priority IN ('high','medium','low')),     /* low/medium/high */
    due_date          TEXT,                      /* UTC ISO-8601 format (YYYY-MM-DDTHH:mm:ss) */
    completed_at      TEXT,                      /* 最后完成时间戳 → 用于判断"周期任务"已完成状态 */
    repeat_frequency  INTEGER DEFAULT 1,         /* 周期单位：每日/每周/每月/自定义 (天) */
    repeat_rule       TEXT DEFAULT 'daily',      /* daily/weekly/monthly/custom: N days */
    status            TEXT DEFAULT 'PENDING' CHECK(status IN ('PENDING','COMPLETED','ARCHIVED')), -- PENDING:待办完成 → COMPLETED 已归档/删除 | ARCHIVED:已彻底完成（用户主动）
    is_cycle_task     BOOLEAN DEFAULT FALSE,          /* 是否为"周期任务":TRUE→勾选后次日继续」显示，FALSE→单次任务 */
    
    created_at        TEXT NOT NULL DEFAULT (datetime('now','utc')),
    updated_at        TEXT NOT NULL DEFAULT (datetime('now','utc')),
    
    /* === Indexing for Performance ===== */
    INDEX idx_completed  (completed),
    INDEX idx_due_date   (due_date DESC),
    INDEX idx_category   (category, priority)
);

-- 关联子任务表（支持分解大任务为多步骤）
CREATE TABLE subtasks (
    id        TEXT PRIMARY KEY,
    todo_id   TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    description TEXT NOT NULL DEFAULT '',
    completed_at TEXT,
    INDEX idx_todo (todo_id)
);

-- 重复任务循环记录（用于审计 + 恢复）
CREATE TABLE cycle_history (
    id          TEXT PRIMARY KEY,
    task_id     TEXT NOT NULL REFERENCES tasks(id),
    completed_date TEXT NOT NULL, /* 实际完成的日期 */
    archived_date TEXT DEFAULT NULL   /* 用户主动归档的日期 → NULL:仍在周期中显示 */
);

/* === Views for Dashboard Analytics ===== */
CREATE VIEW pending_tasks_view AS
SELECT 
    t.id, t.title, t.due_date, t.priority, t.category,
    CASE WHEN t.completed_at <= datetime('now') THEN TRUE ELSE FALSE END as overdue
FROM tasks t WHERE t.is_cycle_task = TRUE AND (archived_date IS NULL OR archived_date > datetime('now'));

CREATE VIEW today_tasks_view AS
SELECT * FROM tasks WHERE date(due_date) = date('now') ORDER BY priority DESC, due_date ASC;
```

---

## 🔄 三、业务逻辑流程设计 (FSM)

### 3.1 任务状态机（状态变更规则）

```typescript
// === Task State Machine Definition =====
export interface TaskState {
  type: 'PENDING' | 'COMPLETED' | 'ARCHIVED'; // ✅ PENDING: 待办中 → COMPLETED: 已勾选完成 (次日重置）→ ARCHIVED: 彻底完成（用户主动删除/归档）  
}

export const taskStateMachine = {
  states: ['PENDING', 'COMPLETED'],       /* === 状态流转规则 ===== */  
  transitions: [
    { from: 'PENDING', to: 'COMPLETED', action: 'toggleComplete' },   /* ✅ 勾选完成 → 次日继续显示（周期任务）/ 归档删除结束（单次任务）→ ARCHIVED */
    
    // ⚠️ 例外：用户点击"跳过今日/不处理"按钮 → 自动移至 ARCHIVED
    { from: 'PENDING', to: 'ARCHIVED', condition: 'isSkipButtonClicked' }
  ]
};

// === 状态变更触发器 =====
const events = ['task-completed', 'skip-task', 'manual-archive'] as const;
events.forEach((name) => { /* Vue Event Bus binding */ });
```

### 3.2 重复任务循环逻辑（GTD-style）

```typescript
// === Task Cycle Logic Engine (Cycle Logic) =====
class CycleEngine {
  private tasks: Task[] = [];   // Vue reactive task list
  
  async handleTaskToggle(taskId: string): Promise<Task | null> {
    return new Promise(async(resolve) => {
      const task = this.tasks.find(t => t.id === taskId);
      if (!task || !task.is_cycle_task) return resolve(null as Task);   // 单次任务无周期 → 正常流程处理
      
      // ✅ 任务勾选完成 → 次日继续显示 (PENDING)状态重置为 true，status=ARCHIVED)
      await db.prepare(`UPDATE tasks SET completed_at=datetime('now','utc'), status='ARCHIVED' WHERE id=?`).run(taskId);

      // === 智能循环判断规则 ===== */  
      const isToday = task.due_date.split('T')[0] === new Date().toISOString().split('T')[0];
      const isOverdue = !isToday && isNaN(safeTime(task.due_date)); /* === 任务处理状态判断：今日到期未勾选 → 逾期标记 */
      
      if (task.category === 'meet') {   // ⚠️ 开会类别 → 按会议日程安排，不自动循环（需 manually confirm）  
        return resolve(task); 
      } else if (task.repeat_frequency <= 3 && task.priority === 'high' && isOverdue) {   
        // High priority tasks: Auto-loop to next day until completed (PENDING status reset to true + status=ARCHIVED)
        const tomorrow = new Date(Date.now() + 86400000).toISOString().split('T')[0]; 
        await db.prepare(`UPDATE tasks SET due_date=?,priority='high' WHERE id=?`).run(tomorrow, task.id);
        resolve(task);   /* ✅ 次日继续显示 (PENDING) → 用户主动删除/归档才能结束 */  
      } else {   
        // Low-priority: Skip for current day → move to ARCHIVED immediately 
        return resolve(null);
      }
    });
  }

  async markTaskTerminated(taskId: string, reason: 'user-deleted' | 'archived'): Promise<void> {
    await db.prepare(`UPDATE tasks SET status='ARCHIVED',completed_at=datetime('now','utc') WHERE id=?`).run(taskId);
    // Log cycle history (for analytics) → track repeat_frequency count before termination
  }
}
```

---

## 🎨 四、看板前端实现（Vue + Draggable）

### 4.1 拖拽排序组件实现

```vue
<!-- src/components/KanbanColumn.vue -->
<script setup>
import { ref, computed, onMounted } from 'vue';
import draggableItem from './DraggableItem.vue'

interface KanbanColumnProps {
  columnId: string;           /* todo/inprogress/review/done */
  filterByPriority?: boolean; // 根据优先级过滤显示任务（高/中/低）  
}

const props = defineProps({columnId: String, filterByPriority: Boolean}) as KanbanColumnProps;
const taskId = ref<string>('');     /* === Drag Handle (Drag-to-reorder) ===== */
```

### 4.2 看板视图完整组件（Vue SFC）

```vue
<!-- src/views/KanbanView.vue -->
<script setup>
import { reactive, computed, watch } from 'vue';

/* === Kanban View: Task List Rendering ===== */  
interface KanbanTask {
  title: string;              /* 任务标题 + emoji (最多 60 个字符) */
  priority: 'high'|'medium'|'low';   /* low/medium/high */
  category: string;           /* work/personal/study/meet */
  due_date?: string;          /* due_date (UTC ISO) format */
}

interface KanbanColumn {
  id: string;                 /* todo/inprogress/review/done */
  label: string;              /* 自定义列名：待办/进行中/等待审核/已完成 */
  filterByPriority?: boolean; /* 按优先级过滤显示任务（高/中/低） */
}

const columns = reactive<KanbanColumn[]>([                         /* === Kanban Columns Definition ===== */  
  { id: 'todo', label: '待处理', filterByPriority: false },         /* ✅ 基础过滤器：仅显示 PENDING → is_cycle_task=true tasks */
  { id: 'inprogress', label: '进行中', filterByPriority: true },    /* ⚠️ 需勾选完成才能流转至下一列（drag-and-drop across columns） */
  { id: 'review', label: '等待审核', filterByPriority: false },       /* review column (customizable by user) */
  { id: 'done', label: '已完成', filterByPriority: true }           /* done column → show only COMPLETED tasks (not PENDING) */  
]);

/* === Computed Properties ===== */  
const filteredTasks = computed<KanbanTask[]>(() => { 
  const now = new Date().toISOString();
  return tasks.value.filter(t => {
    const isDone = t.completed_at;
    if (!isDone || isNaN(parseInt(isDone.toString()))) return false; /* === Task State Filter ===== */  
    if (t.category === 'meet') {   /* ⚠️ 开会类别 → 不自动循环 → manually confirm only */ 
      return now >= t.due_date && !isOverdue(now);
    } else {   
      /* === Cycle Logic: Only show pending tasks that haven't been skipped ===== -->  
      const today = date.now().toISOString().split('T')[0];
      return t.cycle_status === 'PENDING' && t.category !== 'meet';
    } 
  }); // Vue computed property (lazy-load)  
});
</script>

<template>
  <div class="kanban-board">
    <!-- Column Header ===== -->
    <h3 style="font-size:12px;font-weight:600;color:#9ca3af;margin-top:auto;">{{ currentColumn.label }} - {{ currentColumn.filterByPriority?' (按优先级过滤)':'' }}</h3>
    
    <!-- Task List (Draggable Items Rendering ===== -->  
    <div v-draggable :model-value="filteredTasks" item-key="id"> 
      <!-- Card Container ===== -->
      <draggable-item item-as="div" class="kanban-task-card" v-for="(task,index) in filteredTasks" 
          :key="index + task.id"
          @click="onTaskCardClick(task)">
        <!-- Task Title (Max 2 lines) ===== -->  
        <span style="font-size:13px;font-weight:500;color:#475569;overflow:hidden;text-overflow ellipsis">
          {{ task.title.slice(0, 60) }}{{ task.due_date ? formatDate(task.due_date):'' }}
          <!-- @ Mention (Optional: Auto-link other notes/tasks ===== -->  
          <span v-if="task.title.includes('@')" style="color:#3b82f6;border:1px solid #e4e4e7;padding:6px 12px;margin-top:auto">
            @{{ cleanAtMention(task.title) }} (可选：点击跳转关联笔记/任务）
          </span>
        </span> 
        <!-- Priority Indicator & Category Badge ===== -->  
        <div :class="[getPriorityColor(task.priority), 'priority-dot']"></div>

        <!-- Time Stamp (Today/Overdue highlighting) ========= -->
        <span v-html="formatItemTime(task)" :style="getTimeBadgeStyle(task)", style="margin-left:auto;font-size:10px;background:#f4f4f5;color:#a1a1aa;padding:1px 6px;border-radius:4px;"></span>

        <!-- Category Badge (Color-Coded ===== -->
        <span class="task-badge {{ task.category.toLowerCase() }}-badge" :style="getCategoryBadgeStyle(task)">{{ getCategoryColor(task.category).text }}</span>
</div>

    </div><!-- Preview Line Indicator ===== -->
    
  </div>  
</template>

<script setup>
import { ref } from 'vue';

/* === Task Filtering Logic ===== */
const filterRule = reactive({priority: [] as string[],category: [] as string[]});
const todayDate = computed(() => (new Date().toISOString().split('T')[0]));
</script>

<style scoped>
/* === Kanban Board Layout ===== */
.kanban-board { display:flex;gap:12px;overflow-x:auto;padding-top:16px; }

/* === Drag Handle Animation ===== --> 
.draggable-item:hover{transform:translateY(-3px);box-shadow: 0 4px 8px rgba(0,0,0,.05);}
</style>
```

---

## 🌐 五、浏览器兼容性与降级策略

|特性 |Chrome |Firefox |Safari (iOS) |Edge (Chromium) |兼容性等级 |
|------|-------|--------|--------------|------------------|----------|
|drag-and-drop API |✅ ES6+ | ⚠️ 需 polyfill | ⚠️ iOS15+ | ✅ Chromium |⚡ P2/P3 |  
|`transform:translateY()` |⚡ GPU合成 | ⚡ CSS3 | iOS CoreGraphics | ✅ WebGL2 |🎯 MVP |

**降级策略**: 
- Safari 10+ → 使用静态元素替代动画效果
- Android 8.0 → 禁用复杂过渡（`transition: none`）  

---

## 📊 六、性能优化与验收标准

|指标 |目标值 |实现方式 |
|------|-------|----------|
|看板视图冷启动时间 | <1s (首屏)<br/>✅ Vue SSR + Lazy-load组件 | Vue Suspense懒加载图片/图表（避免冷启动卡顿） |
|跨列拖放延迟 | <10ms (GPU合成) | 使用 `transform`+`will-change`替代直接修改盒模型 → CPU 布局抖动（Reflow/Repaint Prevention） |

**验收测试用例**:  
- TC-KAN-001: 跨列拖放重排成功（视觉无跳变）  
- TC-KAN-002: 超过 50 项任务虚拟滚动正常渲染 ≤ 30fps (GPU合成)  

---

**报告撰写**: AI Assistant  
**业务分析师**: Kanban Product Owner (待配置）  
**评审日期**: 2026 年 8 月  
