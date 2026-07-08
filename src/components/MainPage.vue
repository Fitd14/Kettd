<template>
  <div class="body">
    <nav class="sidebar">
      <div class="nav-section">
        <div 
          v-for="item in navItems" 
          :key="item.id"
          class="nav-item"
          :class="{ active: currentView === item.id }"
          @click="changeView(item.id)"
        >
          <div class="nav-icon" :style="{ background: item.color }">
            <svg v-html="item.icon" width="12" height="12" viewBox="0 0 12 12" fill="none"></svg>
          </div>
          <span class="nav-label">{{ item.label }}</span>
          <span class="nav-count">{{ getNavCount(item.id) }}</span>
        </div>
      </div>
      <div class="sidebar-divider"></div>
      <div class="sidebar-label">分类</div>
      <div class="nav-section">
        <div 
          v-for="cat in categories" 
          :key="cat.id"
          class="nav-item category-item"
          :class="{ active: currentView === cat.id }"
          @click="changeView(cat.id)"
        >
          <div class="cat-dot" :style="{ background: cat.color }"></div>
          <span class="nav-label">{{ cat.label }}</span>
          <span class="nav-count">{{ getCategoryCount(cat.label) }}</span>
        </div>
      </div>
      <div class="sidebar-spacer"></div>
      <div class="sidebar-footer">
        <div class="progress-info">
          <span>今日进度</span>
          <span class="progress-text">{{ todayProgress }}</span>
        </div>
        <div class="progress-bar">
          <div class="progress-fill" :style="{ width: todayProgressPercent + '%' }"></div>
        </div>
      </div>
    </nav>

    <main class="main-content">
      <div class="task-panel">
        <div class="filter-bar">
          <button 
            v-for="filter in filters" 
            :key="filter.id"
            class="filter-btn"
            :class="{ active: currentFilter === filter.id }"
            @click="changeFilter(filter.id)"
          >
            {{ filter.label }}
          </button>
        </div>
        <div class="stats-row">
          <div class="stat-card">
            <div class="stat-label">今日任务</div>
            <div class="stat-value" style="color:#18181b;">{{ stats.today }}</div>
          </div>
          <div class="stat-card">
            <div class="stat-label">已完成</div>
            <div class="stat-value" style="color:#16a34a;">{{ stats.completed }}</div>
          </div>
          <div class="stat-card">
            <div class="stat-label">已逾期</div>
            <div class="stat-value" style="color:#dc2626;">{{ stats.overdue }}</div>
          </div>
        </div>
        <div class="section-header">
          <div class="section-title-wrap">
            <h2>{{ sectionTitle }}</h2>
            <span class="section-count">({{ filteredTasks.length }})</span>
          </div>
          <div class="sort-btn">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <path d="M1.5 4.5L7 10L12.5 4.5" stroke="#71717a" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            <span>排序</span>
          </div>
        </div>
        <div class="task-list">
          <div 
            v-for="task in filteredTasks" 
            :key="task.id"
            class="task-item"
            :class="{ completed: task.completed }"
            @click="selectTask(task)"
          >
            <div 
              class="task-checkbox"
              :class="{ checked: task.completed }"
              @click.stop="handleToggleTask(task.id)"
            >
              <svg v-if="task.completed" width="12" height="12" viewBox="0 0 12 12" fill="none">
                <path d="M3 6L5 8L9 4" stroke="#ffffff" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </div>
            <div class="task-content">
              <div class="task-title">{{ task.title }}</div>
              <div class="task-meta">
                <span v-if="formatDate(task.due_date) === '已逾期'" class="overdue-badge">已逾期</span>
                <span class="task-date">{{ task.completed ? formatDate(task.due_date).split(' ')[0] : formatDate(task.due_date) }}</span>
                <span class="task-badge" :style="getCategoryBadgeStyle(task.category)">{{ task.category }}</span>
              </div>
            </div>
            <div class="task-actions">
              <template v-if="!task.completed">
                <div class="priority-dot" :style="{ background: getPriorityColor(task.priority) }"></div>
                <svg class="action-icon" @click.stop="$emit('open-modal', task)" width="16" height="16" viewBox="0 0 16 16">
                  <path d="M10.5 2.5L13.5 5.5M2 10L2.5 7.5L10 2L13 4L6.5 11.5L4 12L2 10Z" stroke="#a1a1aa" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
                <svg class="action-icon" @click.stop="handleDeleteTask(task.id)" width="16" height="16" viewBox="0 0 16 16">
                  <path d="M3 2L13 2L13 4L8 8.5L8 13L5 14L5 8.5L3 4Z" stroke="#a1a1aa" stroke-width="1.2" stroke-linejoin="round"/>
                </svg>
              </template>
              <span v-else class="completed-badge">已完成</span>
            </div>
          </div>
        </div>
      </div>

      <div class="detail-panel">
        <div v-if="selectedTask" class="detail-content">
          <h1>{{ selectedTask.title }}</h1>
          <div class="detail-status-bar">
            <span :class="selectedTask.completed ? 'status-completed' : 'status-active'">{{ selectedTask.completed ? '已完成' : '进行中' }}</span>
            <div class="priority-info">
              <div class="priority-dot" :style="{ background: getPriorityColor(selectedTask.priority) }"></div>
              <span>优先级：{{ priorityLabels[selectedTask.priority] }}</span>
            </div>
          </div>
          <div class="detail-info">
            <div class="info-item">
              <svg width="15" height="15" viewBox="0 0 15 15" fill="none">
                <rect x="1.5" y="3" width="12" height="10" rx="1.5" stroke="#a1a1aa" stroke-width="1.2"/>
                <path d="M5 1.5V3M10 1.5V3" stroke="#a1a1aa" stroke-width="1.2" stroke-linecap="round"/>
                <path d="M4.5 7H10.5" stroke="#a1a1aa" stroke-width="1" stroke-linecap="round"/>
              </svg>
              <span>截止日期：{{ selectedTask.due_date }}</span>
            </div>
            <div class="info-item">
              <div class="cat-dot" :style="{ background: getCategoryColor(selectedTask.category).dot }"></div>
              <span>分类：{{ selectedTask.category }}</span>
            </div>
          </div>
          <div class="detail-divider"></div>
          <div class="detail-section">
            <h3>描述</h3>
            <p>{{ selectedTask.description || '暂无描述' }}</p>
          </div>
          <div class="detail-divider"></div>
          <div class="detail-section">
            <h3>子任务 ({{ completedSubtasksCount }}/{{ selectedTask.subtasks.length }})</h3>
            <div class="subtasks-list">
              <div 
                v-for="subtask in selectedTask.subtasks" 
                :key="subtask.id"
                class="subtask-item"
                :class="{ completed: subtask.completed }"
              >
                <div 
                  class="subtask-checkbox"
                  :class="{ checked: subtask.completed }"
                  @click="handleToggleSubtask(selectedTask.id, subtask.id)"
                >
                  <svg v-if="subtask.completed" width="9" height="9" viewBox="0 0 9 9" fill="none">
                    <path d="M2 4.5L3.5 6L7 2.5" stroke="#fafafa" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                  </svg>
                </div>
                <span :class="{ completed: subtask.completed }">{{ subtask.title }}</span>
              </div>
            </div>
          </div>
          <div class="detail-divider"></div>
          <div class="detail-section">
            <h3>备注 ({{ selectedTask.notes.length }})</h3>
            <div class="notes-list">
              <div v-for="note in selectedTask.notes" :key="note.id" class="note-item">
                <div class="note-avatar" :style="{ background: note.author === '我' ? 'linear-gradient(135deg,#6366f1,#8b5cf6)' : 'linear-gradient(135deg,#f59e0b,#ef4444)' }">
                  {{ note.author.charAt(0) }}
                </div>
                <div class="note-content">
                  <div class="note-header">
                    <span class="note-author">{{ note.author }}</span>
                    <span class="note-time">{{ note.created_at }}</span>
                  </div>
                  <p>{{ note.content }}</p>
                </div>
              </div>
            </div>
            <textarea v-model="newNote" class="note-input" placeholder="添加备注..."></textarea>
            <button class="add-note-btn" @click="handleAddNote">添加备注</button>
          </div>
        </div>
        <div v-else class="empty-detail">
          <div class="empty-icon">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
              <path d="M9 5H7C5.9 5 5 5.9 5 7V19C5 20.1 5.9 21 7 21H17C18.1 21 19 20.1 19 19V7C19 5.9 18.1 5 17 5H15" stroke="#a1a1aa" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
              <path d="M9 5C9 3.9 9.9 3 11 3H13C14.1 3 15 3.9 15 5C15 6.1 14.1 7 13 7H11C9.9 7 9 6.1 9 5Z" stroke="#a1a1aa" stroke-width="1.5"/>
              <path d="M9 12H15M9 16H13" stroke="#a1a1aa" stroke-width="1.5" stroke-linecap="round"/>
            </svg>
          </div>
          <p class="empty-title">选择一个任务查看详情</p>
          <p class="empty-desc">点击左侧任务列表中的任务<br/>即可在此处查看详细信息</p>
        </div>

        <div v-if="selectedTask" class="detail-footer">
          <button class="btn btn-primary" @click="$emit('open-modal', selectedTask)">
            <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
              <path d="M8.5 2.5H4.5C3.5 2.5 2.5 3 2.5 4.5V8.5C2.5 10 3.5 10.5 4.5 10.5H8.5C9.5 10.5 10.5 10 10.5 8.5V4.5C10.5 3 9.5 2.5 8.5 2.5Z" stroke="#fafafa" stroke-width="1.2"/>
              <path d="M5 6.5L6 7.5L8.5 5" stroke="#fafafa" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            编辑
          </button>
          <button class="btn btn-danger" @click="handleDeleteTask(selectedTask.id)">
            <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
              <path d="M3.5 3.5L9.5 9.5M9.5 3.5L3.5 9.5" stroke="#ef4444" stroke-width="1.2" stroke-linecap="round"/>
            </svg>
            删除
          </button>
          <div class="footer-spacer"></div>
          <button class="close-detail-btn" @click="selectedTask = null">
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
              <path d="M8 3L4 7M4 3L8 7" stroke="#71717a" stroke-width="1.2" stroke-linecap="round"/>
            </svg>
          </button>
        </div>
      </div>
    </main>
  </div>
