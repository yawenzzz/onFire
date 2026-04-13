import unittest
from pathlib import Path


class DeliveryDocsTests(unittest.TestCase):
    def test_readme_contains_delivery_sections(self) -> None:
        text = Path('polymarket_arb/README.md').read_text()
        for marker in [
            'Implemented Features',
            'Manual Verification',
            'Current Safety Posture',
        ]:
            self.assertIn(marker, text)

    def test_handoff_doc_exists(self) -> None:
        text = Path('docs/delivery-handoff.md').read_text()
        self.assertIn('What is implemented', text)
        self.assertIn('What is not implemented', text)
        self.assertIn('How to verify manually', text)

    def test_feature_inventory_exists(self) -> None:
        text = Path('docs/feature-inventory.md').read_text()
        self.assertIn('Data ingestion', text)
        self.assertIn('Shadow pipeline', text)
        self.assertIn('CLI and scripts', text)

    def test_manual_verification_checklist_exists(self) -> None:
        text = Path('docs/manual-verification-checklist.md').read_text()
        self.assertIn('Run full tests', text)
        self.assertIn('Run shadow demo', text)
        self.assertIn('Inspect archive bundle', text)
