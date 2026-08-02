# AGENT.md — Jentayu

Instructions for AI agents working on this repository.

## Codename

**Jentayu** (also spelled _Jatayu_ in related traditions) is a mythical bird of Nusantara / Malay epic lore — a soaring observer.
The name fits a resource monitor that watches processes and containers from above.

## Mission

Build and maintain a **Rust** terminal tool that shows real-time **CPU, memory, disk, and network** usage for:

1. Host processes (via `sysinfo`)
2. Running Docker containers (via `bollard` / Docker Engine API)

Optional capabilities: name filter, top-N consumers, Prometheus export, threshold alerts, interactive TUI.

Source of product intent: `../CONTAINER_RESOURCE_MONITOR_MVP.md`.

## Branches (git)

| Branch     | Role                                                                 |
| ---------- | -------------------------------------------------------------------- |
| **carbon** | Staging — default integration line. Open feature PRs **into `carbon`**. |
| **master** | Production — promote from `carbon` only when ready to ship.          |

- Do **not** open feature/fix PRs against `master` unless the user explicitly asks for a prod hotfix.
- Do **not** push directly to `carbon` or `master` when branch protection requires a PR (use a topic branch → PR → `carbon`).
- Release tags (e.g. `v0.1.0`) should land on the commit intended for prod; promote `carbon` → `master` before or with the public release as the user directs.
- Topic branch names stay free-form (e.g. `m5-a/serve-metrics`); base them on current `carbon`.

## Stack (do not casually replace)

| Layer                | Choice              |
| -------------------- | ------------------- |
| Language             | Rust (edition 2021) |
| Async runtime        | Tokio               |
| CLI                  | clap (derive)       |
| Host/process metrics | sysinfo             |
| Docker API           | bollard             |
| TUI                  | ratatui + crossterm |
| Tables               | comfy-table         |
| Errors               | thiserror + anyhow  |
| Logging              | tracing             |

## Layout

```
jentayu/
├── AGENT.md          ← you are here
├── QUALITY.md        ← extreme constraints (binding Definition of Done)
├── PLAN.md           ← roadmap & milestones
├── SECURITY.md       ← privilege, export trust boundary, -o, planned serve
├── SKILL.md          ← Grok/Claude skill for this project
├── README.md
├── Cargo.toml
├── .cargo/mutants.toml
├── scripts/
│   ├── coverage.sh   # cargo-llvm-cov (pure / full / html / lcov)
│   └── mutants.sh    # cargo-mutants
├── .github/workflows/ci.yml
├── tests/
│   ├── filter_rank.rs
│   ├── features_runner.rs  # cucumber-rs (filter/rank, alerts, prometheus)
│   └── features/     # Gherkin scenarios + scenario→test map
└── src/
    ├── main.rs       # CLI entry
    ├── lib.rs        # public crate root
    ├── cli.rs
    ├── metrics.rs    # ResourceSample, Snapshot, filter_and_rank, snapshot_for_export
    ├── alert.rs
    ├── error.rs
    ├── util.rs       # sanitize_control_chars (TTY free-text)
    ├── collector/
    │   ├── mod.rs    # CollectMode + Collector orchestrator
    │   ├── host.rs
    │   ├── process.rs
    │   ├── process_net.rs  # Linux per-process net (sock-diag /proc); 0 elsewhere
    │   └── container.rs
    ├── display/
    │   ├── table.rs
    │   └── tui.rs
    └── export/
        └── prometheus.rs
```

## Working rules

