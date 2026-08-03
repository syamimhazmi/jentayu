# Security notes — Jentayu

Privilege, disclosure, and trust-boundary guidance for operators and agents.
Product roadmap context: [`PLAN.md`](./PLAN.md) (M4.5 done; M5 next).

## Privilege model

- Run as a **normal unprivileged user**. Never require setuid/setgid.
- **Docker socket access ≈ root-equivalent** on many hosts. Jentayu uses the
  Docker Engine API for **read-only** list + stats only. It does **not**
  start, stop, remove, or exec into containers.
- Without Docker or with limited `/proc`, the tool **degrades gracefully**
  (empty containers / zero process-net); process and host monitoring continue.

## Confidential inventory

Process names, PIDs, and container names are **confidential inventory**, not
public telemetry by default. Treat table, TUI, Prometheus, and JSON output like
`ps` + `docker stats` output: suitable for local operators, not for untrusted
networks without an explicit design (auth, bind policy, cardinality caps).

## Export ranking policy (`prometheus` / `json`)

| Flags                                        | Export targets                                                      |
| -------------------------------------------- | ------------------------------------------------------------------- |
| Neither `-f` / `--filter` nor `-n` / `--top` | **Full** per-target inventory (sensitive on dense hosts)            |
| `-f` and/or `-n`                             | Same ranking as table (`filter_and_rank`); `-s` / `--sort` applies  |
| Host-level metrics                           | **Always** included                                                 |
| Alerts (`--alert-cpu` / `--alert-mem`)       | Evaluated on the **full** collected snapshot, not the export subset |

There is no `--all-targets` flag in M4.5. Full dump without filter/top is
intentional and **sensitive** — prefer `-n` / `-f` when sharing scrapes.

## File output (`-o`)

- Destination path must be **trusted**. Running as root with an untrusted `-o`
  path is **unsupported**.
- Writes use a **same-directory** temporary file (name includes pid + nanos)
  then `rename`. Atomic on the **same filesystem** only; cross-device rename
  is not claimed atomic.
- The destination is **overwritten** if it already exists. Refusing symlink
  overwrite is a later optional hardening (not M4.5-a).

## Terminal / free-text display

Process/container **id**, **name**, and **status** printed to the TTY are
passed through control-character sanitization (C0 + DEL → `?`) so hostile
names cannot inject BEL/CSI sequences into table or TUI output.

Alert human `message` strings sanitize the embedded name the same way.
Programmatic fields `target_id` / `target_name` remain **raw** for machine
consumers — if you print `target_name` yourself, sanitize first.

Prometheus label escaping (`escape_label`) is orthogonal and unchanged;
export inventory strings are **not** control-sanitized (terminal injection
only).

## `serve` scrape server (M5-a — shipped)

| Control | Behavior |
| ------- | -------- |
| Default bind | `127.0.0.1:9105` (loopback). `--bind` accepts **IP literals only** (no DNS). |
| Path | `GET /metrics` Prometheus text (`jentayu_*`). `/healths` not shipped. |
| Ranking | Always rank: default top **50**, hard cap **500** (independent of CLI `prometheus`/`json` full-dump when unset). |
| Auth | Optional on loopback. Bearer via `JENTAYU_METRICS_TOKEN` (wins) or `--token-file` (max 8 KiB, load once). When configured, every scrape needs `Authorization: Bearer …`. |
| Non-loopback | Startup refuse unless **both** `--allow-remote` and auth are set. |
| Timeout | Cooperative **2s** collect budget → **503** with no metric series. Sync `sysinfo` / process-net work may overrun the budget and still hold the collector lock — residual self-DoS under dense PID lists. |
| Metrics confidentiality | Process/container names and IDs remain sensitive inventory. Enabling remote bind exposes that inventory to the network; TLS is out of scope (reverse-proxy assumption). |

### Residual risks (serve)

| Risk | Note |
| ---- | ---- |
| Unauthenticated loopback | Intentional local Prometheus agent model; multi-user machines can scrape each other’s metrics on 127.0.0.1 |
| Cooperative timeout | Not a hard abort of CPU-bound process sampling |
| No TLS | Operators who need encryption must terminate TLS elsewhere after dual-gate remote bind |

## Supply chain (M4.5-c)

- **CI:** `cargo deny check` runs on every PR/push (advisories hard-fail; licenses allowlist; crates.io-only sources — no unexpected git deps).
- **Config:** root [`deny.toml`](./deny.toml). Local: `cargo install cargo-deny --locked && cargo deny check`.
- **Updates:** Dependabot weekly for Cargo and GitHub Actions (`.github/dependabot.yml`).
- **Tokio:** jentayu enables `macros`, `rt-multi-thread`, `time`, `net`, `sync`, `signal`, `io-util` (not `full`); bollard still unifies additional net/io features it needs.
- **Accepted license outliers in the tree:** Unicode-3.0 (ICU data), BlueOak-1.0.0 (synthez), WTFPL (terminfo via TUI stack), plus common duals (Unlicense/MIT, Zlib, …).

## Residual risks (accepted for M4.5)

| Risk                          | Note                                                                                                                                                                                       |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Cross-device `rename`         | Documented; same-dir temp mitigates same-FS case                                                                                                                                           |
| Concurrent `-o` same path     | Last writer wins; CLI single-user assumption                                                                                                                                               |
| Full export without `-f`/`-n` | Intentional; documented sensitive                                                                                                                                                          |
| Raw `Alert.target_name`       | Message + `print_alerts` sanitized; field stays raw                                                                                                                                        |
| TUI panic restore             | **Shipped (M4.5-b):** Drop → `ratatui::restore`; `try_init` installs panic hook. Residual: release sets `panic = "abort"` (skips Drop; hook still runs if installed); restore failures only logged to stderr |
| Supply chain (S9)             | **Mitigated (M4.5-c):** cargo-deny CI + Dependabot; residual: new advisories can fail CI overnight; license allowlist may need updates when deps change                                   |

## See also

- [`AGENT.md`](./AGENT.md) — Docker read-only safety for agents
- [`README.md`](./README.md) — CLI and development
- [`PLAN.md`](./PLAN.md) — M4.5 / M5 security milestones
