<template>
  <div class="page-scheduled">
    <div class="page-header">
      <div class="header-title">
        <h1>定时任务</h1>
        <span class="header-date">{{ currentDateStr }}</span>
      </div>
    </div>

    <div class="overview-row">
      <div class="overview-card">
        <div class="overview-header">
          <span>今日待办</span>
          <div class="overview-icon" style="background:#fef3c7;">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <rect x="1" y="2" width="12" height="10" rx="1.5" stroke="#f59e0b" stroke-width="1.2"/>
              <path d="M4 0V2M10 0V2" stroke="#f59e0b" stroke-width="1.2" stroke-linecap="round"/>
              <path d="M3.5 6H10.5" stroke="#f59e0b" stroke-width="1" stroke-linecap="round"/>
            </svg>
          </div>
        </div>
        <div class="overview-value">{{ overview.today }}</div>
        <div class="overview-desc">截止今日的任务数</div>
      </div>
      <div class="overview-card">
        <div class="overview-header">
          <span>今日已完成</span>
          <div class="overview-icon" style="background:#dcfce7;">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <circle cx="7" cy="7" r="5" stroke="#16a34a" stroke-width="1.2"/>
              <path d="M4.5 7L6 8.5L9.5 5" stroke="#16a34a" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </div>
        </div>
        <div class="overview-value" style="color:#16a34a;">{{ overview.completed }}</div>
        <div class="overview-desc">完成进度 {{ overview.progress }}%</div>
      </div>
      <div class="overview-card">
        <div class="overview-header">
          <span>今日专注</span>
          <div class="overview-icon" style="background:#e0e7ff;">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <circle cx="7" cy="7" r="5" stroke="#6366f1" stroke-width="1.2"/>
              <path d="M7 4V7L9 8.5" stroke="#6366f1" stroke-width="1.2" stroke-linecap="round"/>
            </svg>
          </div>
        </div>
        <div class="overview-value" style="color:#6366f1;">{{ focusTime }}</div>
        <div class="overview-desc">今日专注时长</div>
      </div>
    </div>

    <div class="timer-row">
      <div class="timer-card">
        <div class="timer-header">
          <div>
            <h3>专注计时器</h3>
            <p>保持专注，提高效率</p>
          </div>
          <span class="timer-status" :style="timerStatusStyle">{{ timerStatus }}</span>
        </div>
        <div class="timer-display">
          <div class="timer-ring">
            <div class="timer-ring-bg"></div>
            <div class="timer-ring-progress" :style="{ transform: `rotate(${timerProgress}deg)` }"></div>
            <div class="timer-text-wrap">
              <div class="timer-text">{{ formatTime(timerSeconds) }}</div>
              <div class="timer-subtext">{{ timerLabel }}</div>
            </div>
          </div>
        </div>
        <div class="timer-controls">
          <button v-if="!timerRunning" class="timer-btn start" @click="startTimer">
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
              <path d="M7 5L16 10L7 15V5Z" stroke="#fafafa" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </button>
          <button v-else class="timer-btn pause" @click="pauseTimer">
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
              <rect x="6" y="4" width="4" height="12" rx="1" fill="#ffffff"/>
              <rect x="10" y="4" width="4" height="12" rx="1" fill="#ffffff"/>
            </svg>
          </button>
          <button class="timer-btn reset" @click="resetTimer">
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
              <path d="M13 3H7C5.9 3 5 3.9 5 5V15C5 16.1 5.9 17 7 17H13C14.1 17 15 16.1 15 15V5C15 3.9 14.1 3 13 3Z" stroke="#71717a" stroke-width="1.5" stroke-linecap="round"/>
              <path d="M7 9V5C7 3.9 7.9 3 9 3H11C12.1 3 13 3.9 13 5V9" stroke="#71717a" stroke-width="1.5" stroke-linecap="round"/>
            </svg>
          </button>
          <button v-if="timerRunning" class="timer-btn stop" @click="stopTimer">
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
              <rect x="5" y="5" width="10" height="10" rx="2" stroke="#ffffff" stroke-width="2" stroke-linecap="round"/>
            </svg>
          </button>
        </div>
        <div class="timer-settings">
          <button 
            v-for="preset in timerPresets" 
            :key="preset"
            class="timer-preset"
            :class="{ active: timerTotalSeconds === preset * 60 }"
            @click="setTimerPreset(preset)"
          >
            {{ preset }}分钟
          </button>
        </div>
      </div>
    </div>

    <div class="reminder-section">
      <div class="section-header">
        <h2>今日提醒</h2>
        <span class="reminder-count">({{ reminders.length }})</span>
      </div>
      <div class="reminder-table">
        <div 
          v-for="reminder in reminders" 
          :key="reminder.id"
          class="reminder-row"
        >
          <div 
            class="reminder-checkbox"
            :class="{ checked: reminder.completed }"
            @click="handleToggleReminder(reminder.id)"
          >
            <svg v-if="reminder.completed" width="10" height="10" viewBox="0 0 10 10" fill="none">
              <path d="M3 5L4.5 6.5L8 3" stroke="#fafafa" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </div>
          <div class="reminder-content">
            <div class="reminder-title">{{ reminder.title }}</div>
            <div class="reminder-time">{{ reminder.time }}</div>
          </div>
          <span class="reminder-category" :style="getCategoryBadgeStyle(reminder.category)">{{ reminder.category }}</span>
          <div class="reminder-actions">
            <svg class="delete-icon" @click="handleDeleteReminder(reminder.id)" width="14" height="14" viewBox="0 0 14 14">
              <path d="M3 3L11 11M11 3L3 11" stroke="#a1a1aa" stroke-width="1.2" stroke-linecap="round"/>
            </svg>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onUnmounted } from 'vue';
