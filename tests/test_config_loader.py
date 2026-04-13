import json
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.config.loader import load_launch_gate


class ConfigLoaderTests(unittest.TestCase):
    def test_loads_launch_gate_from_json_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "launch_gate.json"
            path.write_text(json.dumps({
                "surface_resolved": True,
                "surface_id": "polymarket-us",
                "jurisdiction_eligible": True,
                "market_state_all_open": True,
                "preview_success_rate": 1.0,
                "invalid_tick_or_price_reject_rate": 0.0,
                "api_429_count": 0,
                "ambiguous_rule_trade_count": 0,
                "collateral_return_dependency_for_safety": 0,
                "hedge_completion_rate_shadow": 0.995,
                "false_positive_rate": 0.02,
                "shadow_window_days": 14
            }))
            gate = load_launch_gate(path)
            self.assertTrue(gate.launch_eligible())
