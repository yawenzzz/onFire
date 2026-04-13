import unittest

from polymarket_arb.venue.live_ws_listener import capture_messages


class StubWS:
    def __init__(self, messages):
        self.messages = messages

    def iter_messages(self, limit: int):
        for idx, msg in enumerate(self.messages):
            if idx >= limit:
                break
            yield msg


class LiveWSListenerTests(unittest.TestCase):
    def test_capture_messages_respects_limit(self) -> None:
        ws = StubWS(['a', 'b', 'c'])
        got = capture_messages(ws, limit=2)
        self.assertEqual(got, ['a', 'b'])
