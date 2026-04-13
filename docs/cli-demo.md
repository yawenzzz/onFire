# CLI Demo

Minimal demo command:

```bash
PYTHONPATH=polymarket_arb python -m polymarket_arb.app.entrypoint \
  --session-id demo-s1 \
  --surface-id polymarket-us \
  --outcome-count 2 \
  --ordered-thresholds \
  --surface-resolved \
  --jurisdiction-eligible \
  --output ./demo-report.json
```

This runs the minimal shadow session path and writes a certification report file.
