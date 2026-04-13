import asyncio
import json
import unittest

from polymarket_arb.venue.realtime_market_client import capture_market_messages


class StubConn:
    def __init__(self, messages):
        self.messages = messages
        self.sent = []
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


class RealtimeMarketClientTests(unittest.TestCase):
    def test_sends_subscription_before_receiving(self) -> None:
        async def run_test():
            holder = {}
            def factory(url):
                conn = StubConn(['{"market_id":"m1","market_state":"OPEN","best_bid":0.4,"best_ask":0.5}'])
                holder['conn'] = conn
                return conn
            msgs = await capture_market_messages(factory, 'wss://example.test', ['m1'], limit=1)
            self.assertEqual(len(msgs), 1)
            sent = json.loads(holder['conn'].sent[0])
            self.assertEqual(sent['type'], 'market')
            self.assertEqual(sent['assets_ids'], ['m1'])
        asyncio.run(run_test())
