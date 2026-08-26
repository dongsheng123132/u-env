#!/usr/bin/env node
// 本境 uenv ⟷ U-King envfp 的**口径对账**。
//
// 为什么要有：同一件事实（「这台 Windows 有什么炸点」）两边各有一份实现。
// 宪法第 8 条 —— 同一事实存在几份就会漂移几份，而**漏掉的那一份不会报错**。
// 2026-08-08 首次对账：同一台机器，U-King 说 1 个问题，uenv 说 3 个（见 docs/12）。
//
// 这个脚本不判谁对谁错，它只做一件事：**把差异变响**。
//   - 两边都覆盖 → 比结论一致不一致
//   - 只有一边覆盖 → 列为覆盖缺口（**不是失败**，但必须说出来）
//
// 用法：
//   node tools/crosscheck-uking.mjs [--uking <u-king-mini.exe 路径>] [--json]
//
// 退出码：0 = 没有「结论冲突」；1 = 有冲突（覆盖缺口不算冲突，只报告）

import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

const HERE = dirname(fileURLToPath(import.meta.url))
const ROOT = join(HERE, '..')
const argv = process.argv.slice(2)
const arg = (k, d) => { const i = argv.indexOf(k); return i >= 0 ? argv[i + 1] : d }

const UENV = existsSync(join(ROOT, 'target/release/uenv.exe'))
  ? join(ROOT, 'target/release/uenv.exe')
  : join(ROOT, 'target/debug/uenv.exe')
const UKING = arg('--uking', 'C:/Users/<user>/Desktop/claude/u-claw/u-king简化版-u盘版本/src-tauri/target/debug/u-king-mini.exe')

/**
 * 映射表 —— **本境的 rule_id 是命名真源**，U-King 的指纹位映射到它。
 *
 * `uking: null` = U-King 侧**没有这个维度**（覆盖缺口，如实列出，别假装齐平）。
 * `note` 写清两边判据的差别 —— 判据不同的「一致」是假一致。
 */
const MAP = [
  {
    rule: 'security.defender-scans-project',
    uking: (fp) => fp.defender_rt,
    note: 'U-King 只看「实时保护开没开」；本境还要求「排除项没覆盖项目目录」。判据更细的一方是本境',
    // U-King 只要开着就报，本境要开着且没排除才报 → U-King=true 而本境没命中是**预期内**的
    laxerSide: 'uking',
  },
  {
    rule: 'fs.project-path-non-ascii',
    uking: null,
    note: 'U-King 的 path_nonascii 量的是**用户家目录**，不是项目路径 —— 名字像、事实不同，不能对齐',
  },
  { rule: 'webview2.missing', uking: null, note: '★ U-King 指纹里没有 WebView2，而缺它时 U-King 是静默假死' },
  { rule: 'windows.long-paths', uking: (fp) => !fp.long_paths, note: 'U-King 记「开着吗」，本境报「没开」' },
  { rule: 'git.longpaths-disabled', uking: null, note: 'U-King 只看系统级 LongPaths，不看 git 的 core.longpaths' },
  { rule: 'node.multiple-in-path', uking: null, note: 'U-King 只记 node 版本，不查 PATH 里有几个' },
  { rule: 'rust.multiple-cargo-in-path', uking: null, note: 'U-King 不涉及 Rust 工具链（客户机不编译）' },
  // 🔴 这条一度被我映射成 `fp.proxy`，对账当场报「冲突」—— 而那是**映射错了**，不是产品漂移：
  // U-King 的 `proxy` 是「有没有配代理」，本境这条是「几处代理设置**自相矛盾**」。
  // 跟 path_nonascii 一模一样的陷阱：名字像、量的东西不同。写这张表时最容易犯的就是这个错，
  // 所以留在这儿当标本 —— 对不齐就老实写 null，别硬对。
  { rule: 'net.proxy-inconsistent', uking: null, note: 'U-King 的 proxy 只是「有没有配代理」，不是「设置自相矛盾」，两者对不齐' },
  { rule: 'windows.developer-mode-disabled', uking: null, note: 'U-King 无此维度' },
  { rule: 'python.store-alias-shadow', uking: null, note: 'U-King 无此维度（但它装 Hermes 依赖 Python，值得补）' },
  // 以下 6 条本境有、U-King 一条都没有。显式列出来而不是让它们从对账里消失。
  { rule: 'path.duplicate-entries', uking: null, note: 'U-King 无此维度' },
  { rule: 'path.missing-entries', uking: null, note: '★ U-King 栽过：客户 PATH 被改坏丢了 System32 导致装不上，正是这条' },
  { rule: 'node.version-drift', uking: null, note: 'U-King 只取单一 node 版本，不比对多处声明' },
  { rule: 'node.package-manager-mismatch', uking: null, note: 'U-King 无此维度' },
  { rule: 'node.multiple-lockfiles', uking: null, note: 'U-King 无此维度（客户机不开发，优先级低）' },
  { rule: 'git.autocrlf-true', uking: null, note: 'U-King 只记 git 版本，不看 autocrlf' },
]