</template>

<script setup>
import { ref, computed, watch } from 'vue';
import { toggleTask, deleteTask, toggleSubtask, addNote, generateId, formatDate, getCategoryColor, getPriorityColor } from '../api';

const props = defineProps({
  tasks: { type: Array, default: () => [] },
  searchQuery: { type: String, default: '' }
});

const emit = defineEmits(['task-updated', 'open-scheduled', 'open-modal']);

const currentView = ref('inbox');
const currentFilter = ref('all');
const selectedTaskId = ref(null);
const newNote = ref('');

const selectedTask = computed(() => {
  if (!selectedTaskId.value) return null;
  return props.tasks.find(t => t.id === selectedTaskId.value) || null;
});

watch(() => props.tasks, () => {
  if (selectedTaskId.value && !props.tasks.find(t => t.id === selectedTaskId.value)) {
    selectedTaskId.value = null;
  }
}, { deep: true });

const priorityLabels = {
  high: '高',
  medium: '中',
  low: '低'
};

const navItems = [
  { id: 'inbox', label: '收件箱', color: '#3b82f6', icon: '<path d="M2 3L6 1L10 3V8L6 11L2 8V3Z" stroke="white" stroke-width="1.2"/>' },
  { id: 'today', label: '今天', color: '#f59e0b', icon: '<circle cx="6" cy="6" r="4.5" stroke="white" stroke-width="1.2"/><path d="M6 3.5V6L8 7.5" stroke="white" stroke-width="1.2" stroke-linecap="round"/>' },
  { id: 'upcoming', label: '即将到来', color: '#10b981', icon: '<rect x="1" y="3" width="10" height="8" rx="1.5" stroke="white" stroke-width="1.2"/><path d="M3 1V3M9 1V3" stroke="white" stroke-width="1.2" stroke-linecap="round"/>' },
  { id: 'completed', label: '已完成', color: '#8b5cf6', icon: '<circle cx="6" cy="6" r="4.5" stroke="white" stroke-width="1.2"/><path d="M4 6L5.5 7.5L8.5 4.5" stroke="white" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>' }
];

