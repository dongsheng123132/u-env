// Gate 0 冒烟测试：CDP 直连无头 Chrome，打开测试页，抽取页面事实。
// 用法：先起 Chrome（remote-debugging-port=9222），再 node smoke-cdp.mjs
const PORT = process.env.CDP_PORT || 9222;
const fixture = process.argv[2] || 'order-form.html';
const fileUrl = 'file:///' + process.cwd().replace(/\\/g, '/') + '/fixtures/' + fixture;
const encoded = encodeURI(fileUrl);

const tab = await fetch(`http://127.0.0.1:${PORT}/json/new?${encoded}`, { method: 'PUT' }).then(r => r.json());

const ws = new WebSocket(tab.webSocketDebuggerUrl);
let id = 0;
const pending = new Map();
function send(method, params = {}) {
  return new Promise((resolve, reject) => {
    const msgId = ++id;
    pending.set(msgId, { resolve, reject });
    ws.send(JSON.stringify({ id: msgId, method, params }));
  });
}
ws.onmessage = (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    if (msg.error) reject(new Error(msg.error.message)); else resolve(msg.result);
  }
};
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
await send('Runtime.enable');
await new Promise(r => setTimeout(r, 1200));

// 等页面加载完成
await send('Page.enable');
await new Promise(r => setTimeout(r, 500));

const script = `JSON.stringify({
  url: location.href,
  title: document.title,
  grandTotal: (document.getElementById('grand-total') || {}).textContent,
  productRows: document.querySelectorAll('.product-row').length,
  submitBtn: (document.getElementById('submit-btn') || {}).textContent
})`;
const evalRes = await send('Runtime.evaluate', { expression: script, returnByValue: true });
console.log(evalRes.result.value);
ws.close();
