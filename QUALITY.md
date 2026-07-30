# QUALITY.md — Extreme agent constraints

**Authority:** Binding for every agent (human or AI) changing this repository.
If work conflicts with this file, **stop, shrink scope, or add tests** — do not ship.

Companion docs: `AGENT.md` (rules), `SKILL.md` (skill entry), `PLAN.md` (roadmap).

---

## Hard stop (Definition of Done)

A change is **not done** until all of the following are true (or each skip is **written** with reason):

| # | Gate | Command / artifact |
| - | ---- | ------------------ |
| 1 | Format | `cargo fmt --check` |
| 2 | Lint | `cargo clippy --all-targets -- -D warnings` |
| 3 | Unit + integration tests | `cargo test` |
| 4 | Release build | `cargo build --release` |
| 5 | Behavior scenarios | Gherkin in `tests/features/` updated if user-visible |
| 6 | Coverage intent | Pure logic covered; `./scripts/coverage.sh` green (or gap documented) |
| 7 | Mutation testing | Pure-module change ⇒ `./scripts/mutants.sh` (or `in-diff`) green / survivors fixed |
| 8 | Smoke (if user-facing) | Relevant CLI path exercised once |
| 9 | Docker-optional | Process/host path still works without daemon |

**Forbidden claims:** “tests pass” without running them; “LGTM” without the checklist; shipping public API changes without tests in the same change.

**Prefer:** smaller PR / feature slice that meets the bar over a large untested change.

---

## 1. Unit tests

### Required

- Every **pure function** and **non-trivial branch** has unit tests:
  - happy path
  - edge cases (empty, zero, max/min, `None`, filter matches nothing)
  - error / degrade paths where the function returns `Result` or optional failure
- Prefer library surface (`src/lib.rs` modules, `#[cfg(test)]` in module) over binary-only logic.
- New or changed **public** types/functions ship with tests **in the same change**.
- Assertions check **outcomes** (values, order, labels, thresholds), not only “did not panic”.

### Placement

| Kind | Where |
| ---- | ----- |
| Pure helpers | `#[cfg(test)] mod tests` in the same file, or `tests/*.rs` for multi-module flows |
| Collectors (I/O) | Unit-test pure formula helpers; integration/smoke for live sysinfo/Docker |
| CLI parsing | Prefer `try_parse_from` style tests if flags change |

### Core modules (minimum bar)

| Module | Must stay well-tested |
| ------ | --------------------- |
| `metrics` | `filter_and_rank`, sort keys, filter semantics |
| `alert` | threshold evaluate, boundary equality |
| `export/prometheus` | metric names (`jentayu_`), labels, sample lines |
| Container CPU math | Docker-compatible percent formula helpers |

---

## 2. Gherkin / BDD tests

### Required for user-visible behavior

Express observable behavior as Gherkin under `tests/features/`:

```gherkin
Feature: …
  Scenario: …
    Given …
    When …
    Then …
```

### Scope that needs scenarios

- CLI modes: `once`, `watch`, `tui`, `prometheus`, `json`
- Filters (`--filter`), top-N (`--top`), sort (`--sort`)
- Alerts (`--alert-cpu`, `--alert-mem`)
- Export formats (Prometheus prefix, JSON shape at high level)
- Docker optional: daemon down ⇒ process monitoring still works

### Rules

- Language describes **what the user observes**, not internal struct names.
- Each scenario maps to an executable test (Rust integration/`cargo test`) **or** a `tests/features/README.md` row documenting the mapping until steps are wired.
- Changing user-visible behavior without updating scenarios is a **DoD failure**.

### Layout

```
tests/
├── features/
│   ├── README.md           # scenario → test mapping + cucumber notes
│   ├── filter_and_rank.feature   # cucumber-wired
│   ├── alerts.feature            # cucumber-wired
│   ├── prometheus_export.feature # cucumber-wired
│   ├── cli_modes.feature         # @wip living spec
│   └── docker_optional.feature   # @wip + docker_mock tests
├── features_runner.rs      # cucumber harness (cargo test --test features)
└── filter_rank.rs          # extra ranking integration coverage
```

**Executable BDD:** high-value pure scenarios run via cucumber-rs (`cargo test --test features`).
Other features may stay `@wip` living specs with a README map to Rust tests. **Scenarios as
living specs** remain mandatory for user-visible behavior.

---

## 3. QA procedures

Run **before** claiming done. Order matters: format → lint → test → release → smoke.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

### Smoke (when flags/modes change)

```bash
cargo run -- once -n 5
cargo run -- prometheus | head
# if filter/alert changed:
cargo run -- once -f unlikely_name_xyz -n 3
# Docker-optional:
# stop daemon or unset socket; process table must still print
```

### Skip policy

If a step cannot run in the environment (no Docker, no TTY for TUI):

1. State which step was skipped and why.
2. Note residual risk.
3. Do not invent “green” for unrun checks.

### Regression checklist (mental)

- [ ] Clippy clean under `-D warnings`
- [ ] All tests green
- [ ] Prometheus names still `jentayu_*`
- [ ] Alerts fire only at documented thresholds
- [ ] Docker down does not hard-fail process mode