const categories = [
  { id: 'work', label: '工作', color: '#ef4444' },
  { id: 'personal', label: '个人', color: '#3b82f6' },
  { id: 'study', label: '学习', color: '#10b981' }
];

const filters = [
  { id: 'all', label: '全部' },
  { id: 'work', label: '工作' },
  { id: 'personal', label: '个人' },
  { id: 'study', label: '学习' },
  { id: 'important', label: '重要' }
];

const viewTitles = {
  inbox: '收件箱',
  today: '今天',
  upcoming: '即将到来',
  completed: '已完成',
  work: '工作',
  personal: '个人',
  study: '学习'
};

const today = new Date().toISOString().split('T')[0];

const stats = computed(() => {
  const todayTasks = props.tasks.filter(t => {
    const dueDate = t.due_date.split(' ')[0];
    return dueDate === today || t.due_date.includes(today);
  });
  const completedCount = props.tasks.filter(t => t.completed).length;
  const overdueCount = props.tasks.filter(t => {
    if (t.completed) return false;
    const dueDate = new Date(t.due_date.split(' ')[0]);
    dueDate.setHours(0, 0, 0, 0);
    return dueDate < new Date();
  }).length;

  return {
    today: todayTasks.length,
    completed: completedCount,
    overdue: overdueCount
  };
});

