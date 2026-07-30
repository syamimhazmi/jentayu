# Jentayu

**Real-time process & Docker container resource monitor** — written in Rust.

> _Jentayu_ is a mythical bird of Nusantara lore, said to soar high above the land.
> This tool takes that vantage: watching CPU, memory, disk, and network across your
> host processes and running containers.

Based on [`CONTAINER_RESOURCE_MONITOR_MVP.md`](../CONTAINER_RESOURCE_MONITOR_MVP.md).

## Features (MVP)

- Live **CPU / memory / disk / network** for processes (`sysinfo`) and containers (`bollard`)
- **Process network (Linux best-effort):** TCP bytes via netlink `SOCK_DIAG` + `/proc` socket inodes; private netns falls back to `/proc/<pid>/net/dev`; other OS keep process net at 0
- **Delta I/O rates** (B/s over the sample interval) for disk and network in watch/TUI; first sample falls back to cumulative counters
- **Filter** by name, **top N**, sort by cpu · memory · disk · network (disk/net prefer rates when known)
- **Table** one-shot & watch modes
- Interactive **TUI** (ratatui)
- **Prometheus** text export (`*_bytes_total` counters + `*_bytes_per_second` gauges) and **JSON** snapshots
- Optional **threshold alerts** (`--alert-cpu`, `--alert-mem`)
- Docker is optional — process monitoring works without the daemon

## Quick start

```bash
cd jentayu
cargo run --release -- once -n 15
cargo run --release -- tui
cargo run --release -- prometheus
cargo run --release -- watch -i 1 -f nginx
```

### Install binary

```bash
cargo install --path .
jentayu --help
```

## CLI

| Command                        | Description                  |
| ------------------------------ | ---------------------------- |
| `jentayu` / `jentayu once`     | One-shot table (default)     |
| `jentayu watch`                | Refreshing table             |
| `jentayu tui`                  | Interactive dashboard        |
| `jentayu prometheus [-o file]` | Prometheus exposition format |
| `jentayu json [-o file]`       | JSON snapshot                |

### Global flags

| Flag                   | Meaning                                |
| ---------------------- | -------------------------------------- |
| `-f, --filter <name>`  | Substring filter (case-insensitive)    |
| `-n, --top <N>`        | Keep top N after sort                  |
| `-s, --sort <metric>`  | `cpu` · `memory` · `disk` · `network`  |
| `-i, --interval <sec>` | Watch/TUI refresh interval (default 2) |
| `--containers-only`    | Skip host processes                    |
| `--no-processes`       | Same as containers-focused host view   |
| `--alert-cpu <pct>`    | Warn when CPU ≥ threshold              |
| `--alert-mem <pct>`    | Warn when memory % ≥ threshold         |
| `-v / -vv`             | Verbose logging                        |

### TUI keys

| Key         | Action            |
| ----------- | ----------------- |
| `q` / `Esc` | Quit              |
| `r`         | Refresh now       |
| `s`         | Cycle sort column |
| `j` / `↓`   | Next row          |
| `k` / `↑`   | Previous row      |

## Project docs

| File                                 | Purpose                                                  |
| ------------------------------------ | -------------------------------------------------------- |
| [AGENT.md](./AGENT.md)               | Instructions for AI agents                               |
| [QUALITY.md](./QUALITY.md)           | Extreme quality constraints (binding Definition of Done) |
| [PLAN.md](./PLAN.md)                 | Roadmap & milestones                                     |
| [SKILL.md](./SKILL.md)               | Grok/Claude skill definition                             |
| [tests/features/](./tests/features/) | Gherkin behavior scenarios                               |

## Architecture

```
CLI → Collector (host + processes + Docker)
         → Snapshot
              → table | tui | prometheus | json
              → alerts
```

Key crates: `sysinfo`, `bollard`, `clap`, `tokio`, `ratatui`, `crossterm`.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Docker collector integration:

- **Always on:** `tests/docker_mock.rs` speaks a wiremock Docker Engine HTTP API (no socket needed).
- **Optional live:** `cargo test --features docker-integration --test docker_live` — skips cleanly if the daemon is down.

### Coverage & mutation testing (M4)

Install once:

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov cargo-mutants --locked
```

```bash
./scripts/coverage.sh        # pure modules (metrics/alert/prometheus), ≥80% lines
./scripts/coverage.sh html   # open target/llvm-cov/html
./scripts/mutants.sh         # cargo-mutants using .cargo/mutants.toml
./scripts/mutants.sh list    # list mutants without running
```

CI (`.github/workflows/ci.yml`) runs fmt, clippy, tests, release build, pure llvm-cov, and mutants.

## Prometheus sample

```text
# HELP jentayu_host_cpu_percent Host CPU usage percent
# TYPE jentayu_host_cpu_percent gauge
jentayu_host_cpu_percent 12.5 1710000000000
# TYPE jentayu_cpu_percent gauge
jentayu_cpu_percent{kind="process",id="42",name="nginx"} 3 1710000000000
```

## License

MIT
