import asyncio
import unittest

from polymarket_arb.venue.real_ws_connector import iter_messages_from_source


class StubSource:
    def __init__(self, messages):
        self.messages = messages

    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc, tb):
        return False

    async def recv(self):
        if not self.messages:
            raise StopAsyncIteration
        return self.messages.pop(0)


class RealWSConnectorTests(unittest.TestCase):
    def test_iter_messages_from_source_yields_up_to_limit(self) -> None:
        async def run_test():
            out = []
            async for msg in iter_messages_from_source(lambda: StubSource(['a', 'b', 'c']), limit=2):
                out.append(msg)
            self.assertEqual(out, ['a', 'b'])
        asyncio.run(run_test())
