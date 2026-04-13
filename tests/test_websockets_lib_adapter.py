import asyncio
import unittest

from polymarket_arb.venue.websockets_lib_adapter import WebsocketsLibAdapter


class StubConnection:
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


class WebsocketsLibAdapterTests(unittest.TestCase):
    def test_connect_returns_async_source(self) -> None:
        async def run_test():
            adapter = WebsocketsLibAdapter(connect_impl=lambda url: StubConnection(['x', 'y']))
            async with adapter.connect('wss://example.test') as source:
                first = await source.recv()
                self.assertEqual(first, 'x')
        asyncio.run(run_test())
