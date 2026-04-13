import asyncio
import unittest

from polymarket_arb.venue.async_websocket_client import AsyncWebSocketClient


class AsyncWebSocketClientTests(unittest.TestCase):
    def test_iter_messages_yields_stub_messages(self) -> None:
        async def run_test():
            client = AsyncWebSocketClient(url='wss://example.test/ws', connect_impl=lambda url: ['a', 'b', 'c'])
            out = []
            async for msg in client.iter_messages(limit=2):
                out.append(msg)
            self.assertEqual(out, ['a', 'b'])
        asyncio.run(run_test())
