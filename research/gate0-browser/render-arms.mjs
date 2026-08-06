// 三臂渲染：对每个任务，把同一页面快照渲染成三种输入表示，写入 runs/<task>/arms/。
//   A-text.txt      → 纯文本（document.body.innerText）
//   B-shot.png      → 纯截图（Page.captureScreenshot）
//   C-package.json  → 本象包（identity + graph 对象图 + 约束声明 + atlas 引用）
import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { launchChrome, stopChrome } from './lib/chrome.mjs';
import { snapshotFixture } from './lib/snapshot.mjs';
import { TASKS } from './tasks.mjs';

const PORT = 9223;
process.env.CDP_PORT = String(PORT);

const outRoot = join(dirname(fileURLToPath(import.meta.url)), 'runs');

// 从通用快照构造本象包 graph 对象图
function buildGraph(snap) {
  const objects = [{ id: 'page', type: 'page', props: { title: snap.identity.title, hash: snap.identity.hash } }];
  let i = 0;
  for (const el of snap.facts.inputs) objects.push({ id: `in:${el.stableId ?? 'i' + i++}`, type: 'input', props: { name: el.name, type: el.type, value: el.value, placeholder: el.placeholder } });
  for (const el of snap.facts.buttons) objects.push({ id: `btn:${el.id ?? 'b' + i++}`, type: 'button', props: { text: el.text, disabled: el.disabled, visible: el.visible } });
  for (const el of snap.facts.links) objects.push({ id: `lnk:${el.id ?? 'l' + i++}`, type: 'link', props: { text: el.text, href: el.href } });
  for (const el of snap.facts.selects) objects.push({ id: `sel:${el.id ?? 's' + i++}`, type: 'select', props: { value: el.value, options: el.options } });
  for (const el of snap.facts.keyValues) objects.push({ id: `kv:${el.id}`, type: 'data', props: { text: el.text, visible: el.visible } });
  for (const el of snap.facts.containers) objects.push({ id: `kv:${el.id}`, type: 'container', props: { tag: el.tag, visible: el.visible } });
  return { objects };
}

const chrome = await launchChrome({ port: PORT });
try {
  for (const [taskId, task] of Object.entries(TASKS)) {
    const dir = outRoot + '/' + taskId + '/arms';
    mkdirSync(dir, { recursive: true });
    const snap = await snapshotFixture(task.fixture);

    writeFileSync(dir + '/A-text.txt', snap.text, 'utf8');
    writeFileSync(dir + '/B-shot.png', Buffer.from(snap.pngBase64, 'base64'));

    const pkg = {
      spec: 'webredline/package/v0.1',
      identity: snap.identity,
      graph: { ...buildGraph(snap), constraints: task.constraints },
      atlas: 'B-shot.png',
      evidence: { snapshot_ts: new Date().toISOString(), screenshot: 'B-shot.png' },
    };
    writeFileSync(dir + '/C-package.json', JSON.stringify(pkg, null, 2), 'utf8');

    console.log(`[${taskId}] 渲染完成 → ${dir}`);
    console.log(`   A 文本 ${snap.text.length} 字符 · B 截图 ${(snap.pngBase64.length * 3 / 4 / 1024).toFixed(1)} KB · C 包 ${pkg.graph.objects.length} 个对象 / ${pkg.graph.constraints.length} 条约束`);
  }
} finally {
  stopChrome(chrome);
}
