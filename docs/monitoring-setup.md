# Monitoring Setup

## Prometheus
- use `monitoring/prometheus.yml`
- scrape the `/status` endpoint from the status server

## Grafana
- import `monitoring/grafana-dashboard.json`
- visualize preview success, health, and alert status

## Alertmanager
- use `monitoring/alertmanager.yml`
- load rules from `monitoring/alerts.yml`
