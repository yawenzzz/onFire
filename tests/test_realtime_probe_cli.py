import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.realtime_probe_cli import main


class RealtimeProbeCliTests(unittest.TestCase):
    def test_probe_cli_fails_closed_without_factory(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / 'probe.json'
            code = main(['--ws-url', 'wss://example.test', '--market-ids', 'm1', '--limit', '1', '--output', str(out)], optional_factory_resolver=lambda: None)
            self.assertEqual(code, 2)

    def test_probe_cli_writes_probe_output_with_injected_factory(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / 'probe.json'

            class StubConn:
                def __init__(self):
                    self.sent = []
                    self.messages = ['{"market_id":"m1","market_state":"OPEN","best_bid":0.4,"best_ask":0.5}']
                async def send(self, text):
                    self.sent.append(text)
                async def recv(self):
                    if not self.messages:
                        raise StopAsyncIteration
                    return self.messages.pop(0)
                async def __aenter__(self):
                    return self
                async def __aexit__(self, exc_type, exc, tb):
                    return False

            code = main([
                '--ws-url', 'wss://example.test',
                '--market-ids', 'm1',
                '--limit', '1',
                '--output', str(out),
            ], connect_factory=lambda url, headers=None: StubConn(), optional_factory_resolver=lambda: None)
            self.assertEqual(code, 0)
            data = json.loads(out.read_text())
            self.assertEqual(data['captured_count'], 1)

    def test_probe_cli_accepts_auth_triplet(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / 'probe.json'

            class StubConn:
                def __init__(self):
                    self.sent = []
                    self.messages = []
                async def send(self, text):
                    self.sent.append(text)
                async def recv(self):
                    raise StopAsyncIteration
                async def __aenter__(self):
                    return self
                async def __aexit__(self, exc_type, exc, tb):
                    return False

            seen = {}
            def factory(url, headers=None):
                seen['headers'] = headers
                return StubConn()

            code = main([
                '--ws-url', 'wss://example.test',
                '--market-ids', 'm1',
                '--limit', '1',
                '--output', str(out),
                '--access-key', 'kid',
                '--signature', 'sig',
                '--timestamp', '123',
            ], connect_factory=factory, optional_factory_resolver=lambda: None)
            self.assertEqual(code, 0)
            self.assertEqual(seen['headers']['X-PM-Access-Key'], 'kid')
