// 验证：Chrome 持久 profile 能否跨进程保留状态（localStorage/cookie）。
// 本地 HTTP 服务 + http://127.0.0.1 标准 origin → 设 localStorage → 关 Chrome → 同 profile 重开 → 读回。
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtempSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const CHROME = 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const PORT = 9224;
const HTTP_PORT = 9230;
const profile = mkdtempSync(join(tmpdir(), 'persist-'));
process.env.CDP_PORT = String(PORT);

// 本地 HTTP 服务
const http = createServer((req, res) => {
  res.writeHead(200, { 'content-type': 'text/html' });
  res.end('<!DOCTYPE html><html><body><h1>persist-test</h1></body></html>');
});
await new Promise(r => http.listen(HTTP_PORT, '127.0.0.1', r));

function launch() {
  return spawn(CHROME, ['--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`, '--no-first-run', 'about:blank'], { stdio: 'ignore' });
}
async function waitReady() {
  for (let i = 0; i < 40; i++) { try { const r = await fetch(`http://127.0.0.1:${PORT}/json/version`); if (r.ok) return; } catch {} await new Promise(r => setTimeout(r, 250)); }
  throw new Error('CDP 未就绪');
}
async function evalInTab(expr) {
  const url = `http://127.0.0.1:${HTTP_PORT}/`;
  const tab = await fetch(`http://127.0.0.1:${PORT}/json/new?${encodeURIComponent(url)}`, { method: 'PUT' }).then(r => r.json());
  const ws = new WebSocket(tab.webSocketDebuggerUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  let id = 0; const pend = new Map();
  ws.onmessage = (ev) => { const m = JSON.parse(ev.data); if (m.id && pend.has(m.id)) { const p = pend.get(m.id); pend.delete(m.id); m.error ? p.reject(new Error(m.error.message)) : p.resolve(m.result); } };
  const send = (method, params = {}) => new Promise((res, rej) => { const i = ++id; pend.set(i, { resolve: res, reject: rej }); ws.send(JSON.stringify({ id: i, method, params })); });
  await send('Runtime.enable');
  await new Promise(r => setTimeout(r, 600));
  const out = await send('Runtime.evaluate', { expression: expr, returnByValue: true });
  const val = out.result?.value;
  const exc = out.exceptionDetails?.text ?? null;
  ws.close();
  await fetch(`http://127.0.0.1:${PORT}/json/close/${tab.id}`);
  return { val, exc };
}

let p = launch();
await waitReady();
const w1 = await evalInTab(`localStorage.setItem('uking-login-token', 'demo-12345'); 'set:' + localStorage.getItem('uking-login-token')`);
console.log('写:', w1);
p.kill();
await new Promise(r => setTimeout(r, 1200));

p = launch();
await waitReady();
const w2 = await evalInTab(`'read:' + localStorage.getItem('uking-login-token')`);
console.log('读:', w2);
p.kill();
http.close();

const ok = w2.val === 'read:demo-12345';
console.log(ok ? 'PASS ✅ 持久 profile 跨进程保留状态（登录态可复用）' : 'FAIL ❌');
