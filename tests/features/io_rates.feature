@wip
Feature: Delta I/O rates
  As an operator watching processes and containers
  I want disk and network shown as bytes-per-second over the sample interval
  So that I can see live throughput instead of only lifetime counters

  Scenario: First sample has no rate yet
    Given a fresh collector with no previous I/O counters
    When the first snapshot is taken
    Then disk and network rate fields are absent
    And cumulative byte counters are still present

  Scenario: Second sample reports B/s over the interval
    Given a previous snapshot with known cumulative I/O counters
    And a later snapshot for the same targets after a known interval
    When rates are derived from the counter delta
    Then each rate equals (current - previous) / elapsed_seconds
    And host network rates are included when host counters advance

  Scenario: Counter reset does not produce a negative rate
    Given a previous cumulative counter value
    When the next sample reports a lower cumulative value
    Then the computed rate is zero for that counter

  Scenario: Ranking by disk prefers rates when available
    Given two targets where the slower-growing total still has a higher lifetime counter
    When samples are sorted by disk
    Then the higher interval rate ranks first

  Scenario: Prometheus exports rate gauges alongside totals
    Given a snapshot that includes interval rates
    When the snapshot is rendered as Prometheus text
    Then cumulative metrics remain jentayu_*_bytes_total counters
    And rate gauges named jentayu_*_bytes_per_second are present
    And every metric name is prefixed with jentayu_

  Scenario: Table and TUI show per-second units when rates are known
    Given a snapshot with disk and network rates
    When the snapshot is rendered for interactive or table display
    Then disk and network columns use per-second formatting
