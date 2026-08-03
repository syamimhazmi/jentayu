# Gherkin features — scenario ↔ test map

Living behavior specs for Jentayu. Agents **must** update scenarios when user-visible
behavior changes (see `QUALITY.md` §2).

## Executable runner

High-value pure scenarios run via **cucumber-rs**:

```bash
cargo test --test features
```

| Detail | Value |
| ------ | ----- |
| Harness | `tests/features_runner.rs` (`harness = false`) |
| Wired | `filter_and_rank`, `alerts`, `prometheus_export`, `serve` (no `@wip` tag) |
| Deferred | Features tagged `@wip` — still living specs; covered by mapped Rust tests until steps land |

Adding a new executable scenario: write/adjust the `.feature` step text, implement
`#[given]` / `#[when]` / `#[then]` in `features_runner.rs`, and drop `@wip` from that feature.

## Scenario map

| Feature file | Scenario focus | Executable coverage |
| ------------ | -------------- | ------------------- |
| `filter_and_rank.feature` | filter, sort, top-N | **cucumber** + `tests/filter_rank.rs` + `src/metrics.rs` unit tests |
| `alerts.feature` | CPU/memory thresholds | **cucumber** + `src/alert.rs` unit tests |
| `prometheus_export.feature` | `jentayu_*` exposition; filter/top export ranking; full inventory when unset | **cucumber** + `src/export/prometheus.rs` + `snapshot_for_export` unit tests |
| `serve.feature` | serve top default 50 / hard cap 500; always-rank; bind dual-gate | **cucumber** + `src/serve/policy.rs` unit tests; HTTP auth/503 in `src/serve/mod.rs` tests |
| `cli_modes.feature` | once / watch / tui / prometheus / json / serve | `@wip` — manual smoke + future CLI integration tests; serve HTTP covered in `src/serve/mod.rs` |
| `docker_optional.feature` | daemon down still monitors processes | `@wip` — `tests/docker_mock.rs` (`disabled_docker_*`, `containers_only_with_disabled_*`) |
| `docker_integration.feature` | mock Docker list/stats; optional live socket | `@wip` — `tests/docker_mock.rs`; `tests/docker_live.rs` (`--features docker-integration`) |
| `container_stats_fanout.feature` | concurrent stats cap (8), skip failures | `@wip` — `src/collector/container.rs` unit tests |
| `io_rates.feature` | interval B/s rates vs cumulative I/O | `@wip` — `src/metrics.rs` (`io_rate_bps`, `IoRateState`), prometheus rate gauges |
| `process_network.feature` | per-process net on Linux; 0 elsewhere | `@wip` — `src/collector/process_net.rs` unit tests |

## Writing rules

1. Describe **observable** outcomes (table order, exit, metric lines), not private types.
2. One behavior change ⇒ update the matching `.feature` in the same change.
3. Prefer cucumber steps for pure ranking / alerts / export; keep I/O-heavy scenarios `@wip` with a Rust map until high value justifies wiring.
4. If a scenario has no executable path yet, keep `@wip` and note the gap in the table above.
