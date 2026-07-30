@wip
Feature: Process network counters (Linux best-effort)
  As an operator sorting processes by network
  I want per-process cumulative network bytes when the OS can provide them
  So that network sort and rates are meaningful on Linux, and stay zero elsewhere

  Scenario: Unsupported OS keeps process network at zero
    Given a host that is not Linux
    When process metrics are sampled
    Then each process reports net receive and transmit cumulative bytes as 0
    And network rate fields stay absent until a second sample with advancing counters

  Scenario: Linux attributes TCP socket bytes via sock-diag and /proc
    Given a Linux host with readable /proc and netlink SOCK_DIAG
    When process metrics are sampled
    Then processes that hold TCP sockets may report non-zero cumulative network bytes
    And processes without attributable sockets remain at 0

  Scenario: Private network namespace falls back to /proc/pid/net/dev
    Given a Linux process in a network namespace distinct from the monitor
    And sock-diag did not attribute sockets to that PID
    When process metrics are sampled
    Then that process may receive cumulative bytes from its netns /proc/net/dev totals

  Scenario: Failures degrade to zero without aborting the snapshot
    Given /proc entries or netlink dumps that are missing or permission-denied
    When process metrics are sampled
    Then the process list is still returned
    And affected processes keep network counters at 0

  Scenario: Interval rates work once cumulative network advances
    Given two successive samples of the same process with increasing net counters
    When IoRateState derives rates over the interval
    Then net receive and transmit B/s are present for that process
