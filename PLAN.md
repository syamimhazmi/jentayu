# PLAN.md — Jentayu implementation plan

## Goal

Ship a usable MVP terminal monitor for process and Docker container resources, aligned with `CONTAINER_RESOURCE_MONITOR_MVP.md`.

**Agent quality bar:** all implementation work must satisfy [`QUALITY.md`](./QUALITY.md) (unit tests, Gherkin, QA, metrics, mutation-minded tests, coverage). Shrink scope rather than ship under the bar.

## Why this project

| Benefit   | Detail                                                                      |
| --------- | --------------------------------------------------------------------------- |
| Learning  | Ownership, borrowing, concurrent collection (Tokio), parsing Docker/sysinfo |
| Utility   | Immediately useful while developing hosting / PaaS tooling                  |
| Relevance | Direct line to cloud hosting resource observability                         |

## Success criteria (MVP)

- [x] One-shot table of CPU / memory / disk / network for processes
- [x] Docker container stats when daemon available
- [x] Filter by name (`--filter`)
- [x] Top N (`--top`)
- [x] Sort by cpu | memory | disk | network
- [x] Prometheus text export
- [x] JSON export
- [x] Watch mode + interactive TUI
- [x] Optional CPU/memory threshold alerts
- [ ] Cross-platform CI (Linux primary; macOS smoke)
- [ ] Optional HTTP `/metrics` scrape server
- [ ] SQLite historical sampling (extension)

## Milestones

### M0 — Scaffold (done)

- Crate `jentayu`, module layout, AGENT/PLAN/SKILL/README
- Dependencies: sysinfo, clap, tokio, bollard, ratatui, crossterm

### M1 — Collectors (done)

1. **HostCollector** — global CPU, memory, swap, disks, networks, load average
2. **ProcessCollector** — per-PID CPU%, RSS, disk R/W
3. **ContainerCollector** — bollard list + one-shot stats; CPU% Docker-compatible formula
4. **Collector** orchestrator with `CollectMode::{Processes, Containers, All}`

### M2 — Display & export (done)

1. comfy-table one-shot / watch
2. ratatui TUI (sort cycle, refresh, navigation)
3. Prometheus exposition (`jentayu_*` metrics)
4. JSON snapshot dump

### M3 — CLI polish (done)

```
jentayu                 # once (default)
jentayu watch
jentayu tui
jentayu prometheus [-o file]
jentayu json [-o file]
jentayu -f nginx -n 10 -s memory
jentayu --containers-only
jentayu --alert-cpu 80 --alert-mem 90
```

### M4 — Hardening (done)

| Task                               | Notes                                                                                          |
| ---------------------------------- | ---------------------------------------------------------------------------------------------- |
| Rate-limit container stats fan-out | ✅ `buffer_unordered(8)` + unit tests / Gherkin                                                |
| Delta I/O rates                    | ✅ B/s over interval (`IoRateState`); display + Prometheus gauges                              |
| Process network (Linux)            | ✅ sock-diag TCP_INFO + private-netns `/proc`; 0 on other OS                                   |
| Clippy + fmt in CI                 | ✅ `.github/workflows/ci.yml`                                                                  |
| Integration test with mock Docker  | ✅ wiremock always-on; live socket via `--features docker-integration`                         |
| Enforce QUALITY.md in CI           | ✅ fmt, clippy `-D warnings`, test, release + pure llvm-cov job                                |
| llvm-cov pure-module floor         | ✅ `./scripts/coverage.sh` (CI fail-under 80% lines pure set)                                  |
| Mutation testing (cargo-mutants)   | ✅ `.cargo/mutants.toml` + `./scripts/mutants.sh` (local; too slow for CI hard-fail)           |
| Executable Gherkin / cucumber      | ✅ cucumber-rs: filter/rank, alerts, prometheus; other features `@wip`                         |
| Kill remaining MISSED mutants      | ✅ Stronger pure tests; equivalent format_bps boundary excluded                                |
| Raise pure coverage floor          | ✅ High coverage on pure set (`metrics` / `alert` / prometheus); CI floor remains 80%          |

### M5 — Extensions (post-MVP)

1. **Alerts v2** — webhook / desktop notify; hysteresis to avoid flapping
2. **History** — append samples to SQLite (`rusqlite`); `jentayu history --since 1h`
3. **Scrape server** — `jentayu serve --bind 0.0.0.0:9105` exposing `/metrics`
4. **Web dashboard** — minimal axum + HTMX or static charts
5. **cgroup v2 direct** — for environments without Docker API but with container cgroups

## Architecture

```
                    ┌─────────────┐
                    │  clap CLI   │
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │  Collector  │
                    └──┬───┬───┬──┘
           host ◄──────┘   │   └──────► containers (bollard)
                           │
                    processes (sysinfo)
                           │
                    ┌──────▼──────┐
                    │  Snapshot   │
                    └──┬───┬───┬──┘
           table ◄─────┘   │   └─────► prometheus / json
                           │
                          tui
                           │
                        alerts
```

## Risks & mitigations

| Risk                            | Mitigation                                  |
| ------------------------------- | ------------------------------------------- |
| Docker socket missing           | Graceful disable; warn once                 |
| High PID count freezes TUI      | `--top` default suggestion; refresh budgets |
| macOS process disk stats sparse | Document OS differences; don't fail hard    |
| Bollard API churn               | Pin version; wrap in thin adapter           |

## Definition of done for v0.1.0

1. `cargo test` green on developer machine
2. `cargo run -- once -n 15` shows live data
3. `cargo run -- prometheus` emits valid-looking exposition text
4. `cargo run -- tui` navigable without panic
5. README documents install, CLI, and codename story

## Open questions

- Default collect mode: both (current) vs processes-only for lower overhead?
- Should Prometheus export include only ranked/filtered targets or full set? (currently full set)
- Bundle as single static binary for hosting node agents?

## Timeline (suggestive)

| Week | Focus                                          |
| ---- | ---------------------------------------------- |
| 1    | M0–M3 MVP usable daily                         |
| 2    | M4 hardening + CI                              |
| 3+   | M5 extensions as needed for hosting case study |
