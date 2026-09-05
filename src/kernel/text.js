/**
 * 共享内核 · 文本工具（ADR-0006）
 *
 * 规则：本目录下的模块必须是**纯 ESM、零全局依赖、零 Tauri 依赖** ——
 * vanilla 三窗与 src-react 共用同一份，node 测试可直接 import。
 * 任何 import '@tauri-apps/*' 或读写 window/document 的逻辑都不许放这里。
 */

/** HTML 转义。所有拼 innerHTML 的地方都必须过它。 */
export function esc(s) {
  return String(s == null ? '' : s).replace(/[&<>"']/g, (c) => (
    { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
