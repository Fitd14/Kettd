import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useTaskStore } from './taskStore';

vi.mock('../api', () => ({
  getTasks: vi.fn(),
  getTask: vi.fn(),
  addTask: vi.fn(),
  updateTask: vi.fn(),
  deleteTask: vi.fn(),
  toggleTask: vi.fn(),
  toggleSubtask: vi.fn(),
  addSubtask: vi.fn(),
  deleteSubtask: vi.fn(),
  addNote: vi.fn(),
  deleteNote: vi.fn(),
  getReminders: vi.fn(),
  addReminder: vi.fn(),
  updateReminder: vi.fn(),
  deleteReminder: vi.fn(),
  toggleReminder: vi.fn(),
  getTheme: vi.fn(),
  setTheme: vi.fn(),
  getTaskStats: vi.fn(),
  search: vi.fn(),
  generateId: vi.fn(),
  getHistory: vi.fn(),
  getHistoryByDate: vi.fn(),
  getHistoryByType: vi.fn(),
  getHistoryByAction: vi.fn(),
  getHistoryStats: vi.fn(),
  clearHistory: vi.fn(),
  deleteHistoryItem: vi.fn(),
}));

import {
  getTasks,
  getTask,
  addTask,
  updateTask,
  deleteTask,
  toggleTask,
  getReminders,
} from '../api';

describe('taskStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should load tasks', async () => {
    const mockTasks = [
      { id: '1', title: 'Test Task', completed: false },
      { id: '2', title: 'Another Task', completed: true },
    ];
    getTasks.mockResolvedValue(mockTasks);

    const { state, loadTasks } = useTaskStore();
    await loadTasks();

    expect(getTasks).toHaveBeenCalled();
    expect(state.tasks).toEqual(mockTasks);
  });

  it('should filter pending tasks', async () => {
    const mockTasks = [
      { id: '1', title: 'Pending', completed: false },
      { id: '2', title: 'Completed', completed: true },
    ];
    getTasks.mockResolvedValue(mockTasks);

    const { pendingTasks, loadTasks } = useTaskStore();
    await loadTasks();

    expect(pendingTasks.value).toHaveLength(1);
    expect(pendingTasks.value[0].title).toBe('Pending');
  });

  it('should filter completed tasks', async () => {
    const mockTasks = [
      { id: '1', title: 'Pending', completed: false },
      { id: '2', title: 'Completed', completed: true },
    ];
    getTasks.mockResolvedValue(mockTasks);

    const { completedTasks, loadTasks } = useTaskStore();
    await loadTasks();

    expect(completedTasks.value).toHaveLength(1);
    expect(completedTasks.value[0].title).toBe('Completed');
  });

  it('should create a task', async () => {
    const newTask = { id: '', title: 'New Task', completed: false };
    const mockTasks = [{ id: '1', title: 'New Task', completed: false }];
    addTask.mockResolvedValue();
    getTasks.mockResolvedValue(mockTasks);

    const { createTask, state } = useTaskStore();
    await createTask(newTask);

    expect(addTask).toHaveBeenCalledWith(newTask);
    expect(getTasks).toHaveBeenCalled();
    expect(state.tasks).toEqual(mockTasks);
  });

  it('should update a task', async () => {
    const taskId = '1';
    const updatedTask = { id: '1', title: 'Updated', completed: false };
    const mockTasks = [{ id: '1', title: 'Updated', completed: false }];
    updateTask.mockResolvedValue();
    getTasks.mockResolvedValue(mockTasks);

    const { modifyTask, state } = useTaskStore();
    await modifyTask(taskId, updatedTask);

    expect(updateTask).toHaveBeenCalledWith(taskId, updatedTask);
    expect(state.tasks).toEqual(mockTasks);
  });

  it('should delete a task', async () => {
    const taskId = '1';
    const mockTasks = [];
    deleteTask.mockResolvedValue();
    getTasks.mockResolvedValue(mockTasks);

    const { removeTask, state } = useTaskStore();
    await removeTask(taskId);

    expect(deleteTask).toHaveBeenCalledWith(taskId);
    expect(state.tasks).toEqual(mockTasks);
  });

  it('should toggle a task', async () => {
    const taskId = '1';
    const mockTasks = [{ id: '1', title: 'Task', completed: true }];
    toggleTask.mockResolvedValue();
    getTasks.mockResolvedValue(mockTasks);

    const { markTask, state } = useTaskStore();
    await markTask(taskId);

    expect(toggleTask).toHaveBeenCalledWith(taskId);
    expect(state.tasks[0].completed).toBe(true);
  });

  it('should handle errors', async () => {
    const errorMsg = 'Failed to load tasks';
    getTasks.mockRejectedValue(new Error(errorMsg));

    const { state, loadTasks } = useTaskStore();
    await loadTasks();

    expect(state.error).toBe(errorMsg);
  });

  it('should load reminders', async () => {
    const mockReminders = [{ id: '1', title: 'Reminder', time: '09:00' }];
    getReminders.mockResolvedValue(mockReminders);

    const { state, loadReminders } = useTaskStore();
    await loadReminders();

    expect(getReminders).toHaveBeenCalled();
    expect(state.reminders).toEqual(mockReminders);
  });

  it('should get a single task', async () => {
    const taskId = '1';
    const mockTask = { id: '1', title: 'Single Task' };
    getTask.mockResolvedValue(mockTask);

    const { loadTask } = useTaskStore();
    const result = await loadTask(taskId);

    expect(getTask).toHaveBeenCalledWith(taskId);
    expect(result).toEqual(mockTask);
  });
});
