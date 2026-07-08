<template>
  <div id="app-shell">
    <div class="title-bar">
      <div class="logo-icon">
        <svg width="8" height="8" viewBox="0 0 8 8" fill="none">
          <rect x="0.5" y="0.5" width="3" height="3" rx="0.5" fill="white"/>
          <rect x="4.5" y="0.5" width="3" height="3" rx="0.5" fill="white" opacity="0.6"/>
          <rect x="0.5" y="4.5" width="3" height="3" rx="0.5" fill="white" opacity="0.6"/>
          <rect x="4.5" y="4.5" width="3" height="3" rx="0.5" fill="white" opacity="0.3"/>
        </svg>
      </div>
      <span class="title-text">{{ windowTitle }}</span>
      <div class="window-controls">
        <div class="min-btn" @click="minimizeWindow">
          <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="#a1a1aa"/></svg>
        </div>
        <div class="max-btn" @click="maximizeWindow">
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none"><rect x="0.5" y="0.5" width="9" height="9" rx="1" stroke="#a1a1aa" stroke-width="1"/></svg>
        </div>
        <div class="close-btn" @click="closeWindow">
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none"><path d="M1 1L9 9M9 1L1 9" stroke="#a1a1aa" stroke-width="1.2" stroke-linecap="round"/></svg>
        </div>
      </div>
    </div>

    <div class="toolbar">
      <div v-if="currentPage === 'main'" class="search-wrap">
        <svg class="search-icon" width="16" height="16" viewBox="0 0 16 16" fill="none">
          <circle cx="6.5" cy="6.5" r="5" stroke="#a1a1aa" stroke-width="1.5"/>
          <path d="M10.5 10.5L14 14" stroke="#a1a1aa" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
        <input type="text" class="search-input" v-model="searchQuery" placeholder="搜索任务..." />
      </div>
      <button v-if="currentPage === 'scheduled'" class="btn btn-secondary" @click="goToPage('main')">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M8 11L3 7L8 3" stroke="#18181b" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
        <span>返回</span>
      </button>
      <div class="toolbar-spacer"></div>
      <button v-if="currentPage === 'main'" class="btn btn-primary" @click="openNewTaskModal">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M7 1V13M1 7H13" stroke="#fafafa" stroke-width="1.5" stroke-linecap="round"/></svg>
        <span>新建任务</span>
      </button>
      <button v-if="currentPage === 'main'" class="btn btn-secondary" @click="openFloatWindow">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><rect x="1" y="3" width="12" height="9" rx="2" stroke="#18181b" stroke-width="1.2"/><path d="M4 3V2a3 3 0 0 1 6 0v1" stroke="#18181b" stroke-width="1.2" stroke-linecap="round"/><circle cx="7" cy="8" r="1.2" fill="#18181b"/></svg>
        <span>悬浮面板</span>
      </button>
      <button v-if="currentPage === 'scheduled'" class="btn btn-primary" @click="openAddReminderModal">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M7 1V13M1 7H13" stroke="#fafafa" stroke-width="1.5" stroke-linecap="round"/></svg>
        <span>添加提醒</span>
      </button>
      <div class="avatar" @click="toggleTheme">{{ themeAvatar }}</div>
    </div>

    <MainPage v-if="currentPage === 'main'" 
      :tasks="state.tasks" 
      :reminders="state.reminders"
      :search-query="searchQuery"
      @task-updated="loadData"
      @open-scheduled="goToPage('scheduled')"
      @open-modal="openTaskModal" />
    
    <ScheduledPage v-if="currentPage === 'scheduled'" 
      :tasks="state.tasks" 
      :reminders="state.reminders"
      @reminder-updated="loadData" />
  </div>

  <div v-if="showModal" class="modal-overlay" @click.self="closeModal">
    <div class="modal">
      <h2>{{ isEditing ? '编辑任务' : '新建任务' }}</h2>
      <div class="form-group">
        <label>任务标题</label>
        <input type="text" v-model="formData.title" placeholder="输入任务标题" />
      </div>
      <div class="form-group">
        <label>描述</label>
        <textarea v-model="formData.description" placeholder="输入任务描述"></textarea>
      </div>
      <div class="form-row">
        <div class="form-group">
          <label>分类</label>
          <select v-model="formData.category">
            <option value="工作">工作</option>
            <option value="个人">个人</option>
            <option value="学习">学习</option>
          </select>
        </div>
        <div class="form-group">
          <label>优先级</label>
          <select v-model="formData.priority">
            <option value="high">高</option>
            <option value="medium">中</option>
            <option value="low">低</option>
          </select>
        </div>
      </div>
      <div class="form-group">
        <label>截止日期</label>
        <input type="datetime-local" v-model="formData.due_date" />
      </div>
      <div class="modal-footer">
        <button class="btn btn-secondary" @click="closeModal">取消</button>
        <button class="btn btn-primary" @click="saveTask">保存</button>
      </div>
    </div>
  </div>

  <div v-if="showReminderModal" class="modal-overlay" @click.self="closeReminderModal">
    <div class="modal">
      <h2>添加提醒</h2>
      <div class="form-group">
        <label>提醒标题</label>
        <input type="text" v-model="reminderForm.title" placeholder="输入提醒内容" />
      </div>
      <div class="form-group">
        <label>提醒时间</label>
        <input type="datetime-local" v-model="reminderForm.time" />
      </div>
      <div class="form-group">
        <label>分类</label>
        <select v-model="reminderForm.category">
          <option value="工作">工作</option>
          <option value="个人">个人</option>
          <option value="学习">学习</option>
        </select>
      </div>
      <div class="modal-footer">
        <button class="btn btn-secondary" @click="closeReminderModal">取消</button>
        <button class="btn btn-primary" @click="saveReminder">保存</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
