Feature: Prometheus text export
  As an observability pipeline
  I want Jentayu metrics in Prometheus exposition format
  So that scrapers and dashboards can ingest them

  Scenario: Metrics use jentayu_ prefix
    Given a non-empty snapshot
    When the snapshot is rendered as Prometheus text
    Then every metric name starts with jentayu_

  Scenario: Process and container samples are labeled
    Given process and container samples
    When the snapshot is rendered as Prometheus text
    Then series include identifying labels for id or name and kind

  Scenario: Host gauges are present
    Given a snapshot with host metrics
    When the snapshot is rendered as Prometheus text
    Then host-level CPU and memory metrics are present

  Scenario: I/O rate gauges accompany cumulative counters
    Given a snapshot with interval disk and network rates
    When the snapshot is rendered as Prometheus text
    Then jentayu_*_bytes_total counters remain present
    And jentayu_*_bytes_per_second gauges are present for known rates

  Scenario: Filter and top limit export target series
    Given a multi-target snapshot for export ranking
    When the snapshot is exported with filter "nginx" and top 1 sorted by cpu
    Then only matching top-N targets appear in Prometheus text
    And host-level metrics remain present

  Scenario: Full inventory when filter and top unset
    Given a multi-target snapshot for export ranking
    When the snapshot is rendered as Prometheus text
    Then all process/container targets appear in Prometheus text
