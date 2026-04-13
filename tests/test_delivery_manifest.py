import json
import unittest
from pathlib import Path


class DeliveryManifestTests(unittest.TestCase):
    def test_manifest_exists_with_expected_top_level_keys(self) -> None:
        manifest = json.loads(Path('docs/final-delivery-manifest.json').read_text())
        for key in [
            'name',
            'delivery_type',
            'primary_strategy',
            'trading_posture',
            'implemented_capabilities',
            'verification_commands',
            'core_artifacts',
            'remaining_gaps',
        ]:
            self.assertIn(key, manifest)

    def test_manifest_names_core_strategy_and_no_trade_posture(self) -> None:
        manifest = json.loads(Path('docs/final-delivery-manifest.json').read_text())
        self.assertIn('structural arbitrage', manifest['primary_strategy'])
        self.assertIn('NO_TRADE', manifest['trading_posture'])