import MainPage from './components/MainPage.vue';
import ScheduledPage from './components/ScheduledPage.vue';
import { useTaskStore } from './stores/taskStore';

const { state, loadTasks, loadReminders, loadTheme, changeTheme, createTask, modifyTask, createReminder, getNewId } = useTaskStore();

const currentPage = ref('main');
const windowTitle = ref('kettd');
const searchQuery = ref('');
const showModal = ref(false);
const showReminderModal = ref(false);
const isEditing = ref(false);
const editingTaskId = ref(null);
const themeAvatar = ref('U');

const formData = ref({
  title: '',
  description: '',
  category: '工作',
  priority: 'medium',
  due_date: ''
});

const reminderForm = ref({
  title: '',
  time: '',
  category: '工作'
});

async function loadData() {
  await Promise.all([loadTasks(), loadReminders()]);
}

async function toggleTheme() {
  await changeTheme(state.theme === 'light' ? 'dark' : 'light');
  document.documentElement.className = state.theme;
}

function goToPage(page) {
  currentPage.value = page;
}

function openFloatWindow() {
  if (window.__TAURI__) {
    window.__TAURI__.invoke('open_main_window');
  }
}

function minimizeWindow() {
  if (window.__TAURI__) {
    window.__TAURI__.window.getCurrent().then(win => win.minimize());
  }
}

function maximizeWindow() {
  if (window.__TAURI__) {
    window.__TAURI__.window.getCurrent().then(win => win.toggleMaximize());
  }
}

function closeWindow() {
  window.close();
}

function openNewTaskModal() {
  isEditing.value = false;
  editingTaskId.value = null;
  formData.value = {
    title: '',
    description: '',
    category: '工作',
    priority: 'medium',
    due_date: ''
  };
  showModal.value = true;
}

function openTaskModal(task) {
  isEditing.value = true;
  editingTaskId.value = task.id;
  formData.value = {
    title: task.title,
    description: task.description,
    category: task.category,
    priority: task.priority,
    due_date: formatDateTimeLocal(task.due_date)
  };
  showModal.value = true;
}

function closeModal() {
  showModal.value = false;
}

async function saveTask() {
  if (!formData.value.title.trim()) return;
  const now = new Date();
  const taskData = {
    id: isEditing.value ? editingTaskId.value : await getNewId(),
    title: formData.value.title.trim(),
    description: formData.value.description,
    category: formData.value.category,
    priority: formData.value.priority,
    due_date: formData.value.due_date || now.toISOString().split('T')[0],
    completed: isEditing.value ? state.tasks.find(t => t.id === editingTaskId.value)?.completed || false : false,
    subtasks: isEditing.value ? state.tasks.find(t => t.id === editingTaskId.value)?.subtasks || [] : [],
    notes: isEditing.value ? state.tasks.find(t => t.id === editingTaskId.value)?.notes || [] : [],
    created_at: isEditing.value ? state.tasks.find(t => t.id === editingTaskId.value)?.created_at || now.toISOString().split('T')[0] : now.toISOString().split('T')[0],
    updated_at: now.toISOString().split('T')[0]
  };
  if (isEditing.value) {
    await modifyTask(editingTaskId.value, taskData);
  } else {
    await createTask(taskData);
  }
  closeModal();
}