---

## 4. Quality metrics

Track and **do not regress without explicit justification** in the change summary:

| Metric | Expectation | Regression = |
| ------ | ----------- | ------------ |
| Clippy | 0 warnings with `-D warnings` | Any warning allowed without fix/justification |
| Unit/integration | Green on claimed-done | Failing or skipped suite |
| Public pure API | Covered by unit tests | New branch in `metrics`/`alert`/`export` with no test |
| Behavior docs | Gherkin scenarios for user-facing flows | Behavior change, scenarios stale |
| Alert/export correctness | Names, thresholds, formats match docs/tests | Silent rename or off-by-one threshold |
| Mutation resistance | Critical tests kill obvious mutants | Tests that only assert `is_ok()` / non-empty |

Agents should report in the handoff: what was run, what passed, what was skipped.

---

## 5. Mutation testing

### Mindset (always)

Write tests as if a mutator will:

- flip `<` ↔ `<=`, `>` ↔ `>=`
- swap sort direction
- change threshold comparison
- rename or drop a Prometheus label/metric prefix
- off-by-one on `top N`

If the suite would still pass after such a mutation, **strengthen asserts**.

### Tooling (M4 — required when pure modules change)

Config: [`.cargo/mutants.toml`](./.cargo/mutants.toml) (examine `metrics`, `alert`, `export/prometheus`, container CPU math).

Install (once):

```bash
cargo install cargo-mutants --locked
```

Commands:

```bash
./scripts/mutants.sh              # full pure-module run (config defaults)
./scripts/mutants.sh list         # list mutants only
./scripts/mutants.sh in-diff      # mutants in git diff vs HEAD
./scripts/mutants.sh metrics      # single file
cargo mutants --list              # same list via cargo
```

- On pure-logic changes: run `./scripts/mutants.sh` (or at least `in-diff` / the touched file) and **fix high-value MISSED mutants**.
- Do not chase noise (display formatting, Docker socket I/O) — those are excluded by config.
- CI job `mutants` runs the same pure-module set; survivors fail the job.

### Minimum mutation-minded targets

- `filter_and_rank` sort order and top-N truncation
- Alert threshold boundaries (equal-to-threshold)
- Prometheus metric name prefix and key lines
- Container CPU percent formula (numerator/denominator)

---

## 6. Test coverage

### Targets

| Area | Coverage intent |
| ---- | --------------- |
| Pure logic (`metrics`, `alert`, pure export render) | **High** — CI floor **80% lines** on pure set |
| OS / Docker I/O boundaries | Lower OK; document; cover with smoke/integration when possible |
| `main` wiring | Thin; logic lives in lib and is tested there |

### Tooling (M4 — llvm-cov)

Install (once):

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
```

Commands:

```bash
./scripts/coverage.sh             # pure modules + fail-under-lines=80
./scripts/coverage.sh full        # whole-crate summary (no floor)
./scripts/coverage.sh html        # HTML → target/llvm-cov/html
./scripts/coverage.sh lcov        # lcov.info for CI artifacts
PURE_FAIL_UNDER_LINES=85 ./scripts/coverage.sh pure   # tighten floor locally
```

Pure-module filter ignores `main`, `cli`, `display/`, `collector/`, `export/mod.rs` so floors measure logic that unit tests own.

### Rules

- New code paths must not leave large untested gaps in core modules.
- Prefer meaningful assertions over coverage-gaming tests (no assert-true-only).
- When changing public types in `metrics.rs`, update **Prometheus export + unit tests + Gherkin** (if behavior-facing) together.
- Pure-logic PRs: run `./scripts/coverage.sh` before claiming done (or justify skip).

### Coverage anti-patterns (banned)

- Tests that only compile the code
- Asserting only that a string is non-empty when a specific metric line is required
- Ignoring `Result` errors in tests without asserting the error kind

---

## Agent report template (end of task)

```text
## Quality report
- fmt: pass | fail | skipped (why)
- clippy -D warnings: pass | fail | skipped (why)
- cargo test: pass | fail | skipped (why) — N passed
- release build: pass | fail | skipped (why)
- Gherkin: updated | N/A (not user-visible) | missing (blocker)
- llvm-cov (./scripts/coverage.sh): pass | fail | skipped (why) — % lines if run
- cargo mutants (./scripts/mutants.sh): pass | fail | skipped (why) — MISSED count
- Smoke: commands run …
- Residual risks: …
```

---

## Enforcement summary

```
NO tests for pure logic change     → STOP
User-visible change, no Gherkin    → STOP
Clippy warnings                    → STOP
Claimed done without QA checklist  → STOP
Public metrics change, no export/tests → STOP
Pure change, coverage/mutants red  → STOP (or justified skip)
```

---

## Install tooling (dev machine / CI)

```bash
rustup component add llvm-tools-preview rustfmt clippy
cargo install cargo-llvm-cov cargo-mutants --locked
```

CI: [`.github/workflows/ci.yml`](./.github/workflows/ci.yml) runs fmt, clippy, test, release, pure llvm-cov, and cargo-mutants.
