import unittest
from pathlib import Path


class FinalSummaryDocsTests(unittest.TestCase):
    def test_final_delivery_summary_exists(self) -> None:
        text = Path('docs/final-delivery-summary.md').read_text()
        self.assertIn('What is implemented', text)
        self.assertIn('What is not implemented', text)
        self.assertIn('Current safety posture', text)

    def test_gap_register_exists(self) -> None:
        text = Path('docs/gap-register.md').read_text()
        self.assertIn('Highest-priority remaining gaps', text)
        self.assertIn('Lower-priority gaps', text)
