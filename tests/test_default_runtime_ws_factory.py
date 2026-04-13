import unittest

from polymarket_arb.venue.default_runtime_ws_factory import resolve_runtime_ws_factory


class DefaultRuntimeWSFactoryTests(unittest.TestCase):
    def test_uses_default_optional_factory_when_no_override(self) -> None:
        factory = resolve_runtime_ws_factory(lambda: 'x')
        self.assertEqual(factory, 'x')

    def test_override_wins_when_provided(self) -> None:
        factory = resolve_runtime_ws_factory(lambda: 'default', override_resolver=lambda: 'override')
        self.assertEqual(factory, 'override')
