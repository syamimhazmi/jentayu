---
name: jentayu
description: >
  Work on Jentayu, the Rust process/Docker container resource monitor (Nusantara
  mythical-bird codename). Use when the user mentions jentayu, container resource
  monitor, process metrics TUI, Docker stats CLI, Prometheus export for containers,
  or runs /jentayu. Covers building, testing, extending collectors, TUI, and exporters.
---

# Jentayu skill

You are working inside the **jentayu** project — a Rust CLI/TUI that monitors CPU,
memory, disk, and network for host processes and Docker containers.

## Always read first

1. `AGENT.md` — stack, layout, rules, **git branches** (`carbon` = staging, `master` = prod)
2. `QUALITY.md` — **binding** extreme constraints (DoD, unit/Gherkin/QA/metrics/mutation/coverage)
3. `PLAN.md` — milestones and non-goals
4. `../CONTAINER_RESOURCE_MONITOR_MVP.md` — original product brief

## Branches

- **Base PRs on `carbon`** (staging). Promote to **`master`** (prod) only when shipping.
- Do not push straight to protected `carbon`/`master`; topic branch → PR → `carbon`.

## Codename

**Jentayu**: mythical Nusantara bird; metaphor = high vantage resource observer.

## Commands you should know

```bash
# from jentayu/
cargo build
cargo test
cargo run -- once -n 20
cargo run -- watch -i 1 -s memory
cargo run -- tui --containers-only
cargo run -- prometheus
cargo run -- json -o /tmp/snap.json
cargo run -- -f postgres --alert-cpu 80
```

## Architecture map

| Need                           | Module                         |
| ------------------------------ | ------------------------------ |
| CLI flags                      | `src/cli.rs`                   |
| Snapshot types / sort / filter | `src/metrics.rs`               |
| Orchestration                  | `src/collector/mod.rs`         |
| Processes                      | `src/collector/process.rs`     |
| Process network (Linux)        | `src/collector/process_net.rs` |
| Docker                         | `src/collector/container.rs`   |
| Host                           | `src/collector/host.rs`        |
| Table UI                       | `src/display/table.rs`         |
| TUI                            | `src/display/tui.rs`           |
| Prometheus / JSON              | `src/export/`                  |
| Thresholds                     | `src/alert.rs`                 |

## How to extend (patterns)

### Add a new metric field

1. Add field to `ResourceSample` or `HostSample` in `metrics.rs`
2. Populate in the relevant collector
3. Surface in `display/table.rs` and `display/tui.rs`
4. Export in `export/prometheus.rs` with `jentayu_` prefix
5. Unit-test pure formatting / ranking if logic is non-trivial
6. Update Gherkin under `tests/features/` if user-visible; keep README mapping current

### Add a CLI subcommand

1. Extend `Commands` in `cli.rs`
2. Handle arm in `main.rs`
3. Prefer library functions in `lib` modules over fat `main`
4. Add/adjust `tests/features/cli_modes.feature` and executable tests where practical

### Docker unavailable

Never hard-fail the whole app. `ContainerCollector::disabled()` is the pattern.
Keep `docker_optional.feature` honest when changing degrade behavior.

## Extreme quality bar (mandatory)

Full rules: **`QUALITY.md`**. You may not claim done without its Definition of Done.

### Hard stops

- Pure logic change without unit tests → **STOP**
- User-visible change without Gherkin update → **STOP**
- Clippy warnings (`-D warnings`) → **STOP**
- Public `metrics` change without export + tests → **STOP**

### Six constraints

| Constraint           | Requirement                                                                            |
| -------------------- | -------------------------------------------------------------------------------------- |
| **Unit tests**       | Happy / edge / error paths for pure functions; same-change as public APIs              |
| **Gherkin**          | Scenarios in `tests/features/`; observable behavior; map in README                     |
| **QA procedures**    | fmt check → clippy `-D warnings` → test → release build → smoke → Docker-optional      |
| **Quality metrics**  | No silent regression on clippy, green tests, coverage of pure APIs, export correctness |
| **Mutation testing** | Asserts that kill flipped operators/bounds/labels; run mutants tooling if available    |
| **Test coverage**    | High on `metrics` / `alert` / `export`; document I/O gaps; no coverage-gaming          |

### Commands before finishing

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
# when pure modules (metrics/alert/export/container CPU) change:
./scripts/coverage.sh
./scripts/mutants.sh          # or: ./scripts/mutants.sh in-diff
```

Manual smoke (when user-facing):

```bash
cargo run -- once -n 5
cargo run -- prometheus | head
```

### Quality report (paste at end of task)

```text
## Quality report
- fmt / clippy / test / release: pass|fail|skipped
- Gherkin: updated | N/A | missing
- llvm-cov: pass|fail|skipped
- cargo mutants: pass|fail|skipped
- Residual risks: …
```

## Style

- Idiomatic Rust: small types, explicit errors at boundaries, `tracing` for diagnostics
- No `unwrap()` in library paths except tests
- Keep MVP scope; park large features in `PLAN.md` M5 rather than half-implementing
- Prefer shrinking scope over shipping under the quality bar
- **SOLID + Clean Code** when generating code — binding detail in `AGENT.md` (single responsibility modules, pure core / I/O edges, clear names, small functions, soft-fail optional sources)

## Do not

- Rewrite in another language
- Require Docker for process monitoring
- Add unrelated monorepo tools into this crate
- Commit secrets or live host dumps with sensitive env data
- Ship untested pure logic or stale Gherkin for behavior changes
- Claim “done” without the QA checklist (or written skips)
