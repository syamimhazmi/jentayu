@wip
Feature: Docker collector integration
  As a developer of Jentayu
  I want container collection verified against a mock (and optional live) Docker Engine API
  So that list/stats mapping stays correct without requiring a daemon in every CI job

  Scenario: Mock daemon list and stats produce container samples
    Given a mock Docker Engine HTTP API with running containers
    When the container collector connects over HTTP and samples
    Then each successful container appears as a container resource sample
    And names come from the list response (slash-stripped)
    And CPU percent uses the Docker-compatible formula
    And containers whose stats fail are omitted without failing the sample

  Scenario: Orchestrator containers-only uses mock Docker only
    Given a mock Docker Engine with container stats
    When the collector runs in containers-only mode
    Then the snapshot targets are containers only
    And host metrics are still present on the snapshot

  Scenario: Live daemon when socket present
    Given the Docker socket is available and docker-integration is enabled
    When the container collector connects with local defaults
    Then sampling succeeds without hard error
    And All mode still includes process samples
