/* 事件总线（ADR-0006 内核件 · E14「事件补发队列」的唯一实现）
 *
 * 语义（自 vanilla api.js 移植，测试 test/event-bus.test.mjs 逐条固定，移植必须保真）：
 * - 订阅前到达的事件先入 earlyQueue（每事件名暂存最近 20 条），挂上监听即补发，不丢；
 *   它是「启动时主窗隐藏收不到 store-changed」这一真实竞态的唯一补丁。
 * - subscribed 记账：某事件名的全部监听被移除后，迟到事件**不再回灌**（防旧事件僵尸复活）。
 * - tauriBound 记账：同一事件名只向后端 listen 挂一次；挂接失败（Promise reject/throw）
 *   允许下一次 onEvent 重试。
 * - payload 归一：后端事件可能是 { payload } 包裹或裸值。
 * - 单个订阅者异常不阻断其他订阅者（补发路径同样容错）。
 */

/**
 * @param {{ listen?: (name: string, handler: (ev: any) => void) => any }} tauri
 *        桥的 listen（globalThis.__TAURI__.event.listen 或等价物）；缺省 = 纯本地总线（测试态）。
 */
export function createEventBus(tauri) {
  const rawListen = tauri && typeof tauri.listen === 'function' ? tauri.listen : null;

  const listeners = Object.create(null); // name -> [handler]
  const earlyQueue = Object.create(null); // name -> [payload]（无监听器时暂存最近 20 条）
  const tauriBound = Object.create(null); // name -> true（该事件已挂上后端监听）
  const subscribed = Object.create(null); // name -> true（取消订阅后不再回灌旧事件）

  function pump(name, payload) {
    const subs = (listeners[name] || []).slice();
    if (!subs.length) {
      if (subscribed[name]) return;
      const q = earlyQueue[name] || (earlyQueue[name] = []);
      if (q.length < 20) q.push(payload);
      return;
    }
    for (const handler of subs) {
      try { handler(payload); } catch (_) { /* 单个订阅者异常不阻断其他订阅者 */ }
    }
  }

  /** 返回取消订阅函数；注册时先补发队列里积压的事件 */
  function onEvent(name, handler) {
    (listeners[name] || (listeners[name] = [])).push(handler);
    subscribed[name] = true;
    const queued = earlyQueue[name] || [];
    earlyQueue[name] = [];
    for (const p of queued) {
      try { handler(p); } catch (_) { /* 补发同样容错 */ }
    }
    if (typeof rawListen === 'function' && !tauriBound[name]) {
      tauriBound[name] = true;
      try {
        const pr = rawListen(name, (ev) => pump(name, ev && typeof ev === 'object' && 'payload' in ev ? ev.payload : ev));
        if (pr && typeof pr.catch === 'function') pr.catch(() => { tauriBound[name] = false; });
      } catch (_) { tauriBound[name] = false; }
    }
    return () => {
      const arr = listeners[name] || [];
      const i = arr.indexOf(handler);
      if (i >= 0) arr.splice(i, 1);
    };
  }

  function eventOnce(name, handler) {
    const off = onEvent(name, (p) => { off(); handler(p); });
    return off;
  }

  return { onEvent, eventOnce, pump };
}