1. **Rust only** for application code. No Node/Python rewrites of the core.
2. Prefer small, testable pure functions (`filter_and_rank`, `render_prometheus`, `evaluate` alerts).
3. Docker must be **optional** — if the daemon is down, process/host monitoring still works.
4. Never block the async runtime with long `std::thread::sleep` inside request handlers; collector warm-up sleeps on first construction are acceptable.
5. Keep CLI UX stable: `once` (default), `watch`, `tui`, `prometheus`, `json`.
6. Metric names for Prometheus are prefixed with `jentayu_`.
7. When changing public types in `metrics.rs`, update Prometheus export, unit tests, and Gherkin (if behavior-facing) together.
8. **Quality is not optional.** Read and obey `QUALITY.md` on every change. Prefer shrinking scope over shipping untested code.
9. Pure-logic changes: run `./scripts/coverage.sh` and `./scripts/mutants.sh` (or `in-diff`) before claiming done.
10. **SOLID + Clean Code are binding** when generating or changing code (see below). Do not ship god-modules, mixed concerns, or unclear names.

## SOLID principles (binding when generating code)

Apply SOLID in a **Rust-idiomatic** way (modules, traits, pure functions — not forced class hierarchies).

| Letter | Principle             | How to apply here                                                                                                                                                                                                                      |
| ------ | --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **S**  | Single Responsibility | One module / type / function does one job. Collectors collect; `metrics` ranks/filters; `display` renders; `export` formats. Do not put Docker I/O inside Prometheus rendering or CLI parsing inside pure math.                        |
| **O**  | Open/Closed           | Extend via new modules, enums, or traits rather than rewriting stable cores. Prefer adding a collector path or export format over bloating `main.rs`. Keep pure helpers stable and open to new callers.                                |
| **L**  | Liskov Substitution   | Trait impls and mode variants must honor the same contract (e.g. disabled Docker collector still samples safely; unsupported OS process-net returns empty, not panic). Callers must not need special-case knowledge of “broken” impls. |
| **I**  | Interface Segregation | Prefer small traits and focused function APIs over kitchen-sink surfaces. Do not force callers to depend on Docker, TUI, or netlink types they do not use.                                                                             |
| **D**  | Dependency Inversion  | Depend on pure data (`ResourceSample`, `Snapshot`) and narrow interfaces, not on concrete OS/Docker details in upper layers. Inject or gate I/O at collector boundaries; keep ranking, alerts, and export free of syscalls.            |

### SOLID hard stops

```
New logic that mixes collect + display + export in one function → STOP, split
Pure ranking/alert/export calling Docker/sysinfo/netlink directly → STOP, invert
“Disabled” / unsupported path panics or returns a different error shape → STOP
Public API that requires unused dependencies to call → STOP, narrow the surface
```

## Clean code (binding when generating code)

Write code that is **easy to read, test, and change**. Prefer clarity over cleverness.

### Naming

- Names reveal intent: `filter_and_rank`, `sample_process_net`, `evaluate` — not `do_stuff`, `tmp2`, `data`.
- Booleans and flags read as predicates: `is_available`, `process_net_supported`.
- Avoid abbreviations unless project-standard (`rx`/`tx`, `bps`, `pid` are fine).

### Functions & modules

- Small functions; one level of abstraction per function when practical.
- Pure logic (no I/O) at the core; side effects at the edges (`collector`, `main`, display loop).
- Early returns over deep nesting; limit `else` chains.
- No dead code, commented-out blocks, or speculative “just in case” APIs.

### Structure

- Keep the layout in **Layout** honest: put new code in the module that already owns that concern.
- DRY only when duplication is real and stable — do not abstract one-use helpers into clever generics.
- Errors: typed at boundaries (`thiserror` / `JentayuError`); soft-fail optional sources (Docker, process net) with `tracing` + empty/zero, not silent `unwrap`.
- Logging: `tracing` with meaningful fields; no `println!` in library paths.

### Comments

- Comment **why**, not what the code already says.
- Public items get short rustdoc when behavior is non-obvious (OS limits, formulas, metric names).
- Do not leave TODOs without a PLAN.md / issue hook for non-trivial work.

### Clean-code hard stops

```
unwrap()/expect() in non-test library paths without proven invariant → STOP
Copy-paste of non-trivial logic across modules without shared helper → STOP
Function that both fetches remote/OS state and formats user output → STOP
Names that lie (e.g. get_x that mutates or performs I/O) → STOP
```

