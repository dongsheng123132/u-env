// 无头 Chrome 生命周期：spawn + 等 CDP 就绪 + kill。
import { spawn } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const CHROME_BIN = process.env.CHROME_BIN
  ?? (process.platform === 'win32'
    ? 'C:/Program Files/Google/Chrome/Application/chrome.exe'
    : 'google-chrome');

export async function launchChrome({ port = 9222 } = {}) {
  const profile = mkdtempSync(join(tmpdir(), 'gate0-'));
  const p = spawn(CHROME_BIN, [
    '--headless=new',
    `--remote-debugging-port=${port}`,
    `--user-data-dir=${profile}`,
    '--no-first-run', '--no-default-browser-check',
    '--disable-gpu', '--no-sandbox',
    'about:blank',
  ], { stdio: 'ignore' });

  for (let i = 0; i < 40; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (r.ok) return p;
    } catch {}
    await new Promise(r => setTimeout(r, 250));
  }
  p.kill();
  throw new Error('Chrome CDP 未在 10s 内就绪');
}

export function stopChrome(p) {
  if (p && !p.killed) { try { p.kill(); } catch {} }
}