const todayProgress = computed(() => {
  const todayTasks = props.tasks.filter(t => {
    const dueDate = t.due_date.split(' ')[0];
    return dueDate === today || t.due_date.includes(today);
  });
  const pending = todayTasks.filter(t => !t.completed).length;
  const completed = todayTasks.filter(t => t.completed).length;
  return `${completed}/${pending + completed}`;
});

const todayProgressPercent = computed(() => {
  const todayTasks = props.tasks.filter(t => {
    const dueDate = t.due_date.split(' ')[0];
    return dueDate === today || t.due_date.includes(today);
  });
  const pending = todayTasks.filter(t => !t.completed).length;
  const completed = todayTasks.filter(t => t.completed).length;
  const total = pending + completed;
  return total > 0 ? (completed / total) * 100 : 0;
});

const sectionTitle = computed(() => viewTitles[currentView.value] || '收件箱');

const filteredTasks = computed(() => {
  let filtered = [...props.tasks];
  
  if (props.searchQuery) {
    const query = props.searchQuery.toLowerCase();
    filtered = filtered.filter(t => t.title.toLowerCase().includes(query));
  }

  if (currentView.value === 'completed') filtered = filtered.filter(t => t.completed);
  else if (currentView.value === 'today') {
    filtered = filtered.filter(t => {
      const dueDate = t.due_date.split(' ')[0];
      return dueDate === today || t.due_date.includes(today);
    });
  } else if (currentView.value === 'upcoming') filtered = filtered.filter(t => !t.completed);
  else if (currentView.value === 'work' || currentView.value === 'personal' || currentView.value === 'study') {
    const names = { 'work': '工作', 'personal': '个人', 'study': '学习' };
    filtered = filtered.filter(t => t.category === names[currentView.value]);
  }

  if (currentFilter.value === 'work') filtered = filtered.filter(t => t.category === '工作');
  else if (currentFilter.value === 'personal') filtered = filtered.filter(t => t.category === '个人');
  else if (currentFilter.value === 'study') filtered = filtered.filter(t => t.category === '学习');
  else if (currentFilter.value === 'important') filtered = filtered.filter(t => t.priority === 'high');

  return filtered;
});