import { toggleReminder, deleteReminder, getCategoryColor } from '../api';

const props = defineProps({
  tasks: { type: Array, default: () => [] },
  reminders: { type: Array, default: () => [] }
});

const emit = defineEmits(['reminder-updated']);

const timerInterval = ref(null);
const timerSeconds = ref(25 * 60);
const timerTotalSeconds = ref(25 * 60);
const timerRunning = ref(false);

const timerPresets = [25, 45, 60];

const today = new Date().toISOString().split('T')[0];

const currentDateStr = computed(() => {
  return new Date().toLocaleDateString('zh-CN', { month: 'long', day: 'numeric', weekday: 'long' });
});

const overview = computed(() => {
  const todayTasks = props.tasks.filter(t => {
    const dueDate = t.due_date.split(' ')[0];
    return dueDate === today || t.due_date.includes(today);
  });
  const completedCount = todayTasks.filter(t => t.completed).length;
  const total = todayTasks.length;
  const progress = total > 0 ? Math.round((completedCount / total) * 100) : 0;
  
  return {
    today: todayTasks.length,
    completed: completedCount,
    progress
  };
});

const focusTime = ref('2h 15m');

const timerStatus = computed(() => {
  if (!timerRunning.value) {
    if (timerSeconds.value === timerTotalSeconds.value) return '就绪';
    return '已暂停';
  }
  return '专注中';
});

const timerStatusStyle = computed(() => {
  if (!timerRunning.value) {
    if (timerSeconds.value === timerTotalSeconds.value) {
      return { background: '#f4f4f5', color: '#71717a' };
    }
    return { background: '#fef3c7', color: '#92400e' };
  }
  return { background: '#dcfce7', color: '#166534' };
});

const timerLabel = computed(() => '专注时间');

const timerProgress = computed(() => {
  return ((timerTotalSeconds.value - timerSeconds.value) / timerTotalSeconds.value) * 360;
});

function formatTime(seconds) {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
}

function startTimer() {
  if (timerRunning.value) return;
  timerRunning.value = true;
  
  timerInterval.value = setInterval(() => {
    if (timerSeconds.value > 0) {
      timerSeconds.value--;
    } else {
      stopTimer();
      alert('专注时间结束！');
    }
  }, 1000);
}

function pauseTimer() {
  if (!timerRunning.value) return;
  timerRunning.value = false;
  clearInterval(timerInterval.value);
}

function resetTimer() {
  pauseTimer();
  timerSeconds.value = timerTotalSeconds.value;
}

function stopTimer() {
  pauseTimer();
}

function setTimerPreset(duration) {
  timerTotalSeconds.value = duration * 60;
  resetTimer();
}

function getCategoryBadgeStyle(category) {
  const color = getCategoryColor(category);
  return {
    background: color.bg,
    color: color.text
  };
}

async function handleToggleReminder(id) {
  await toggleReminder(id);
  emit('reminder-updated');
}

async function handleDeleteReminder(id) {
  if (confirm('确定删除此提醒？')) {
    await deleteReminder(id);
    emit('reminder-updated');
  }
}

onUnmounted(() => {
  if (timerInterval.value) {
    clearInterval(timerInterval.value);
  }
});
</script>

