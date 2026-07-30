@wip
Feature: Docker is optional
  As an operator without Docker
  I want process and host monitoring to keep working
  So that Jentayu remains useful on plain hosts

  Scenario: Daemon unavailable still shows processes
    Given the Docker daemon is unreachable or the socket is missing
    When I collect with default process-inclusive mode
    Then process samples are still returned
    And the tool does not hard-fail solely because Docker is down

  Scenario: Containers-only with no Docker degrades gracefully
    Given the Docker daemon is unavailable
    When I collect containers-only
    Then the tool reports no containers or a clear degrade path
    And it does not panic
