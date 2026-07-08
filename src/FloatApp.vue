<template>
  <div class="float-window">
    <div class="mini-title-bar">
      <div class="title-left">
        <div class="title-dot"></div>
        <span class="title-text">kettd</span>
      </div>
      <div class="title-right">
        <div class="min-btn" title="最小化" @click="minimizeWindow">─</div>
        <div class="close-btn" title="关闭" @click="closeWindow">&times;</div>
      </div>
    </div>

    <div class="window-content">
      <div class="section" style="padding-bottom:10px;">
        <div class="section-title">
          <span class="section-label">今日概览</span>
          <span class="section-date">{{ currentDateStr }}</span>
        </div>
        <div class="stats-text">{{ statsText }}</div>
        <div class="progress-bar">
          <div class="progress-fill" :style="{ width: progressPercent + '%' }"></div>
        </div>
      </div>

      <div class="section">
        <div class="section-title">
          <span class="section-label">即将到来</span>
          <span class="section-badge">近2小时</span>
        </div>
        <div class="tasks-container">
          <div 
            v-for="task in upcomingTasks" 
            :key="task.id"
            class="task-item"
          >
            <div 
              class="checkbox"
              :class="{ checked: task.completed }"
              @click.stop="handleToggleTask(task.id)"
            >
              <svg v-if="task.completed" width="9" height="9" viewBox="0 0 9 9" fill="none">
                <path d="M2 4.5L3.5 6L7 2.5" stroke="#fafafa" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </div>
            <span class="task-title">{{ task.title }}</span>
            <span class="time-badge" :style="getTimeBadgeStyle(task.due_date)">{{ formatDate(task.due_date) }}</span>
          </div>
        </div>
      </div>

      <div class="section">
        <div class="section-title">
          <span class="section-label">计时器</span>
          <span class="open-main-btn" @click="openMainWindow">管理</span>
        </div>
        <div class="timer-card" style="border-left-color:#16a34a;">
          <div>
            <div class="timer-label">专注工作</div>
            <div class="timer-value">25:00</div>
          </div>
          <span class="timer-action">暂停</span>
        </div>
        <div class="timer-card" style="border-left-color:#3b82f6;">
          <div>
            <div class="timer-label">休息提醒</div>
            <div class="timer-value">05:00</div>
          </div>
          <span class="timer-action">暂停</span>
        </div>
      </div>

      <div class="section" style="border-bottom:none;">
        <div class="section-title important-title">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="#f59e0b" stroke="none">
            <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/>
          </svg>
          <span class="section-label">重要任务</span>
        </div>
        <div class="tasks-container">
          <div 
            v-for="task in importantTasks" 
            :key="task.id"
            class="task-item"
          >
            <div class="priority-dot" :style="{ background: getPriorityColor(task.priority) }"></div>
            <span class="task-title">{{ task.title }}</span>
            <span class="time-badge" :style="getImportantBadgeStyle(task.due_date)">{{ formatDate(task.due_date) }}</span>
          </div>
        </div>
      </div>

      <div class="scroll-spacer"></div>
    </div>

    <div class="quick-add-bar">
      <input type="text" class="quick-add-input" v-model="newTaskTitle" placeholder="快速添加待办..." @keyup.enter="handleQuickAdd" />
      <div class="add-btn" @click="handleQuickAdd">+</div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { getTasks, toggleTask, addTask, generateId, formatDate, getPriorityColor, openMainWindow } from './api';

const tasks = ref([]);
const newTaskTitle = ref('');
let updateInterval = null;

const today = new Date();
const todayStr = today.toISOString().split('T')[0];

const currentDateStr = computed(() => {
  const days = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'];
  return `${today.getFullYear()}年${today.getMonth() + 1}月${today.getDate()}日 ${days[today.getDay()]}`;
});

const statsText = computed(() => {
  const pending = tasks.value.filter(t => !t.completed).length;
  const completed = tasks.value.filter(t => t.completed).length;
  const overdue = tasks.value.filter(t => {
    if (t.completed) return false;
    const dueDate = new Date(t.due_date.split(' ')[0]);
    dueDate.setHours(0, 0, 0, 0);
    return dueDate < today;
  }).length;
  return `${pending} 待办 | ${completed} 已完成 | ${overdue} 逾期`;
});

const progressPercent = computed(() => {
  const todayTasks = tasks.value.filter(t => {
    const dueDate = t.due_date.split(' ')[0];
    return dueDate === todayStr || t.due_date.includes(todayStr);
  });
  const pending = todayTasks.filter(t => !t.completed).length;
  const completed = todayTasks.filter(t => t.completed).length;
  const total = pending + completed;
  return total > 0 ? (completed / total) * 100 : 0;
});

const upcomingTasks = computed(() => {
  return tasks.value
    .filter(t => !t.completed)
    .sort((a, b) => new Date(a.due_date) - new Date(b.due_date))
    .slice(0, 5);
});

const importantTasks = computed(() => {
  return tasks.value
    .filter(t => !t.completed && (t.priority === 'high' || formatDate(t.due_date) === '已逾期'))
    .slice(0, 3);
});

function getTimeBadgeStyle(dueDate) {
  const formatted = formatDate(dueDate);
  if (formatted === '已逾期') {
    return { background: '#fef2f2', color: '#b91c1c' };
  }
  if (formatted.includes('今天')) {
    return { background: '#fef3c7', color: '#92400e' };
  }
  return { background: '#f4f4f5', color: '#a1a1aa' };
}

