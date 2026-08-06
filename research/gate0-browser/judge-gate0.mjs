// Gate 0 判分：qwen-plus 按任务真值检查点逐项打分（判分只用 qwen-plus——LongMemEval 教训）。
// - 已有 scores 文件时默认直接复用（不重调 qwen-plus），只重新聚合；
// - --overwrite 强制重判。
// 用法：node judge-gate0.mjs [taskId] [--overwrite]
import { readFileSync, writeFileSync, mkdirSync, readdirSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { TASKS } from './tasks.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), 'runs');
const taskId = process.argv[2] ?? 'task1';
const overwrite = process.argv.includes('--overwrite');
const task = TASKS[taskId];

const device = JSON.parse(readFileSync('C:/Users/<user>/.uking/device.json', 'utf8'));
const BASE = 'https://api.u-claw.org.cn/v1';
const KEY = device.key;

async function judgeOne(qid, question, checklist, answer) {
  const prompt = [
    '你是严格的判分员。下面是一个网页操作任务的问题、agent 的回答，以及回答应覆盖的检查点清单。',
    '逐项判断：agent 的回答是否覆盖该检查点（内容正确、明确提及、无自相矛盾即算覆盖）。',
    '注意：scores 的 key 必须逐字等于检查点清单原文，逐项列出，不得增删、不得合并、不得改写。',
    '只输出 JSON，不要输出任何其他文字：',
    JSON.stringify({ format: { scores: { '<检查点原文>': '0 或 1' }, reason: '一句话理由' } }),
    '--- 问题 ---', question,
    '--- 检查点清单 ---', checklist.map((c, i) => `${i + 1}. ${c}`).join('\n'),
    '--- agent 回答 ---', answer,
  ].join('\n');

  const t0 = Date.now();
  let lastErr = null;
  let data = null;
  for (let attempt = 1; attempt <= 3; attempt++) {
    const res = await fetch(`${BASE}/chat/completions`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', authorization: `Bearer ${KEY}` },
      body: JSON.stringify({ model: 'qwen-plus', max_tokens: 1500, messages: [{ role: 'user', content: prompt }] }),
    });
    if (res.ok) { data = await res.json(); break; }
    lastErr = new Error(`qwen-plus ${res.status}: ${(await res.text()).slice(0, 200)}`);
    await new Promise(r => setTimeout(r, 1500 * attempt));
  }
  if (!data) throw lastErr;
  const raw = data.choices?.[0]?.message?.content ?? '';
  const m = raw.match(/\{[\s\S]*\}/);
  let parsed = null;
  if (m) { try { parsed = JSON.parse(m[0]); } catch {} }
  const scores = parsed?.scores ?? parsed?.format?.scores ?? null;
  const reason = parsed?.reason ?? parsed?.format?.reason ?? null;
  return { raw, parsed: { scores, reason }, usage: data.usage, ms: Date.now() - t0 };
}

const resultsDir = join(ROOT, taskId, 'results');
const scoresDir = join(ROOT, taskId, 'scores');
mkdirSync(scoresDir, { recursive: true });

const files = readdirSync(resultsDir).filter(f => f.endsWith('.json'));
const questions = Object.fromEntries(task.questions.map(q => [q.id, q.text]));

let n = 0, err = 0;
const agg = {};
function accumulate(arm, qid, judgeScores) {
  const checklist = task.groundTruth[qid];
  const s = judgeScores ?? {};
  const key = `${arm}.${qid}`;
  agg[key] ??= { items: {}, rounds: 0 };
  agg[key].rounds++;
  const valueOf = (item) => {
    if (s[item] !== undefined) return Number(s[item]) === 1 ? 1 : 0;
    const hit = Object.entries(s).find(([k, v]) => (k.includes(item) || item.includes(k)) && Number(v) === 1);
    return hit ? 1 : 0;
  };
  for (const item of checklist) {
    agg[key].items[item] ??= { sum: 0, count: 0 };
    agg[key].items[item].sum += valueOf(item);
    agg[key].items[item].count++;
  }
}

for (const f of files) {
  const r = JSON.parse(readFileSync(join(resultsDir, f), 'utf8'));
  if (!r.output || !questions[r.qid]) continue;
  const outFile = join(scoresDir, f);

  if (!overwrite && existsSync(outFile)) {
    const jr = JSON.parse(readFileSync(outFile, 'utf8'));
    accumulate(r.arm, r.qid, jr.judge?.scores ?? null);
    console.log(`[reuse] ${taskId} ${f}`);
    continue;
  }
  try {
    const j = await judgeOne(r.qid, questions[r.qid], task.groundTruth[r.qid], r.output);
    const jr = { ...r, judge: { scores: j.parsed?.scores ?? null, reason: j.parsed?.reason ?? null, raw: j.raw, usage: j.usage, ms: j.ms } };
    writeFileSync(outFile, JSON.stringify(jr, null, 2), 'utf8');
    n++;
    accumulate(r.arm, r.qid, jr.judge.scores);
    const s = jr.judge.scores ?? {};
    console.log(`[judge ok] ${taskId} ${f} · ${Object.keys(s).length} 项 · ${Object.values(s).filter(v => Number(v) === 1).length} 通过`);
  } catch (e) {
    err++;
    console.error(`[judge err] ${f}: ${e.message?.slice(0, 140)}`);
  }
}

console.log('\n=== 汇总 ===');
const summary = {};
for (const [key, g] of Object.entries(agg)) {
  const [arm, qid] = key.split('.');
  const perItem = Object.values(g.items).map(it => it.sum / it.count);
  const acc = perItem.reduce((a, b) => a + b, 0) / Math.max(perItem.length, 1);
  summary[key] = { arm, qid, rounds: g.rounds, item_accuracy: +acc.toFixed(3), n_items: perItem.length };
  console.log(`${arm} ${qid}: 准确率 ${(acc * 100).toFixed(1)}% (${g.rounds} 轮 × ${perItem.length} 项)`);
}
writeFileSync(join(scoresDir, 'summary.json'), JSON.stringify(summary, null, 2), 'utf8');
console.log(`\n判分 ${n} 条，复用 ${files.length - n - err} 条，失败 ${err}。汇总在 scores/summary.json`);
