import unittest

from polymarket_arb.venue.default_ws_connect import resolve_connect_factory


class DefaultWSConnectTests(unittest.TestCase):
    def test_returns_none_when_optional_factory_unavailable(self) -> None:
        self.assertIsNone(resolve_connect_factory(lambda: None))

    def test_returns_factory_when_optional_factory_available(self) -> None:
        factory = resolve_connect_factory(lambda: (lambda url: object()))
        self.assertTrue(callable(factory))
