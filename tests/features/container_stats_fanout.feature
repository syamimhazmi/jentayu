@wip
Feature: Rate-limited container stats fan-out
  As an operator with many running containers
  I want container stats collected with bounded concurrency
  So that the Docker daemon is not overwhelmed by parallel stats calls

  Scenario: Concurrency cap is eight
    Given the M4 container stats fan-out configuration
    Then at most 8 Docker stats requests run at once per sample tick

  Scenario: Failures skip without aborting the sample
    Given multiple containers are listed
    And stats for some containers fail
    When a sample is collected with the concurrency-capped fan-out
    Then successful containers still appear in the result
    And failed containers are omitted rather than failing the whole sample

  Scenario: Disabled collector returns no containers
    Given Docker is unavailable and the container collector is disabled
    When a sample is requested
    Then the sample is empty
    And no error is raised

  Scenario: List entries without an id are ignored
    Given a container list entry has no id
    When jobs are built for stats fan-out
    Then that entry is not queried