## Extreme quality constraints (agents)

**Canonical detail:** [`QUALITY.md`](./QUALITY.md). This section is the short mandatory summary.

Agents operate under **extreme constraints**. A feature, refactor, or bugfix is **not done** until the Definition of Done in `QUALITY.md` is met. End every task with the **Quality report** template from that file.

### Hard stops

```
NO tests for pure logic change              → STOP
User-visible change, no Gherkin update      → STOP
Clippy warnings under -D warnings           → STOP
Claimed done without QA checklist           → STOP
metrics.rs public change without export/tests → STOP
```

### 1. Unit tests

- Every pure function and non-trivial branch: happy path, edge cases, error paths.
- Prefer library surface (`lib` modules) over binary-only paths.
- New public APIs ship with tests **in the same change**.
- Assert real outcomes (values, order, labels, thresholds) — not only “does not panic”.

### 2. Gherkin / BDD tests

- User-visible behavior lives in `tests/features/*.feature`.
- Keep `tests/features/README.md` scenario→test mapping current.
- Scenario language = **observable behavior**, not internal types.
- Changing CLI/filter/alert/export behavior without updating scenarios is a DoD failure.

### 3. QA procedures

Before marking complete, run (or justify skip in writing):

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test`
4. `cargo build --release`
5. Manual smoke of changed CLI paths (`once`, `watch`, `tui`, `prometheus`, `json` as applicable)
6. Confirm Docker-optional: process/host still works if daemon is down
7. Record residual risks for any skipped step

### 4. Quality metrics

Do not regress without explanation:

| Metric                   | Expectation                                            |
| ------------------------ | ------------------------------------------------------ |
| Clippy                   | Zero warnings under `-D warnings`                      |
| Unit/integration tests   | Green on every claimed-done change                     |
| Public API coverage      | Critical pure logic and exporters covered              |
| Behavior docs            | Gherkin scenarios for user-facing flows                |
| Alert/export correctness | Metric names, thresholds, and formats match docs/tests |
| Mutation resistance      | Critical tests would fail if operators/labels flip     |

### 5. Mutation testing

- Write tests that mutation testing would catch (operators, bounds, sort direction, `jentayu_` prefix).
- Config: `.cargo/mutants.toml`. Runner: `./scripts/mutants.sh` (`list` / `in-diff` / per-module).
- On pure-module changes: run mutants; fix high-value **MISSED** survivors. CI enforces the pure set.
- Targets: ranking, alerts, Prometheus render, container CPU % math.

### 6. Test coverage

- No large untested gaps in `metrics`, `alert`, `export`, pure filter/rank logic.
- Runner: `./scripts/coverage.sh` (pure floor **80% lines**; `full` / `html` / `lcov` modes).
- High coverage on pure logic; lower OK only for OS/Docker I/O — document why.
- Prefer meaningful assertions over coverage-gaming tests.
- Public `metrics.rs` changes ⇒ export + tests + scenarios (if behavior-facing) in one change.

## Non-goals (MVP)

- Multi-host / Kubernetes cluster aggregation
- Auth / multi-tenant SaaS control plane
- Full historical TSDB (SQLite history is a post-MVP extension)
- Web dashboard (post-MVP)

## Extension hooks (planned)

Documented in `PLAN.md`:

- Threshold alerts (partially shipped)
- SQLite historical sampling
- Lightweight HTTP `/metrics` scrape endpoint
- Simple web dashboard

## Safety

- Read-only against Docker (list + stats). Do not start/stop/remove containers from this tool unless an explicit future subcommand is designed for that.
- Do not require root; degrade gracefully when `/proc` or Docker socket permissions are limited.

## Commit style

- Imperative mood: `Add container CPU percent calculation`
- Reference the milestone when relevant: `M2: export Prometheus text format`
- Default PR base: **`carbon`** (staging), not `master` (prod) — see **Branches** above
