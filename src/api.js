import { invoke } from '@tauri-apps/api/core';

export async function getTasks() {
  try {
    return await invoke('get_tasks');
  } catch (e) {
    console.error('Failed to get tasks:', e);
    return [];
  }
}

export async function getTask(id) {
  try {
    return await invoke('get_task', { id });
  } catch (e) {
    console.error('Failed to get task:', e);
    return null;
  }
}

export async function addTask(task) {
  try {
    return await invoke('add_task', { task });
  } catch (e) {
    console.error('Failed to add task:', e);
    throw e;
  }
}

export async function updateTask(id, task) {
  try {
    return await invoke('update_task', { id, updated_task: task });
  } catch (e) {
    console.error('Failed to update task:', e);
    throw e;
  }
}

export async function deleteTask(id) {
  try {
    return await invoke('delete_task', { id });
  } catch (e) {
    console.error('Failed to delete task:', e);
    throw e;
  }
}

export async function toggleTask(id) {
  try {
    return await invoke('toggle_task', { id });
  } catch (e) {
    console.error('Failed to toggle task:', e);
    throw e;
  }
}

export async function toggleSubtask(taskId, subtaskId) {
  try {
    return await invoke('toggle_subtask', { task_id: taskId, subtask_id: subtaskId });
  } catch (e) {
    console.error('Failed to toggle subtask:', e);
    throw e;
  }
}

export async function addNote(taskId, note) {
  try {
    return await invoke('add_note', { task_id: taskId, note });
  } catch (e) {
    console.error('Failed to add note:', e);
    throw e;
  }
}

export async function getReminders() {
  try {
    return await invoke('get_reminders');
  } catch (e) {
    console.error('Failed to get reminders:', e);
    return [];
  }
}

export async function addReminder(reminder) {
  try {
    return await invoke('add_reminder', { reminder });
  } catch (e) {
    console.error('Failed to add reminder:', e);
    throw e;
  }
}

export async function updateReminder(id, reminder) {
  try {
    return await invoke('update_reminder', { id, updated: reminder });
  } catch (e) {
    console.error('Failed to update reminder:', e);
    throw e;
  }
}

export async function deleteReminder(id) {
  try {
    return await invoke('delete_reminder', { id });
  } catch (e) {
    console.error('Failed to delete reminder:', e);
    throw e;
  }
}

export async function toggleReminder(id) {
  try {
    return await invoke('toggle_reminder', { id });
  } catch (e) {
    console.error('Failed to toggle reminder:', e);
    throw e;
  }
}

export async function getTheme() {
  try {
    return await invoke('get_theme');
  } catch (e) {
    console.error('Failed to get theme:', e);
    return 'light';
  }
}

export async function setTheme(theme) {
  try {
    return await invoke('set_theme', { theme });
  } catch (e) {
    console.error('Failed to set theme:', e);
    throw e;
  }
}

export async function openMainWindow() {
  try {
    await invoke('open_main_window');
  } catch (e) {
    console.error('Failed to open main window:', e);
  }
}

export async function closeMainWindow() {
  try {
    await invoke('close_main_window');
  } catch (e) {
    console.error('Failed to close main window:', e);
  }
}

export function generateId() {
  return Date.now().toString(36) + Math.random().toString(36).substr(2);
}

export function formatDate(dateStr) {
  if (!dateStr) return '';
  const date = new Date(dateStr);
  if (isNaN(date.getTime())) return dateStr;
  
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const taskDate = new Date(date);
  taskDate.setHours(0, 0, 0, 0);
  
  const diffTime = taskDate.getTime() - today.getTime();
  const diffDays = Math.ceil(diffTime / (1000 * 60 * 60 * 24));
  
  if (diffDays < 0) {
    return '已逾期';
  } else if (diffDays === 0) {
    const timeStr = dateStr.split(' ')[1];
    return timeStr ? `今天 ${timeStr}` : '今天';
  } else if (diffDays === 1) {
    return '明天';
  } else if (diffDays <= 7) {
    const days = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'];
    return days[date.getDay()];
  }
  return dateStr;
}

export function getCategoryColor(category) {
  const colors = {
    '工作': { bg: '#fef2f2', text: '#dc2626', dot: '#ef4444' },
    '个人': { bg: '#eff6ff', text: '#3b82f6', dot: '#3b82f6' },
    '学习': { bg: '#ecfdf5', text: '#16a34a', dot: '#10b981' },
  };
  return colors[category] || { bg: '#f4f4f5', text: '#71717a', dot: '#a1a1aa' };
}

export function getPriorityColor(priority) {
  const colors = {
    'high': '#dc2626',
    'medium': '#f59e0b',
    'low': '#a1a1aa',
  };
  return colors[priority] || '#a1a1aa';
}