<style scoped>
.page-scheduled { padding:28px 32px 40px; overflow-y:auto; flex:1; }
.page-header { display:flex; align-items:center; justify-content:space-between; margin-bottom:28px; }
.header-title { display:flex; align-items:center; gap:10px; }
.header-title h1 { font-size:22px; font-weight:700; color:#18181b; margin:0; }
.header-date { font-size:13px; color:#71717a; }

.overview-row { display:flex; gap:16px; margin-bottom:28px; }
.overview-card { flex:1; background:#ffffff; border-radius:16px; box-shadow:0 2px 8px rgba(0,0,0,0.04); padding:20px 22px; }
.overview-header { display:flex; align-items:center; justify-content:space-between; margin-bottom:12px; }
.overview-header span { font-size:13px; color:#71717a; }
.overview-icon { width:28px; height:28px; border-radius:8px; display:flex; align-items:center; justify-content:center; }
.overview-value { font-size:32px; font-weight:700; color:#18181b; line-height:1; }
.overview-desc { font-size:12px; color:#71717a; margin-top:4px; }

.timer-row { display:flex; gap:16px; margin-bottom:28px; }
.timer-card { flex:1; background:#ffffff; border-radius:16px; box-shadow:0 2px 8px rgba(0,0,0,0.04); border-left:4px solid #18181b; padding:20px 22px; }
.timer-header { display:flex; align-items:center; justify-content:space-between; margin-bottom:16px; }
.timer-header h3 { font-size:16px; font-weight:600; color:#18181b; margin:0; }
.timer-header p { font-size:12px; color:#71717a; margin:2px 0 0 0; }
.timer-status { font-size:11px; padding:3px 10px; border-radius:12px; font-weight:500; }
.timer-display { display:flex; flex-direction:column; align-items:center; justify-content:center; padding:24px 0; }
.timer-ring { width:160px; height:160px; border-radius:50%; position:relative; display:flex; align-items:center; justify-content:center; }
.timer-ring-bg { width:100%; height:100%; border-radius:50%; border:8px solid #f4f4f5; }
.timer-ring-progress { position:absolute; width:100%; height:100%; border-radius:50%; border:8px solid transparent; border-top-color:#18181b; }
.timer-text-wrap { text-align:center; }
.timer-text { font-size:48px; font-weight:700; color:#18181b; line-height:1; }
.timer-subtext { font-size:13px; color:#71717a; margin-top:8px; }
.timer-controls { display:flex; gap:12px; justify-content:center; margin-top:16px; }
.timer-btn { width:48px; height:48px; border-radius:50%; display:flex; align-items:center; justify-content:center; cursor:pointer; border:none; font-size:18px; }
.timer-btn.start { background:#18181b; color:#fafafa; }
.timer-btn.pause { background:#f59e0b; color:#ffffff; }
.timer-btn.reset { background:#f4f4f5; color:#71717a; }
.timer-btn.stop { background:#ef4444; color:#ffffff; }
.timer-settings { display:flex; align-items:center; gap:8px; margin-top:12px; justify-content:center; }
.timer-preset { padding:6px 14px; border-radius:6px; font-size:12px; font-weight:500; cursor:pointer; border:1px solid #e4e4e7; background:#ffffff; }
.timer-preset.active { background:#18181b; color:#ffffff; border-color:#18181b; }

.reminder-section { margin-top:0; }
.section-header { display:flex; align-items:center; justify-content:space-between; margin-bottom:16px; }
.section-header h2 { font-size:16px; font-weight:600; color:#18181b; margin:0; }
.reminder-count { font-size:13px; color:#71717a; }
.reminder-table { background:#ffffff; border-radius:12px; box-shadow:0 2px 8px rgba(0,0,0,0.04); overflow:hidden; }
.reminder-row { display:flex; align-items:center; padding:12px 20px; border-bottom:1px solid #f4f4f5; transition:background 0.15s; }
.reminder-row:last-child { border-bottom:none; }
.reminder-row:hover { background:#fafafa; }
.reminder-checkbox { width:18px; height:18px; border-radius:4px; border:2px solid #d4d4d8; flex-shrink:0; margin-right:12px; display:flex; align-items:center; justify-content:center; cursor:pointer; }
.reminder-checkbox.checked { background:#18181b; border-color:#18181b; }
.reminder-content { flex:1; min-width:0; }
.reminder-title { font-size:13px; font-weight:500; color:#18181b; margin-bottom:3px; }
.reminder-time { font-size:11px; color:#71717a; }
.reminder-category { font-size:11px; padding:2px 8px; border-radius:6px; font-weight:500; flex-shrink:0; }
.reminder-actions { display:flex; align-items:center; gap:8px; flex-shrink:0; opacity:0; transition:opacity 0.15s; }
.reminder-row:hover .reminder-actions { opacity:1; }
.delete-icon { color:#a1a1aa; cursor:pointer; }
.delete-icon:hover { color:#ef4444; }
</style>
