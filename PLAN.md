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

## Versioning (locked)

| Version                 | Meaning                                                                                                                                 |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| **v0.1.0**              | Current MVP (M0–M4): local CLI, Docker-optional, table/TUI/export/alerts. Tag when release hygiene is ready; **does not require M4.5**. |
| **v0.1.x / v0.2.0-pre** | **M4.5** security & trust-boundary patch — **required before any network-facing M5 work**.                                              |
| **v0.2.0+**             | M5 extensions (`serve`, history, alerts v2, …) after M4.5 gate.                                                                         |

Crates.io / git tags: optional until first public release; prefer semver above.

## Success criteria

### MVP feature set (M0–M4) — done

- [x] One-shot table of CPU / memory / disk / network for processes
- [x] Docker container stats when daemon available
- [x] Filter by name (`--filter`)
- [x] Top N (`--top`)
- [x] Sort by cpu | memory | disk | network
- [x] Prometheus text export
- [x] JSON export
- [x] Watch mode + interactive TUI
- [x] Optional CPU/memory threshold alerts

### Pre-serve security gate (M4.5) — required before M5 kick-off

- [x] Display control-char sanitization
- [x] Export trust boundary (filter/top policy locked below)
- [x] `-o` atomic write + docs
- [x] `SECURITY.md` + README link
- [x] TUI Drop guard (terminal restore)

### Later (not blocking M4.5)

- [ ] Cross-platform CI (Linux primary; macOS smoke — definition under M4.5 Should)
- [x] Optional HTTP `/metrics` scrape server (M5-a; localhost default, caps, dual-gate auth)
- [ ] SQLite historical sampling (M5)

## Privilege model

- Run as a **normal unprivileged user**. Never require setuid/setgid.
- Optional **Docker group** (or root) only if container metrics are needed; socket access is root-equivalent on many hosts — treat as privileged observability.
- Without Docker or with limited `/proc`, **degrade gracefully** (empty containers / zero process-net); do not fail the whole process sample.
- Metrics confidentiality ≈ `ps` + `docker stats` output: process names, PIDs, container names are **sensitive inventory**, not public telemetry by default.

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

| Task                               | Notes                                                                                 |
| ---------------------------------- | ------------------------------------------------------------------------------------- |
| Rate-limit container stats fan-out | ✅ `buffer_unordered(8)` + unit tests / Gherkin                                       |
| Delta I/O rates                    | ✅ B/s over interval (`IoRateState`); display + Prometheus gauges                     |
| Process network (Linux)            | ✅ sock-diag TCP_INFO + private-netns `/proc`; 0 on other OS                          |
| Clippy + fmt in CI                 | ✅ `.github/workflows/ci.yml`                                                         |
| Integration test with mock Docker  | ✅ wiremock always-on; live socket via `--features docker-integration`                |
| Enforce QUALITY.md in CI           | ✅ fmt, clippy `-D warnings`, test, release + pure llvm-cov job                       |
| llvm-cov pure-module floor         | ✅ `./scripts/coverage.sh` (CI fail-under 80% lines pure set)                         |
| Mutation testing (cargo-mutants)   | ✅ `.cargo/mutants.toml` + `./scripts/mutants.sh` (local; too slow for CI hard-fail)  |
| Executable Gherkin / cucumber      | ✅ cucumber-rs: filter/rank, alerts, prometheus; other features `@wip`                |
| Kill remaining MISSED mutants      | ✅ Stronger pure tests; equivalent format_bps boundary excluded                       |
| Raise pure coverage floor          | ✅ High coverage on pure set (`metrics` / `alert` / prometheus); CI floor remains 80% |

### M4.5 — Pre-M5 security & trust-boundary hardening (done — gate open for M5)

> **Gate:** Do not start network-facing M5 work (`serve`, webhooks, dashboard) until the **Must ship** items below are done (or each skip has a written residual-risk note using the template at the end of this section).
>
> Source: full-repo security review (local CLI threat model; Docker + export + planned scrape surface). No critical remote RCE in current surface; issues are privilege, disclosure, self-DoS, and future bind/auth footguns.

#### Threat model (current vs post-M5)

