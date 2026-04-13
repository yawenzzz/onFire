# Manual Verification Checklist

## Run full tests
- Execute: `make test`
- Expect: all tests green

## Run shadow demo
- Execute: `make shadow-demo`
- Expect: `demo-report.json` created with a verdict field

## Inspect archive bundle
- Execute: `bash scripts/run_capture_archive_shadow.sh`
- Expect files:
  - `certification-report.json`
  - `dashboard.json`
  - `summary.txt`

## Inspect real sample inputs
- Open:
  - `examples/live/events-sample.json`
  - `examples/live/market-book-sample.json`
