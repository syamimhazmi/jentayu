Feature: Threshold alerts
  As an operator
  I want warnings when CPU or memory exceed thresholds
  So that I can react before saturation

  Scenario: CPU alert fires at threshold
    Given an alert CPU threshold of 80 percent
    And a sample at exactly 80 percent CPU
    When alerts are evaluated
    Then a CPU alert is raised for that sample

  Scenario: CPU alert does not fire below threshold
    Given an alert CPU threshold of 80 percent
    And a sample at 79.9 percent CPU
    When alerts are evaluated
    Then no CPU alert is raised for that sample

  Scenario: Memory alert uses percent of limit when present
    Given an alert memory threshold of 90 percent
    And a sample using 90 percent of its memory limit
    When alerts are evaluated
    Then a memory alert is raised for that sample

  Scenario: No thresholds configured
    Given no CPU or memory alert thresholds
    When alerts are evaluated for any samples
    Then no alerts are raised
