Feature: HTTP scrape server policy
  As an operator scraping jentayu with Prometheus
  I want safe defaults for bind, ranking, and auth
  So that metrics are usable locally without open inventory on the network

  Scenario: Serve default top is 50 when top is unset
    When the serve effective top is computed with top unset
    Then the serve effective top is 50

  Scenario: Serve hard-caps top at 500
    When the serve effective top is computed with top 600
    Then the serve effective top is 500

  Scenario: Serve ranking always applies default top ordering
    Given sample targets with mixed CPU for serve ranking
    When the snapshot is ranked for serve without filter or top
    Then the first ranked target name is "nginx-worker"
    And the host metrics remain present

  Scenario: Non-loopback bind is refused without allow-remote and auth
    When bind policy is checked for 0.0.0.0 without allow-remote or auth
    Then bind policy is refused

  Scenario: Non-loopback bind is allowed with allow-remote and auth
    When bind policy is checked for 0.0.0.0 with allow-remote and auth
    Then bind policy is allowed

  Scenario: Loopback bind is allowed without auth
    When bind policy is checked for 127.0.0.1 without allow-remote or auth
    Then bind policy is allowed
