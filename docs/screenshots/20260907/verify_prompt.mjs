// qa2-04 验证：window.prompt 在 Tauri v1 (wry 0.24.12 / WebView2) 是否可用。
// 判据：Runtime.evaluate 调 prompt 后，CDP Page.javascriptDialogOpening 事件到达 = 原生对话框弹出 = prompt 可用；
//       4s 内无事件且 evaluate 立即返回 null = 无 UI 直接吞掉 = 死控件。
// 全程只读：对话框一律 cancel，不点应用 UI，不改数据。
const PORT = process.env.CDP_PORT || '9223'
const base = `http://127.0.0.1:${PORT}`

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

const targets = await (await fetch(base + '/json/list')).json()
console.error('targets:\n  ' + targets.map((t) => `${t.type} | ${t.title} | ${t.url}`).join('\n  '))
const page =
  targets.find((t) => t.type === 'page' && /index\.html/.test(t.url)) ||
  targets.find((t) => t.type === 'page')
if (!page) {
  console.log(JSON.stringify({ verdict: 'ERROR', reason: 'no page target' }))
  process.exit(3)
}

const ws = new WebSocket(page.webSocketDebuggerUrl)
await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws open failed')) })

let msgId = 0
const pending = new Map()
const events = []
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data)
  if (msg.id && pending.has(msg.id)) {
    const p = pending.get(msg.id)
    pending.delete(msg.id)
    msg.error ? p.reject(new Error(JSON.stringify(msg.error))) : p.resolve(msg.result)
  } else if (msg.method === 'Page.javascriptDialogOpening') {
    events.push(msg.params)
  }
}
const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const id = ++msgId
    pending.set(id, { resolve, reject })
    ws.send(JSON.stringify({ id, method, params }))
  })

await send('Page.enable')
await send('Runtime.enable')
const ua = await send('Runtime.evaluate', { expression: 'navigator.userAgent', returnByValue: true })

const evalDone = send('Runtime.evaluate', {
  expression: "window.prompt('qa-verify')",
  returnByValue: true,
})

const sawDialog = await new Promise((resolve) => {
  const t0 = Date.now()
  const timer = setInterval(() => {
    if (events.length) { clearInterval(timer); resolve(true) }
    else if (Date.now() - t0 > 4000) { clearInterval(timer); resolve(false) }
  }, 50)
})

let cancelErr = null
if (sawDialog) {
  await sleep(120)
  try { await send('Page.handleJavaScriptDialog', { actionName: 'cancel' }) } catch (e) { cancelErr = String(e) }
}
let evalResult = null
try { evalResult = await Promise.race([evalDone, sleep(3000).then(() => null)]) } catch (e) { evalResult = { err: String(e) } }

const out = {
  verdict: sawDialog ? 'PROMPT_WORKS' : 'PROMPT_DEAD',
  dialogEvent: events[0] ?? null,
  evalResult: evalResult?.result?.result ?? null,
  cancelErr,
  ua: ua?.result?.value ?? null,
  target: { title: page.title, url: page.url },
}
console.log(JSON.stringify(out, null, 2))
ws.close()
process.exit(sawDialog ? 0 : 2)