const completedSubtasksCount = computed(() => {
  if (!selectedTask.value) return 0;
  return selectedTask.value.subtasks.filter(s => s.completed).length;
});

function getNavCount(viewId) {
  if (viewId === 'inbox') return props.tasks.length;
  if (viewId === 'today') {
    return props.tasks.filter(t => {
      const dueDate = t.due_date.split(' ')[0];
      return dueDate === today || t.due_date.includes(today);
    }).length;
  }
  if (viewId === 'upcoming') return props.tasks.filter(t => !t.completed).length;
  if (viewId === 'completed') return props.tasks.filter(t => t.completed).length;
  return 0;
}

function getCategoryCount(category) {
  return props.tasks.filter(t => t.category === category).length;
}

function getCategoryBadgeStyle(category) {
  const color = getCategoryColor(category);
  return {
    background: color.bg,
    color: color.text
  };
}

function changeView(viewId) {
  currentView.value = viewId;
}

function changeFilter(filterId) {
  currentFilter.value = filterId;
}

function selectTask(task) {
  selectedTaskId.value = task.id;
}

async function handleToggleTask(taskId) {
  await toggleTask(taskId);
  emit('task-updated');
}

async function handleDeleteTask(taskId) {
  if (confirm('确定删除此任务？')) {
    await deleteTask(taskId);
    emit('task-updated');
    selectedTaskId.value = null;
  }
}

async function handleToggleSubtask(taskId, subtaskId) {
  await toggleSubtask(taskId, subtaskId);
  emit('task-updated');
}

async function handleAddNote() {
  if (!newNote.value.trim() || !selectedTask.value) return;
  await addNote(selectedTask.value.id, {
    id: generateId(),
    author: '我',
    content: newNote.value.trim(),
    created_at: new Date().toLocaleString('zh-CN')
  });
  newNote.value = '';
  emit('task-updated');
}
</script>

