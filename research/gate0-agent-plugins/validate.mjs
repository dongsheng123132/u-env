#!/usr/bin/env node
// validate.mjs —— 零依赖 Agent Plugins 1.0.0 结构冒烟校验 + 可移植性扫描
//
// 结构规则摘自 agent-plugins.org 1.0.0 规范（plugin.schema.json / mcp.schema.json /
// §4.1 路径围堵 / §7.1 skills 布局）。官方一致性校验用 agent-plugin-ts（需 npm），
// 本脚本是纯 std 的快速冒烟版，任何机器有 node 就能跑。
//
// 用法: node validate.mjs <plugin-dir>
//
// 退出码: 0 = 结构通过（可移植性警告仍会列出）; 1 = 结构不合法

import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import path from 'node:path';

const SCHEMA_URL = 'https://agent-plugins.org/schemas/1.0.0/plugin.schema.json';
const NAME_RE = /^(?!.*(?:--|\.\.))[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/;
const ALLOWED_MANIFEST_FIELDS = new Set([
  '$schema', 'name', 'version', 'description', 'author',
  'homepage', 'repository', 'license', 'keywords', 'extensions',
]);

const root = path.resolve(process.argv[2] ?? '.');
let fail = 0;
const warn = [];

function check(ok, label) {
  console.log(`${ok ? '✅' : '❌'} ${label}`);
  if (!ok) fail++;
}

// ---- 1. plugin.json 清单 ----
if (!existsSync(path.join(root, 'plugin.json'))) {
  check(false, 'plugin.json 存在于根目录');
  process.exit(1);
}
check(true, 'plugin.json 存在于根目录');

let manifest;
try {
  manifest = JSON.parse(readFileSync(path.join(root, 'plugin.json'), 'utf8'));
  check(true, 'plugin.json 是合法 JSON');
} catch {
  check(false, 'plugin.json 是合法 JSON');
  process.exit(1);
}
check(manifest.$schema === SCHEMA_URL, `$schema 指向 1.0.0 常量 (${manifest.$schema ?? '缺失'})`);
check(typeof manifest.name === 'string' && manifest.name.length >= 1 && manifest.name.length <= 64 && NAME_RE.test(manifest.name), `name 满足 pattern/长度约束 (${manifest.name ?? '缺失'})`);
const unknown = Object.keys(manifest).filter(k => !ALLOWED_MANIFEST_FIELDS.has(k));
check(unknown.length === 0, `无未知顶层字段${unknown.length ? `（发现: ${unknown.join(', ')}）` : ''}`);

// ---- 2. skills/ 布局 ----
const skillsRoot = path.join(root, 'skills');
const skillDirs = existsSync(skillsRoot) ? readdirSync(skillsRoot) : [];
let skillsOk = 0;
for (const d of skillDirs) {
  const sd = path.join(skillsRoot, d);
  if (!statSync(sd).isDirectory()) { warn.push(`skills/${d} 不是目录（应忽略或清理）`); continue; }
  const sm = path.join(sd, 'SKILL.md');
  if (!existsSync(sm)) { check(false, `skills/${d}/SKILL.md 存在`); continue; }
  check(true, `skills/${d}/SKILL.md 存在`);
  const txt = readFileSync(sm, 'utf8');
  const fm = /^---\r?\n([\s\S]*?)\r?\n---/.exec(txt);
  const hasName = fm && /\bname\s*:/.test(fm[1]);
  const hasDesc = fm && /\bdescription\s*:/.test(fm[1]);
  check(!!fm && hasName && hasDesc, `skills/${d}/SKILL.md 有 frontmatter (name+description)`);
  if (fm && hasName && hasDesc) skillsOk++;
}
check(skillDirs.length === 0 || skillsOk > 0, `skills/ 布局（${skillsOk}/${skillDirs.length} 个目录含合法 SKILL.md${skillDirs.length ? '' : '，目录缺失非错误'}）`);

// ---- 3. mcp.json（可选）----
if (existsSync(path.join(root, 'mcp.json'))) {
  try {
    const mcp = JSON.parse(readFileSync(path.join(root, 'mcp.json'), 'utf8'));
    check(mcp.$schema === 'https://agent-plugins.org/schemas/1.0.0/mcp.schema.json', 'mcp.json $schema');
    check(!!mcp.mcpServers && typeof mcp.mcpServers === 'object', 'mcp.json 含 mcpServers');
  } catch {
    check(false, 'mcp.json 是合法 JSON');
  }
} else {
  console.log('ℹ️ mcp.json 缺席 —— 本插件只有 skills 层，符合规范（mcp 可选）');
}

// ---- 4. 路径围堵（§4.1）：SKILL.md 里不得引用插件根之外的相对路径 ----
for (const d of skillDirs) {
  const sm = path.join(skillsRoot, d, 'SKILL.md');
  if (!existsSync(sm)) continue;
  const txt = readFileSync(sm, 'utf8');
  const escapes = txt.match(/(?:`|\s|"|\()(\.\.\/)+/g);
  if (escapes) warn.push(`skills/${d}/SKILL.md 出现 ${escapes.length} 处 ../ 引用（围堵检查，看是否越界）`);
}

// ---- 5. 可移植性扫描：硬编码本机路径 ----
for (const d of skillDirs) {
  const sm = path.join(skillsRoot, d, 'SKILL.md');
  if (!existsSync(sm)) continue;
  const txt = readFileSync(sm, 'utf8');
  const hits = [];
  for (const m of txt.matchAll(/~\/\.uking|\b[CcDd]:\\|\.uking\/skills/g)) hits.push(m[0]);
  if (hits.length) warn.push(`skills/${d}/SKILL.md 硬编码本机路径 ${hits.length} 处（${[...new Set(hits)].join(', ')}）→ 不可移植`);
}

// ---- 汇总 ----
console.log('\n── 汇总 ──');
console.log(`结构校验: ${fail === 0 ? '全部通过 ✅' : `${fail} 项失败 ❌`}`);
console.log(`可移植性警告: ${warn.length} 条`);
for (const w of warn) console.log(`  ⚠️  ${w}`);
process.exit(fail === 0 ? 0 : 1);
