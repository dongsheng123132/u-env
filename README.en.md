# u-env (Origin Environment Protocol reference implementation)

> **One command to find out why your Windows project won't run.**
>
> `uenv doctor` — scans your dev environment and tells you: can this machine run this project? What's missing, and how to fix it.

```
$ uenv doctor --project .

[Warning] 4 items:
  [node.multiple-in-path(Warning)] Multiple Node.js in PATH
    ...
  [rust.multiple-cargo-in-path(Warning)] Multiple cargo in PATH
  [fs.project-path-non-ascii(Warning)] Project path contains non-ASCII chars
  [git.autocrlf-true(Warning)] git core.autocrlf=true is risky for Rust projects

[Info] 3 items:
  [path.duplicate-entries(Info)] Duplicate PATH entries (21 groups!)
  [path.missing-entries(Info)] PATH entries pointing to non-existent dirs (13!)
  [security.defender-scans-project(Info)] Defender real-time protection does not exclude the project dir

Summary: 0 error / 4 warning / 3 info
```

(The output above is a real `uenv doctor` run on this repo's machine — those problems are real. Your machine will differ.)

> **Read-only promise**: uenv only reads; it never writes. Redaction is on by default
> (usernames / machine names / key-like strings are masked). Zero network uploads, no telemetry.
> It suggests fix commands but **never runs them** — every fix is executed by you, on purpose.

## Install

**v0.0.1-alpha**, either way:

**A. Download binary (recommended — no Rust toolchain needed)**

Grab `uenv-x86_64-pc-windows-msvc.zip` from the
[Releases](https://github.com/dongsheng123132/u-env/releases) page, verify the sha256,
unzip, and put `uenv.exe` on your PATH.

**B. Build from source (Rust 1.88+)**

```bash
git clone https://github.com/dongsheng123132/u-env.git
cd u-env
cargo build --release
# binary at target/release/uenv.exe — add to PATH
```

## Quick start

```bash
# Scan the current project environment (produces environment.origin.json)
uenv scan --project . --out environment.origin.json

# Diagnose: can this machine run this project?
uenv doctor --project .

# Environment fingerprint (same machine, valid env → identical hash)
uenv fingerprint

# Good machine vs broken machine — see exactly what differs
uenv diff environment.origin.json fixtures/env-broken.json

# Human-readable report
uenv report --format markdown --project . --out report.md
```

Agent-friendly mode (JSON, non-interactive, exit 1 on failure):

```bash
uenv doctor --project . --agent
```

## Support matrix

| Framework | Scan | Diagnose | Fix | Capsule | Reproduce |
|---|---:|---:|---:|---:|---:|
| Tauri | ✅ | ✅ | Planned | Planned | Planned |
| Electron | ✅ | ✅ | Planned | Planned | Planned |
| Node.js | ✅ | ✅ | Planned | Planned | Planned |
| WinUI/.NET | Beta | Beta | — | Planned | — |

> ⚠️ Honest by design: anything not implemented is marked "Planned".
> Currently supports environment scanning & diagnosis for Tauri / Electron / Node
> projects on Windows 10/11. Other frameworks are being extended gradually.

## Philosophy: Origin Environment Protocol

**Code defines what software does; the environment defines the world in which it can work.**

In the AI era the question is no longer "how to write code" but "in which environment will this code work".
The Origin Environment Protocol turns "environment" into an object that can be scanned, compared, and reproduced:

- **Environment** — the product of one scan (`environment.origin.json`)
- **Fingerprint** — a state hash of the environment; diff a good machine vs a broken one and see exactly what differs
- **Finding + Fix** — diagnostics with executable fix suggestions
- **Capsule** — (planned) freezing an incident scene into a reproducible capsule

> Protocol draft: `docs/protocol-v0.0.1.md` (derived from implementation, evolves with it).

## Repository layout

```
crates/
├── uenv-core/        # data model + JSON Schema export
├── uenv-scanner/     # 26 detectors: Host 10 + Toolchain 9 + Project 5 + samples 2
├── uenv-rules/       # rule engine + 24 diagnostic rules
├── uenv-adapters/    # framework adapters (tauri/electron/node)
├── uenv-fingerprint/ # normalization + fingerprint + diff
├── uenv-report/      # markdown/json reports
└── uenv-cli/         # CLI (scan/doctor/report/fingerprint/diff)
adapters/             # adapter sources
schemas/              # environment.schema.json (exported by uenv-core)
```

## Contributing

Submit a Windows bug you've hit, so it never bothers anyone again.
Five contribution entry points (Detector / Rule / Adapter / Capsule / Recipe) — see `CONTRIBUTING.md`.

## Roadmap

- **Phase 1 (current)**: Windows environment doctor — scan, diagnose, fix suggestions (Tauri/Electron/Node)
- **Phase 2**: Incident reproduction network — Capsule, Recipe
- **Phase 3**: Protocol independence — shared environment state layer for multi-agent systems

## License

Apache-2.0 (code) · CC BY 4.0 (docs, see `docs/LICENSE-DOCS`)

[中文](README.md)