<style scoped>
.body { display:flex; flex:1; overflow:hidden; }
.sidebar { width:260px; flex-shrink:0; background:#ffffff; border-right:1px solid #e4e4e7; display:flex; flex-direction:column; padding:8px 0; overflow-y:auto; }
.nav-section { padding:0 8px; }
.nav-item { display:flex; align-items:center; height:38px; padding:0 12px; border-radius:6px; color:#18181b; font-size:13px; transition:background 0.15s; cursor:pointer; }
.nav-item.category-item { height:36px; }
.nav-item:hover { background:#f4f4f5; }
.nav-item.active { background:#f4f4f5; }
.nav-icon { width:20px; height:20px; border-radius:5px; flex-shrink:0; display:flex; align-items:center; justify-content:center; margin-right:10px; }
.nav-label { flex:1; }
.nav-count { font-size:11px; color:#71717a; background:#f4f4f5; padding:1px 8px; border-radius:10px; }
.cat-dot { width:8px; height:8px; border-radius:50%; flex-shrink:0; margin-right:12px; margin-left:6px; }
.sidebar-divider { height:1px; background:#e4e4e7; margin:8px 16px; }
.sidebar-label { padding:4px 20px; font-size:11px; font-weight:600; color:#71717a; text-transform:uppercase; letter-spacing:0.05em; }
.sidebar-spacer { flex:1; }
.sidebar-footer { padding:12px 16px; margin:8px; background:#f4f4f5; border-radius:8px; }
.progress-info { display:flex; justify-content:space-between; font-size:12px; color:#71717a; margin-bottom:6px; }
.progress-text { color:#18181b; font-weight:600; }
.progress-bar { height:4px; background:#e4e4e7; border-radius:2px; overflow:hidden; }
.progress-fill { height:100%; background:#18181b; border-radius:2px; transition:width 0.3s; }

.main-content { flex:1; overflow-y:auto; background:#fafafa; padding:0; display:flex; }
.task-panel { flex:1; padding:24px 28px; overflow-y:auto; }
.filter-bar { display:flex; gap:8px; margin-bottom:20px; flex-wrap:wrap; }
.filter-btn { height:30px; padding:0 14px; border-radius:6px; border:none; font-size:12px; font-weight:500; cursor:pointer; font-family:inherit; }
.filter-btn.active { background:#18181b; color:#ffffff; }
.filter-btn:not(.active) { background:#ffffff; color:#18181b; border:1px solid #e4e4e7; }
.stats-row { display:flex; gap:12px; margin-bottom:24px; }
.stat-card { flex:1; background:#ffffff; border:1px solid #e4e4e7; border-radius:12px; padding:16px 20px; }
.stat-label { font-size:12px; color:#71717a; margin-bottom:4px; }
.stat-value { font-size:28px; font-weight:700; line-height:1; }
.section-header { display:flex; align-items:center; justify-content:space-between; margin-bottom:16px; }
.section-title-wrap { display:flex; align-items:baseline; gap:8px; }
.section-title-wrap h2 { font-size:16px; font-weight:600; color:#18181b; margin:0; }
.section-count { font-size:13px; color:#71717a; }
.sort-btn { display:flex; align-items:center; gap:4px; font-size:12px; color:#71717a; cursor:pointer; }
.task-list { display:flex; flex-direction:column; border:1px solid #e4e4e7; border-radius:12px; background:#ffffff; overflow:hidden; }
.task-item { display:flex; align-items:center; padding:14px 20px; border-bottom:1px solid #e4e4e7; cursor:pointer; transition:background 0.12s; }
.task-item:last-child { border-bottom:none; }
.task-item:hover { background:#fafafa; }
.task-item.completed { background:#fafaf9; }
.task-checkbox { width:20px; height:20px; border-radius:50%; border:2px solid #d4d4d8; flex-shrink:0; margin-right:14px; display:flex; align-items:center; justify-content:center; cursor:pointer; }
.task-checkbox.checked { background:#16a34a; border-color:#16a34a; }
.task-content { flex:1; min-width:0; }
.task-title { font-size:13px; font-weight:500; color:#18181b; margin-bottom:3px; }
.task-item.completed .task-title { color:#a1a1aa; text-decoration:line-through; }
.task-meta { display:flex; align-items:center; gap:8px; }
.overdue-badge { font-size:11px; color:#dc2626; font-weight:500; }
.task-date { font-size:11px; color:#71717a; }
.task-item.completed .task-date { color:#a1a1aa; }
.task-badge { font-size:11px; padding:1px 8px; border-radius:10px; font-weight:500; }
.task-actions { display:flex; align-items:center; gap:8px; flex-shrink:0; margin-left:12px; opacity:0; transition:opacity 0.15s; }
.task-item:hover .task-actions { opacity:1; }
.priority-dot { width:6px; height:6px; border-radius:50%; }
.action-icon { color:#a1a1aa; cursor:pointer; }
.action-icon:hover { color:#18181b; }
.completed-badge { font-size:11px; color:#16a34a; font-weight:500; }

.detail-panel { width:380px; flex-shrink:0; border-left:1px solid #e4e4e7; background:#ffffff; display:flex; flex-direction:column; overflow:hidden; }
.detail-content { flex:1; overflow-y:auto; padding:24px; }
.detail-content h1 { font-size:20px; font-weight:700; color:#18181b; margin:0 0 16px 0; line-height:1.3; }
.detail-status-bar { display:flex; align-items:center; gap:10px; flex-wrap:wrap; margin-bottom:16px; }
.status-active { height:26px; padding:0 10px; display:inline-flex; align-items:center; border-radius:6px; font-size:12px; font-weight:500; background:#fef3c7; color:#92400e; }
.status-completed { height:26px; padding:0 10px; display:inline-flex; align-items:center; border-radius:6px; font-size:12px; font-weight:500; background:#dcfce7; color:#166534; }
.priority-info { display:flex; align-items:center; gap:5px; }
.priority-info span { font-size:12px; color:#71717a; }
.detail-info { display:flex; flex-direction:column; gap:10px; margin-bottom:20px; }
.info-item { display:flex; align-items:center; gap:8px; }
.info-item span { font-size:13px; color:#18181b; }
.detail-divider { height:1px; background:#e4e4e7; margin:0 0 20px 0; }
.detail-section h3 { font-size:12px; font-weight:600; color:#71717a; text-transform:uppercase; letter-spacing:0.05em; margin:0 0 8px 0; }
.detail-section p { font-size:13px; color:#3f3f46; line-height:1.7; margin:0; }
.subtasks-list { display:flex; flex-direction:column; gap:6px; }
.subtask-item { display:flex; align-items:center; gap:8px; padding:6px 8px; border-radius:6px; }
.subtask-item.completed { background:#fafafa; }
.subtask-checkbox { width:16px; height:16px; border-radius:4px; border:1.5px solid #d4d4d8; flex-shrink:0; display:flex; align-items:center; justify-content:center; cursor:pointer; }
.subtask-checkbox.checked { background:#18181b; }
.subtask-item span { font-size:12px; color:#18181b; }
.subtask-item.completed span { color:#71717a; text-decoration:line-through; }
.notes-list { display:flex; flex-direction:column; gap:12px; }
.note-item { display:flex; gap:10px; }
.note-avatar { width:28px; height:28px; border-radius:50%; flex-shrink:0; display:flex; align-items:center; justify-content:center; font-size:10px; font-weight:600; color:#fff; }
.note-content { flex:1; }
.note-header { display:flex; align-items:baseline; gap:8px; margin-bottom:3px; }
.note-author { font-size:12px; font-weight:600; color:#18181b; }
.note-time { font-size:11px; color:#a1a1aa; }
.note-content p { font-size:12px; color:#3f3f46; line-height:1.6; margin:0; }
.note-input { width:100%; padding:8px 12px; border:1px solid #e4e4e7; border-radius:6px; font-size:12px; font-family:inherit; resize:none; min-height:48px; margin-top:12px; }
.add-note-btn { margin-top:8px; height:28px; padding:0 12px; border-radius:6px; border:none; background:#18181b; color:#fafafa; font-size:12px; cursor:pointer; }
.detail-footer { flex-shrink:0; padding:12px 24px; border-top:1px solid #e4e4e7; display:flex; align-items:center; gap:8px; background:#ffffff; }
.footer-spacer { flex:1; }
.btn-danger { height:32px; padding:0 16px; display:inline-flex; align-items:center; border-radius:6px; font-size:13px; font-weight:500; background:#ffffff; color:#ef4444; border:1px solid #e4e4e7; cursor:pointer; gap:6px; }
.close-detail-btn { width:32px; height:32px; display:flex; align-items:center; justify-content:center; border-radius:6px; border:1px solid #e4e4e7; cursor:pointer; }

.empty-detail { display:flex; flex-direction:column; align-items:center; justify-content:center; padding:40px 32px; text-align:center; flex:1; }
.empty-icon { width:56px; height:56px; border-radius:12px; background:#f4f4f5; display:flex; align-items:center; justify-content:center; margin:0 auto 16px; }
.empty-title { font-size:14px; color:#71717a; margin:0 0 6px; font-weight:500; }
.empty-desc { font-size:12px; color:#a1a1aa; margin:0; }
</style>
