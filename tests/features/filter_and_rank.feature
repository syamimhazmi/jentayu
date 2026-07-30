Feature: Filter and rank resource samples
  As an operator
  I want to filter by name and rank by resource usage
  So that I can focus on the noisiest processes or containers

  Scenario: Sort by CPU descending
    Given samples with different CPU percentages
    When I rank by cpu with no filter and no top limit
    Then samples are ordered by CPU percent descending

  Scenario: Sort by memory descending
    Given samples with different memory footprints
    When I rank by memory
    Then the highest memory sample is first

  Scenario: Sort by disk descending
    Given samples with different disk read totals
    When I rank by disk
    Then the highest disk sample is first

  Scenario: Sort by network descending
    Given samples with different network totals
    When I rank by network
    Then the highest network sample is first

  Scenario: Case-insensitive name filter
    Given samples named "nginx", "Postgres", and "redis"
    When I filter by "post"
    Then only the Postgres sample remains

  Scenario: Top N truncates after sort
    Given more than N samples
    When I rank with top N equal to 1
    Then exactly one sample is returned
    And it is the highest ranked by the chosen metric

  Scenario: Empty input
    Given no samples
    When I rank with any sort and filter
    Then the result is empty
