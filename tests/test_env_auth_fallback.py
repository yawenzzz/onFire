import json
import os
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.realtime_probe_cli import main


class EnvAuthFallbackTests(unittest.TestCase):
    def test_probe_cli_can_use_env_auth(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / 'probe.json'
            os.environ['PM_ACCESS_KEY'] = 'kid'
            os.environ['PM_SIGNATURE'] = 'sig'
            os.environ['PM_TIMESTAMP'] = '123'

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

            code = main(['--ws-url', 'wss://example.test', '--market-ids', 'm1', '--limit', '1', '--output', str(out)], connect_factory=factory, optional_factory_resolver=lambda: None)
            self.assertEqual(code, 0)
            self.assertEqual(seen['headers']['X-PM-Access-Key'], 'kid')
