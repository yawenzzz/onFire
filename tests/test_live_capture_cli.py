import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.live_capture_cli import main


class LiveCaptureCliTests(unittest.TestCase):
    def test_fails_closed_without_connect_factory(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / 'capture.jsonl'
            code = main(['--output', str(out), '--limit', '1', '--ws-url', 'wss://example.test'], optional_factory_resolver=lambda: None)
            self.assertEqual(code, 2)

    def test_uses_injected_connect_factory(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / 'capture.jsonl'

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

            code = main(
                ['--output', str(out), '--limit', '1', '--ws-url', 'wss://example.test'],
                connect_factory=lambda url, headers=None: StubSource([{'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}]),
                optional_factory_resolver=lambda: None,
            )
            self.assertEqual(code, 0)
            first = json.loads(out.read_text().splitlines()[0])
            self.assertEqual(first['market_id'], 'm1')

    def test_uses_optional_ws_factory_when_available(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / 'capture.jsonl'

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

            code = main(
                ['--output', str(out), '--limit', '1', '--ws-url', 'wss://example.test'],
                optional_factory_resolver=lambda: (lambda url, headers=None: StubSource([{'market_id': 'm9', 'market_state': 'OPEN', 'best_bid': 0.7, 'best_ask': 0.8}])),
            )
            self.assertEqual(code, 0)
            first = json.loads(out.read_text().splitlines()[0])
            self.assertEqual(first['market_id'], 'm9')

    def test_accepts_auth_triplet_and_passes_headers(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / 'capture.jsonl'

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

            seen = {}
            def factory(url, headers=None):
                seen['headers'] = headers
                return StubSource([{'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}])

            code = main(
                [
                    '--output', str(out), '--limit', '1', '--ws-url', 'wss://example.test',
                    '--access-key', 'kid', '--signature', 'sig', '--timestamp', '123'
                ],
                connect_factory=factory,
                optional_factory_resolver=lambda: None,
            )
            self.assertEqual(code, 0)
            self.assertEqual(seen['headers']['X-PM-Access-Key'], 'kid')
