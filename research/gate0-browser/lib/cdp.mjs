// Gate 0 最小 CDP 封装：连浏览器 → 开标签页 → 导航 fixture → 执行/抽取。
// 零依赖，Node 22 自带 fetch + WebSocket。
const port = () => parseInt(process.env.CDP_PORT, 10) || 9222;

function connectWs(url) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url);
    ws.onopen = () => resolve(ws);
    ws.onerror = (e) => reject(new Error('ws error: ' + (e.message || 'unknown')));
  });
}

export async function start() {
  const version = await fetch(`http://127.0.0.1:${port()}/json/version`).then(r => r.json());
  const ws = await connectWs(version.webSocketDebuggerUrl);

  let id = 0;
  const pending = new Map();
  ws.onmessage = (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) reject(new Error(msg.error.message)); else resolve(msg.result);
    }
  };

  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const msgId = ++id;
    pending.set(msgId, { resolve, reject });
    ws.send(JSON.stringify({ id: msgId, method, params }));
  });

  const fixturesDir = process.cwd().replace(/\\/g, '/') + '/fixtures/';

  async function goto(fixture) {
    const fileUrl = 'file:///' + fixturesDir + fixture;
    const encoded = encodeURI(fileUrl);
    const tab = await fetch(`http://127.0.0.1:${port()}/json/new?${encoded}`, { method: 'PUT' }).then(r => r.json());
    const tws = await connectWs(tab.webSocketDebuggerUrl);

    let tid = 0;
    const tpend = new Map();
    tws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && tpend.has(msg.id)) {
        const { resolve, reject } = tpend.get(msg.id);
        tpend.delete(msg.id);
        if (msg.error) reject(new Error(msg.error.message)); else resolve(msg.result);
      }
    };
    const tsend = (method, params = {}) => new Promise((resolve, reject) => {
      const msgId = ++tid;
      tpend.set(msgId, { resolve, reject });
      tws.send(JSON.stringify({ id: msgId, method, params }));
    });

    await tsend('Runtime.enable');
    await new Promise(r => setTimeout(r, 1500)); // 等页面加载

    return {
      send: tsend,
      async evalJson(expr) {
        const res = await tsend('Runtime.evaluate', { expression: expr, returnByValue: true });
        return res.result.value;
      },
      async close() {
        tws.close();
        await fetch(`http://127.0.0.1:${port()}/json/close/${tab.id}`);
      }
    };
  }

  return { ws, send, goto };
}