| Phase                                    | Attack surface                                          | Primary risks                                                                                                |
| ---------------------------------------- | ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| **Today (CLI)**                          | Local operator; Docker socket; `-o` paths; stdout/table | Privileged observability, process inventory disclosure, overwrite via `-o`, terminal garble, high-PID freeze |
| **After `serve` / webhooks / dashboard** | Network listeners, outbound HTTP, browser               | Open scrape of host inventory, SSRF, unauthenticated bind on all interfaces, metrics treated as public       |

Treat M5 network features as a **security milestone**, not only a product milestone.

#### What is already solid (do not regress)

| Area                       | Notes                                                      |
| -------------------------- | ---------------------------------------------------------- |
| Docker optional            | Soft-disable; process/host path still works if daemon down |
| Stats concurrency          | `MAX_CONCURRENT_STATS = 8` + tests                         |
| Prometheus labels          | `escape_label` for `\`, `"`, `\n` + unit tests             |
| No shell / `Command`       | No classic injection surface in collectors                 |
| No open non-loopback scrape | M5-a ships loopback-only by default; dual-gate for remote |
| Saturating counters        | Overflow does not panic                                    |
| Filter is substring        | No regex → no ReDoS                                        |
| Export process _name_ only | Do not add full argv / environ without an explicit design  |

#### Findings → owner milestone

| ID  | Severity        | Topic                         | Detail                                                                      | Owner                                                                |
| --- | --------------- | ----------------------------- | --------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| S1  | High (ops)      | Docker socket privilege       | Socket access ≈ root-equivalent; tool is read-only (list + stats)           | Docs (`SECURITY.md`); keep read-only forever unless dedicated design |
| S2  | Medium          | Full process inventory export | `prometheus` / `json` export full snapshot; table/TUI use `filter_and_rank` | **M4.5-a** (locked policy below)                                     |
| S3  | Low–Med         | `-o` file write               | `std::fs::write` overwrites; follows symlinks                               | **M4.5-a** (atomic write + docs)                                     |
| S4  | Low             | Terminal / ANSI injection     | Names/status printed without stripping control chars                        | **M4.5-a** (sanitize)                                                |
| S5  | Low             | Linux process_net unsafe      | Hand-rolled netlink; no recv timeout; fragile ABI                           | **Deferred** (next process-net touch)                                |
| S6  | Low–Med         | Collection self-DoS           | Full PID refresh; huge export on dense hosts                                | **Should** (default top); **M5-serve** hard cap                      |
| S7  | Low             | TUI terminal restore          | Panic may leave raw mode / alt screen                                       | **M4.5-b** ✅ shipped                                                |
| S8  | High (if wrong) | Future `serve` footgun        | Open bind without auth                                                      | **M5-serve** security requirements                                   |
| S9  | Medium          | Supply chain                  | No deny/audit in CI; `tokio` `full`                                         | **M4.5-c** ✅ shipped (deny CI + narrow tokio; before serve deps)    |

#### M4.5 decision (locked): export ranking policy

Applies to `prometheus` and `json` only (table/TUI already ranked).

1. **When `-f` / `--filter` and/or `-n` / `--top` are set:** apply the same `filter_and_rank` path as table (also honor `-s` / `--sort` for order before top-N).
2. **When neither filter nor top is set:** export the **full per-target inventory** (current behavior). Document as **sensitive** in `SECURITY.md` / README.
3. **Do not add `--all-targets` in M4.5** — only needed if a later milestone default-caps export cardinality.
4. **Host-level metrics always included** (CPU, memory, load, host net rates, etc.); ranking applies only to per-target series/rows.
5. **Alerts** continue to evaluate the **full collected snapshot** (ops signal), not only the visible/export subset. Document this; do not add `--alert-on-visible-only` in M4.5.
6. **Done when:** CLI help + README + Gherkin describe the policy; unit tests assert filter/top change export body when set, and full dump when unset.

#### Sanitize rule (locked)

For any free-text printed to a TTY (process/container **name**, **status**, and alert messages that embed them):

