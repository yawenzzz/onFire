import tempfile
import unittest
from pathlib import Path

from polymarket_arb.venue.real_rule_source import LocalRuleSource


class RealRuleSourceTests(unittest.TestCase):
    def test_reads_rule_text_from_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'rule.txt'
            path.write_text('Rule A')
            source = LocalRuleSource(path)
            self.assertEqual(source.read_text(), 'Rule A')
