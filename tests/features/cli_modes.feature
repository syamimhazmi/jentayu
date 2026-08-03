@wip
Feature: CLI output modes
  As an operator
  I want multiple ways to view the same snapshot
  So that I can use tables, live UI, or exporters

  Scenario: Default once mode prints a table
    Given the jentayu binary is available
    When I run jentayu once with top 5
    Then the process exits successfully
    And resource rows are printed to stdout

  Scenario: Prometheus mode emits exposition text
    Given the jentayu binary is available
    When I run jentayu prometheus
    Then stdout contains metrics prefixed with jentayu_

  Scenario: JSON mode emits a snapshot object
    Given the jentayu binary is available
    When I run jentayu json
    Then stdout is valid JSON representing a snapshot

  Scenario: Watch mode refreshes on an interval
    Given the jentayu binary is available
    When I run jentayu watch with a short interval
    Then the display refreshes until interrupted

  Scenario: TUI mode starts interactive dashboard
    Given a TTY is available
    When I run jentayu tui
    Then an interactive dashboard is shown until quit

  Scenario: Serve mode listens for Prometheus scrapes
    Given the jentayu binary is available
    When I run jentayu serve on loopback
    Then GET /metrics returns exposition text prefixed with jentayu_