function formatDateTimeLocal(dateStr) {
  if (!dateStr) return '';
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return '';
  return date.toISOString().slice(0, 16);
}

function openAddReminderModal() {
  reminderForm.value = {
    title: '',
    time: '',
    category: '工作'
  };
  showReminderModal.value = true;
}

function closeReminderModal() {
  showReminderModal.value = false;
}

async function saveReminder() {
  if (!reminderForm.value.title.trim() || !reminderForm.value.time) return;
  await createReminder({
    id: await getNewId(),
    title: reminderForm.value.title.trim(),
    time: reminderForm.value.time,
    category: reminderForm.value.category,
    completed: false
  });
  closeReminderModal();
}

onMounted(async () => {
  await loadTheme();
  document.documentElement.className = state.theme;
  await loadData();
});
</script>

<style>
body { margin:0; padding:0; display:flex; align-items:center; justify-content:center; min-height:100vh; background:#e4e4e7; }
#app-shell { width:1440px; height:900px; display:flex; flex-direction:column; background:#fafafa; font-family:'Segoe UI','Microsoft YaHei',system-ui,sans-serif; color:#18181b; overflow:hidden; border-radius:8px; box-shadow:0 8px 30px rgba(0,0,0,0.12); }
.title-bar { display:flex; align-items:center; height:32px; background:#18181b; color:#fafafa; padding:0 4px 0 12px; flex-shrink:0; user-select:none; }
.logo-icon { width:14px; height:14px; background:#3b82f6; border-radius:3px; margin-right:8px; flex-shrink:0; display:flex; align-items:center; justify-content:center; }
.title-text { font-size:12px; font-weight:400; letter-spacing:0.02em; flex:1; }
.window-controls { display:flex; align-items:center; height:32px; }
.min-btn, .max-btn, .close-btn { width:46px; height:32px; display:flex; align-items:center; justify-content:center; cursor:pointer; }
.close-btn { border-radius:0 8px 0 0; }
.close-btn:hover { background:#ef4444; }
.toolbar { display:flex; align-items:center; height:48px; background:#ffffff; border-bottom:1px solid #e4e4e7; padding:0 16px; flex-shrink:0; gap:12px; }
.search-wrap { flex:1; max-width:420px; position:relative; }
.search-icon { position:absolute; left:10px; top:50%; transform:translateY(-50%); }
.search-input { width:100%; height:32px; border:1px solid #e4e4e7; border-radius:6px; padding:0 12px 0 34px; font-size:13px; background:#fafafa; outline:none; box-sizing:border-box; }
.toolbar-spacer { flex:1; }
.btn { display:flex; align-items:center; gap:6px; height:32px; padding:0 14px; border-radius:6px; font-size:13px; font-weight:500; cursor:pointer; border:none; font-family:inherit; }
.btn-primary { background:#18181b; color:#fafafa; }
.btn-secondary { background:#ffffff; color:#18181b; border:1px solid #e4e4e7; }
.avatar { width:32px; height:32px; border-radius:50%; background:linear-gradient(135deg,#6366f1,#8b5cf6); display:flex; align-items:center; justify-content:center; cursor:pointer; flex-shrink:0; color:#ffffff; font-size:12px; font-weight:600; }

.modal-overlay { position:fixed; top:0; left:0; right:0; bottom:0; background:rgba(0,0,0,0.5); display:flex; align-items:center; justify-content:center; z-index:1000; }
.modal { background:#ffffff; border-radius:12px; padding:24px; width:480px; max-height:90vh; overflow-y:auto; }
.modal h2 { margin:0 0 20px 0; font-size:18px; font-weight:600; }
.form-group { margin-bottom:16px; }
.form-group label { display:block; font-size:12px; font-weight:500; color:#71717a; margin-bottom:6px; }
.form-group input, .form-group textarea, .form-group select { width:100%; padding:8px 12px; border:1px solid #e4e4e7; border-radius:6px; font-size:13px; font-family:inherit; outline:none; box-sizing:border-box; }
.form-group textarea { resize:vertical; min-height:80px; }
.form-row { display:flex; gap:12px; }
.form-row .form-group { flex:1; }
.modal-footer { display:flex; justify-content:flex-end; gap:8px; margin-top:20px; }
</style>