const run = (exe, args) =>
  execFileSync(exe, args, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, stdio: ['ignore', 'pipe', 'ignore'] })

let uenvOut, fp
try {
  uenvOut = JSON.parse(run(UENV, ['doctor', '--project', '.', '--agent']))
} catch (e) {
  console.error(`跑不动 uenv（${UENV}）：${e.message}\n先 cargo build --release`)
  process.exit(2)
}
try {
  fp = JSON.parse(run(UKING, ['--envfp']))
} catch (e) {
  console.error(`跑不动 U-King（${UKING}）：${e.message}\n用 --uking 指到它的 exe`)
  process.exit(2)
}

const data = uenvOut.data ?? uenvOut
const hit = new Map((data.findings ?? []).map((f) => [f.rule_id, f.severity]))
const skipped = new Set(data.skipped_rules ?? [])
const known = new Set([...hit.keys(), ...skipped])

const rows = []
for (const m of MAP) {
  const inUenv = hit.has(m.rule) // 本境是否报了这条
  if (m.uking === null) {
    rows.push({ rule: m.rule, kind: 'gap', uenv: inUenv ? hit.get(m.rule) : 'clean', uking: '未覆盖', note: m.note })
    continue
  }
  const ukingFlag = !!m.uking(fp)
  // 判据宽的一方多报，是**预期内**的，不算冲突 —— 否则这条对账会天天叫。
  const conflict = m.laxerSide === 'uking' ? (!ukingFlag && inUenv) : ukingFlag !== inUenv
  rows.push({
    rule: m.rule,
    kind: conflict ? 'conflict' : 'agree',
    uenv: inUenv ? hit.get(m.rule) : 'clean',
    uking: ukingFlag ? 'flagged' : 'clean',
    note: m.note,
  })
}

// 映射表没跟上本境新增规则时，必须喊 —— 否则新规则会静默地不进对账。
const unmapped = [...known].filter((r) => !MAP.some((m) => m.rule === r))
const conflicts = rows.filter((r) => r.kind === 'conflict')
const gaps = rows.filter((r) => r.kind === 'gap')

if (argv.includes('--json')) {
  console.log(JSON.stringify({ conflicts: conflicts.length, gaps: gaps.length, unmapped, rows }, null, 2))
} else {
  console.log('# 本境 uenv ⟷ U-King envfp 口径对账\n')
  console.log(`  本境规则 ${known.size} 条（命中 ${hit.size}）· U-King 指纹 ${Object.keys(fp).length} 位\n`)
  for (const r of rows) {
    const mark = r.kind === 'conflict' ? '✗ 冲突' : r.kind === 'gap' ? '· 缺口' : '✓ 一致'
    console.log(`  ${mark}  ${r.rule.padEnd(34)} 本境=${String(r.uenv).padEnd(8)} U-King=${r.uking}`)
    if (r.kind !== 'agree') console.log(`          ${r.note}`)
  }
  console.log(`\n  一致 ${rows.length - conflicts.length - gaps.length} / 冲突 ${conflicts.length} / U-King 未覆盖 ${gaps.length}`)
  if (unmapped.length) {
    console.log(`\n⚠ 本境有 ${unmapped.length} 条规则不在映射表里，没进对账：`)
    for (const r of unmapped) console.log(`    ${r}`)
    console.log('  → 补进 MAP，或显式写成 uking:null（覆盖缺口不丢人，静默才丢人）')
  }
  console.log(
    conflicts.length
      ? '\n✗ 有结论冲突 —— 同一台机器两边说法不一样，客户会看到互相矛盾的建议'
      : '\n✓ 无结论冲突（覆盖缺口见上，那是待补不是错）',
  )
}

process.exit(conflicts.length ? 1 : 0)