function getImportantBadgeStyle(dueDate) {
  const formatted = formatDate(dueDate);
  if (formatted === '已逾期') {
    return { background: '#fef2f2', color: '#b91c1c' };
  }
  return { background: '#fef3c7', color: '#92400e' };
}

async function loadTasks() {
  tasks.value = await getTasks();
}

async function handleToggleTask(taskId) {
  await toggleTask(taskId);
  await loadTasks();
}

async function handleQuickAdd() {
  if (!newTaskTitle.value.trim()) return;
  const now = new Date();
  const task = {
    id: generateId(),
    title: newTaskTitle.value.trim(),
    description: '',
    category: '个人',
    priority: 'low',
    due_date: now.toISOString().split('T')[0],
    completed: false,
    subtasks: [],
    notes: [],
    created_at: now.toISOString().split('T')[0],
    updated_at: now.toISOString().split('T')[0]
  };
  await addTask(task);
  newTaskTitle.value = '';
  await loadTasks();
}

function minimizeWindow() {
  if (window.__TAURI__) {
    window.__TAURI__.window.getCurrent().then(win => win.minimize());
  }
}

function closeWindow() {
  window.close();
}

onMounted(async () => {
  await loadTasks();
  updateInterval = setInterval(loadTasks, 30000);
});

onUnmounted(() => {
  if (updateInterval) {
    clearInterval(updateInterval);
  }
});
</script>

<style scoped>
.mini-title-bar {
  height: 32px;
  background: #18181b;
  border-radius: 12px 12px 0 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  flex-shrink: 0;
}

.title-left {
  display: flex;
  align-items: center;
  gap: 6px;
}

.title-dot {
  width: 14px;
  height: 14px;
  background: #3b82f6;
  border-radius: 3px;
  flex-shrink: 0;
}

.title-text {
  font-size: 11px;
  color: #d4d4d8;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.title-right {
  display: flex;
  align-items: center;
  gap: 4px;
}

.window-content {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}

.section {
  padding: 10px 14px 6px;
  border-bottom: 1px solid #e4e4e7;
}

.section-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.section-label {
  font-size: 13px;
  font-weight: 600;
  color: #18181b;
}

.section-date {
  font-size: 11px;
  color: #a1a1aa;
}

.section-badge {
  font-size: 10px;
  background: #f4f4f5;
  color: #71717a;
  padding: 1px 6px;
  border-radius: 4px;
}

.important-title {
  display: flex;
  align-items: center;
  gap: 4px;
}

.stats-text {
  font-size: 12px;
  color: #71717a;
  margin-bottom: 8px;
}

.progress-bar {
  width: 100%;
  height: 3px;
  background: #f4f4f5;
  border-radius: 2px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: #18181b;
  border-radius: 2px;
  transition: width 0.3s;
}

.tasks-container {
  display: flex;
  flex-direction: column;
}

.task-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 0;
  border-bottom: 1px solid #f4f4f5;
  cursor: pointer;
  transition: background 0.12s;
}

.task-item:last-child {
  border-bottom: none;
}

.task-item:hover {
  background: #fafafa;
}

.checkbox {
  width: 16px;
  height: 16px;
  border: 2px solid #d4d4d8;
  border-radius: 4px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.checkbox.checked {
  background: #16a34a;
  border-color: #16a34a;
}

.priority-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.task-title {
  font-size: 12px;
  color: #18181b;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.time-badge {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 4px;
  flex-shrink: 0;
}

.timer-card {
  background: #fafafa;
  border-radius: 8px;
  padding: 10px 12px;
  margin-bottom: 6px;
  border-left: 3px solid;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.timer-label {
  font-size: 11px;
  color: #71717a;
  margin-bottom: 2px;
}

.timer-value {
  font-size: 20px;
  font-family: 'Consolas', 'Courier New', monospace;
  font-weight: 600;
  color: #18181b;
  letter-spacing: 1px;
}

.timer-action {
  font-size: 11px;
  color: #71717a;
  cursor: pointer;
  padding: 3px 8px;
  border-radius: 4px;
  border: 1px solid #e4e4e7;
}

.timer-action:hover {
  background: #e4e4e7;
}

.scroll-spacer {
  height: 44px;
  flex-shrink: 0;
}

.open-main-btn {
  font-size: 11px;
  color: #71717a;
  cursor: pointer;
}

.open-main-btn:hover {
  color: #18181b;
}

.quick-add-bar {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 44px;
  background: #fafafa;
  border-top: 1px solid #e4e4e7;
  display: flex;
  align-items: center;
  padding: 0 12px;
  gap: 8px;
  flex-shrink: 0;
}

.quick-add-input {
  flex: 1;
  height: 28px;
  border: 1px solid #e4e4e7;
  border-radius: 6px;
  padding: 0 10px;
  font-size: 13px;
  background: #ffffff;
  color: #18181b;
  outline: none;
  font-family: inherit;
}

.quick-add-input:focus {
  border-color: #18181b;
}

.add-btn {
  width: 28px;
  height: 28px;
  background: #18181b;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #ffffff;
  font-size: 16px;
  font-weight: 300;
  cursor: pointer;
  flex-shrink: 0;
  line-height: 1;
}

.add-btn:hover {
  background: #27272a;
}

.min-btn, .close-btn {
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  cursor: pointer;
  color: #a1a1aa;
  font-size: 14px;
  line-height: 1;
}

.min-btn:hover, .close-btn:hover {
  background: #3f3f46;
  color: #fafafa;
}
</style>
