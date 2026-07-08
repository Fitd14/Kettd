import { reactive, computed } from 'vue';
import {
  getTasks,
  getTask,
  addTask,
  updateTask,
  deleteTask,
  toggleTask,
  toggleSubtask,
  addSubtask,
  deleteSubtask,
  addNote,
  deleteNote,
  getReminders,
  addReminder,
  updateReminder,
  deleteReminder,
  toggleReminder,
  getTheme,
  setTheme,
  getTaskStats,
  search,
  generateId,
  getHistory,
  getHistoryByDate,
  getHistoryByType,
  getHistoryByAction,
  getHistoryStats,
  clearHistory,
  deleteHistoryItem,
} from '../api';

const state = reactive({
  tasks: [],
  reminders: [],
  history: [],
  theme: 'light',
  loading: false,
  error: null,
  stats: null,
  searchResults: null,
});

export function useTaskStore() {
  const pendingTasks = computed(() => state.tasks.filter(t => !t.completed));
  const completedTasks = computed(() => state.tasks.filter(t => t.completed));
  const todayTasks = computed(() => {
    const today = new Date().toISOString().split('T')[0];
    return state.tasks.filter(t => !t.completed && t.due_date.startsWith(today));
  });
  const overdueTasks = computed(() => {
    const today = new Date().toISOString().split('T')[0];
    return state.tasks.filter(t => !t.completed && t.due_date && t.due_date < today && !t.due_date.includes(' '));
  });

  const setError = (error) => {
    state.error = error;
    setTimeout(() => {
      state.error = null;
    }, 5000);
  };

  const setLoading = (loading) => {
    state.loading = loading;
  };

  const loadTasks = async () => {
    setLoading(true);
    try {
      state.tasks = await getTasks();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const loadTask = async (id) => {
    setLoading(true);
    try {
      return await getTask(id);
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const createTask = async (task) => {
    setLoading(true);
    try {
      await addTask(task);
      await loadTasks();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const modifyTask = async (id, task) => {
    setLoading(true);
    try {
      await updateTask(id, task);
      await loadTasks();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const removeTask = async (id) => {
    setLoading(true);
    try {
      await deleteTask(id);
      await loadTasks();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const markTask = async (id) => {
    setLoading(true);
    try {
      await toggleTask(id);
      await loadTasks();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const markSubtask = async (taskId, subtaskId) => {
    setLoading(true);
    try {
      await toggleSubtask(taskId, subtaskId);
      await loadTasks();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const createSubtask = async (taskId, title) => {
    setLoading(true);
    try {
      await addSubtask(taskId, title);
      await loadTasks();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const removeSubtask = async (taskId, subtaskId) => {
    setLoading(true);
    try {
      await deleteSubtask(taskId, subtaskId);
      await loadTasks();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const createNote = async (taskId, note) => {
    setLoading(true);
    try {
      await addNote(taskId, note);
      await loadTasks();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const removeNote = async (taskId, noteId) => {
    setLoading(true);
    try {
      await deleteNote(taskId, noteId);
      await loadTasks();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const loadReminders = async () => {
    setLoading(true);
    try {
      state.reminders = await getReminders();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const createReminder = async (reminder) => {
    setLoading(true);
    try {
      await addReminder(reminder);
      await loadReminders();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const modifyReminder = async (id, reminder) => {
    setLoading(true);
    try {
      await updateReminder(id, reminder);
      await loadReminders();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const removeReminder = async (id) => {
    setLoading(true);
    try {
      await deleteReminder(id);
      await loadReminders();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const markReminder = async (id) => {
    setLoading(true);
    try {
      await toggleReminder(id);
      await loadReminders();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const loadTheme = async () => {
    try {
      state.theme = await getTheme();
    } catch (e) {
      setError(e.message);
    }
  };

  const changeTheme = async (theme) => {
    try {
      await setTheme(theme);
      state.theme = theme;
    } catch (e) {
      setError(e.message);
    }
  };

  const loadStats = async () => {
    try {
      state.stats = await getTaskStats();
    } catch (e) {
      setError(e.message);
    }
  };

  const performSearch = async (keyword) => {
    setLoading(true);
    try {
      state.searchResults = await search(keyword);
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const clearSearch = () => {
    state.searchResults = null;
  };

  const loadHistory = async () => {
    setLoading(true);
    try {
      state.history = await getHistory();
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const loadHistoryByDate = async (date) => {
    setLoading(true);
    try {
      state.history = await getHistoryByDate(date);
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const loadHistoryByType = async (type) => {
    setLoading(true);
    try {
      state.history = await getHistoryByType(type);
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const loadHistoryByAction = async (action) => {
    setLoading(true);
    try {
      state.history = await getHistoryByAction(action);
    } catch (e) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  };

  const loadHistoryStats = async () => {
    try {
      return await getHistoryStats();
    } catch (e) {
      setError(e.message);
    }
  };

  const clearAllHistory = async () => {
    try {
      await clearHistory();
      state.history = [];
    } catch (e) {
      setError(e.message);
    }
  };

  const removeHistoryItem = async (id) => {
    try {
      await deleteHistoryItem(id);
      await loadHistory();
    } catch (e) {
      setError(e.message);
    }
  };

  const getNewId = async () => {
    try {
      return await generateId();
    } catch (e) {
      setError(e.message);
      return Math.random().toString(36).substr(2, 9);
    }
  };

  return {
    state,
    pendingTasks,
    completedTasks,
    todayTasks,
    overdueTasks,
    loadTasks,
    loadTask,
    createTask,
    modifyTask,
    removeTask,
    markTask,
    markSubtask,
    createSubtask,
    removeSubtask,
    createNote,
    removeNote,
    loadReminders,
    createReminder,
    modifyReminder,
    removeReminder,
    markReminder,
    loadTheme,
    changeTheme,
    loadStats,
    performSearch,
    clearSearch,
    loadHistory,
    loadHistoryByDate,
    loadHistoryByType,
    loadHistoryByAction,
    loadHistoryStats,
    clearAllHistory,
    removeHistoryItem,
    getNewId,
  };
}
