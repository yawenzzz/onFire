import unittest

from polymarket_arb.rules.clarification_snapshot import ClarificationSnapshot


class ClarificationSnapshotTests(unittest.TestCase):
    def test_same_text_matches_same_hash(self) -> None:
        snapshot = ClarificationSnapshot.from_text("clarification A")
        self.assertTrue(snapshot.matches_text("clarification A"))

    def test_changed_text_detects_drift(self) -> None:
        snapshot = ClarificationSnapshot.from_text("clarification A")
        self.assertFalse(snapshot.matches_text("clarification B"))