- Replace every character in **U+0000–U+001F** and **U+007F** with `?`.
- Do not collapse to empty solely because of controls (replacement preserves length intent).
- Leave other valid UTF-8 printable characters as-is (no NFC requirement in M4.5).
- **Orthogonal:** Prometheus labels keep using `escape_label` (`\`, `"`, `\n`); sanitization is for human display paths, not a substitute for label escaping.
- **Unit tests:** BEL and ANSI CSI sequences (e.g. name containing `\x1b[31m`) must not appear raw in table/alert output strings.

#### `-o` write policy (locked for M4.5-a)

- **Mandatory:** write via temp file in the **same directory** as the destination, then `rename` (atomic on the same filesystem).
- **Document:** destination path must be trusted; running as root with untrusted `-o` is **unsupported**.
- **Optional later (not M4.5 must):** refuse overwrite when the path is an existing symlink (`symlink_metadata`).

#### M4.5 non-goals

- No listen socket / `serve`
- No SQLite history
- No webhooks / desktop notify
- No packaging / static binary
- No default `--top` product change (that is **Should**, not Must)
- No process-net rewrite

#### M4.5 touch points (guidance)

| Item                  | Primary                                                                                     | Also                                                       |
| --------------------- | ------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| Sanitize names/status | Shared pure helper (prefer `display` or small `util` used by table + `alert::print_alerts`) | TUI cell text if any raw string path; tests next to helper |
| Export filter/top     | `main.rs` (build ranked target list / snapshot view before export)                          | Keep `export/*` pure on data; no Docker I/O in export      |
| `-o` atomic write     | `export/mod.rs` (`write_json` / `write_prometheus`)                                         | README + SECURITY                                          |
| TUI Drop guard        | `display/tui.rs`                                                                            | QA note: panic/restore                                     |
| Security docs         | **`SECURITY.md`** + README link “Security notes”                                            | Cross-link AGENT safety (Docker read-only)                 |
| README hygiene        | Fix false claim if any that CI hard-fails mutants (mutants are **local-only**)              | Keep Development section accurate                          |

#### M4.5 test expectations (QUALITY-aligned)

| Kind    | Expectation                                                                                                                                                                           |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Unit    | `sanitize_*` edges (empty, only controls, mixed ANSI); export body includes/excludes targets under `-f`/`-n` per locked policy; atomic write creates expected file content (temp dir) |
| Gherkin | Extend `prometheus_export.feature` for filter/top on export; add JSON scenario or feature if user-visible; optional table sanitize scenario                                           |
| Map     | Update `tests/features/README.md` scenario → test mapping                                                                                                                             |
| Smoke   | `cargo run -- prometheus -n 3 -f …`; `cargo run -- json -n 3`; once/table still works; TUI quit restores terminal                                                                     |
| Pure QA | If helpers land in pure modules: `./scripts/coverage.sh` + `./scripts/mutants.sh in-diff` (or module run) before claiming done                                                        |

#### Must ship before M5 kick-off — **done (2026-08-01)**

1. [x] **Display sanitization** — rule above on table, watch, and alert print paths.
2. [x] **Export trust boundary** — locked policy (filter/top when set; full dump when unset + docs).
3. [x] **`-o` atomic write** — mandatory temp + rename; docs on trust/overwrite.
4. [x] **`SECURITY.md` + README link** — required bullets:
   - Docker socket ≈ root; read-only list/stats only; no lifecycle control
   - Process names / PIDs / container names are confidential inventory
   - Full export when no `-f`/`-n` is sensitive
   - `-o` overwrite / trusted path only
   - Planned `serve` default bind `127.0.0.1` (not all interfaces)
5. [x] **TUI Drop guard** — always restore terminal (raw mode + leave alternate screen) on panic/early exit (`TerminalRestoreGuard` + `try_init`).
6. [x] **M5 security requirements** in this plan remain accurate for implementers (below).

#### Should ship soon (can overlap early M5 if residual risk written)

| Item                                    | Notes                                                                                                                                                                                    |
| --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Default `--top` in watch/TUI when unset | e.g. 25–50; once/export unlimited unless `-n`/`-f` (export policy already locked)                                                                                                        |
| Collection budgets                      | Skip process-net when sort does not need network; document cost                                                                                                                          |
| CI supply chain (**M4.5-c**)            | ✅ `deny.toml` + CI job: advisories fail CI; licenses MIT/Apache/BSD-ish (+ tree outliers); crates.io-only; Dependabot weekly Cargo/Actions                                              |
| Narrow Tokio features                   | ✅ Drop `full`; jentayu uses `macros`, `rt-multi-thread`, `time` (bollard unifies net/io-util/sync/fs)                                                                                   |
| Cross-platform CI                       | **macOS smoke definition:** `macos-latest` job with `cargo test --lib` + `cargo build --release`; skip docker-live; document process-net zeros on non-Linux. Not required for M4.5 Must. |
| Wire remaining Gherkin `@wip`           | cli_modes, docker-optional per QUALITY.md                                                                                                                                                |

#### Explicitly deferred

| Item                                 | When / trigger                                                                           |
| ------------------------------------ | ---------------------------------------------------------------------------------------- |
| Replace hand-rolled netlink          | Next dedicated process-net change; if keeping custom: `SO_RCVTIMEO` + max-message budget |
| Symlink refuse on `-o`               | Optional hardening after atomic write                                                    |
| Static single-binary packaging       | Packaging milestone                                                                      |
| `cargo-mutants` CI hard-fail         | Remains local (too slow)                                                                 |
| `--all-targets` / default export cap | Only if serve or product chooses default cardinality cap                                 |

#### Suggested PR slices (M4.5)

| PR             | Focus                                                                        | Gate role                                                  |
| -------------- | ---------------------------------------------------------------------------- | ---------------------------------------------------------- |
| **M4.5-a**     | Sanitize + export filter/top when set + atomic `-o` + `SECURITY.md` / README | **Must**                                                   |
| **M4.5-b**     | TUI Drop guard only                                                          | **Must** (do not bundle optional default top here)         |
| **M4.5-b-opt** | Default `--top` in watch/TUI                                                 | Should                                                     |
| **M4.5-c**     | Supply-chain CI + shrink tokio features                                      | ✅ Should shipped; **before** adding axum/hyper serve deps |

#### Residual-risk template (if deferring a Must item)

```
Finding ID:
Deferred because:
Residual risk:
Trigger to revisit:
Owner milestone:
Signed off in PR:
```

### M5 — Extensions (post-MVP; after M4.5 gate)

**Kick-off rule:** M4.5 **Must ship** checklist complete (or residual risks written in the PR that opens M5). **Status: gate open** (Must-ship done 2026-08-01; Should items optional).

Recommended product order (security-aware):

1. **Scrape server** — `jentayu serve` exposing `/metrics` (highest value for hosting/PaaS story)
2. **Alerts v2** — hysteresis first; then webhook / desktop notify with SSRF controls
3. **History** — append samples to SQLite (`rusqlite`); `jentayu history --since 1h`
4. **Web dashboard** — minimal axum + HTMX or static charts (only after authenticated scrape story is clear)
5. **cgroup v2 direct** — environments without Docker API but with container cgroups

#### M5 security requirements (binding)

| Feature                        | Requirements before merge                                                                                                                                                                                                                                           |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`serve`**                    | See **serve defaults** below. Default bind loopback only. Metrics confidential. Cardinality cap. Timeouts. Auth or refuse before non-localhost recommend. Integration tests.                                                                                        |
| **Webhooks (alerts v2)**       | Default **deny** private/link-local/cloud-metadata targets (incl. `169.254.169.254`, `fe80::/10`, localhost); prefer `https:` to allowlisted hosts; connect/read timeouts; optional dev-only `--webhook-allow-http-localhost`. Hysteresis to avoid flapping/storms. |
| **SQLite history**             | Default path under XDG / user data dir; file not world-writable; no untrusted SQL from CLI; safe migrations.                                                                                                                                                        |
| **Web dashboard**              | Auth story; CSP basics; no secrets in metrics; prefer same-origin to localhost scrape.                                                                                                                                                                              |
| **Docker TCP / `DOCKER_HOST`** | If CLI-exposed: document plaintext risk; prefer TLS non-local; default remains local socket; `connect_http` OK for tests.                                                                                                                                           |
| **Container lifecycle**        | Non-goal unless dedicated audited subcommand — no start/stop/remove/exec from serve or dashboard.                                                                                                                                                                   |

#### serve defaults (**locked M5-a**)

| Setting                   | Locked value                                                                                                                                   |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Bind                      | `127.0.0.1:9105`; `--bind` is **IP literal** `SocketAddr` only (no DNS)                                                                        |
| Global flags              | Reuse `-f`, `-n`, `-s`, collect mode, `-v`. **Ignore `-i`**. Alert flags no-op on serve                                                         |
| Default top if `-n` unset | **50** (serve only; independent of CLI export full-dump policy)                                                                                |
| Hard cap                  | **500** targets per scrape                                                                                                                     |
| Handler timeout           | Cooperative **2s** (test-injectable); **503** plain text, no series; sync process sample may overrun                                           |
| Non-localhost bind        | **Refuse** unless **both** `--allow-remote` **and** Bearer auth (`JENTAYU_METRICS_TOKEN` or `--token-file`)                                    |
| Paths                     | `GET /metrics` only; `/healths` deferred                                                                                                       |
| Series budget (guidance)  | Host gauges + ~fixed metric names × N targets; label keys stay `kind`, `id`, `name`                                                            |
| Tests                     | Loopback `/metrics` + `jentayu_`; auth matrix; injectable timeout 503; bind dual-gate unit/Gherkin                                             |

#### M5 sketch (non-normative CLI)

```
# After M4.5 — scrape locally
jentayu serve                         # 127.0.0.1:9105 /metrics
jentayu serve --bind 127.0.0.1:9105
jentayu serve -n 50 -f nginx

jentayu history --since 1h
# alerts v2 hysteresis (flag names TBD)
```

**Do not** document or example `0.0.0.0` as a default bind.

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
                           │
              (M5) serve / history / webhooks
```

## Risks & mitigations

| Risk                                      | Mitigation                                                                     |
| ----------------------------------------- | ------------------------------------------------------------------------------ |
| Docker socket missing                     | Graceful disable; warn once                                                    |
| Docker socket = privileged observability  | `SECURITY.md`; read-only API only; no lifecycle control casually               |
| Full process inventory on export / scrape | M4.5: filter/top when set + docs; M5 serve: default top + hard cap + localhost |
| High PID count freezes TUI                | Should: default `--top` in watch/TUI; collection budgets                       |
| Hostile process/container names (ANSI)    | M4.5 sanitize rule on display/alert paths                                      |
| `-o` overwrites sensitive files           | Atomic write; trusted path docs                                                |
| `serve` on all interfaces without auth    | Forbidden default; refuse non-localhost without auth                           |
| Webhook SSRF                              | Default deny private/metadata; timeouts; allowlist                             |
| macOS process disk stats sparse           | Document OS differences; don't fail hard                                       |
| Bollard API churn                         | Pin version; wrap in thin adapter                                              |
| Supply-chain / dep advisories             | M4.5-c deny/audit; minimal tokio features                                      |
| Hand-rolled netlink fragility             | Soft-fail; deferred crate / recv timeout                                       |
| TUI panic leaves broken terminal          | M4.5-b Drop guard ✅ shipped                                                   |

## Definition of done for v0.1.0 (MVP tag)

1. `cargo test` green on developer machine
2. `cargo run -- once -n 15` shows live data
3. `cargo run -- prometheus` emits valid-looking exposition text
4. `cargo run -- tui` navigable without panic
5. README documents install, CLI, and codename story

## Definition of done for M4.5 (pre-M5 gate) — **met 2026-08-01**

1. [x] Sanitize rule implemented + unit tests (table/watch/alert paths)
2. [x] Export policy implemented (filter/top when set; full when unset) + unit tests + Gherkin (`prometheus_export` / json) + `tests/features/README.md` map
3. [x] Atomic `-o` write + docs (trusted path / overwrite)
4. [x] `SECURITY.md` with required bullets + README link; README CI/mutants claims accurate
5. [x] TUI always restores terminal on exit/panic (`TerminalRestoreGuard` + `try_init`)
6. [x] No deferred Must items (S5 netlink / S6 budgets / S9 supply-chain remain Should or deferred)
7. [x] QUALITY.md checklist for the change set (fmt, clippy `-D warnings`, test; pure helpers covered under M4.5-a)

## Open questions

| Question                                                   | Status                                            | Resolves by                             |
| ---------------------------------------------------------- | ------------------------------------------------- | --------------------------------------- |
| Default collect mode: both vs processes-only?              | Lean **both**; flags already allow focus          | README only unless overhead forces flip |
| Prometheus/JSON ranked vs full?                            | **Locked** in M4.5 decision                       | M4.5-a                                  |
| Default `--top` in watch/TUI when unset?                   | Lean yes (25–50); not Must                        | M4.5-b-opt / early M5                   |
| Bundle static binary for node agents?                      | Desirable post-serve                              | Packaging milestone                     |
| `serve` auth: bearer vs Unix socket vs reverse-proxy-only? | **Locked M5-a:** Bearer env/file; dual gate for non-loopback | Done (Unix socket deferred) |
| Alert hysteresis default?                                  | TBD (percent band or consecutive breaches)                   | Alerts v2 design             |
| Serve default top 50 / hard cap 500 / 2s timeout?          | **Locked M5-a** (timeout cooperative)                        | Done                         |

## Decision log

| Date       | Decision                                                                                                                                            |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-07-31 | Add **M4.5** security gate from full-repo review; M5 network work blocked until Must-ship done.                                                     |
| 2026-07-31 | Export: apply `-f`/`-n`/`-s` when set; **full inventory when unset**; no `--all-targets` in M4.5; host metrics always on; alerts use full snapshot. |
| 2026-07-31 | Sanitize: replace U+0000–001F and U+007F with `?` on TTY free text.                                                                                 |
| 2026-07-31 | `-o`: mandatory same-dir temp + rename; symlink refuse optional later.                                                                              |
| 2026-07-31 | Docs: ship `SECURITY.md` + README link (not README-only buried note).                                                                               |
| 2026-07-31 | Versioning: v0.1.0 = MVP without M4.5; M4.5 is pre-serve patch; serve is v0.2.0+.                                                                   |
| 2026-07-31 | `serve` default bind **127.0.0.1:9105**; never example `0.0.0.0` as default.                                                                        |
| 2026-08-01 | **M4.5 Must-ship complete** (a + b). M5 kick-off unblocked. Should items (c, b-opt, macOS CI, …) remain optional.                                   |
| 2026-08-01 | **M4.5-c shipped:** cargo-deny CI (advisories hard-fail), Dependabot weekly, tokio features narrowed (no `full`).                                   |
| 2026-08-03 | **M5-a locked/shipped:** bind `127.0.0.1:9105` (SocketAddr IP only); serve top 50 / hard cap 500; cooperative 2s → 503; dual gate `--allow-remote`+Bearer; env/file token; auth before lock; ignore `-i`; no `/healths`; crate `0.2.0`. |

## Timeline

| Phase                  | Focus                                                                                | Status                              |
| ---------------------- | ------------------------------------------------------------------------------------ | ----------------------------------- |
| Weeks 1–2 (historical) | M0–M4 MVP + hardening + CI                                                           | Done                                |
| M4.5                   | Pre-serve security gate (a sanitize/export/`-o`/docs + b TUI Drop guard)             | **Done** (2026-08-01)               |
| M5-a                   | `jentayu serve` scrape (localhost, caps, auth dual-gate)                             | **Done (2026-08-03)**               |
| Next                   | Optional M4.5 Should (b-opt default top, macOS CI); M5 alerts v2 / history           | After M5-a on carbon                |
| After gate             | M5: alerts hysteresis → history → dashboard / cgroup as needed                       | **Unblocked** (Must-ship satisfied) |

## Related docs

| File                           | Role                                                                                               |
| ------------------------------ | -------------------------------------------------------------------------------------------------- |
| [`AGENT.md`](./AGENT.md)       | Agent rules, SOLID, safety (Docker read-only)                                                      |
| [`QUALITY.md`](./QUALITY.md)   | Definition of Done, tests, coverage, mutants — **Gherkin hard-stop applies to M4.5 export change** |
| [`README.md`](./README.md)     | User-facing install & CLI                                                                          |
| [`SECURITY.md`](./SECURITY.md) | **Shipped (M4.5-a)** — privilege, disclosure, `-o`, serve defaults                                 |
| [`SKILL.md`](./SKILL.md)       | Skill entry for AI agents                                                                          |
