// Gate 0 批量执行 runner：任务 × 问题 × 臂 × 轮，调执行模型（默认 hermes=deepseek-v4-flash HTTP 直连）。
// 复用本象协议 shadowbench-w 的 model.mjs（createModel），不重写、防漂移。
// 用法：
//   node run-gate0.mjs                 # 试点：task1 全部问题 × 臂 A,C × 1 轮
//   node run-gate0.mjs --task task2 --arms A,C --rounds 2
//   node run-gate0.mjs --dry           # 只生成提示词文件，不调模型
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { TASKS } from './tasks.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), 'runs');
const args = process.argv.slice(2);
const get = (k) => { const i = args.indexOf('--' + k); return i >= 0 ? args[i + 1] : null; };
const taskId = get('task') ?? 'task1';
const arms = (get('arms') ?? 'A,C').split(',').map(s => s.trim().toUpperCase());
const rounds = parseInt(get('rounds') ?? '1', 10);
const dry = args.includes('--dry');

const task = TASKS[taskId];
if (!task) { console.error('未知任务:', taskId); process.exit(1); }

// --- 加载各臂内容 ---
const armContent = {};
if (arms.includes('A')) armContent.A = readFileSync(join(ROOT, taskId, 'arms/A-text.txt'), 'utf8');
if (arms.includes('B')) armContent.B = '<截图 PNG，需视觉模型读取（见 runs/<task>/arms/B-shot.png）>';
if (arms.includes('C')) {
  const pkg = JSON.parse(readFileSync(join(ROOT, taskId, 'arms/C-package.json'), 'utf8'));
  armContent.C = JSON.stringify({ title: pkg.identity.title, hash: pkg.identity.hash, graph: pkg.graph, atlas: pkg.atlas });
}

const ARM_INTRO = {
  A: '[页面表示 · 纯文本]（该页面的文本内容）',
  B: '[页面表示 · 截图]（附 B-shot.png，图片）',
  C: '[页面表示 · 本象包]（identity + graph 对象图 + constraints 不变量声明；constraints 是页面必须满足的不变量，可据此推导与验证）',
};

function buildPrompt(arm, qid, question) {
  return [
    '你是网页操作助手。下面给出一个网页的「表示」。页面处于初始未操作状态。',
    '请严格基于给出的表示回答后面的问题。不要编造表示里没有的信息。回答用中文，精炼。',
    '',
    ARM_INTRO[arm],
    armContent[arm],
    '',
    `[问题 ${qid}] ${question}`,
  ].join('\n');
}

const outDir = join(ROOT, taskId, 'results');
mkdirSync(outDir, { recursive: true });

// --- 模型（dry 模式不加载） ---
let model = null;
if (!dry) {
  const mod = await import(pathToFileURL('D:/uking编程/本象协议/benchmark/shadowbench-w/arms/lib/model.mjs').href);
  const provider = get('provider') ?? 'hermes';
  model = mod.createModel({ provider, retry: true });
  console.log(`执行模型: ${model.id}`);
}

for (const q of task.questions) {
  for (const arm of arms) {
    for (let r = 1; r <= rounds; r++) {
      const prompt = buildPrompt(arm, q.id, q.text);
      const file = join(outDir, `${arm}.${q.id}.r${r}.json`);
      writeFileSync(file, JSON.stringify({ task: taskId, qid: q.id, arm, round: r, prompt }, null, 2));
      if (dry) { console.log(`[dry] ${taskId} ${q.id} ${arm} r${r} → 提示词 ${prompt.length} 字符`); continue; }
      try {
        // deepseek-v4-flash 是推理模型：max_tokens 给足，否则 reasoning 吃光配额、content 为空
        let res = await model.complete({ prompt, maxTokens: 32768 });
        if (!res.raw || !res.raw.trim()) {
          console.warn(`[retry 空输出] ${taskId} ${q.id} ${arm} r${r}（finish=${res.finishReason}），重试一次`);
          res = await model.complete({ prompt, maxTokens: 32768 });
        }
        writeFileSync(file, JSON.stringify({ task: taskId, qid: q.id, arm, round: r, prompt, output: res.raw, finish: res.finishReason ?? null, usage: res.usage ?? null }, null, 2));
        console.log(`[ok] ${taskId} ${q.id} ${arm} r${r} · ${(res.usage?.inputTokens ?? 0)} in / ${(res.usage?.outputTokens ?? 0)} out · ${res.raw.length} 字符输出`);
      } catch (e) {
        writeFileSync(file, JSON.stringify({ task: taskId, qid: q.id, arm, round: r, prompt, error: String(e) }, null, 2));
        console.error(`[err] ${taskId} ${q.id} ${arm} r${r}: ${e.message?.slice(0, 120)}`);
      }
    }
  }
}
console.log('完成。结果在', outDir);
