# Security notes — Jentayu

Privilege, disclosure, and trust-boundary guidance for operators and agents.
Product roadmap context: [`PLAN.md`](./PLAN.md) (M4.5 / M5).

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

| Flags | Export targets |
| ----- | -------------- |
| Neither `-f` / `--filter` nor `-n` / `--top` | **Full** per-target inventory (sensitive on dense hosts) |
| `-f` and/or `-n` | Same ranking as table (`filter_and_rank`); `-s` / `--sort` applies |
| Host-level metrics | **Always** included |
| Alerts (`--alert-cpu` / `--alert-mem`) | Evaluated on the **full** collected snapshot, not the export subset |

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

## Planned `serve` (M5 — not shipped)

When HTTP scrape lands, the **default bind must be `127.0.0.1`** (loopback),
not all interfaces. Metrics remain confidential inventory. See PLAN.md M5
security requirements (cardinality cap, timeouts, auth before non-localhost).

## Residual risks (accepted for M4.5-a)

| Risk | Note |
| ---- | ---- |
| Cross-device `rename` | Documented; same-dir temp mitigates same-FS case |
| Concurrent `-o` same path | Last writer wins; CLI single-user assumption |
| Full export without `-f`/`-n` | Intentional; documented sensitive |
| Raw `Alert.target_name` | Message + `print_alerts` sanitized; field stays raw |
| TUI panic restore | M4.5-b Drop guard (not this slice) |

## See also

- [`AGENT.md`](./AGENT.md) — Docker read-only safety for agents
- [`README.md`](./README.md) — CLI and development
- [`PLAN.md`](./PLAN.md) — M4.5 / M5 security milestones
