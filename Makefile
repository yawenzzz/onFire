.PHONY: test lint shadow-demo

test:
	PYTHONPATH=polymarket_arb python3 -m unittest discover -s tests -v

lint:
	PYTHONPATH=polymarket_arb python3 -m compileall -q polymarket_arb/polymarket_arb

shadow-demo:
	PYTHONPATH=polymarket_arb python3 -m polymarket_arb.app.entrypoint --input-file examples/shadow-input.json --output demo-report.json
