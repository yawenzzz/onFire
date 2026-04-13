FROM python:3.9-slim
WORKDIR /app
ENV PYTHONPATH=polymarket_arb
COPY . /app
CMD ["python3", "-m", "polymarket_arb.app.entrypoint", "--input-file", "examples/shadow-input.json", "--output", "/tmp/demo-report.json"]
