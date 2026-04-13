import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.capture_daemon_cli import main


class CaptureDaemonCliTests(unittest.TestCase):
    def test_main_writes_capture_file_in_demo_mode(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / 'capture.jsonl'
            code = main(['--output', str(out), '--limit', '1', '--demo'])
            self.assertEqual(code, 0)
            self.assertTrue(out.exists())
            first = json.loads(out.read_text().splitlines()[0])
            self.assertIn('market_id', first)

    def test_main_fails_closed_when_ws_url_given_but_no_factory(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / 'capture.jsonl'
            code = main(['--output', str(out), '--limit', '1', '--ws-url', 'wss://example.test'])
            self.assertEqual(code, 2)
            self.assertFalse(out.exists())

    def test_main_uses_injected_connect_factory_for_ws_mode(self) -> None:
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
                connect_factory=lambda: (lambda url: StubSource([{'market_id': 'm1', 'market_state': 'OPEN', 'best_bid': 0.4, 'best_ask': 0.5}]))
            )
            self.assertEqual(code, 0)
            self.assertTrue(out.exists())
