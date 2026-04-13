import unittest

from polymarket_arb.app.realtime_capture_cli import main


class RealtimeCaptureCliTests(unittest.TestCase):
    def test_fails_closed_when_default_ws_factory_unavailable(self) -> None:
        code = main(['--output', '/tmp/ignore.jsonl', '--limit', '1', '--ws-url', 'wss://example.test'], optional_factory_resolver=lambda: None)
        self.assertEqual(code, 2)

    def test_uses_default_ws_factory_when_available(self) -> None:
        class StubSource:
            def __init__(self, messages):
                self.messages = messages
            async def recv(self):
                if not self.messages:
                    raise StopAsyncIteration
                return self.messages.pop(0)
            async def __aenter__(self):
                return self
            async def __aexit__(self, exc_type, exc, tb):
                return False

        import tempfile
        from pathlib import Path
        import json
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / 'capture.jsonl'
            code = main(
                ['--output', str(out), '--limit', '1', '--ws-url', 'wss://example.test'],
                optional_factory_resolver=lambda: (lambda url: StubSource([{'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}]))
            )
            self.assertEqual(code, 0)
            first = json.loads(out.read_text().splitlines()[0])
            self.assertEqual(first['market_id'], 'm1')
