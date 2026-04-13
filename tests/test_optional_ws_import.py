import unittest

from polymarket_arb.venue.optional_ws_lib import websocket_connect_factory


class OptionalWSImportTests(unittest.TestCase):
    def test_factory_returns_callable_or_none(self) -> None:
        factory = websocket_connect_factory()
        self.assertTrue(callable(factory) or factory is None)
