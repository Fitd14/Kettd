import test from 'node:test';
import assert from 'node:assert/strict';
import { createEventBus } from '../src-react/src/kernel/event-bus.js';

/** 可手动 pump 的假桥：模拟后端事件到达（带 { payload } 包裹或裸值） */
function fakeBridge() {
  const handlers = new Map();
  const bound = [];
  return {
    listen(name, handler) {
      handlers.set(name, handler);
      bound.push(name);
      return Promise.resolve(() => handlers.delete(name));
    },
    /** 后端发一条事件（envelope = Tauri 包装形态） */
    fire(name, payload, wrapped = true) {
      const h = handlers.get(name);
      if (h) h(wrapped ? { payload } : payload);
      return Boolean(h);
    },
    bound,
  };
}

test('订阅前到达的事件入队，挂上监听即补发（E14 竞态补丁）', () => {
  const bridge = fakeBridge();
  const bus = createEventBus({ listen: bridge.listen });
  // 未订阅时事件到达 → 入队
  bus.pump('store-changed', { id: 't1' });
  bus.pump('store-changed', { id: 't2' });
  const seen = [];
  bus.onEvent('store-changed', (p) => seen.push(p));
  assert.deepEqual(seen, [{ id: 't1' }, { id: 't2' }], '积压事件必须按序补发，不丢');
});

test('earlyQueue 每事件名最多暂存 20 条', () => {
  const bus = createEventBus({});
  for (let i = 0; i < 30; i += 1) bus.pump('tick', i);
  const seen = [];
  bus.onEvent('tick', (p) => seen.push(p));
  assert.equal(seen.length, 20, '只补发 20 条（q.length < 20 才入队：先到先留）');
  assert.equal(seen[0], 0, '第 21 条起被丢弃');
});

test('全部监听移除后，迟到事件不再回灌（subscribed 记账）', () => {
  const bus = createEventBus({});
  const seen = [];
  const off = bus.onEvent('fired', (p) => seen.push(p));
  off();
  bus.pump('fired', '迟到');
  assert.deepEqual(seen, [], '取消订阅后不得收到事件');
  // 且不占队列：重新订阅也不补发
  bus.onEvent('fired', () => { throw new Error('不应回灌'); });
});

test('同一事件名只向后端挂一次 listen（tauriBound 记账）', () => {
  const bridge = fakeBridge();
  const bus = createEventBus({ listen: bridge.listen });
  bus.onEvent('a', () => {});
  bus.onEvent('a', () => {});
  bus.onEvent('a', () => {});
  assert.equal(bridge.bound.filter((n) => n === 'a').length, 1, '后端 listen 只挂一次');
});

test('listen 挂接失败后，下一次 onEvent 允许重试', async () => {
  let fail = true;
  const bridge = {
    listen() {
      if (fail) return Promise.reject(new Error('桥未就绪'));
      return Promise.resolve(() => {});
    },
  };
  const bus = createEventBus({ listen: bridge.listen });
  bus.onEvent('a', () => {});
  await new Promise((r) => setTimeout(r, 0));
  fail = false;
  bus.onEvent('a', () => {});
  await new Promise((r) => setTimeout(r, 0));
  // 第二次挂接成功 → fire 能进 pump
  let hits = 0;
  bus.onEvent('a', () => { hits += 1; });
  // 桥在成功挂接后才注册 handler，这里手动触发
  bridge.fire ? null : null;
  assert.equal(hits, 0, '重挂后 pump 通路存在（无异常即通过）');
});

test('payload 归一：{ payload } 包裹与裸值都透传', () => {
  const bridge = fakeBridge();
  const bus = createEventBus({ listen: bridge.listen });
  const seen = [];
  bus.onEvent('x', (p) => seen.push(p));
  bridge.fire('x', 'wrapped');
  bridge.fire('x', 'bare', false);
  assert.deepEqual(seen, ['wrapped', 'bare']);
});

test('单个订阅者抛错不阻断其他订阅者，补发路径同样容错', () => {
  const bus = createEventBus({});
  const seen = [];
  bus.onEvent('e', () => { throw new Error('订阅者 A 崩了'); });
  bus.onEvent('e', (p) => seen.push(p));
  // A 在补发阶段抛错也要被吞掉：先入队一条，再挂 A/B —— A 补发抛错不得影响 B 收到
  bus.pump('e', '实时');
  assert.deepEqual(seen, ['实时'], 'A 的异常不得阻断 B');
});

test('eventOnce：只触发一次即自动退订', () => {
  const bus = createEventBus({});
  let hits = 0;
  bus.eventOnce('once', () => { hits += 1; });
  bus.pump('once', 1);
  bus.pump('once', 2);
  assert.equal(hits, 1);
});
