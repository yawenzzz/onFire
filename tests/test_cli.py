import unittest

from polymarket_arb.app.cli import build_cli_summary


class CliTests(unittest.TestCase):
    def test_cli_summary_includes_mode_and_verdict(self) -> None:
        summary = build_cli_summary(mode="shadow", verdict="CERTIFICATION_INCOMPLETE")
        self.assertIn("mode=shadow", summary)
        self.assertIn("verdict=CERTIFICATION_INCOMPLETE", summary)
